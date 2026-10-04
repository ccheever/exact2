// `pointerdown`/`pointerup` in a real browser (LLP 1005 §Events): the input
// glue's `pointer`, driven by CDP mouse events. Down before any press, up
// wherever the button lifts (heard on the document; no click), the primary button only,
// nothing on a disabled node; `pointermove` and the `PointerEvent` record
// (LLP 1056 §8.6): a held pointer's moves anywhere, a free one's over it.
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
  <button id="mic" style="width:100px;height:100px">m</button>
  <button id="off" disabled style="width:100px;height:50px">d</button>
  <div id="outer" style="width:200px;height:120px;padding:10px"><button id="inner" style="width:100px;height:60px">i</button></div>
  <div id="wrap" style="width:200px;height:80px;padding:10px"><button id="child" style="width:100px;height:40px">c</button></div>
  <div id="dparent" style="width:200px;height:80px;padding:10px"><div id="dkid" disabled style="width:100px;height:40px">k</div></div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  const h = createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  window.log = []; window.records = [];
  for (const id of ['mic', 'off', 'outer', 'inner', 'wrap', 'dparent', 'dkid']) {
    const el = document.getElementById(id);
    el.exactHandlers = ['pointerdown', 'pointerup', 'press', ...(id === 'mic' ? ['pointermove'] : [])];
    const on = (type, f) => el.addEventListener(type, f);
    let p;
    const own = () => p ??= h.pointer(el, on, (k, r) => { window.log.push(id + (k === 29 ? ' down' : k === 30 ? ' up' : ' move')); window.records.push(r); });
    on('pointerdown', e => own()(e));
    on('pointerover', () => own());
    on('click', () => window.log.push(id + ' press'));
  }
  document.getElementById('child').addEventListener('click', () => window.log.push('child press'));
  window.ready = true;
</script>`;

check('down before the press, up wherever the button lifts, nothing when disabled', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-pointer-'));
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
    const centre = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    // `force` as a mouse's hardware reports it: DOM's 0.5 while a button is down.
    const mouse = (type, [x, y], button = 'left', buttons = type === 'mouseReleased' ? 0 : button === 'left' ? 1 : 2) => call('Input.dispatchMouseEvent', { type, x, y, button, buttons, force: buttons ? 0.5 : 0, clickCount: 1 });
    const log = () => evaluate('window.log.splice(0)');
    const frame = () => evaluate('new Promise(r => requestAnimationFrame(() => r()))');
    const records = () => evaluate('window.records.splice(0).map(r => r.split(","))');

    const mic = await centre('mic');
    await mouse('mousePressed', mic);
    // Focus moving (the press focuses the button, blurring what had it)
    // does not end the hold.
    await evaluate(`document.getElementById('off').focus(); document.getElementById('mic').focus()`);
    expect(await log()).toEqual(['mic down']);
    await mouse('mouseReleased', mic);
    expect(await log()).toEqual(['mic up', 'mic press']);
    // Lifted far away: the up still arrives, and there is no click; the
    // held pointer's move is the mic's there too, from its content box.
    await records();
    await mouse('mousePressed', mic);
    await mouse('mouseMoved', [500, 800], 'left', 1);
    await frame();
    await mouse('mouseReleased', [500, 800]);
    expect(await log()).toEqual(['mic down', 'mic move', 'mic up']);
    const [down, held, up] = await records();
    const box = await evaluate(`(() => { const e = document.getElementById('mic'), r = e.getBoundingClientRect(), s = getComputedStyle(e); return [r.x + parseFloat(s.borderLeftWidth) + parseFloat(s.paddingLeft), r.y + parseFloat(s.borderTopWidth) + parseFloat(s.paddingTop)]; })()`);
    expect(down.slice(2)).toEqual(['1', '0.5', 'mouse', '1', '']); // no modifier held (gallery F20)
    expect([+held[0], +held[1]]).toEqual([500 - box[0], 800 - box[1]]);
    expect(held.slice(2, 5)).toEqual(['1', '0.5', 'mouse']);
    expect(up.slice(2, 4)).toEqual(['0', '0']);
    // A free pointer moving over it: its move, no button down.
    await mouse('mouseMoved', mic, 'none', 0);
    await frame();
    expect(await log()).toEqual(['mic move']);
    expect((await records())[0].slice(2, 5)).toEqual(['0', '0', 'mouse']);
    // The secondary button is not the pointer's.
    await mouse('mousePressed', mic, 'right');
    await mouse('mouseReleased', mic, 'right');
    expect((await log()).filter(l => !l.endsWith('press'))).toEqual([]);
    // A disabled node hears nothing.
    const off = await centre('off');
    await mouse('mousePressed', off);
    await mouse('mouseReleased', off);
    expect(await log()).toEqual([]);
    // Nested pointer nodes: the innermost takes the pointer.
    const inner = await centre('inner');
    await mouse('mousePressed', inner);
    await mouse('mouseReleased', inner);
    expect((await log()).filter(l => !l.endsWith('press'))).toEqual(['inner down', 'inner up']);
    // A pointer node around a pressable child leaves the child its press.
    const kid = await centre('child');
    await mouse('mousePressed', kid);
    await mouse('mouseReleased', kid);
    expect(await log()).toEqual(['wrap down', 'wrap up', 'child press', 'wrap press']);
    // A disabled non-control node passes the pointer to its enabled parent.
    const dkid = await centre('dkid');
    await mouse('mousePressed', dkid);
    await mouse('mouseReleased', dkid);
    expect((await log()).filter(l => !l.endsWith('press'))).toEqual(['dparent down', 'dparent up']);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 30_000);
