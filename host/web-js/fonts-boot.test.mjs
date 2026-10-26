// The JS target's declared fonts are `font-display: optional` faces
// (src/faces.rs): a face first needed while it still loads is never used on
// the page. On the clone (one Inter file, a face per weight) the faces past
// the one its preload served were fetched at their first use, inside a long
// first render, and served without a cache (conform's `no-store`) came too
// late: its names and tabs kept the fallback's width, where the wasm page,
// which waits up to 100 ms for its faces before its first render (glue.js
// prepareFonts), drew Inter. The JS page now does the same (build.mjs's
// main.js): every face starts loading before the first render, which waits
// for them up to 100 ms.
import { test, expect } from 'bun:test';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const root = resolve(import.meta.dir, '../..');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.ttf': 'font/ttf', '.wasm': 'application/wasm' };

async function built(contract, faces) {
  const dir = mkdtempSync(resolve(root, 'target/fonts-boot-test-'));
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Fonts Boot Test', app: { id: 'test.fontsboot', name: 'Fonts Boot Test' }, host: { web: {} } }));
  mkdirSync(resolve(dir, 'assets'));
  if (faces) cpSync(resolve(root, 'host/apple/tests/fonts/InterVariable.ttf'), resolve(dir, 'assets/InterVariable.ttf'));
  writeFileSync(resolve(dir, 'app.contract'), contract);
  const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'fonts-boot-test', '--out', resolve(dir, 'dist')], {
    cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
  });
  const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
  expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
  return dir;
}

// `hold`, when given, is a promise the font responses wait for.
function serve(dir, hold) {
  return Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    let path = new URL(req.url).pathname;
    if (path.endsWith('/')) path += 'index.html';
    const file = Bun.file(resolve(dir, 'dist', '.' + path));
    if (!(await file.exists())) return new Response('', { status: 404 });
    if (extname(path) === '.ttf' && hold) await hold;
    return new Response(file, { headers: { 'content-type': TYPES[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' } });
  } });
}

// When each face's load began and settled, and when the root got its first
// node, from the page's own clock.
const watch = () => {
  globalThis.__fonts = { loads: [], mounted: null, early: null };
  // `globalThis.exact` as last seen before the runtime's state (the font
  // wait): its `ready` must be a promise the boot settles.
  const look = setInterval(() => {
    const x = globalThis.exact;
    if (x && !x.state) {
      __fonts.early = { ready: typeof x.ready?.then };
      if (x.ready?.then && !x.ready.watched) { x.ready.watched = true; x.ready.then(() => { __fonts.readyAt = performance.now(); }); }
    }
    if (x?.state) clearInterval(look);
  }, 1);
  const load = FontFace.prototype.load;
  FontFace.prototype.load = function () {
    const row = { weight: this.weight, began: performance.now(), done: null };
    __fonts.loads.push(row);
    return load.call(this).finally(() => { row.done = performance.now(); });
  };
  new MutationObserver((_, o) => {
    const el = document.getElementById('exact-root');
    if (el?.firstElementChild) { __fonts.mounted = performance.now(); o.disconnect(); }
  }).observe(document, { childList: true, subtree: true });
};

const INTER = [
  'font "Inter"',
  '  400 = "assets/InterVariable.ttf"',
  '  600 = "assets/InterVariable.ttf"',
  '  700 = "assets/InterVariable.ttf"',
  'component App',
  '  view',
  '    column',
  '      text "EXACT" font-family="Inter" font-size=24',
  '      text "EXACT EXACT" testId="word" font-family="Inter" font-size=24 font-weight=600',
  '',
].join('\n');

test('every declared face starts loading before the first render, which waits for them up to 100 ms', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const dir = await built(INTER, true);
  let browser, fast, slow;
  try {
    browser = await suiteBrowser();
    // Served at once: each face is loaded before the root's first node, and
    // the word draws in Inter (a canvas measures the same face; the fallback
    // is wider apart).
    fast = serve(dir, null);
    let page = await browser.newPage();
    await page.addInitScript(watch);
    await page.goto(fast.url.href);
    await page.waitForSelector('[data-testid="word"]');
    const quick = await page.evaluate(() => {
      const el = document.querySelector('[data-testid="word"]'), s = getComputedStyle(el), range = document.createRange();
      range.selectNodeContents(el);
      const ctx = document.createElement('canvas').getContext('2d');
      ctx.font = `${s.fontWeight} ${s.fontSize} ${s.fontFamily.split(',')[0]}`;
      const face = ctx.measureText(el.textContent).width;
      ctx.font = `${s.fontWeight} ${s.fontSize} system-ui`;
      return { ...__fonts, shown: range.getBoundingClientRect().width, face, fallback: ctx.measureText(el.textContent).width };
    });
    // One variable file pinned at 400, 600 and 700 is one face over 400–700
    // (LLP 1019 D2, bafdeea824): loaded once.
    expect(quick.loads.map(l => l.weight).sort()).toEqual(['400 700']);
    // Each face loaded before the first node, and the word is drawn in it;
    // or the 100 ms ran out first (a busy machine), and the first node waited
    // for them.
    const began = Math.min(...quick.loads.map(l => l.began));
    if (quick.loads.every(l => l.done != null && l.done <= quick.mounted)) {
      expect(Math.abs(quick.face - quick.fallback)).toBeGreaterThan(2);
      expect(Math.abs(quick.shown - quick.face)).toBeLessThan(0.5);
    } else expect(quick.mounted - began).toBeGreaterThanOrEqual(95);
    await page.close();
    // A font file that never comes in time: the first render waits about
    // 100 ms after the loads begin, not for the file.
    let release;
    slow = serve(dir, new Promise(r => { release = r; }));
    // Its own context: no font the first page loaded is in its memory.
    page = await (await browser.newContext()).newPage();
    // An early `globalThis.exact`, as the TypeScript data module makes one
    // before the runtime boots (ts-data.js).
    await page.addInitScript(() => { globalThis.exact = { early: true }; });
    await page.addInitScript(watch);
    await page.goto(slow.url.href);
    // The font file held until the page has rendered: it rendered without
    // it, about 100 ms after the loads began.
    await page.waitForSelector('[data-testid="word"]');
    const late = await page.evaluate(() => __fonts);
    release();
    const start = Math.min(...late.loads.map(l => l.began));
    expect(late.loads.every(l => l.done == null || l.done > late.mounted)).toBe(true);
    expect(late.mounted - start).toBeGreaterThanOrEqual(95);
    // A reader finding the early `globalThis.exact` meanwhile waited on a
    // `ready` the boot settled, once it had rendered.
    expect(late.early?.ready).toBe('function');
    expect(late.readyAt).toBeGreaterThanOrEqual(late.mounted);
  } finally { await browser?.close(); fast?.stop(true); slow?.stop(true); rmSync(dir, { recursive: true, force: true }); }
}, 120000);

test('a page with no declared fonts loads none', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const dir = await built('component App\n  view\n    text "hello" testId="word"\n', false);
  let browser, server;
  try {
    browser = await suiteBrowser();
    server = serve(dir, null);
    const page = await browser.newPage();
    await page.addInitScript(watch);
    await page.goto(server.url.href);
    await page.waitForSelector('[data-testid="word"]');
    expect((await page.evaluate(() => __fonts)).loads).toEqual([]);
  } finally { await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true }); }
}, 120000);
