// The wasm host's twin of host/web-js/intersect-boot.test.mjs: a view's first intersection entry (`intersect=`) is
// the page's to settle, as its first size is (glue.js's intersect kind). The wasm page won the race in recorded runs
// (conform's reference), but a loaded machine could make it lose, as the JS target's did. A plan runs here as conform
// runs a synthetic one: on a wasm Caltrain dist, by `exact.reload`. The entry is held back, as a loaded machine holds it.
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
  '  state calls = 0',
  '  state seen = "none"',
  '  state shown = true',
  '  action observe(name: string, e: IntersectionObserverEntry)',
  '    calls = calls + 1',
  '    seen = `${name}:${e.isIntersecting}`',
  '  action hide',
  '    shown = false',
  '  view',
  '    column',
  '      button "hide" testId="hide" press=hide',
  '      if shown',
  '        box testId="target" width=100 height=50 intersect=observe("row")',
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
    if (path === '/__plain') return new Response(Bun.file(`${plan}.plain`));
    if (path.endsWith('/')) path += 'index.html';
    const file = Bun.file(resolve(wasmDist, '.' + path));
    if (!(await file.exists())) return new Response('', { status: 404 });
    if (gated && gated.test(path)) await opened;
    return new Response(file, { headers: { 'content-type': TYPES[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' } });
  } });
  return Object.assign(server, { open });
}

// When the target is observed (the intersect glue attached to it): `__observed`, a promise.
const watchObserve = () => {
  let seen;
  globalThis.__observed = new Promise((r) => { seen = r; });
  const observe = IntersectionObserver.prototype.observe;
  IntersectionObserver.prototype.observe = function (el, ...rest) { if (el.dataset?.testid === 'target') seen(); return observe.call(this, el, ...rest); };
};

// The plan's entries held until the page says `__entries()` (a loaded machine's late entry), Caltrain's own
// observers left alone, and no 2 s bound, so only its own release lets a settle go.
const heldEntries = () => {
  globalThis.__exactFirstSizeBound = 1e9;
  const Real = globalThis.IntersectionObserver, held = [], ours = (x) => x.target.dataset?.testid === 'target';
  let open = false;
  globalThis.__entries = () => { open = true; for (const f of held.splice(0)) f(); };
  globalThis.IntersectionObserver = class extends Real {
    constructor(callback, options) {
      super((entries, observer) => {
        if (!entries.some(ours)) return callback(entries, observer);
        const deliver = () => callback(entries, observer);
        if (open) deliver(); else held.push(deliver);
      }, options);
    }
  };
};

const booted = (page) => page.waitForFunction(() => document.getElementById('exact-root')?.dataset.bootMs != null, null, { timeout: 30000 })
  .then(() => page.evaluate(() => globalThis.exact.ready));

check("the wasm host: an agent's settle waits for a view's first intersection entry, however late it comes, and a view gone first, or reloaded away, holds nothing", async () => {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-wasm-intersect-boot-')), plan = resolve(tmp, 'app.plan');
  let browser, server;
  try {
    writeFileSync(resolve(tmp, 'app.contract'), APP);
    const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(tmp, 'app.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
    expect(c.status, c.stderr).toBe(0);
    // The plan a reload brings in its place: no target, so nothing new to wait on.
    writeFileSync(resolve(tmp, 'plain.contract'), 'component App\n  view\n    text "plain"\n');
    const p = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(tmp, 'plain.contract'), '-o', `${plan}.plain`], { cwd: ROOT, encoding: 'utf8' });
    expect(p.status, p.stderr).toBe(0);
    browser = await suiteBrowser();
    server = serve(plan);
    const page = await browser.newPage();
    await page.addInitScript(heldEntries);
    await page.addInitScript(watchObserve);
    await page.goto(`${server.url.href}?agent=1`);
    await booted(page);
    // Entries held until 300 ms into the settle, begun once the target is observed: it waits for them (before,
    // nothing held it and it answered first, calls 0).
    const held = await page.evaluate(async () => {
      await globalThis.exact.agentSettled({ op: 'clock', settle: true });
      const bytes = await fetch('/__plan').then((r) => r.arrayBuffer()), loading = globalThis.exact.reload(new Uint8Array(bytes), true);
      await globalThis.__observed;
      await loading;
      const began = performance.now(), settling = globalThis.exact.agentSettled({ op: 'clock', settle: true });
      setTimeout(() => globalThis.__entries(), 300);
      const settled = await settling;
      return { settled, slots: (await globalThis.exact.agentSettled({ op: 'state' })).slots, ms: performance.now() - began };
    });
    expect(held.settled).toMatchObject({ settled: true });
    expect(held.ms).toBeGreaterThanOrEqual(300);
    expect(held.slots).toMatchObject({ calls: 1, seen: 'row:true' });
    await page.close();
    // The view gone before its entry comes (the entry never released, no bound): it holds nothing.
    const gone = await browser.newPage();
    await gone.addInitScript(heldEntries);
    await gone.addInitScript(watchObserve);
    await gone.goto(`${server.url.href}?agent=1`);
    await booted(gone);
    const dropped = await gone.evaluate(async () => {
      await globalThis.exact.agentSettled({ op: 'clock', settle: true });
      const bytes = await fetch('/__plan').then((r) => r.arrayBuffer()), loading = globalThis.exact.reload(new Uint8Array(bytes), true);
      await globalThis.__observed;
      await loading;
      document.querySelector('[data-testid="hide"]').click();
      const began = performance.now(), settled = await globalThis.exact.agentSettled({ op: 'clock', settle: true });
      return { settled, slots: (await globalThis.exact.agentSettled({ op: 'state' })).slots, ms: performance.now() - began };
    });
    expect(dropped.settled).toMatchObject({ settled: true });
    expect(dropped.ms).toBeLessThan(1500);
    expect(dropped.slots).toMatchObject({ calls: 0, shown: false });
    await gone.close();
    // A reload while a settle waits on the entry (never released, no bound): the settle lets it go with the old views.
    const swapped = await browser.newPage();
    await swapped.addInitScript(heldEntries);
    await swapped.addInitScript(watchObserve);
    await swapped.goto(`${server.url.href}?agent=1`);
    await booted(swapped);
    const reloaded = await swapped.evaluate(async () => {
      await globalThis.exact.agentSettled({ op: 'clock', settle: true });
      const bytes = new Uint8Array(await fetch('/__plan').then((r) => r.arrayBuffer())), loading = globalThis.exact.reload(bytes, true);
      await globalThis.__observed;
      await loading;
      const began = performance.now(), settling = globalThis.exact.agentSettled({ op: 'clock', settle: true });
      await new Promise((r) => setTimeout(r, 100));
      await globalThis.exact.reload(new Uint8Array(await fetch('/__plain').then((r) => r.arrayBuffer())), true);
      const settled = await settling;
      return { settled, ms: performance.now() - began };
    });
    expect(reloaded.ms).toBeLessThan(1500);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(tmp, { recursive: true, force: true });
  }
}, 300_000);
