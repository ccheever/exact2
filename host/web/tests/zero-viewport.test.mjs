// A cold WebView may run before layout. Exercise the real wasm runner in a
// zero-sized iframe, including a served document and a dev-plan restart.
import { expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { extname, resolve } from 'node:path';
import { assertWebDistApp } from '../../../scripts/agent.mjs';
import { chromium, refuseStale, suiteBrowser, webChanges } from '../../../scripts/agent-launch.mjs';
import { resolveApp } from '../../../scripts/app.mjs';
import { need, notApplicable } from '../../../scripts/coverage.mjs';
import { jsTargetBuild } from '../serve.mjs';
import { withLook } from '../look.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));
async function prerequisite() {
  const { unavailable } = chromium();
  if (unavailable) return ['chrome', unavailable];
  try {
    const app = resolveApp('caltrain');
    await assertWebDistApp(dist, app);
    if (jsTargetBuild(dist)) throw new Error('zero-viewport needs a wasm build: bun host/web/build.mjs caltrain-web --wasm');
    refuseStale('web', resolve(dist, '.exact-build.json'), webChanges(dist, app).all,
      'bun host/web/build.mjs caltrain-web --wasm');
  } catch (error) { return ['fixture', error.message]; }
  return ['chrome', null];
}
const check = process.env.EXACT_GLUE_FAST === '1'
  ? notApplicable('the web-ax-test step runs this test against the built wasm Caltrain app') : need(...await prerequisite());
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' };

// Observe entry to the viewport wait, or a boot attempt on a host without
// that wait. This lets a negative assertion follow the boot's work, without
// sleeping and mistaking a slow download for a correctly deferred boot.
function observeBoot() {
  globalThis.__bootCalls = []; globalThis.__resizeCalls = []; globalThis.__waiting = 0;
  const listen = window.addEventListener.bind(window);
  window.addEventListener = (kind, fn, options) => {
    if (kind === 'resize' && options?.once && globalThis.exact?.wasm) globalThis.__waiting++;
    return listen(kind, fn, options);
  };
  const instantiate = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async (...args) => {
    const result = await instantiate(...args), exports = result.instance.exports;
    if (!exports.exact_boot) return result;
    const wrapped = { ...exports };
    for (const name of ['exact_boot', 'exact_boot_plan', 'exact_resize']) {
      wrapped[name] = (...args) => {
        const size = name === 'exact_boot_plan' ? args.slice(1, 3) : args.slice(0, 2);
        (name === 'exact_resize' ? __resizeCalls : __bootCalls).push(size);
        return exports[name](...args);
      };
    }
    return { module: result.module, instance: { exports: wrapped } };
  };
}

const status = frame => frame.evaluate(() => ({
  size: [innerWidth, innerHeight], boots: __bootCalls, resizes: __resizeCalls,
  generation: globalThis.exact?.generation ?? 0,
  error: document.getElementById('exact-root')?.dataset.error ?? null,
  booted: document.getElementById('exact-root')?.dataset.bootMs != null,
}));
const waiting = (frame, previous = 0) => frame.waitForFunction(n => __waiting > n
  || document.getElementById('exact-root')?.dataset.error || globalThis.__reloadError,
previous, { polling: 50, timeout: 30000 });
async function resize(page, frame, width, height) {
  await frame.evaluate(() => { globalThis.__resized = new Promise(done => addEventListener('resize', done, { once: true })); });
  await page.locator('iframe').evaluate((el, [w, h]) => { el.style.width = `${w}px`; el.style.height = `${h}px`; }, [width, height]);
  await frame.evaluate(() => globalThis.__resized);
}
async function booted(frame) {
  await frame.waitForFunction(() => document.getElementById('exact-root')?.dataset.bootMs != null,
    null, { polling: 50, timeout: 30000 });
  await frame.evaluate(() => globalThis.exact.ready);
  expect((await status(frame)).error).toBeNull();
  expect(await frame.locator('[data-testid="change-station"]').count()).toBeGreaterThan(0);
}

function serve(documentPage) {
  return Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === '/frame') return new Response(`<!doctype html><iframe
      style="border:0;width:${Number(url.searchParams.get('w'))}px;height:${Number(url.searchParams.get('h'))}px"
      src="/${url.searchParams.has('document') ? '' : 'index.html'}?agent=1"></iframe>`,
    { headers: { 'content-type': 'text/html' } });
    if (url.pathname === '/') return new Response(documentPage, { headers: { 'content-type': 'text/html' } });
    const file = Bun.file(resolve(dist, '.' + url.pathname));
    return await file.exists() ? new Response(file, { headers: { 'content-type': TYPES[extname(url.pathname)] ?? 'application/octet-stream' } })
      : new Response('', { status: 404 });
  } });
}

