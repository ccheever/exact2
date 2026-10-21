// The wasm host's twin of host/web-js/resize-boot.test.mjs: a view's first size (`resize=`) is the page's to settle,
// and it waits for the geometry its action's `frame()` reads (glue.js's resize kind). The wasm page won both races in
// every recorded run (conform's reference), but a loaded machine could make it lose, as the JS target's did (the
// clone's tab bar: `tabWidths -12`). A plan runs here as conform runs a synthetic one: on a wasm Caltrain dist, by
// `exact.reload`. The report is held back, as a loaded machine holds it.
import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { extname, resolve } from 'node:path';
import { assertWebDistApp } from '../../../scripts/agent.mjs';
import { chromium, refuseStale, suiteBrowser, webChanges } from '../../../scripts/agent-launch.mjs';
import { resolveApp } from '../../../scripts/app.mjs';
import { jsTargetBuild } from '../serve.mjs';
import { need } from '../../../scripts/coverage.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const wasmDist = resolve(process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' };

async function prerequisite() {
  const { unavailable } = chromium();
  if (unavailable) return ['chrome', unavailable];
  try {
    const app = resolveApp('caltrain');
    await assertWebDistApp(wasmDist, app);
    if (jsTargetBuild(wasmDist)) return ['fixture', `${wasmDist} is a JS-target build; this test reads the wasm target (EXACT_WEB_LINK=all bun host/web/build.mjs caltrain-web --wasm)`];
    // Without every capability linked the module imports no geometry, and `frame()` reads nothing whatever the order.
    let receipt;
    try { receipt = JSON.parse(readFileSync(resolve(wasmDist, 'bake.json'), 'utf8')); } catch { return ['fixture', `${wasmDist} has no readable bake receipt`]; }
    if (receipt.binary?.configuration?.flags?.EXACT_WEB_LINK !== 'all') return ['fixture', `${wasmDist} was not built with EXACT_WEB_LINK=all`];
    refuseStale('web', resolve(wasmDist, '.exact-build.json'), webChanges(wasmDist, app).all, 'EXACT_WEB_LINK=all bun host/web/build.mjs caltrain-web --wasm');
  } catch (error) { return ['fixture', error.message]; }
  return null;
}
const missing = await prerequisite();
const check = need(...(missing ?? ['chrome', null]));

const APP = [
  'component App',
  '  state widths = []',
  '  state hidden = 0',
  '  action measure(w: number, h: number)',
  '    widths = [frame("tab-a").width, frame("tab-b").width]',
  '  action unseen(w: number, h: number)',
  '    hidden = hidden + 1',
  '  view',
  '    column',
  '      row testId="tabs" resize=measure gap=8',
  '        text "Following" id="tab-a" testId="tab-a"',
  '        text "Discover" id="tab-b" testId="tab-b"',
  '      // No box: no first report comes, and a settle does not wait for one.',
  '      box testId="unseen" display="none" resize=unseen',
  '',
].join('\n');

