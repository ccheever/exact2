// A wasm client page given no frames still activates (LLP 1122's queue item
// (3), its QUEUE entry "The wasm client shell activates only after two
// animation frames", from T4's gate): a cross-origin frame scrolled out of
// view, the design tool's canvas scrolled away, gets no requestAnimationFrame.
// glue.js activates a second on all the same, once, as the JS page's input
// does (rt.js `input()`); a page with frames activates after its two frames,
// as before, and also once. A canvas's after-paint pieces likewise (its QUEUE
// follow-up, navigation.js `afterPaint`, canvas2d-glue.js's first geometry):
// in a cross-origin frame below the fold, which Chrome gives no frames and no
// ResizeObserver entries, a Canvas 2D page loads its piece once and is ready.
import { afterEach, expect, test } from 'bun:test';
import { createServer } from 'node:net';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { spawnOwned, stopAll } from '../../../scripts/test-children.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
afterEach(() => stopAll());
const free = () => new Promise(ok => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => ok(port)); }); });

// Counts activation's marks: each activation sets the root's `data-module-ready` (glue.js `activateData`),
// and a set is a mutation record whether or not the value changes; and the runner's `exact_data_ready`
// calls, through a plain copy of the exports (a Proxy may not wrap their frozen functions).
const COUNT = () => {
  window.__ready = 0; window.__dataReady = 0;
  new MutationObserver(records => { for (const r of records) if (r.attributeName === 'data-module-ready') window.__ready++; })
    .observe(document, { attributes: true, subtree: true, attributeFilter: ['data-module-ready'] });
  const streaming = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async (...args) => {
    const result = await streaming(...args), exports = result.instance.exports;
    if (typeof exports.exact_data_ready !== 'function') return result;
    const counted = { ...exports, exact_data_ready: (...a) => { window.__dataReady++; return exports.exact_data_ready(...a); } };
    return { module: result.module, instance: { exports: counted } };
  };
  if (new URL(location.href).searchParams.has('noframes')) window.requestAnimationFrame = () => 0;
};

// An app's wasm dev loop, served and its first generation ready, each heard as it is said (the tail kept for a
// failure is the last 8 KB): a generation published later would restart the page (glue.js), a second activation by
// design. (A first compile that fails says `error:` (dev.mjs) and never a ready line: said at once, not after the wait.)
async function devLoop(app, dist, env = {}) {
  const port = await free();
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', app, '--wasm', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '', served = false, planned = false, refused = false;
  const hear = d => { said += d; served ||= said.includes(`127.0.0.1:${port}/`); planned ||= /plan ready:|module generation ready/.test(said); refused ||= !planned && /^error: /m.test(said); said = said.slice(-8000); };
  dev.stdout.on('data', hear);
  dev.stderr.on('data', hear);
  for (const end = Date.now() + 600_000; !served || !planned;) { if (Date.now() > end || dev.exitCode !== null || refused) { await dev.stop(15_000); throw new Error(`the dev loop never served: ${said}`); } await Bun.sleep(100); }
  return { dev, base: `http://127.0.0.1:${port}` };
}

