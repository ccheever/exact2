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

// Each ResizeObserver report held until the page says `__reports()` (a loaded machine's late report); the box with
// no layout (`unseen`) never reported at all, so only its own release lets a settle go.
const heldReports = () => {
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

// The settle, with `release` run 300 ms after it began (in the page: no driver's gap), and the state as it answered.
const settleThen = (page, release) => page.evaluate(async (release) => {
  const began = performance.now(), settling = globalThis.exact.agentSettled({ op: 'clock', settle: true });
  setTimeout(() => { if (release) globalThis[release](); }, 300);
  const settled = await settling;
  const state = await globalThis.exact.agentSettled({ op: 'state' });
  return { settled, slots: state.slots, ms: performance.now() - began };
}, release);

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
    await page.goto(`${server.url.href}?agent=1`);
    await page.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    const held = await settleThen(page, '__reports');
    expect(held.settled).toMatchObject({ settled: true });
    expect(held.slots.widths.length).toBe(2);
    expect(held.slots.widths.every((w) => w > 10)).toBe(true);
    // The box with no layout, never reported, held nothing: the settle answered long before the 2 s bound.
    expect(held.slots.hidden).toBe(0);
    expect(held.ms).toBeLessThan(1500);
    await page.close();
    server.stop(true);
    // The page's geometry served only 300 ms into the settle (frames answer unavailable until it loads): the first
    // size waits for it, so the tabs are measured, not read as nothing (the clone's `-12`).
    server = serve(dir, /geometry-glue/);
    const late = await (await browser.newContext()).newPage();
    await late.exposeFunction('__geometry', () => server.open());
    await late.goto(`${server.url.href}?agent=1`);
    await late.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    const slow = await settleThen(late, '__geometry');
    expect(slow.settled).toMatchObject({ settled: true });
    expect(slow.slots.widths.length).toBe(2);
    expect(slow.slots.widths.every((w) => w > 10)).toBe(true);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 300_000);
