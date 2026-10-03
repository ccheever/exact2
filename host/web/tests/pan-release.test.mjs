// `panrelease` in a real browser (LLP 1057 §10.6 phase 2): the input glue's
// pan, driven by CDP mouse and touch events. A pan that began ends with one
// release carrying the tracker's velocity over the contact's samples (each at
// its event's own timestamp); a pan that never left the slop releases nothing;
// a cancelled contact releases at rest. The tracker itself is the engine's
// (host/web/src/pan_velocity.rs); here a stand-in records what the glue feeds it.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

const page = `<!doctype html>
<div id="exact-root" style="padding:20px">
  <div id="card" style="width:300px;height:300px;background:#f06;touch-action:none"></div>
  <div id="plain" style="width:300px;height:100px;background:#06f;touch-action:none"></div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  const views = new Map([[1, document.getElementById('card')], [2, document.getElementById('plain')]]);
  window.log = [];
  const samples = [];
  // A least-squares stand-in with the engine's contract: samples since the
  // first, velocity in px/s over the event timestamps.
  const velocity = {
    sample(id, x, y, t, first) { if (first) samples.length = 0; samples.push([t, x, y]); window.log.push(['sample', id, first]); },
    velocity(id, t) {
      const s = samples.splice(0); if (s.length < 2) return [0, 0];
      const [t0, x0, y0] = s[0], [t1, x1, y1] = s[s.length - 1], dt = (t1 - t0) / 1000;
      return dt > 0 ? [(x1 - x0) / dt, (y1 - y0) / dt] : [0, 0];
    },
  };
  const handlers = createInputHandlers({ root, views, retiredViews: new Set(), ready: () => true, inertAncestor: () => false,
    dispatch: (id, payload) => window.log.push(['pan', id, payload]),
    release: (id, payload) => window.log.push(['panrelease', id, payload]), velocity });
  views.get(1).exactHandlers = ['pan', 'panrelease'];
  views.get(2).exactHandlers = ['pan'];
  for (const [id, el] of views) { const on = (type, handle) => el.addEventListener(type, handle); el.addEventListener('pointerdown', handlers.pan(el, id, on)); }
  window.ready = true;
</script>`;

check('a pan that began releases once with its velocity; a tap none; a cancel at rest', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-panrelease-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    const frame = () => evaluate('new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)))');
    const mouse = (type, x, y) => call('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1 });
    const take = () => evaluate('window.log.splice(0)');
    const kinds = (log) => log.map(([kind]) => kind);

    // A rightward flick: 200 px in six moves.
    await mouse('mousePressed', 100, 150);
    for (let i = 1; i <= 6; i++) { await mouse('mouseMoved', 100 + i * 33, 150); await Bun.sleep(16); }
    await mouse('mouseReleased', 300, 150);
    await frame();
    let log = await take();
    expect(log[0]).toEqual(['sample', 1, true]);
    const pans = log.filter(([kind]) => kind === 'pan');
    expect(pans.length).toBeGreaterThan(0);
    expect(pans.reduce((sum, [, , p]) => sum + Number(p.split(',')[0]), 0)).toBe(200);
    const releases = log.filter(([kind]) => kind === 'panrelease');
    expect(releases.length).toBe(1);
    expect(kinds(log).lastIndexOf('pan')).toBeLessThan(kinds(log).indexOf('panrelease')); // the last delta, then the release
    const [vx, vy] = releases[0][2].split(',').map(Number);
    expect(vx).toBeGreaterThan(100); // 200 px over the CDP round trips' real time
    expect(Math.abs(vy)).toBeLessThan(1);

    // A tap that never leaves the slop: no pan, no release.
    await mouse('mousePressed', 100, 150); await mouse('mouseMoved', 102, 151); await mouse('mouseReleased', 102, 151);
    await frame();
    log = await take();
    expect(kinds(log).filter(k => k !== 'sample')).toEqual([]);

    // A node without a panrelease handler pans and releases nothing.
    await mouse('mousePressed', 100, 390); await mouse('mouseMoved', 180, 390); await Bun.sleep(20); await mouse('mouseReleased', 180, 390);
    await frame();
    log = await take();
    expect(kinds(log)).toContain('pan');
    expect(kinds(log)).not.toContain('panrelease');

    // A touch contact the platform cancels after the pan began releases at rest.
    await call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 2 });
    await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: 100, y: 150 }] });
    for (let i = 1; i <= 4; i++) { await call('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: 100 + i * 20, y: 150 }] }); await Bun.sleep(16); }
    await frame();
    await call('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] });
    await frame();
    log = await take();
    expect(kinds(log)).toContain('pan');
    expect(log.filter(([kind]) => kind === 'panrelease')).toEqual([['panrelease', 1, '0,0']]);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 60000);
