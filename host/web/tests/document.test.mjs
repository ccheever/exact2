// The document parity check (LLP 1048.000 §4): Caltrain's page as the
// projection renders it, parsed by Chrome with JavaScript off, against the
// live host's DOM at the same URL once the runtime is ready. A difference
// names the view. What the browser owns after layout — a symbol's sized
// source, a surface's pixel size, focus — is left out of both sides.
import { test, expect } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open, assertWebDistApp } from '../../../scripts/agent.mjs';
import { cdpBrowser, chromium, DESKTOP_INPUT, refuseStale, webChanges, suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { hermesBundle, hermesTarget, resolveApp } from '../../../scripts/app.mjs';
import { jsTargetBuild, serveStatic, serveBuildTree } from '../serve.mjs';
import { withLook } from '../look.mjs';
import { need, notApplicable } from '../../../scripts/coverage.mjs';
import { decodePng } from '../../../scripts/png.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
// The shell as a default build ships it (look.mjs): what the stylesheet's own tests below draw on.
const SHIPPED = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'));
const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));
const app = resolveApp('caltrain');
const { unavailable: browserUnavailable } = chromium();
/** How the Weatherlight test is chosen: `need(kind, why)` or `notApplicable(why)`, or `test`. */
function weatherlightCheckFor() {
  if (process.env.EXACT_GLUE_FAST === '1') return notApplicable('the async glue step does not build apps; the web-build-test step runs this test');
  if (!['linux', 'darwin'].includes(process.platform)) return notApplicable(`the Weatherlight wasm build is not provisioned on ${process.platform}`);
  if (browserUnavailable) return need('chrome', browserUnavailable);
  if (process.env.EXACT_JS_ENGINE === 'stub') return need('hermes', 'the Weatherlight document test needs the Hermes executor, not EXACT_JS_ENGINE=stub');
  const bundle = hermesBundle(hermesTarget());
  if (!bundle.installed) return need('hermes', `the Weatherlight wasm build needs ${bundle.target}; missing ${bundle.missing.join(', ')}; run ${bundle.fix}`);
  const needed = [resolve(process.env.EXACT_TSC ?? resolve(ROOT, 'node_modules/.bin/tsc')),
    resolve(process.env.EXACT_ROLLDOWN ?? resolve(ROOT, 'node_modules/.bin/rolldown'))];
  const missing = needed.filter(path => !existsSync(path));
  return need('typescript', missing.length ? `the Weatherlight wasm build needs tsc and rolldown; missing ${missing.join(', ')}` : null);
}
let unavailable = browserUnavailable;
try {
  if (!unavailable) {
    await assertWebDistApp(dist, app);
    if (jsTargetBuild(dist)) throw new Error(`web dist is a JS-target build; document adoption needs app.wasm; run bun host/web/build.mjs ${app.crate('web')} --wasm`);
    refuseStale('web', resolve(dist, '.exact-build.json'), webChanges(dist, app).all,
      `bun host/web/build.mjs ${app.crate('web')} --wasm`);
  }
} catch (error) {
  if (!error.message.startsWith('web dist is not a complete build') && !error.message.startsWith('web dist is a JS-target build') && !error.message.startsWith('web build is stale')) throw error;
  unavailable = error.message;
}
// The Caltrain document tests need a current wasm Caltrain dist (EXACT_WEB_DIST, else host/web/dist).
const check = need(browserUnavailable ? 'chrome' : 'fixture', unavailable);
const browserCheck = need('chrome', browserUnavailable);
const weatherlightCheck = weatherlightCheckFor();
const [width, height] = [390, 844]; // the page viewport documents render at

/** The shell the renderer is given: host/web/index.html in this run's look (look.mjs), which is the dist's (one
 * built in another look is stale, above), so the renderer resolves a role in the column the live page does. */
let shellFile;
function shellInLook() {
  if (!shellFile) {
    const dir = mkdtempSync(resolve(tmpdir(), 'exact-document-shell-'));
    process.on('exit', () => rmSync(dir, { recursive: true, force: true }));
    writeFileSync(shellFile = resolve(dir, 'index.html'), withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8')));
  }
  return shellFile;
}

/** The rendered page: the shell with the renderer's head, the document in
 * `#exact-root` and its checkpoint, as the build writes it. */
function renderedPage(location) {
  const render = spawnSync('cargo', ['run', '-q', '-p', 'caltrain-linux', '--bin', 'caltrain-render', '--',
    '--plan', resolve(dist, 'app.plan'), '--viewport', `${width}x${height}`, '--shell', shellInLook(), location],
  { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' } });
  if (render.status !== 0) throw new Error(`caltrain-render: ${render.stderr}${render.stdout}`);
  const page = JSON.parse(render.stdout.trim().split('\n').at(-1));
  if (page.error) throw new Error(`caltrain-render ${location}: ${page.error}`);
  return page.page;
}

/** `#exact-root`'s elements as comparable records, evaluated in the page. */
const NORMALIZED = `(() => {
  const walk = (el) => {
    const tag = el.localName, attrs = {}, style = {};
    for (const a of el.attributes) if (a.name !== 'style') attrs[a.name] = a.value;
    delete attrs.autofocus;
    if (tag === 'img' && 'data-symbol-path' in attrs) delete attrs.src;
    if (tag === 'canvas' && 'data-surface' in attrs) { delete attrs.width; delete attrs.height; }
    if (tag === 'input' || tag === 'textarea') { delete attrs.value; attrs['.value'] = el.value; }
    if (tag === 'input') { delete attrs.checked; attrs['.checked'] = String(el.checked); }
    if (tag === 'a' && 'href' in attrs) attrs.href = el.href;
    for (const p of el.style) if (!p.startsWith('--exact-symbol-')) style[p] = el.style.getPropertyValue(p) + (el.style.getPropertyPriority(p) ? ' !important' : '');
    // A surface's box follows its host's content box once laid out (canvas2d-glue.js report, LLP 1056 D6).
    if (tag === 'canvas' && 'data-surface' in attrs) for (const p of ['top', 'right', 'bottom', 'left', 'width', 'height']) delete style[p];
    if (tag === 'canvas' && 'data-surface' in attrs) for (const p of Object.keys(style)) if (/^border-.*-radius$/.test(p)) delete style[p];
    const content = [];
    for (const n of el.childNodes) {
      if (n.nodeType === Node.ELEMENT_NODE) content.push(walk(n));
      else if (n.nodeType === Node.TEXT_NODE && tag !== 'textarea') {
        if (typeof content.at(-1) === 'string') content[content.length - 1] += n.data; else content.push(n.data);
      }
    }
    return { view: el.getAttribute('data-view'), tag, attrs, style, content };
  };
  return JSON.stringify([...document.getElementById('exact-root').children].map(walk));
})()`;

/** Recorded from the page's first script: frames where `#exact-root` was
 * empty, and times its document was taken away. The document's elements are
 * marked, so an adopted one is known after the runtime starts. */
const WATCH = `globalThis.__watch = { emptyFrames: 0, swaps: 0 };
  const frame = () => { const r = document.getElementById('exact-root'); if (r && document.readyState !== 'loading' && !r.childElementCount) __watch.emptyFrames++; requestAnimationFrame(frame); };
  requestAnimationFrame(frame);
  addEventListener('DOMContentLoaded', () => {
    for (const el of document.querySelectorAll('#exact-root [data-view]')) el.__served = true;
    new MutationObserver((records) => {
      if (records.some((m) => [...m.removedNodes].some((n) => n.nodeType === 1 && n.dataset?.view === '1' && !n.isConnected))) __watch.swaps++;
    }).observe(document.getElementById('exact-root'), { childList: true });
  });`;
/** Of the live page's views, how many are the served document's own elements. */
const SERVED_VIEWS = `(() => { const views = [...document.querySelectorAll('#exact-root [data-view]')]; return { views: views.length, served: views.filter((el) => el.__served).length }; })()`;
/** Whether the wasm was fetched once, by the document's head preload: the
 * capture script's and the glue's fetches take that response. */
const WASM_ONCE = `(() => {
  const wasm = performance.getEntriesByType('resource').filter((e) => new URL(e.name).pathname === '/app.wasm');
  return wasm.length === 1 && wasm[0].initiatorType === 'link';
})()`;

/** Every difference between two normalized trees, each naming a view. */
function differences(served, live, where = 'root', out = []) {
  const name = (n) => n && typeof n === 'object' ? `${n.tag}[data-view=${n.view}]` : JSON.stringify(n);
  const count = Math.max(served.length, live.length);
  for (let i = 0; i < count; i++) {
    const a = served[i], b = live[i];
    const at = `${where} > ${name(a ?? b)}`;
    if (a === undefined || b === undefined) { out.push(`${at}: ${a === undefined ? 'only live' : 'only served'}`); continue; }
    if (typeof a === 'string' || typeof b === 'string') { if (a !== b) out.push(`${where}: text ${JSON.stringify(a)} served, ${JSON.stringify(b)} live`); continue; }
    if (a.tag !== b.tag || a.view !== b.view) { out.push(`${where}: ${name(a)} served, ${name(b)} live`); continue; }
    for (const [kind, x, y] of [['attribute', a.attrs, b.attrs], ['style', a.style, b.style]]) {
      for (const key of new Set([...Object.keys(x), ...Object.keys(y)])) {
        if (x[key] !== y[key]) out.push(`${at}: ${kind} ${key}: ${JSON.stringify(x[key])} served, ${JSON.stringify(y[key])} live`);
      }
    }
    differences(a.content, b.content, at, out);
  }
  return out;
}

/** Serve the rendered page for `location` beside dist/, launch Chrome, and
 * hand `drive` a way to open tabs on it; everything is torn down after. */
async function withDocument(location, drive, { wasmAfter = null, glueAfter = null, tamper = (page) => page, origin = null, html = null, files = {}, answers = {}, requested = () => {}, hints = false, needs = [] } = {}) {
  let server = null, url = `${origin}${location}`;
  if (!origin) {
    const page = tamper(html ?? renderedPage(location));
    server = createServer(async (req, res) => {
      requested(req.url);
      if (req.url === location || req.url === '/') { res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(page); return; }
      if (files[req.url]) { res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(files[req.url]); return; }
      // Another page, or a response that doesn't navigate (a 204).
      if (answers[req.url]) { const [status, body] = answers[req.url]; res.writeHead(status, { 'content-type': 'text/html; charset=utf-8' }); res.end(body); return; }
      // A slow network, where a test needs the runtime to still be loading.
      if (wasmAfter && req.url.startsWith('/app.wasm')) await wasmAfter;
      if (glueAfter && req.url.startsWith('/glue.js')) await glueAfter;
      serveStatic(dist, req, res);
    });
    await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
    url = `http://127.0.0.1:${server.address().port}${location}`;
  }
  // A browser context of its own: no cookie, storage, cache or history of another test's
  const suite = await cdpBrowser({ needs, args: [`--window-size=${width},${height}`, '--hide-scrollbars', DESKTOP_INPUT] });
  const { cdp, lines } = suite;
  lines.length = 0;
  try {
    const tab = async (scripts, onNew) => {
      const { sessionId } = await suite.page();
      const call = (method, params) => cdp.send(method, params, sessionId);
      await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] }); // the light look these checks expect, in whatever appearance this machine has
      await call('Page.enable');
      await call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
      await call('Emulation.setScriptExecutionDisabled', { value: !scripts });
      if (onNew) await call('Page.addScriptToEvaluateOnNewDocument', { source: onNew });
      // A returning reader's browser sends the viewport hints the server asked for (LLP 1048.006):
      // a first visit gets its user agent's class, a desktop's 1280 px, which a page that breaks
      // between the tab's width and 1280 renders differently and the runtime does not adopt.
      if (hints) {
        await call('Network.enable');
        await call('Network.setExtraHTTPHeaders', { headers: { 'Sec-CH-Viewport-Width': String(width), 'Sec-CH-Viewport-Height': String(height) } });
      }
      const evaluate = async (expression) => {
        const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
        return r.result.value;
      };
      evaluate.call = call;
      evaluate.until = async (expression, what) => {
        const start = Date.now();
        while (!(await evaluate(expression).catch(() => false))) {
          if (Date.now() - start > 60000) throw new Error(`${what} never happened; ${lines.join('\n')}`);
          await Bun.sleep(5);
        }
      };
      await call('Page.navigate', { url });
      return evaluate;
    };
    await drive(tab);
  } finally {
    await suite.close();
    server?.close();
  }
}