// The dist and `/__plan`; `gated`: a file whose name matches is served once `open()` is called (a loaded machine's
// slow module).
function serve(plan, gated = null) {
  let open;
  const opened = new Promise((r) => { open = r; });
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    let path = new URL(req.url).pathname;
    if (path === '/__plan') return new Response(Bun.file(plan));
    if (path.endsWith('/')) path += 'index.html';
    const file = Bun.file(resolve(wasmDist, '.' + path));
    if (!(await file.exists())) return new Response('', { status: 404 });
    if (gated && gated.test(path)) await opened;
    return new Response(file, { headers: { 'content-type': TYPES[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' } });
  } });
  return Object.assign(server, { open });
}

// When the tab row is observed (the resize glue attached to it): `__observed`, a promise.
const watchObserve = () => {
  let seen;
  globalThis.__observed = new Promise((r) => { seen = r; });
  const observe = ResizeObserver.prototype.observe;
  ResizeObserver.prototype.observe = function (el, ...rest) { if (el.dataset?.testid === 'tabs') seen(); return observe.call(this, el, ...rest); };
};

// The plan's reports held until the page says `__reports()` (a loaded machine's late report), Caltrain's own
// observers left alone; the box with no layout (`unseen`) never reported at all; and no 2 s bound, so only its own
// release lets a settle go.
const heldReports = () => {
  globalThis.__exactFirstSizeBound = 1e9;
  const Real = globalThis.ResizeObserver, held = [], ours = (x) => ['tabs', 'unseen'].includes(x.target.dataset?.testid);
  let open = false;
  globalThis.__reports = () => { open = true; for (const f of held.splice(0)) f(); };
  globalThis.ResizeObserver = class extends Real {
    constructor(callback) {
      super((entries, observer) => {
        if (!entries.some(ours)) return callback(entries, observer);
        const shown = entries.filter((x) => x.target.dataset.testid !== 'unseen');
        if (!shown.length) return;
        const deliver = () => callback(shown, observer);
        if (open) deliver(); else held.push(deliver);
      });
    }
  };
};

const booted = (page) => page.waitForFunction(() => document.getElementById('exact-root')?.dataset.bootMs != null, null, { timeout: 30000 })
  .then(() => page.evaluate(() => globalThis.exact.ready));

check("the wasm host: an agent's settle waits for a view's first size, however late its report comes, and that size waits for the page's geometry; an element with no box holds nothing", async () => {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-wasm-resize-boot-')), plan = resolve(tmp, 'app.plan');
  let browser, server;
  try {
    writeFileSync(resolve(tmp, 'app.contract'), APP);
    const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(tmp, 'app.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
    expect(c.status, c.stderr).toBe(0);
    browser = await suiteBrowser();
    // Reports held until 300 ms into the settle: it waits for them (before, nothing held it and it answered first,
    // widths []). Begun once the observer is attached.
    server = serve(plan);
    const page = await browser.newPage();
    await page.addInitScript(heldReports);
    await page.addInitScript(watchObserve);
    await page.goto(`${server.url.href}?agent=1`);
    await booted(page);
    const held = await page.evaluate(async () => {
      // Caltrain's own startup loads (its images' hold, which a clock awaits first) done before the 300 ms begin.
      await globalThis.exact.agentSettled({ op: 'clock', settle: true });
      const bytes = await fetch('/__plan').then((r) => r.arrayBuffer()), loading = globalThis.exact.reload(new Uint8Array(bytes), true);
      await globalThis.__observed;
      await loading;
      const began = performance.now(), settling = globalThis.exact.agentSettled({ op: 'clock', settle: true });
      setTimeout(() => globalThis.__reports(), 300);
      const settled = await settling;
      return { settled, slots: (await globalThis.exact.agentSettled({ op: 'state' })).slots, ms: performance.now() - began };
    });
    expect(held.settled).toMatchObject({ settled: true });
    expect(held.ms).toBeGreaterThanOrEqual(300);
    expect(held.slots.widths.length).toBe(2);
    expect(held.slots.widths.every((w) => w > 10)).toBe(true);
    // The box with no layout, never reported and with no bound to end its wait, held nothing.
    expect(held.slots.hidden).toBe(0);
    await page.close();
    server.stop(true);
    // The page's geometry served only once the resize glue has run and a frame passed (a resize glue that did not wait
    // has observed the tab row by then), and two more frames if it did (its report run on unavailable frames): the
    // first size waits for it, so the tabs are measured, not read as nothing (the clone's `-12`).
    server = serve(plan, /geometry-glue/);
    const late = await (await browser.newContext()).newPage();
    await late.exposeFunction('__geometry', () => server.open());
    await late.addInitScript(watchObserve);
    await late.goto(`${server.url.href}?agent=1`);
    await booted(late);
    const slow = await late.evaluate(async () => {
      const frames = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      const bytes = await fetch('/__plan').then((r) => r.arrayBuffer());
      let observed = false;
      globalThis.__observed.then(() => { observed = true; });
      await globalThis.exact.reload(new Uint8Array(bytes), true);
      const settling = globalThis.exact.agentSettled({ op: 'clock', settle: true }), end = performance.now() + 20000;
      while (!globalThis.exact.observeResize) {
        if (performance.now() > end) throw new Error('the resize glue never ran');
        await new Promise((r) => requestAnimationFrame(r));
      }
      await new Promise((r) => requestAnimationFrame(r));
      if (observed) await frames();
      await globalThis.__geometry();
      const settled = await settling;
      return { settled, slots: (await globalThis.exact.agentSettled({ op: 'state' })).slots };
    });
    expect(slow.settled).toMatchObject({ settled: true });
    expect(slow.slots.widths.length).toBe(2);
    expect(slow.slots.widths.every((w) => w > 10)).toBe(true);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(tmp, { recursive: true, force: true });
  }
}, 300_000);