test('a wasm page given no frames activates within its second, once; one with frames after them, once', async () => {
  const dist = mkdtempSync(join(tmpdir(), 'exact-noframes-dist-'));
  let dev = null, browser = null;
  try {
    let base;
    ({ dev, base } = await devLoop('theme-fixture', dist));
    browser = await (await import('playwright-core')).chromium.launch({ executablePath: chromium().executable, headless: true });
    const page = await browser.newPage();
    await page.addInitScript(COUNT);
    const state = () => page.evaluate(() => ({ ready: document.getElementById('exact-root')?.dataset.moduleReady ?? null, frame: document.getElementById('exact-root')?.dataset.frameCallbackMs ?? null, calls: window.__ready, dataReady: window.__dataReady }));
    // No frames: activated after about a second, with no frame stamped, once.
    await page.goto(`${base}/?noframes=1`);
    const t0 = Date.now();
    for (let s = await state(); s.ready !== 'true'; s = await state()) { if (Date.now() - t0 > 8000) throw new Error(`no activation without frames: ${JSON.stringify(s)}`); await Bun.sleep(50); }
    await Bun.sleep(1500);
    expect(await state()).toEqual({ ready: 'true', frame: null, calls: 1, dataReady: 1 });
    // With frames: after them (the frame stamped), and once though the second's timer fires too.
    await page.goto(`${base}/`);
    const t1 = Date.now();
    for (let s = await state(); s.ready !== 'true'; s = await state()) { if (Date.now() - t1 > 8000) throw new Error(`no activation with frames: ${JSON.stringify(s)}`); await Bun.sleep(50); }
    await Bun.sleep(1500);
    const framed = await state();
    expect([framed.ready, framed.frame !== null, framed.calls, framed.dataReady]).toEqual(['true', true, 1, 1]);
  } finally {
    await browser?.close();
    await dev?.stop(15_000);
    rmSync(dist, { recursive: true, force: true });
  }
}, 900_000);

test('a Canvas 2D page in a cross-origin frame scrolled out of view (no frames, no ResizeObserver) loads its piece once and is ready within its second or so', async () => {
  const dist = mkdtempSync(join(tmpdir(), 'exact-noframes-canvas-'));
  let dev = null, browser = null, host = null;
  try {
    // The page framed by another origin (the design tool's, as the dev loop's `frame-ancestors` names it), below the
    // fold: Chrome gives such a frame no animation frames and no ResizeObserver entries.
    const toolPort = await free();
    let base;
    ({ dev, base } = await devLoop('canvas-gallery', dist, { EXACT_TOOL_PORT: String(toolPort) }));
    const framed = base.replace('127.0.0.1', 'localhost');
    host = Bun.serve({ port: toolPort, hostname: '127.0.0.1', fetch: () => new Response(`<!doctype html><body style="margin:0"><div style="height:6000px"></div><iframe src="${framed}/" style="width:600px;height:400px"></iframe></body>`, { headers: { 'content-type': 'text/html' } }) });
    browser = await (await import('playwright-core')).chromium.launch({ executablePath: chromium().executable, headless: true });
    const page = await browser.newPage({ viewport: { width: 800, height: 600 } }), asked = [];
    page.on('request', r => { if (new URL(r.url()).pathname.endsWith('/canvas2d-glue.js')) asked.push(r.url()); });
    await page.goto(`http://127.0.0.1:${toolPort}/`);
    let frame = null;
    for (const end = Date.now() + 10_000; !frame; await Bun.sleep(50)) { frame = page.frames().find(f => f.url().startsWith(framed)); if (!frame && Date.now() > end) throw new Error('the frame never loaded'); }
    // Ready (input and `exact.ready` wait on the piece and its canvases' first geometry): the piece loaded and settled.
    const t0 = Date.now();
    while ((await frame.evaluate(() => document.getElementById('exact-root')?.dataset.moduleReady ?? null)) !== 'true') { if (Date.now() - t0 > 8000) throw new Error(`not ready offscreen; the piece asked ${asked.length} time(s)`); await Bun.sleep(50); }
    // It is the case it says: the frame gets no animation frames and no ResizeObserver entries in a second.
    const starved = await frame.evaluate(() => new Promise(done => { let frames = 0, sizes = 0; const tick = () => { frames++; requestAnimationFrame(tick); }; requestAnimationFrame(tick); const d = document.createElement('div'); document.body.append(d); new ResizeObserver(() => { sizes++; }).observe(d); setTimeout(() => done({ frames, sizes }), 1000); }));
    expect([starved, asked.length]).toEqual([{ frames: 0, sizes: 0 }, 1]);
  } finally {
    await browser?.close();
    host?.stop(true);
    await dev?.stop(15_000);
    rmSync(dist, { recursive: true, force: true });
  }
}, 900_000);