const settled = 'new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))';

check(`Caltrain's served document is the live host's DOM`, async () => {
  // The live page launches at its own URL, agent query included; the
  // document is rendered at that same location.
  await withDocument('/?agent=1', async (tab) => {
    const plain = await tab(false);
    await plain.until("document.readyState === 'complete' && !!document.getElementById('exact-root')?.firstElementChild", 'the served document');
    const served = JSON.parse(await plain(NORMALIZED));
    // LLP 1048.000 D6: the document stays on screen until the runtime's own
    // tree has settled; its wasm downloads with the document, the runtime
    // starts at idle, from the checkpoint, and adopts the document — its
    // first tree is the document's.
    const live = await tab(true, WATCH);
    await live.until("document.getElementById('exact-root')?.dataset.bootMs != null", 'the first frame');
    await live('exact.ready');
    await live(settled);
    expect(differences(served, JSON.parse(await live(NORMALIZED)))).toEqual([]);
    expect(served.length).toBeGreaterThan(0);
    const { emptyFrames, swaps } = JSON.parse(await live('JSON.stringify(globalThis.__watch)'));
    expect({ emptyFrames, swaps, wasmOnce: await live(WASM_ONCE) }).toEqual({ emptyFrames: 0, swaps: 0, wasmOnce: true });
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(true);
    const { views, served: kept } = await live(SERVED_VIEWS);
    expect(views).toBeGreaterThan(200);
    expect(kept).toBe(views);
    // Nothing the document read was asked again at boot.
    const logs = (await live("exact.agent({op:'logs'})")).lines.join('\n');
    expect(logs).toContain('checkpoint: 7 of 7 answers taken');
    expect(logs).not.toMatch(/query (station|northBoard|allStations):/);
  });
}, 180000);

check(`a document whose digest doesn't match is replaced once, and the journal says where`, async () => {
  await withDocument('/?agent=1', async (tab) => {
    const live = await tab(true, WATCH);
    await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
    await live(settled);
    const { emptyFrames, swaps } = JSON.parse(await live('JSON.stringify(globalThis.__watch)'));
    expect({ emptyFrames, swaps }).toEqual({ emptyFrames: 0, swaps: 1 });
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(false);
    expect((await live(SERVED_VIEWS)).served).toBe(0);
    expect((await live("exact.agent({op:'logs'})")).lines.join('\n')).toContain('document: not adopted: the same');
  }, { tamper: (page) => page.replace(/data-digest="[0-9a-f]{32}"/, `data-digest="${'0'.repeat(32)}"`) });
}, 180000);

check(`a press on the document before the runtime starts is replayed once`, async () => {
  let clicked;
  const wasmAfter = new Promise((resolve) => { clicked = resolve; });
  await withDocument('/?agent=1', async (tab) => {
    const live = await tab(true);
    // The glue is running (a press before it evaluates is a page without
    // JavaScript's), the runtime isn't: its wasm is held back until the click.
    await live.until("typeof globalThis.exact === 'object' && !!document.querySelector('[data-testid=scheme-dark]') && !document.getElementById('exact-root').dataset.bootMs", 'the served document');
    // The press is the first interaction: it starts the runtime, and waits for it.
    const at = await live("(() => { const r = document.querySelector('[data-testid=scheme-dark]').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()");
    for (const type of ['mousePressed', 'mouseReleased']) await live.call('Input.dispatchMouseEvent', { type, x: at[0], y: at[1], button: 'left', clickCount: 1 });
    expect(await live("document.documentElement.style.colorScheme")).toBe('');
    clicked();
    await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
    await live.until("document.documentElement.style.colorScheme === 'dark'", 'the replayed press');
    await live(settled);
    expect(await live("document.documentElement.style.colorScheme")).toBe('dark');
  }, { wasmAfter });
}, 180000);

