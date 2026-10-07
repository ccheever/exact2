// The grouped reorder's ghost (LLP 1094 D6) in a real browser: a deep clone
// of the row in a manual popover (the top layer), carrying the row's look
// inline and none of the five identities a node of the app's has, so no
// glue adopts it, no handler finds it and no driver names it.
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
<style>#exact-root .card { display: flex; gap: 6px; background: rgb(1, 2, 3); }</style>
<div id="exact-root">
  <div data-listitemkey="s:c1" data-view="5" id="wrap" style="translate: 0px 40px; transition: translate 1s">
    <div class="card" id="card" data-view="6" data-exact-on="press" data-agent-view="6" data-testid="card-c1">
      <div id="grip" data-view="7" data-testid="grip-c1" style="width: 20px; height: 20px">g</div>
      <span data-testid="title">Draft</span>
    </div>
  </div>
</div>
<script type="module">
  import { ghostOf } from './group-glue.js';
  const g = ghostOf(document.getElementById('wrap'), false);
  const all = [g.el, ...g.el.querySelectorAll('*')];
  window.result = {
    open: g.el.matches(':popover-open'),
    left: ${'`${STRIP}`'}.split(',').filter(a => all.some(el => el.hasAttribute(a))),
    card: getComputedStyle(g.el.firstElementChild.firstElementChild).backgroundColor,
    translate: g.el.firstElementChild.style.translate,
    pointer: getComputedStyle(g.el).pointerEvents,
    shadow: g.el.style.boxShadow,
  };
</script>`.replace('${STRIP}', 'id,data-view,data-exact-on,data-agent-view,data-testid');

check('the ghost is a stripped clone in the top layer, with the row\'s look', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'group-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-group-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => (await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result.value;
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    let r = null;
    for (let i = 0; !(r = await evaluate('window.result')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    expect(r.open).toBe(true);
    expect(r.left).toEqual([]);
    expect(r.card).toBe('rgb(1, 2, 3)'); // the page's rule, inlined: the top layer is outside the root
    expect(r.translate).toBe('none'); // the row's own offset is not the ghost's
    expect(r.pointer).toBe('none');
    expect(r.shadow).toContain('24px');
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 30_000);

// LLP 1094 D7 (amended 2026-10-07): a centre past a port's edge is that
// edge's, even in a port shorter than the band. A 120 px row in a 24 px
// port, grabbed near its top and held: its centre is ~60 px below the port,
// and the port scrolls down, not up.
const small = `<!doctype html>
<div id="exact-root">
  <div id="port" data-view="2" data-reordergroup="g" style="height: 24px; width: 200px; overflow: auto; margin-top: 100px">
    <div id="wrap" data-view="3" style="height: 120px; background: rgb(9, 9, 9)"><div id="grip" data-view="4" style="height: 20px">g</div></div>
    <div style="height: 400px"></div>
  </div>
</div>
<script type="module">
  import { groupController } from './group-glue.js';
  const port = document.getElementById('port'), wrap = document.getElementById('wrap'), grip = document.getElementById('grip');
  const views = new Map([[2, port], [3, wrap], [4, grip]]);
  const mapping = () => ({ port, revision: 1, scrollSequence: 1, scrollTop: port.scrollTop, portWidth: 200, portHeight: 24, rowWidth: 200, totalExtent: 520, contentY: 0 });
  const collections = { reorderMapping: () => mapping(), retainInteraction: () => 1, releaseRetainedInteraction() {}, releaseInteraction() {}, reorderContact() {} };
  const ctrl = groupController({ views, collections, request: () => ({ accepted: true, token: 1 }), applyBatch() {}, now: () => performance.now(),
    ready: () => true, inert: () => false, root: document.getElementById('exact-root'), gripOf: () => grip });
  const b = { el: grip, row: wrap, list: 2, runtime: 0, handleKey: 4, listKey: 2, wrapperKey: 3, rootKey: 1, rowEpoch: 1 };
  grip.addEventListener('pointerdown', e => ctrl.down(b, e, { group: 'g', keys: false }));
  window.ready = true;
</script>`;

check('a centre past a short port\'s edge scrolls toward that edge', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(small); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'group-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-group-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => (await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result.value;
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    const g = await evaluate('(() => { const r = document.getElementById("grip").getBoundingClientRect(); return [r.left + 10, r.top + 6]; })()');
    const mouse = (type, x, y) => call('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1 });
    await mouse('mousePressed', g[0], g[1]);
    // Past the slop, then held still a few pixels lower, inside the port.
    for (const dy of [4, 9, 12]) { await mouse('mouseMoved', g[0], g[1] + dy); await Bun.sleep(16); }
    const ghost = await evaluate('(() => { const r = document.querySelector("[data-exact-ghost]")?.getBoundingClientRect(); const p = document.getElementById("port").getBoundingClientRect(); return r && [r.top + r.height / 2, p.bottom]; })()');
    expect(ghost).not.toBeNull();
    expect(ghost[0]).toBeGreaterThan(ghost[1]); // the centre is below the port
    await Bun.sleep(300);
    const scrolled = await evaluate('document.getElementById("port").scrollTop');
    await mouse('mouseReleased', g[0], g[1] + 12);
    expect(scrolled).toBeGreaterThan(0); // down, toward the centre's edge
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 30_000);

