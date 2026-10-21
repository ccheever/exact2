// A view's first size (`resize=`) is the page's to settle: the JS target's
// ResizeObserver reports it a frame or more after the observer is attached
// (observers.js, loaded after first paint), and on a loaded machine later
// still. The clone's tab bar measures its tabs so (`resize=measureTabs`), and
// conform's boot step read the tabs before that report (iosfix's stage 12
// gate: `slots.tabWidths.0: wasm 63.70 js -12`). An agent's settle now waits
// for each element's first report (an element with no box waits for none,
// one of no size 2 s at most). The report is held back here,
// as a loaded machine holds it.
import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const root = resolve(import.meta.dir, '../..');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' };

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

async function built() {
  const dir = mkdtempSync(resolve(root, 'target/resize-boot-test-'));
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Resize Boot Test', app: { id: 'test.resizeboot', name: 'Resize Boot Test' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), APP);
  const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'resize-boot-test', '--out', resolve(dir, 'dist')], {
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

// When the tab row is observed (the resize glue attached to it): `__observed`, a promise.
const watchObserve = () => {
  let seen;
  globalThis.__observed = new Promise((r) => { seen = r; });
  const observe = ResizeObserver.prototype.observe;
  ResizeObserver.prototype.observe = function (el, ...rest) { if (el.dataset?.testid === 'tabs') seen(); return observe.call(this, el, ...rest); };
};

// Each ResizeObserver report held until the page says `__reports()` (a loaded machine's late report); the box with
// no layout (`unseen`) never reported at all; and no 2 s bound, so only its own release lets a settle go.
const heldReports = () => {
  globalThis.__exactFirstSizeBound = 1e9;
  const Real = globalThis.ResizeObserver, held = [];
  let open = false;
  globalThis.__reports = () => { open = true; for (const f of held.splice(0)) f(); };
  globalThis.ResizeObserver = class extends Real {
    constructor(callback) {
      super((entries, observer) => {
        const shown = entries.filter((x) => x.target.dataset.testid !== 'unseen');
        if (!shown.length) return;
        const deliver = () => callback(shown, observer);
        if (open) deliver(); else held.push(deliver);
      });
    }
  };
};

// The settle, begun once `ready` (a page promise's name) settles, with `release` run 300 ms in (in the page: no
// driver's gap), and the state as it answered.
const settleThen = (page, ready, release) => page.evaluate(async ([ready, release]) => {
  await globalThis[ready];
  const began = performance.now(), settling = globalThis.exact.agentSettled({ op: 'clock', settle: true });
  setTimeout(() => globalThis[release](), 300);
  const settled = await settling;
  const state = await globalThis.exact.agentSettled({ op: 'state' });
  return { settled, slots: state.slots, ms: performance.now() - began };
}, [ready, release]);

test("an agent's settle waits for a view's first size, however late its report comes, and that size waits for the page's geometry; an element with no box holds nothing", async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const dir = await built();
  let browser, server;
  try {
    browser = await suiteBrowser();
    // Reports held until 300 ms into the settle: it waits for them (on main it answered first, widths []).
    server = serve(dir);
    const page = await browser.newPage();
    await page.addInitScript(heldReports);
    await page.addInitScript(watchObserve);
    await page.goto(`${server.url.href}?agent=1`);
    await page.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    // Begun once the observer is attached: on main its hold was already gone, and the settle answered first.
    const held = await settleThen(page, '__observed', '__reports');
    expect(held.settled).toMatchObject({ settled: true });
    expect(held.ms).toBeGreaterThanOrEqual(300);
    expect(held.slots.widths.length).toBe(2);
    expect(held.slots.widths.every((w) => w > 10)).toBe(true);
    // The box with no layout, never reported and with no bound to end its wait, held nothing.
    expect(held.slots.hidden).toBe(0);
    await page.close();
    server.stop(true);
    // The page's geometry served only 300 ms into the settle (frames answer unavailable until it loads): the first
    // size waits for it, so the tabs are measured, not read as nothing (the clone's `-12`).
    server = serve(dir, /geometry-glue/);
    // Released once the tab row was observed and two frames passed (when a resize glue that did not wait would have
    // reported on unavailable frames), or 1.5 s on with nothing observed (the one that waits).
    const late = await (await browser.newContext()).newPage();
    await late.exposeFunction('__geometry', () => server.open());
    await late.addInitScript(watchObserve);
    await late.addInitScript(() => {
      const frames = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      globalThis.__geometryDue = Promise.race([globalThis.__observed.then(frames), new Promise((r) => setTimeout(r, 1500))]);
    });
    await late.goto(`${server.url.href}?agent=1`);
    await late.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    const slow = await late.evaluate(async () => {
      const settling = globalThis.exact.agentSettled({ op: 'clock', settle: true });
      await globalThis.__geometryDue;
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
    rmSync(dir, { recursive: true, force: true });
  }
}, 300_000);
