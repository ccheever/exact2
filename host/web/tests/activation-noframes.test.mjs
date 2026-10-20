// A wasm client page given no frames still activates (QUEUE "The wasm client
// shell activates only after two animation frames"; LLP 1122 T4's gate): a
// cross-origin frame scrolled out of view, the design tool's canvas scrolled
// away, gets no requestAnimationFrame. glue.js activates a second on all the
// same, once, as the JS page's input does (rt.js `input()`); a page with
// frames activates after its two frames, as before, and also once.
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

test('a wasm page given no frames activates within its second, once; one with frames after them, once', async () => {
  const dist = mkdtempSync(join(tmpdir(), 'exact-noframes-dist-'));
  const port = await free(), base = `http://127.0.0.1:${port}`;
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'theme-fixture', '--wasm', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  dev.stdout.on('data', d => { said = (said + d).slice(-8000); });
  dev.stderr.on('data', d => { said = (said + d).slice(-8000); });
  // Served, and its first plan ready: a generation published later would restart the page (glue.js), a second activation by design.
  for (const end = Date.now() + 600_000; !said.includes(`127.0.0.1:${port}/`) || !said.includes('plan ready:');) { if (Date.now() > end || dev.exitCode !== null) throw new Error(`the dev loop never served: ${said}`); await Bun.sleep(100); }
  const browser = await (await import('playwright-core')).chromium.launch({ executablePath: chromium().executable, headless: true });
  try {
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
    await browser.close();
    await dev.stop(15_000);
    rmSync(dist, { recursive: true, force: true });
  }
}, 900_000);