check('wasm boot waits for both viewport dimensions; zero resizes and plan reloads recover', async () => {
  const browser = await suiteBrowser(), server = serve();
  try {
    for (const [width, height] of [[0, 0], [0, 300], [400, 0]]) {
      const page = await browser.newPage();
      await page.addInitScript(observeBoot);
      await page.goto(`${server.url}frame?w=${width}&h=${height}`);
      const frame = page.frames().find(f => f !== page.mainFrame());
      await waiting(frame);
      expect(await status(frame)).toEqual({ size: [width, height], boots: [], resizes: [], generation: 0, error: null, booted: false });
      await resize(page, frame, 400, 300);
      await booted(frame);
      expect((await status(frame)).boots).toEqual([[400, 300]]);

      // A running runner retains its validated viewport while hidden.
      const before = await status(frame);
      await resize(page, frame, 0, 0);
      expect((await status(frame)).resizes).toEqual(before.resizes);
      await resize(page, frame, 500, 350);
      expect((await status(frame)).resizes.at(-1)).toEqual([500, 350]);
      expect((await status(frame)).generation).toBe(before.generation);

      await resize(page, frame, 0, 0);
      const waits = await frame.evaluate(() => __waiting);
      await frame.evaluate(async () => {
        const bytes = new Uint8Array(await fetch('/app.plan').then(r => r.arrayBuffer()));
        globalThis.__reload = exact.reload(bytes, true).catch(e => { globalThis.__reloadError = String(e); });
      });
      await waiting(frame, waits);
      expect((await status(frame)).boots).toEqual([[400, 300]]);
      expect(await frame.evaluate(() => globalThis.__reloadError ?? null)).toBeNull();
      await resize(page, frame, 420, 320);
      await frame.evaluate(() => __reload);
      expect((await status(frame)).boots).toEqual([[400, 300], [420, 320]]);
      expect((await status(frame)).generation).toBe(before.generation + 1);
      expect(await frame.evaluate(() => exact.agent({ op: 'logs', since: 0 }))).not.toMatchObject({ error: expect.any(String) });
      expect(JSON.stringify(await frame.evaluate(() => exact.agent({ op: 'logs', since: 0 })))).not.toMatch(/InvalidViewport|not booted/);
      await page.close();
    }
  } finally { await browser.close(); server.stop(true); }
}, 120000);

check('a served document stays unbooted at zero size and adopts when it gets a viewport', async () => {
  const dir = mkdtempSync(resolve(ROOT, '.zero-viewport-'));
  let documentPage;
  try {
    const shell = resolve(dir, 'index.html');
    writeFileSync(shell, withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8')));
    const render = spawnSync('cargo', ['run', '-q', '-p', 'caltrain-linux', '--bin', 'caltrain-render', '--',
      '--plan', resolve(dist, 'app.plan'), '--viewport', '400x300', '--shell', shell, '/'],
    { cwd: ROOT, encoding: 'utf8' });
    expect(render.status, render.stderr + render.stdout).toBe(0);
    documentPage = JSON.parse(render.stdout.trim().split('\n').at(-1)).page;
  } finally { rmSync(dir, { recursive: true, force: true }); }
  expect(documentPage).toContain('application/vnd.exact.checkpoint');
  const browser = await suiteBrowser(), server = serve(documentPage);
  try {
    const page = await browser.newPage();
    await page.addInitScript(observeBoot);
    await page.goto(`${server.url}frame?w=0&h=0&document`);
    const frame = page.frames().find(f => f !== page.mainFrame());
    await waiting(frame);
    expect(await frame.locator('#exact-root > *').count()).toBeGreaterThan(0);
    expect((await status(frame)).error).toBeNull();
    expect((await status(frame)).boots).toEqual([]);
    // A partially sized document must not resize the not-yet-booted runner.
    await resize(page, frame, 400, 0);
    expect((await status(frame)).resizes).toEqual([]);
    expect((await status(frame)).boots).toEqual([]);
    await resize(page, frame, 400, 300);
    await booted(frame);
    expect((await status(frame)).boots).toEqual([[400, 300]]);
    expect(await frame.evaluate(async () => (await exact.agent({ op: 'state' })).adopted)).toBe(true);
    expect(JSON.stringify(await frame.evaluate(() => exact.agent({ op: 'logs', since: 0 })))).not.toMatch(/InvalidViewport|not booted/);
  } finally { await browser.close(); server.stop(true); }
}, 120000);