check(`a press before the glue runs is captured, replays once, and a later one isn't doubled`, async () => {
  // LLP 1048.000 D6, 1048.001 D5: the page's inline capture script hears a
  // press from first parse; here the glue is held back until after it.
  let release;
  const glueAfter = new Promise((resolve) => { release = resolve; });
  await withDocument('/?agent=1', async (tab) => {
    const live = await tab(true);
    await live.until("!!document.querySelector('[data-testid=scheme-dark]') && typeof globalThis.exact?.taps === 'function'", 'the served document');
    const press = async (testId) => {
      const at = await live(`(() => { const r = document.querySelector('[data-testid=${testId}]').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
      for (const type of ['mousePressed', 'mouseReleased']) await live.call('Input.dispatchMouseEvent', { type, x: at[0], y: at[1], button: 'left', clickCount: 1 });
    };
    await press('scheme-dark');
    expect(await live('typeof globalThis.exact.agent')).toBe('undefined');
    expect(await live("document.documentElement.style.colorScheme")).toBe('');
    release();
    await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
    await live.until("document.documentElement.style.colorScheme === 'dark'", 'the replayed press');
    await live(settled);
    const presses = async () => (await live("exact.agent({op:'logs'})")).lines.filter((l) => /press view \d+ \(chooseScheme\)/.test(l)).length;
    expect(await presses()).toBe(1);
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(true);
    // After adoption a press is the runtime's own: once.
    await press('scheme-light');
    await live.until("document.documentElement.style.colorScheme === 'light'", 'the live press');
    await live(settled);
    expect(await presses()).toBe(2);
  }, { glueAfter });
}, 180000);

// LLP 1048.000 D6: a link that leaves the page stops its runtime's download,
// which would share the link with the next document; a page that stays gets
// its runtime all the same. Here the wasm is held back, so it is still
// downloading when the link is followed.
const away = (href) => `(() => { const a = document.createElement('a'); a.href = '${href}'; document.body.append(a); a.click(); })()`;
const downloading = "!!globalThis.exact?.runtime && performance.getEntriesByType('resource').some((e) => e.name.endsWith('/document-glue.js'))";

check(`a link that doesn't leave (a 204) stops the runtime's download, and the page downloads it again`, async () => {
  let release;
  const wasmAfter = new Promise((resolve) => { release = resolve; });
  const wasm = [];
  await withDocument('/?agent=1', async (tab) => {
    const live = await tab(true);
    await live.until(downloading, 'the download');
    await live(away('/no-content'));
    expect(await live("exact.runtime.then(() => 'downloaded', (e) => e.name)")).toBe('AbortError');
    expect(await live(`!!document.querySelector('link[href^="./app.wasm"]')`)).toBe(false);
    release();
    await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
    expect(await live('location.pathname')).toBe('/');
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(true);
    expect(wasm.length).toBe(2);
  }, { wasmAfter, answers: { '/no-content': [204, ''] }, requested: (url) => { if (url.startsWith('/app.wasm')) wasm.push(url); } });
}, 180000);

check(`Back from the bfcache downloads the runtime a link stopped`, async () => {
  let release;
  const wasmAfter = new Promise((resolve) => { release = resolve; });
  await withDocument('/?agent=1', async (tab) => {
    const live = await tab(true, "addEventListener('pageshow', (e) => { if (e.persisted) globalThis.__restored = true; });");
    await live.until(downloading, 'the download');
    await live(away('/elsewhere'));
    await live.until("location.pathname === '/elsewhere'", 'the next page');
    await live('history.back()');
    await live.until('globalThis.__restored === true', 'the page from the bfcache');
    release();
    await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(true);
  }, { wasmAfter, answers: { '/elsewhere': [200, '<!doctype html><p>elsewhere</p>'] }, needs: ['bfcache'] });
}, 180000);

/** An app's render server (LLP 1048.000 D10) over its dist, on loopback —
 * Caltrain's over dist/ unless told; `stop` ends the one process it started. */
async function renderServer({ app = 'caltrain', dir = dist, name = 'Caltrain' } = {}) {
  const child = spawn('cargo', ['run', '-q', '-p', `${app}-linux`, '--bin', `${app}-render`, '--', '--serve', dir, '--name', name],
    { cwd: ROOT, env: { ...process.env, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' }, stdio: ['ignore', 'pipe', 'pipe'] });
  const lines = [];
  const origin = await new Promise((ok, fail) => {
    child.stdout.on('data', (d) => {
      for (const line of String(d).split('\n')) {
        lines.push(line);
        const at = /^serving (http:\/\/127\.0\.0\.1:\d+)\//.exec(line);
        if (at) ok(at[1]);
      }
    });
    child.on('exit', (code) => fail(new Error(`${app}-render --serve exited ${code}`)));
  });
  return { origin, lines, stop: () => { try { child.kill('SIGKILL'); } catch {} } };
}

check(`the render server's page is the document, and the runtime adopts it`, async () => {
  const server = await renderServer();
  try {
    const response = await fetch(`${server.origin}/?agent=1`);
    expect(response.status).toBe(200);
    expect(response.headers.get('content-security-policy')).toContain("script-src 'self' 'wasm-unsafe-eval'");
    expect(response.headers.get('cache-control')).toStartWith('public, max-age=0, s-maxage=');
    expect((await fetch(`${server.origin}/nowhere`)).status).toBe(404);
    await withDocument('/?agent=1', async (tab) => {
      const live = await tab(true, WATCH);
      await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
      await live(settled);
      expect((await live("exact.agent({op:'state'})")).adopted).toBe(true);
      const { views, served } = await live(SERVED_VIEWS);
      expect(served).toBe(views);
      const { emptyFrames, swaps } = JSON.parse(await live('JSON.stringify(globalThis.__watch)'));
      expect({ emptyFrames, swaps }).toEqual({ emptyFrames: 0, swaps: 0 });
    }, { origin: server.origin });
    expect(server.lines.some((l) => /^render \/\?agent=1 200 /.test(l))).toBe(true);
  } finally {
    server.stop();
  }
}, 240000);


// A TypeScript app through the server (LLP 1048.000 D6, D11): Weatherlight's
// first frame answers offline (revision 0 is the empty forecast), so its
// document renders at build and per request with no network. The runtime
// adopts it, and the module's realm runs under the server's CSP: its two
// inline scripts are admitted by hash, nothing else is.
weatherlightCheck(`a TypeScript app's served document is adopted, with its module running under the CSP`, async () => {
  const out = mkdtempSync(resolve(tmpdir(), 'exact-weatherlight-'));
  try {
    const build = spawnSync('bun', ['host/web/build.mjs', 'weatherlight', '--wasm'], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20,
      env: { ...process.env, EXACT_WEB_DIST: out, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' } });
    if (build.status !== 0) throw new Error(`weatherlight build: ${build.stderr}${build.stdout}`);
    const server = await renderServer({ app: 'weatherlight', dir: out, name: 'Weatherlight' });
    try {
      const response = await fetch(`${server.origin}/?agent=1`);
      expect(response.status).toBe(200);
      const scripts = /script-src ([^;]*)/.exec(response.headers.get('content-security-policy'))?.[1] ?? '';
      expect(scripts).toStartWith("'self' 'wasm-unsafe-eval' 'sha256-");
      expect(scripts).not.toContain('unsafe-inline');
      await withDocument('/?agent=1', async (tab) => {
        const live = await tab(true, WATCH);
        await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
        await live(settled);
        const state = await live("exact.agent({op:'state'})");
        expect(state.adopted).toBe(true);
        // The module ran in its realm: the logic is ready, and no error was shown.
        expect(state.logic.ready).toBe(true);
        expect(await live("document.getElementById('exact-root').dataset.error ?? null")).toBe(null);
        const { views, served } = await live(SERVED_VIEWS);
        expect(views).toBeGreaterThan(0);
        expect(served).toBe(views);
        const { emptyFrames, swaps } = JSON.parse(await live('JSON.stringify(globalThis.__watch)'));
        expect({ emptyFrames, swaps }).toEqual({ emptyFrames: 0, swaps: 0 });
        // Weatherlight breaks at 1100 px (its wide layout): the tab's hints decide its page.
      }, { origin: server.origin, hints: true });
      expect(server.lines.some((l) => /^render \/\?agent=1 200 /.test(l))).toBe(true);
    } finally {
      server.stop();
    }
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 900000);

// The small entry can be exercised without a compiled application. Its runtime
// consumer here records semantic dispatches, including the first edit and IME.
browserCheck(`interaction documents stay readable, then replay edits and actions once`, async () => {
  const html = `<!doctype html><meta charset="utf-8"><div id="exact-root">
    <div data-view="1" data-exact-on="navigate">
      <p id="reading">Public content</p><a href="#reading" id="link">Read more</a>
      <input data-view="2" data-exact-on="input submit" name="title">
      <button data-view="3" data-exact-on="press">Save</button>
    </div></div>
    <script type="application/vnd.exact.checkpoint" data-activate="interaction" data-digest="test">{}</script>
    <script type="module" src="./document-glue.js"></script>`;
  const files = {
    '/document-glue.js': readFileSync(resolve(ROOT, 'host/web/document-glue.js'), 'utf8'),
    '/glue.js': `globalThis.__loads = (globalThis.__loads ?? 0) + 1;
      globalThis.__events = []; globalThis.__logs = [];
      const views = new Map([...document.querySelectorAll('[data-view]')].map(el => {
        el.exactHandlers = el.dataset.exactOn.split(' '); return [Number(el.dataset.view), el];
      }));
      const page = exact.documentPage.connect({ views, log: line => __logs.push(line),
        dispatch: (id, kind, value) => __events.push([id, kind, value]) });
      globalThis.__adopted = []; // what the caller's completion saw (glue.js activates navigation there)
      globalThis.__release = () => { page.hold({ops:[{op:'adopt',adopted:true}]}); page.release(() => {}, undefined, () => __adopted.push([page.adopted, page.holding])); };`,
  };
  await withDocument('/', async tab => {
    const live = await tab(true);
    await live.until('!!globalThis.exact?.documentPage', 'document entry');
    await live("document.getElementById('link').click()");
    await live(settled);
    expect(await live('globalThis.__loads ?? 0')).toBe(0);
    // An ordinary reading click must not start because the root hears navigate.
    await live("document.getElementById('reading').dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}))");
    await live(settled);
    expect(await live('globalThis.__loads ?? 0')).toBe(0);
    await live(`(() => { const el = document.querySelector('input'); el.focus();
      el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
      el.value = '東'; el.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true})); })()`);
    await live.until('globalThis.__loads === 1', 'interaction runtime');
    await live('__release()');
    expect(await live('exact.documentPage.holding')).toBe(true);
    expect(await live('__adopted')).toEqual([]); // a composition defers the adoption and its completion
    await live(`(() => { const el = document.querySelector('input'); el.value = '東京';
      el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));
      el.dispatchEvent(new InputEvent('input',{bubbles:true})); el.setSelectionRange(1,1);
      el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));
      document.querySelector('button').click(); })()`);
    await live.until('!exact.documentPage.holding && __events.length === 3', 'semantic replay');
    expect(await live('__events')).toEqual([[2,23,'東京'],[2,7,''],[3,0,'']]);
    expect(await live('__adopted')).toEqual([[true, false]]); // once, after the held batches applied
    expect(await live('__logs')).toEqual([]);
    expect(await live("[document.querySelector('input').value,document.querySelector('input').selectionStart,document.activeElement.localName]")).toEqual(['東京',1,'input']);
    expect(await live('__loads')).toBe(1);
    expect(await live("document.getElementById('reading').textContent")).toBe('Public content');
  }, { html, files });
}, 60000);

browserCheck(`early key replay carries capture cancellation to key handlers and Enter submit`, async () => {
  const html = `<!doctype html><meta charset="utf-8"><div id="exact-root">
    <div data-view="1" data-exact-on="keycapture key"><input data-view="2" data-exact-on="key submit" name="draft"></div></div>
    <script type="application/vnd.exact.checkpoint" data-activate="interaction" data-digest="test">{}</script>
    <script type="module" src="./document-glue.js"></script>`;
  const files = {
    '/document-glue.js': readFileSync(resolve(ROOT, 'host/web/document-glue.js'), 'utf8'),
    '/glue.js': `globalThis.__events = [];
      const views = new Map([...document.querySelectorAll('[data-view]')].map(el => {
        el.exactHandlers = el.dataset.exactOn.split(' '); return [Number(el.dataset.view), el];
      }));
      const page = exact.documentPage.connect({ views, dispatch: (id, kind, value, event) => {
        __events.push([id, kind, value]);
        if (kind === 44) { if (value.startsWith('Shift+')) event?.preventDefault(); else if (!value.startsWith('Alt+') && event) event.exactStopped = true; }
        if (kind === 6 && id === 1 && value.startsWith('Alt+')) event?.preventDefault();
      }});
      globalThis.__release = () => { page.hold({ops:[{op:'adopt',adopted:true}]}); page.release(() => {}); };`,
  };
  await withDocument('/', async tab => {
    const live = await tab(true);
    await live.until('!!globalThis.exact?.documentPage', 'document entry');
    await live(`(() => { const el = document.querySelector('input');
      el.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',bubbles:true,cancelable:true}));
      el.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',shiftKey:true,bubbles:true,cancelable:true}));
      el.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',altKey:true,bubbles:true,cancelable:true})); })()`);
    await live.until('typeof globalThis.__release === "function"', 'interaction runtime');
    await live('__release()');
    await live.until('!exact.documentPage.holding && __events.length >= 2', 'early key replay');
    expect(await live('__events.map(([id,kind]) => [id,kind])')).toEqual([[1,44],[2,7],[1,44],[2,6],[1,6],[1,44],[2,6],[1,6]]);
  }, { html, files });
}, 60000);

browserCheck('early presses reject replacement labels after an adoption mismatch', async () => {
  const html = `<!doctype html><div id="exact-root">
    <button data-view="1" data-exact-on="press">Delete Alice</button>
    <button data-view="2" data-exact-on="press">Keep</button></div>
    <script type="application/vnd.exact.checkpoint" data-activate="interaction" data-digest="test">{}</script>
    <script type="module" src="./document-glue.js"></script>`;
  const files = {
    '/document-glue.js': readFileSync(resolve(ROOT, 'host/web/document-glue.js'), 'utf8'),
    '/glue.js': `globalThis.__events = []; globalThis.__logs = [];
      const views = new Map(), root = document.getElementById('exact-root');
      const page = exact.documentPage.connect({ views, log: line => __logs.push(line),
        dispatch: (id, kind) => __events.push([id, kind]) });
      globalThis.__release = () => {
        page.hold({ops:[{op:'adopt',adopted:false}]});
        page.release(() => {
          for (const [id, label] of [[1, 'Delete Bob'], [2, 'Keep']]) {
            const el = document.createElement('button'); el.dataset.view = id;
            el.textContent = label; el.exactHandlers = ['press']; views.set(id, el); root.append(el);
          }
        });
      };`,
  };
  await withDocument('/', async tab => {
    const live = await tab(true);
    await live.until('!!globalThis.exact?.documentPage', 'document entry');
    await live("document.querySelectorAll('button').forEach(el => el.click())");
    await live.until('typeof globalThis.__release === "function"', 'interaction runtime');
    await live('__release()');
    expect(await live('__events')).toEqual([[2, 0]]);
    expect(await live('__logs.some(line => line.includes("early press was dropped (view 1"))')).toBe(true);
  }, { html, files });
}, 60000);

browserCheck(`early capture updates keep the same view's bubble handler`, async () => {
  const html = `<!doctype html><meta charset="utf-8"><div id="exact-root">
    <div data-view="1" data-exact-on="keycapture key"><input data-view="2" data-exact-on="submit"><span id="log">before</span></div></div>
    <script type="application/vnd.exact.checkpoint" data-activate="interaction" data-digest="test">{}</script>
    <script type="module" src="./document-glue.js"></script>`;
  const files = {
    '/document-glue.js': readFileSync(resolve(ROOT, 'host/web/document-glue.js'), 'utf8'),
    '/glue.js': `globalThis.__events = []; globalThis.__logs = [];
      const views = new Map([...document.querySelectorAll('[data-view]')].map(el => {
        el.exactHandlers = el.dataset.exactOn.split(' '); return [Number(el.dataset.view), el];
      }));
      const page = exact.documentPage.connect({ views, log: line => __logs.push(line),
        dispatch: (id, kind, value, event) => {
          __events.push([id, kind]);
          if (kind === 44) document.getElementById('log').textContent = 'after';
          if (kind === 6) event.preventDefault();
        }});
      globalThis.__release = () => { page.hold({ops:[{op:'adopt',adopted:true}]}); page.release(() => {}); };`,
  };
  await withDocument('/', async tab => {
    const live = await tab(true);
    await live.until('!!globalThis.exact?.documentPage', 'document entry');
    await live("document.querySelector('input').dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',bubbles:true,cancelable:true}))");
    await live.until('typeof globalThis.__release === "function"', 'interaction runtime');
    await live('__release()');
    await live.until('!exact.documentPage.holding', 'early key replay');
    expect(await live('__events')).toEqual([[1, 44], [1, 6]]);
    expect(await live('__logs')).toEqual([]);
  }, { html, files });
}, 60000);

// LLP 1069.011.001 D5/D6: the shipped shell isolates button layout and
// truncates only the requested titles. LLP 1116.005 D8 (superseding D16's
// kept accent on the web): a disabled button is the disabled look whatever
// its accent, a glass its own material (form-chrome.test.mjs has the rest).
for (const concern of ['defaults', 'nowrap', 'clamp', 'floor', 'disabled accent', 'disabled plain accent', 'disabled glass accent', 'disabled clear-glass accent']) browserCheck(`native button ${concern}`, async () => {
  const shell = SHIPPED;
  const button = (id, style = '', extra = '') => `<button id="${id}" data-native data-button-style="${id === 'plain-accent' ? 'plain' : id === 'glass-accent' ? 'glass' : id === 'clear-glass-accent' ? 'clear-glass' : 'filled'}" style="display:grid;width:120px;${style}" ${extra}><span data-exact-text>A native title long enough to wrap onto several lines</span></button>`;
  const html = shell.replace('<div id="exact-root"></div>', `<div id="exact-root"><div style="white-space:nowrap;text-align:end">${button('wrap')}${button('nowrap', 'white-space:nowrap;--exact-button-title-min:0px;')}${button('clamp', '--exact-button-clamp:2;--exact-button-title-display:-webkit-box;')}${button('authored', '--exact-accent:#ff0000;', 'disabled')}${button('align', 'text-align:end;')}${button('plain-accent', '--exact-accent:#ff0000;', 'disabled')}${button('default-off', '', 'disabled')}${button('glass-accent', '--exact-accent:#ff0000;', 'disabled')}${button('clear-glass-accent', '--exact-accent:#ff0000;', 'disabled')}</div><div style="display:flex;width:40px">${button('floor', 'width:auto;flex:1 1 0;')}</div><span id="fill" style="background-color:var(--exact-tertiary-fill)"></span></div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'native fixture');
    const facts = await page(`(() => { const read = id => { const b=document.getElementById(id),t=b.firstElementChild,s=getComputedStyle(b),c=getComputedStyle(t); return {whiteSpace:s.whiteSpace,align:s.textAlign,display:c.display,height:t.getBoundingClientRect().height,width:t.clientWidth,scroll:t.scrollWidth,ellipsis:c.textOverflow,background:s.backgroundColor,color:s.color}; }; return Object.fromEntries(['wrap','nowrap','clamp','authored','default-off','align','plain-accent','glass-accent','clear-glass-accent','floor'].map(id=>[id,read(id)])); })()`);
    if (concern === 'defaults') {
      expect(facts.wrap.whiteSpace).toBe('normal');
      expect(facts.wrap.align).toBe('center');
      expect(facts.align.align).toBe('end');
      expect(facts.wrap.display).toBe('block');
    }
    if (concern === 'clamp') {
      expect(facts.wrap.height).toBeGreaterThan(facts.clamp.height);
      // Grid item blockification reports flow-root for the legacy box.
      expect(['-webkit-box', 'flow-root']).toContain(facts.clamp.display);
      expect(facts.clamp.height).toBeCloseTo(facts.nowrap.height * 2, 1);
    }
    if (concern === 'nowrap') {
      expect(facts.nowrap.display).toBe('block');
      expect(facts.nowrap.scroll).toBeGreaterThan(facts.nowrap.width);
      expect(facts.nowrap.ellipsis).toBe('ellipsis');
    }
    // LLP 1116 D1: a wrapping title keeps its min-content floor in a squeezed
    // row, so no word is cut to an ellipsis.
    if (concern === 'floor') {
      expect(facts.floor.width).toBeGreaterThan(40);
      expect(facts.floor.scroll).toBeLessThanOrEqual(facts.floor.width);
    }
    if (concern === 'disabled glass accent') expect(facts['glass-accent'].background).toBe('rgba(255, 255, 255, 0.7)');
    if (concern === 'disabled clear-glass accent') expect(facts['clear-glass-accent'].background).toBe('rgba(255, 255, 255, 0.15)');
    if (concern === 'disabled plain accent') {
      expect(facts['plain-accent'].color).not.toBe('rgb(255, 0, 0)');
      expect(facts['plain-accent'].color).toBe(facts['default-off'].color);
    }
    if (concern === 'disabled accent') {
      // The look's neutral fill (`-exact-tertiary-fill`, which its sheet carries), not the accent's red.
      const fill = await page("getComputedStyle(document.getElementById('fill')).backgroundColor");
      expect(fill).not.toBe('rgba(0, 0, 0, 0)');
      expect(facts.authored.background).toBe(fill);
      expect(facts['default-off'].background).toBe(facts.authored.background);
    }
  }, {html});
});

// A face stacked in a column centres its image on its title when the image is narrower than the title's column, as the
// Linux host paints it; the face keeps the button's alignment.
browserCheck('native button stacked image', async () => {
  // A column's areas and spacing, as `rows` in host/web/src/button_css.rs writes them for `flex-direction: column`.
  const column = `--exact-button-leading-areas:'image' 'space' 'title';--exact-button-columns:minmax(var(--exact-button-title-min,min-content),auto);--exact-button-space:var(--exact-button-row-space);--exact-button-space-width:0px;--exact-button-space-height:var(--exact-button-row-gap,1em);`;
  const button = (id, style = '') => `<button id="${id}" data-native data-button-style="filled" style="display:grid;width:160px;font-size:20px;${column}${style}"><img alt=""><span data-exact-text>Unlock now</span></button>`;
  const html = SHIPPED.replace('<div id="exact-root"></div>', `<div id="exact-root">${button('centre')}${button('start', 'text-align:start;--exact-button-align:start;')}</div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'stacked fixture');
    const facts = await page(`(() => { const mid = r => Math.round(r.left + r.width / 2); return Object.fromEntries(['centre', 'start'].map(id => { const b = document.getElementById(id); return [id, [b.querySelector('img'), b.querySelector('span'), b].map(e => mid(e.getBoundingClientRect()))]; })); })()`);
    const [image, title, button] = facts.centre;
    expect([image, title]).toEqual([button, button]);
    const [startImage, startTitle, startButton] = facts.start;
    expect(startImage).toBe(startTitle);
    expect(startTitle).toBeLessThan(startButton);
  }, {html});
});

// LLP 1116.003 D3.1, as the neutral look draws it (LLP 1115.000 D4 part B):
// the ink on an accent fill is white or black, whichever reads at 4.5:1 or
// better on the accent as the scheme resolves it. A pale dark-scheme accent
// (event-rsvp's, recipe-finder's) takes black, and so does a mid-light blue
// (`#007aff`, on which white is 4.0:1); the unsaid accent and the destructive
// red are dark in light and light in dark, so white and then black; an
// authored colour wins. (LLP 1116 D1's look kept white down to 3:1.)
browserCheck('the ink on an accent fill is chosen for contrast in each scheme', async () => {
  const shell = SHIPPED;
  const cases = {
    pale: ['bordered-prominent', '--exact-accent:light-dark(#6652c5, #b5a4ff);'],
    salmon: ['filled', '--exact-accent:light-dark(#b74e2e, #f49d7d);'],
    teal: ['prominent-glass', '--exact-accent:light-dark(#16766c, #74d6c5);'],
    unsaid: ['bordered-prominent', ''],
    blue: ['filled', '--exact-accent:light-dark(#007aff, #0a84ff);'],
    destructive: ['bordered-prominent', '', 'data-destructive="true"'],
    authored: ['bordered-prominent', '--exact-accent:#b5a4ff;color:rgb(255, 0, 0);'],
    disabled: ['bordered-prominent', '--exact-accent:#b5a4ff;', 'disabled'],
  };
  const buttons = scheme => Object.entries(cases).map(([id, [look, style, extra = '']]) =>
    `<button id="${scheme}-${id}" data-native data-button-style="${look}" style="display:grid;${style}" ${extra}><span data-exact-text>Going</span></button>`).join('');
  const html = shell.replace('<div id="exact-root"></div>', `<div id="exact-root"><div style="color-scheme:light">${buttons('light')}</div><div style="color-scheme:dark">${buttons('dark')}</div><span id="light-gray" style="color-scheme:light;color:var(--exact-gray-text)"></span><span id="dark-gray" style="color-scheme:dark;color:var(--exact-gray-text)"></span></div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'accent fill fixture');
    // Each button's ink and fill as the engine paints them, and the ratio between the two.
    const drawn = await page(`(() => { const x = document.createElement('canvas').getContext('2d', {willReadFrequently: true});
      const rgb = c => { x.clearRect(0, 0, 1, 1); x.fillStyle = c; x.fillRect(0, 0, 1, 1); return [...x.getImageData(0, 0, 1, 1).data].slice(0, 3); };
      const lum = c => { const [r, g, b] = c.map(v => { v /= 255; return v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4; }); return .2126 * r + .7152 * g + .0722 * b; };
      return Object.fromEntries([...document.querySelectorAll('button, #light-gray, #dark-gray')].map(e => { const s = getComputedStyle(e), ink = rgb(s.color), fill = rgb(s.backgroundColor), [hi, lo] = [lum(ink), lum(fill)].sort((a, b) => b - a);
        return [e.id, { ink: ink.join(), ratio: (hi + .05) / (lo + .05) }]; })); })()`);
    const ink = Object.fromEntries(Object.entries(drawn).map(([id, d]) => [id, d.ink]));
    const [white, black] = ['255,255,255', '0,0,0'];
    expect(Object.fromEntries(['pale', 'salmon', 'teal', 'unsaid', 'blue', 'destructive'].flatMap(id => ['light', 'dark'].map(scheme => [`${scheme}-${id}`, ink[`${scheme}-${id}`]])))).toEqual({
      'light-pale': white, 'dark-pale': black, 'light-salmon': white, 'dark-salmon': black, 'light-teal': white, 'dark-teal': black,
      'light-unsaid': white, 'dark-unsaid': black, 'light-blue': black, 'dark-blue': black, 'light-destructive': white, 'dark-destructive': black,
    });
    // On a fill that is the accent itself (a glass is its own material) the ink reads at 4.5:1 or better.
    for (const id of ['pale', 'salmon', 'unsaid', 'blue', 'destructive']) for (const scheme of ['light', 'dark']) expect([scheme, id, drawn[`${scheme}-${id}`].ratio >= 4.5]).toEqual([scheme, id, true]);
    for (const scheme of ['light', 'dark']) {
      expect(ink[`${scheme}-authored`]).toBe('255,0,0');
      expect(ink[`${scheme}-disabled`]).toBe(ink[`${scheme}-gray`]);
    }
  }, {html});
});

// LLP 1116.003 D3.2: a symbol that names no tint is the nearest authored
// accent, as `AccentColor` is (LLP 1116.001 D2) and UIKit's inherited tint
// is: event-rsvp's calendar under `light-dark(#6652c5, #b5a4ff)` drew the
// browser's blue. Both targets' symbol sheets; an authored tint still wins.
for (const sheet of ['host/web/glue.js', 'host/web-js/symbols.js']) browserCheck(`an untinted symbol is the nearest accent (${sheet})`, async () => {
  const style = readFileSync(resolve(ROOT, sheet), 'utf8').match(/'(@property --exact-tint[^']*)'/)[1];
  const shell = SHIPPED;
  const img = (id, css = '') => `<img id="${id}" data-symbol-path="M0 0h16v16H0Z" style="width:16px;height:16px;${css}">`;
  const html = shell.replace('<div id="exact-root"></div>', `<style>${style}</style><div id="exact-root"><div style="--exact-accent:light-dark(#6652c5, #b5a4ff)"><div style="color-scheme:light">${img('light')}</div><div style="color-scheme:dark">${img('dark')}</div>${img('authored', '--exact-tint:rgb(255, 0, 0)')}</div>${img('unsaid')}<span id="system" style="color:var(--exact-accent, AccentColor)"></span></div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'symbol fixture');
    const tint = await page(`(() => Object.fromEntries(['light', 'dark', 'authored', 'unsaid'].map(id => [id, getComputedStyle(document.getElementById(id)).backgroundColor]).concat([['system', getComputedStyle(document.getElementById('system')).color]])))()`);
    expect(tint.light).toBe('rgb(102, 82, 197)');
    expect(tint.dark).toBe('rgb(181, 164, 255)');
    expect(tint.authored).toBe('rgb(255, 0, 0)');
    expect(tint.unsaid).toBe(tint.system); // with no accent authored, the page's own: the look's (the browser's hue, fitted to read)
  }, {html});
});

// LLP 1121 stage 9 B (D10): a symbol nobody tinted whose `color` its author
// wrote (`data-symbol-color`, element.rs) draws in that colour, a
// `light-dark()` one by its scheme and a later one at once; an authored tint
// still wins, and an unmarked symbol stays the accent. Both targets' sheets.
for (const sheet of ['host/web/glue.js', 'host/web-js/symbols.js']) browserCheck(`a symbol draws in its written colour (${sheet})`, async () => {
  const style = readFileSync(resolve(ROOT, sheet), 'utf8').match(/'(@property --exact-tint[^']*)'/)[1];
  const img = (id, css = '', marked = true) => `<img id="${id}" data-symbol-path="M0 0h16v16H0Z"${marked ? ' data-symbol-color' : ''} style="width:16px;height:16px;${css}">`;
  const html = SHIPPED.replace('<div id="exact-root"></div>', `<style>${style}</style><div id="exact-root" style="--exact-accent:rgb(102, 82, 197)"><div id="ink" style="color:rgb(10, 20, 30)">${img('written')}${img('authored', '--exact-tint:rgb(255, 0, 0)')}${img('unmarked', '', false)}${['light', 'dark'].map(scheme => `<div style="color-scheme:${scheme};color:light-dark(rgb(1, 2, 3), rgb(4, 5, 6))">${img(scheme)}</div>`).join('')}</div></div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'symbol fixture');
    const read = `(() => Object.fromEntries(['written', 'authored', 'unmarked', 'light', 'dark'].map(id => [id, getComputedStyle(document.getElementById(id)).backgroundColor])))()`;
    const tint = await page(read);
    expect(tint).toEqual({ written: 'rgb(10, 20, 30)', authored: 'rgb(255, 0, 0)', unmarked: 'rgb(102, 82, 197)', light: 'rgb(1, 2, 3)', dark: 'rgb(4, 5, 6)' });
    await page(`document.getElementById('ink').style.color = 'rgb(0, 128, 0)'`);
    expect((await page(read)).written).toBe('rgb(0, 128, 0)');
  }, {html});
});

// LLP 1115.000 D3.8: an SVG shape nobody filled is its text's colour, where
// the browser's own initial `fill` is black. The page's sheet says it on the
// outermost `svg`, so a `fill` on a shape or on a group wins and reaches a
// nested `svg` below the group; `currentcolor` is each element's own colour;
// a gradient's stop and a filter's flood stay SVG's black.
browserCheck('an unsaid SVG fill is the text colour', async () => {
  const shell = SHIPPED;
  const rect = (id, css = '') => `<rect id="${id}" width="4" height="4"${css ? ` style="${css}"` : ''}/>`;
  const html = shell.replace('<div id="exact-root"></div>', `<div id="exact-root"><div style="color:rgb(10, 20, 30)">
    <svg id="root" viewBox="0 0 20 20" style="width:20px;height:20px">${rect('unsaid')}${rect('own', 'fill:rgb(255, 0, 0)')}
      <g style="fill:rgb(0, 128, 0)">${rect('group')}<svg>${rect('nested')}</svg></g>
      <g style="color:rgb(1, 2, 3)">${rect('recoloured')}<svg>${rect('nested-colour')}</svg></g>
      <g style="fill:none">${rect('none')}</g>
      <linearGradient><stop id="stop"/></linearGradient><filter><feFlood id="flood"/></filter>
    </svg></div><svg id="plain" viewBox="0 0 4 4" style="width:4px;height:4px">${rect('page')}</svg><span id="ink">ink</span></div>`)
    .replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'svg fixture');
    const of = (id, property = 'fill') => `[${JSON.stringify(id)}, getComputedStyle(document.getElementById(${JSON.stringify(id)}))[${JSON.stringify(property)}]]`;
    const got = await page(`Object.fromEntries([${['unsaid', 'own', 'group', 'nested', 'recoloured', 'nested-colour', 'none', 'page'].map((id) => of(id)).join(',')},
      ${of('stop', 'stopColor')}, ${of('flood', 'floodColor')}, ${of('ink', 'color')}])`);
    expect(got.unsaid).toBe('rgb(10, 20, 30)');
    expect(got.own).toBe('rgb(255, 0, 0)');
    expect(got.group).toBe('rgb(0, 128, 0)');
    expect(got.nested).toBe('rgb(0, 128, 0)'); // a nested svg does not end the group's fill
    expect(got.recoloured).toBe('rgb(1, 2, 3)');
    expect(got['nested-colour']).toBe('rgb(1, 2, 3)');
    expect(got.none).toBe('none');
    expect(got.page).toBe(got.ink); // no colour written: the page's own text colour
    expect(got.stop).toBe('rgb(0, 0, 0)');
    expect(got.flood).toBe('rgb(0, 0, 0)');
  }, {html});
});

// LLP 1116.001 D1: a date, time or local date-time control takes the text
// field's look (a 1 px line, 8 px corners, the field's background, 36 px
// tall at the body size) in light and dark, as a border box the kernel
// models; the author's rows win, and `appearance: none` (no marker) is bare.
browserCheck('date and time controls take the field look', async () => {
  const shell = SHIPPED;
  const field = (id, type, extra = '') => `<input id="${id}" type="${type}" ${extra}>`;
  const html = shell.replace('<div id="exact-root"></div>', `<div id="exact-root"><div style="width:300px">${
    field('date', 'date', 'data-exact-field')}${field('time', 'time', 'data-exact-field value="14:30"')}${
    field('local', 'datetime-local', 'data-exact-field')}${field('off', 'date', 'data-exact-field disabled')}${
    field('authored', 'date', 'data-exact-field style="border:2px solid rgb(255, 0, 0);padding:3px;border-radius:0;box-sizing:content-box;width:200px"')}${
    field('bare', 'date', 'style="appearance:none"')}<input id="text" data-native><div style="color-scheme:dark">${
    field('dark', 'date', 'data-exact-field')}<input id="dark-text" data-native></div></div></div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'field fixture');
    const facts = await page(`(() => Object.fromEntries(['date','time','local','off','authored','bare','text','dark','dark-text'].map(id => { const e = document.getElementById(id), s = getComputedStyle(e), r = e.getBoundingClientRect(); return [id, {width: r.width, height: r.height, border: s.borderTopWidth + ' ' + s.borderTopStyle + ' ' + s.borderTopColor, radius: s.borderTopLeftRadius, background: s.backgroundColor, sizing: s.boxSizing, appearance: s.appearance, color: s.color}]; })))()`);
    for (const id of ['date', 'time', 'local']) {
      expect(facts[id].height).toBe(36);
      expect(facts[id].sizing).toBe('border-box');
      expect(facts[id].appearance).toBe('auto');
      expect(facts[id].border).toBe(facts.text.border);
      expect(facts[id].radius).toBe(facts.text.radius);
      expect(facts[id].background).toBe(facts.text.background);
    }
    expect(facts.date.border).toMatch(/^1px solid (?!rgb\(0, 0, 0\)$)/); // the look's line for a control's edge, as the text field's is
    expect(facts.dark.border).toBe(facts['dark-text'].border);
    expect(facts.dark.background).toBe(facts['dark-text'].background);
    expect(facts.dark.background).not.toBe(facts.date.background);
    expect(facts.off.background).not.toBe(facts.date.background);
    expect(facts.authored.border).toBe('2px solid rgb(255, 0, 0)');
    expect(facts.authored.radius).toBe('0px');
    expect(facts.authored.width).toBe(210);
    expect(facts.bare.border).toBe('0px none rgb(0, 0, 0)');
    expect(facts.bare.sizing).toBe('content-box');
  }, {html});
});

// LLP 1116.003: a determinate progress is the platform's bar, as UIKit draws
// its own and Linux paints it: a 4 px capsule centred across its box, the
// accent's over a track of the system fill, the same fill whatever the
// accent, in light and dark. Chrome's own takes its track by contrast with
// the accent (near black under teal in light, near white under indigo in
// dark). An authored background is the box's, under the track, as on iOS.
browserCheck('a determinate progress is the accent over the system fill in light and dark', async () => {
  const shell = SHIPPED;
  // As both targets write a role's accent: `accent-color` and the custom property beside it. A role is Apple's pair
  // (look.mjs, from the schema), light and dark.
  const apple = (role) => JSON.parse(readFileSync(resolve(ROOT, 'kernel/tables/schema.json'), 'utf8')).colors.find(([name]) => name === role).slice(4, 6);
  const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  const [tealPair, indigoPair] = [apple('system-teal'), apple('system-indigo')];
  const accent = (role, [light, dark]) => `accent-color:var(--exact-${role}, light-dark(${light}, ${dark}));--exact-accent:var(--exact-${role}, light-dark(${light}, ${dark}));`;
  const teal = accent('system-teal', tealPair), indigo = accent('system-indigo', indigoPair);
  const bar = (id, style) => `<progress id="${id}" value="0.25" max="1" style="width:160px;margin:8px;${style}"></progress>`;
  // A box of the look's `-exact-fill` over the same ground as the bars beside it: what a track is drawn in.
  const fill = (id, under = '') => `<div style="width:160px;margin:8px;${under}"><div id="${id}" style="height:16px;background-color:var(--exact-fill)"></div></div>`;
  const html = shell.replace('<div id="exact-root"></div>', `<div id="exact-root"><div style="color-scheme:light;background:#fff;padding:8px">${
    bar('teal-light', teal)}${bar('indigo-light', indigo)}${bar('authored', teal + 'background-color:rgb(255, 0, 0)')}${fill('fill-light')}${fill('fill-red', 'background-color:rgb(255, 0, 0)')}</div><div style="color-scheme:dark;background:#000;padding:8px">${
    bar('teal-dark', teal)}${bar('indigo-dark', indigo)}${fill('fill-dark')}</div></div>`).replace('<script type="module" src="./glue.js"></script>', '');
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'progress fixture');
    await page(settled);
    const boxes = await page(`(() => Object.fromEntries([...document.querySelectorAll('progress, [id^="fill-"]')].map(e => { const r = e.getBoundingClientRect(); return [e.id, [r.x, r.y, r.width, r.height]]; })))()`);
    const { data } = await page.call('Page.captureScreenshot', { format: 'png' });
    const shot = decodePng(Buffer.from(data, 'base64'));
    const at = (id, dx, dy) => { const [x, y] = boxes[id], i = (Math.round(y + dy) * shot.width + Math.round(x + dx)) * 4; return [...shot.data.slice(i, i + 3)]; };
    const near = (seen, want, what) => expect(seen.every((c, i) => Math.abs(c - want[i]) <= 3), `${what}: ${seen} for ${want}`).toBe(true);
    for (const id of Object.keys(boxes)) expect(boxes[id].slice(2)).toEqual([160, 16]);
    // The fill over white, over black and over the authored red, as the boxes of it show; it is a fill, so neither ground.
    const [light, dark, red] = ['fill-light', 'fill-dark', 'fill-red'].map((id) => at(id, 80, 8));
    expect([light.join() === '255,255,255', dark.join() === '0,0,0', red.join() === '255,0,0']).toEqual([false, false, false]);
    for (const id of ['teal-light', 'indigo-light']) { near(at(id, 120, 8), light, `${id} track`); near(at(id, 120, 2), [255, 255, 255], `${id} above its track`); }
    for (const id of ['teal-dark', 'indigo-dark']) { near(at(id, 120, 8), dark, `${id} track`); near(at(id, 120, 2), [0, 0, 0], `${id} above its track`); }
    near(at('teal-light', 20, 8), rgb(tealPair[0]), 'teal bar in light');
    near(at('indigo-light', 20, 8), rgb(indigoPair[0]), 'indigo bar in light');
    near(at('teal-dark', 20, 8), rgb(tealPair[1]), 'teal bar in dark');
    near(at('indigo-dark', 20, 8), rgb(indigoPair[1]), 'indigo bar in dark');
    near(at('authored', 120, 2), [255, 0, 0], 'the authored background is the box');
    near(at('authored', 120, 8), red, 'the track over it');
    near(at('authored', 20, 8), rgb(tealPair[0]), 'the bar over it');
  }, {html});
});

// The page's ground is the look's background role (Apple's, black in dark; not each engine's own `Canvas`).
browserCheck('modal routes use the page\'s ground, keep author paint, and reveal the sheet beneath on Back', async () => {
  const css = SHIPPED.match(/<style>([\s\S]*?)<\/style>/)[1];
  const html = `<!doctype html><style>${css}</style><style>#exact-root#exact-root .authored {background-color:rgb(20,40,60)}</style>
    <div id="exact-root"><main id="nav" navigationKey="2" navigationBack="back">
      <section id="home" navigationKey="0"><button>Home</button></section>
      <section id="first" navigationKey="1" navigationPresentation="modal"><button>First sheet</button></section>
      <section id="second" navigationKey="2" navigationPresentation="modal"><button>Second sheet</button></section>
    </main><div id="canvas" style="background-color:var(--exact-background)"></div><div id="bare"></div></div>`;
  await withDocument('/', async tab => {
    const page = await tab(true);
    await page.until("document.readyState === 'complete'", 'modal fixture');
    await page(`import('/navigation.js').then(({navigation}) => {
      globalThis.project = () => navigation.project(document.getElementById('exact-root'), line => {throw new Error(line)});
      project();
    })`);
    const read = `(() => { const read = id => {const el=document.getElementById(id),s=getComputedStyle(el);return {background:s.backgroundColor,visibility:s.visibility,inert:el.inert,covered:el.hasAttribute('data-exact-covered')};};
      return Object.fromEntries(['home','first','second','canvas','bare'].map(id=>[id,read(id)])); })()`;
    const canvases = [];
    for (const scheme of ['light', 'dark']) {
      await page(`document.documentElement.style.colorScheme='${scheme}'`);
      const facts = await page(read);
      canvases.push(facts.canvas.background);
      expect(facts.first.background).toBe(facts.canvas.background);
      expect(facts.second.background).toBe(facts.canvas.background);
      expect(facts.second.background).not.toBe('rgba(0, 0, 0, 0)');
      expect(facts.bare.background).toBe('rgba(0, 0, 0, 0)');
      // The page under the sheets is drawn too, covered: it shows above a narrow sheet's top.
      expect([facts.home.visibility, facts.home.covered, facts.home.inert]).toEqual(['visible', true, true]);
      expect(facts.first.covered).toBe(true);
      expect(facts.first.inert).toBe(true);
      expect(facts.second.inert).toBe(false);
      // Authored static classes and inline rows win, including deliberate transparency.
      await page("document.getElementById('second').className='authored'");
      expect((await page(read)).second.background).toBe('rgb(20, 40, 60)');
      await page("document.getElementById('second').style.backgroundColor='transparent'");
      expect((await page(read)).second.background).toBe('rgba(0, 0, 0, 0)');
      await page("document.getElementById('second').style.backgroundColor='';document.getElementById('second').className=''");
    }
    expect(canvases[0]).not.toBe(canvases[1]);
    await page("document.getElementById('nav').setAttribute('navigationKey','1');project()");
    const back = await page(read);
    expect(back.first.inert).toBe(false);
    expect(back.first.covered).toBe(false);
    expect(back.first.visibility).toBe('visible');
    expect(back.second.visibility).toBe('hidden');
    expect(back.second.inert).toBe(true);
  }, {html, files: {'/navigation.js': readFileSync(resolve(ROOT, 'host/web/navigation.js'), 'utf8')}});
}, 20000);

test('native button WebKit allowance ends at exactly 6.00 px', () => {
  const known = JSON.parse(readFileSync(resolve(ROOT, 'host/web-js/conformance/known-webkit.json'), 'utf8'));
  const pattern = new RegExp(known.find(entry => entry.app === 'synthetic-native-buttons').pattern);
  for (const delta of ['0.00', '4.99', '5.99', '6.00']) expect(pattern.test(`(delta ${delta} px, tolerance 4 px)`)).toBe(true);
  for (const delta of ['6.01', '6.99', '7.00', '16.00']) expect(pattern.test(`(delta ${delta} px, tolerance 4 px)`)).toBe(false);
});

// These event probes need only Chrome and the production document entry.
async function withFocusDocument(drive, {html, files = {}}) {
  const {chromium} = await import('playwright-core');
  const server = Bun.serve({port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    return new Response(path === '/' ? html : files[path] ?? '',
      {headers: {'content-type': path === '/' ? 'text/html' : 'text/javascript'}});
  }});
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    const session = await page.context().newCDPSession(page);
    const live = expression => page.evaluate(expression);
    live.until = expression => page.waitForFunction(expression);
    live.call = (method, params) => session.send(method, params);
    await drive(async () => live);
  } finally { await browser?.close(); server.stop(true); }
}

browserCheck('early focus replay stops at the inner handler for each transition', async () => {
  const html = `<!doctype html><div id="exact-root">
    <div data-view="1" data-exact-on="focusin focusout"><div data-view="2" data-exact-on="focusin focusout">
    <input id="a" data-view="3"><input id="b" data-view="4"></div></div></div>
    <script type="application/vnd.exact.checkpoint" data-activate="interaction">{}</script>
    <script type="module" src="./document-glue.js"></script>`;
  const files = {
    '/document-glue.js': readFileSync(resolve(ROOT, 'host/web/document-glue.js'), 'utf8'),
    '/glue.js': `globalThis.__events = [];
      const views = new Map([...document.querySelectorAll('[data-view]')].map(el => {
        el.exactHandlers = (el.dataset.exactOn ?? '').split(' '); return [Number(el.dataset.view), el];
      }));
      const page = exact.documentPage.connect({ views, log: console.warn,
        dispatch: (id, kind, value, event) => { __events.push([id, kind, value]); if (id === 2 && event) event.exactStopped = true; } });
      globalThis.__release = () => { page.hold({ops:[{op:'adopt',adopted:true}]}); page.release(() => {}); };`,
  };
  await withFocusDocument(async tab => {
    const live = await tab(true);
    await live.until('!!globalThis.exact?.documentPage', 'document entry');
    await live("document.getElementById('a').focus(); document.getElementById('b').focus()");
    await live.until('!!globalThis.__release', 'runtime connection');
    await live('__release()');
    expect(await live('__events')).toEqual([[2,46,'1:a0'],[2,47,'1:a1b'],[2,46,'1:b1a']]);
  }, {html, files});
}, 180000);

browserCheck('Chrome orders edited focus transitions including wrapper removal during blur', async () => {
  await withFocusDocument(async tab => {
    const live = await tab(true);
    await live.until('!!document.getElementById("b")', 'fields');
    const prepare = remove => live(`(() => {
      globalThis.heard = [];
      const a = document.getElementById('a'), b = document.getElementById('b'), wrapper = document.getElementById('wrapper');
      for (const el of [a,b]) for (const kind of ['change','blur','focusout','focus','focusin'])
        el.addEventListener(kind, () => heard.push(el.id + ':' + kind));
      wrapper.addEventListener('focusout', () => heard.push('wrapper:focusout'));
      document.body.addEventListener('focusout', () => heard.push('body:focusout'));
      if (${remove}) a.addEventListener('blur', () => wrapper.remove());
      a.focus(); heard.length = 0;
    })()`);
    await prepare(false);
    await live.call('Input.insertText', {text:'edited'});
    await live("document.getElementById('b').focus()");
    expect(await live('heard')).toEqual(['a:change','a:blur','a:focusout','wrapper:focusout','body:focusout','b:focus','b:focusin']);
    await live("document.body.innerHTML = '<div id=wrapper><input id=a></div><input id=b>'");
    await prepare(true);
    await live.call('Input.insertText', {text:'edited'});
    await live("document.getElementById('b').focus()");
    expect(await live('heard')).toEqual(['a:change','a:blur','a:focusout','wrapper:focusout','b:focus','b:focusin']);
  }, {html:'<!doctype html><div id="wrapper"><input id="a"></div><input id="b">'});
}, 180000);

browserCheck('Chrome runs disabling blur before the next focus', async () => {
  await withFocusDocument(async tab => {
    const live = await tab(true);
    expect(await live(`(() => {
      const a = document.getElementById('a'), b = document.getElementById('b'), heard = [];
      let focused = true;
      a.addEventListener('blur', () => { heard.push('blur'); focused = false; });
      b.addEventListener('focus', () => { heard.push('focus'); focused = true; });
      a.focus(); a.disabled = true; b.hidden = false; b.focus();
      return [heard, focused, document.activeElement.id];
    })()`)).toEqual([['blur', 'focus'], true, 'b']);
  }, {html: '<!doctype html><input id="a"><input id="b" hidden>'});
}, 180000);

async function withFocusContract(target, webDist, contract, drive) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-compiled-focus-'));
  try {
    const source = resolve(dir, 'app.contract'), plan = resolve(dir, 'app.plan');
    writeFileSync(source, contract);
    const compiler = resolve(app.target, 'debug/contract');
    const built = spawnSync(compiler, ['build', source, '-o', plan], { cwd: ROOT, encoding: 'utf8' });
    if (built.error) throw built.error;
    if (built.status !== 0) throw new Error(built.stderr);
    let server;
    let options = { plan, webDist: target === 'wasm' ? resolve(webDist) : undefined };
    if (target === 'JS') {
      const out = resolve(dir, 'dist');
      const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', out, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
      if (build.status !== 0) throw new Error(build.stderr);
      server = createServer((req, res) => serveBuildTree(out, req, res));
      await new Promise(ok => server.listen(0, '127.0.0.1', ok));
      options = { url: `http://127.0.0.1:${server.address().port}/` };
    }
    let s;
    try {
      s = await open({ host: 'web', ...options });
      await drive(s);
    } finally { await s?.close(); server?.close(); }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

// Exercise compiled Contract through both runtimes; raw DOM alone misses retired listeners.
// The wasm fixtures need an EXACT_WEB_LINK=all build for animation support.
for (const [target, webDist] of Object.entries({ wasm: process.env.EXACT_WEB_DIST, JS: true })) {
  const check = browserUnavailable ? need('chrome', browserUnavailable)
    : need('fixture', webDist ? null : 'no wasm dist built with EXACT_WEB_LINK=all: name one with EXACT_WEB_DIST');
  for (const animated of [false, true]) check(`${target} compiled focusout survives ${animated ? 'exit-animated' : 'immediate'} wrapper removal during blur`, async () => {
    await withFocusContract(target, webDist, `keyframes leave
  to opacity=0

component App
  state shown = true
  state log = ""
  action reset
    shown = true
    log = ""
  action changed(value: string)
    log = \`\${log}change;\`
  action remove
    log = \`\${log}blur;\`
    shown = false
  action heard(label: string, e: FocusEvent)
    log = \`\${log}\${label}:\${e.target}:\${match e.relatedTarget { case some(id) => id, case none => "-" }};\`
  view
    column focusout=heard("outer")
      if shown
        for label in ["wrapper"] key=label
          column focusout=heard(label) ${animated ? '-exact-exit-animation="leave 10s linear both"' : ""}
            input id="a" change=changed blur=remove focusout=heard("field")
      input id="b" focusin=heard("in")
      button "Reset" id="reset" press=reset
      button "Remove" id="remove" press=remove
      text log testId="log"
`, async s => {
      const evaluate = s.carrier.evaluate;
      if (animated && target === 'JS') await evaluate("(async () => { while (!globalThis.exact?.presenceLive) await new Promise(requestAnimationFrame); })()", 15000);
      for (const mode of ['focus', 'Tab', 'teardown']) {
        await evaluate("document.activeElement.blur(); document.getElementById('reset').click(); document.getElementById('a').focus()");
        if (mode === 'teardown') {
          await evaluate("document.getElementById('remove').click()");
          expect(await evaluate("document.querySelector('[data-testid=log]').textContent.match(/blur;/g)")).toEqual(['blur;']);
          expect(await evaluate("document.getElementById('a') === null")).toBe(true);
          continue;
        }
        await s.carrier.call('Input.insertText', { text: 'edited' });
        if (mode === 'focus') await evaluate("document.getElementById('b').focus()");
        else {
          await s.carrier.call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
          await s.carrier.call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
        }
        if (animated) expect(await evaluate("!!document.querySelector('[data-exiting]')")).toBe(true);
        expect(await evaluate("document.querySelector('[data-testid=log]').textContent")).toBe('change;blur;field:a:b;wrapper:a:b;in:b:a;');
      }
    });
  }, 240000);
  for (const command of [false, true]) check(`${target} compiled flush blur precedes ${command ? 'focus command' : 'autofocus'}`, async () => {
    await withFocusContract(target, webDist, `component App
  state disabled = false
  state focused = true
  state log = ""
  action move
    disabled = true
    ${command ? 'focus("b")' : ''}
  action left
    focused = false
    log = \`\${log}blur;\`
  action entered
    focused = true
    log = \`\${log}focus;\`
  view
    column
      input id="a" disabled=disabled blur=left
      if disabled
        input id="b" ${command ? '' : 'autofocus'} focus=entered
      button "Move" id="move" press=move
      text log testId="log"
      text \`\${focused}\` testId="focused"
`, async s => {
      const evaluate = s.carrier.evaluate;
      await evaluate("document.getElementById('a').focus(); document.getElementById('move').click()");
      expect(await evaluate("[document.querySelector('[data-testid=log]').textContent, document.querySelector('[data-testid=focused]').textContent, document.activeElement.id]")).toEqual(['blur;focus;', 'true', 'b']);
    });
  }, 240000);
}
