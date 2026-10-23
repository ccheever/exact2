// A view's first intersection entry (`intersect=`) is the page's to settle, as its first size is
// (resize-boot.test.mjs): an IntersectionObserver always delivers one, a frame or more after the
// observer is attached (observers.js, loaded after first paint), and on a loaded machine later still.
// conform's boot step once read the state before it (QUEUE 2026-10-21: `synthetic-intersect`'s
// `slots.calls: wasm 1 js 0`, `seen: wasm "row:false:0" js "none"`). An agent's settle now waits for
// each observation's first entry (2 s at most). The entry is held back here, as a loaded machine holds it.
import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const root = resolve(import.meta.dir, '../..');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' };

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

async function built() {
  const dir = mkdtempSync(resolve(root, 'target/intersect-boot-test-'));
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Intersect Boot Test', app: { id: 'test.intersectboot', name: 'Intersect Boot Test' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), APP);
  const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'intersect-boot-test', '--out', resolve(dir, 'dist')], {
    cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
  });
  const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
  expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
  return dir;
}

// `gated`: a file whose name matches is served once `open()` is called (a loaded machine's slow module).
function serve(dir, gated = null) {
  let open;
  const opened = new Promise((r) => { open = r; });
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    let path = new URL(req.url).pathname;
    if (path.endsWith('/')) path += 'index.html';
    const file = Bun.file(resolve(dir, 'dist', '.' + path));
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

// Each IntersectionObserver entry held until the page says `__entries()` (a loaded machine's late entry), and no
// 2 s bound, so only its own release lets a settle go.
const heldEntries = () => {
  globalThis.__exactFirstSizeBound = 1e9;
  const Real = globalThis.IntersectionObserver, held = [];
  let open = false;
  globalThis.__entries = () => { open = true; for (const f of held.splice(0)) f(); };
  globalThis.IntersectionObserver = class extends Real {
    constructor(callback, options) {
      super((entries, observer) => {
        const deliver = () => callback(entries, observer);
        if (open) deliver(); else held.push(deliver);
      }, options);
    }
  };
};

test("an agent's settle waits for a view's first intersection entry, however late it comes, and a view gone first holds nothing", async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const dir = await built();
  let browser, server;
  try {
    browser = await suiteBrowser();
    server = serve(dir);
    const page = await browser.newPage();
    await page.addInitScript(heldEntries);
    await page.addInitScript(watchObserve);
    await page.goto(`${server.url.href}?agent=1`);
    await page.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    // Begun once the observer is attached (before, its hold was gone by then and the settle answered first:
    // calls 0, seen "none"), the entry released 300 ms in.
    const held = await page.evaluate(async () => {
      await globalThis.__observed;
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
    await gone.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    const dropped = await gone.evaluate(async () => {
      await globalThis.__observed;
      document.querySelector('[data-testid="hide"]').click();
      const began = performance.now(), settled = await globalThis.exact.agentSettled({ op: 'clock', settle: true });
      return { settled, slots: (await globalThis.exact.agentSettled({ op: 'state' })).slots, ms: performance.now() - began };
    });
    expect(dropped.settled).toMatchObject({ settled: true });
    expect(dropped.ms).toBeLessThan(1500);
    expect(dropped.slots).toMatchObject({ calls: 0, shown: false });
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 300_000);
