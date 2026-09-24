// The document parity check (LLP 1048.000 §4): Caltrain's page as the
// projection renders it, parsed by Chrome with JavaScript off, against the
// live host's DOM at the same URL once the runtime is ready. A difference
// names the view. What the browser owns after layout — a symbol's sized
// source, a surface's pixel size, focus — is left out of both sides.
import { test, expect } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp, assertWebDistApp, browserDiagnosticNoise } from '../../../scripts/agent.mjs';
import { resolveApp } from '../../../scripts/app.mjs';
import { serveStatic } from '../serve.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));
let unavailable;
try { await assertWebDistApp(dist, resolveApp('caltrain')); } catch (error) {
  if (!error.message.startsWith('web dist is not a complete build')) throw error;
  unavailable = error.message;
  console.warn(`SKIP: ${unavailable}`);
}
const check = unavailable ? test.skip : test;
const [width, height] = [390, 844]; // the page viewport documents render at

/** The rendered page: the shell with the renderer's head, the document in
 * `#exact-root` and its checkpoint, as the build writes it. */
function renderedPage(location) {
  const render = spawnSync('cargo', ['run', '-q', '-p', 'caltrain-linux', '--bin', 'caltrain-render', '--',
    '--plan', resolve(dist, 'app.plan'), '--viewport', `${width}x${height}`, '--shell', resolve(ROOT, 'host/web/index.html'), location],
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
  const wasm = performance.getEntriesByType('resource').filter((e) => e.name.endsWith('/app.wasm'));
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
async function withDocument(location, drive, { wasmAfter = null, glueAfter = null, tamper = (page) => page, origin = null, html = null, files = {} } = {}) {
  let server = null, url = `${origin}${location}`;
  if (!origin) {
    const page = tamper(html ?? renderedPage(location));
    server = createServer(async (req, res) => {
      if (req.url === location || req.url === '/') { res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(page); return; }
      if (files[req.url]) { res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(files[req.url]); return; }
      // A slow network, where a test needs the runtime to still be loading.
      if (wasmAfter && req.url.startsWith('/app.wasm')) await wasmAfter;
      if (glueAfter && req.url.startsWith('/glue.js')) await glueAfter;
      serveStatic(dist, req, res);
    });
    await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
    url = `http://127.0.0.1:${server.address().port}${location}`;
  }
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-document-'));
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--window-size=${width},${height}`, '--hide-scrollbars',
    `--user-data-dir=${profile}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking',
    '--disable-component-update', '--no-first-run', '--no-default-browser-check', 'about:blank'],
  { detached: true, stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
  const lines = [];
  child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l && !browserDiagnosticNoise(l)) lines.push(l); });
  const exited = new Promise((ok) => child.on('exit', ok));
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const tab = async (scripts, onNew) => {
      const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
      const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
      const call = (method, params) => cdp.send(method, params, sessionId);
      await call('Page.enable');
      await call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
      await call('Emulation.setScriptExecutionDisabled', { value: !scripts });
      if (onNew) await call('Page.addScriptToEvaluateOnNewDocument', { source: onNew });
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
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
    await Promise.race([exited, Bun.sleep(2000)]);
    server?.close();
    rmSync(profile, { recursive: true, force: true });
  }
}

const settled = 'new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))';

check(`Caltrain's served document is the live host's DOM${unavailable ? ` — ${unavailable}` : ''}`, async () => {
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

check(`a document whose digest doesn't match is replaced once, and the journal says where${unavailable ? ` — ${unavailable}` : ''}`, async () => {
  await withDocument('/?agent=1', async (tab) => {
    const live = await tab(true, WATCH);
    await live.until("document.getElementById('exact-root')?.dataset.moduleReady === 'true'", 'the runtime');
    await live(settled);
    const { emptyFrames, swaps } = JSON.parse(await live('JSON.stringify(globalThis.__watch)'));
    expect({ emptyFrames, swaps }).toEqual({ emptyFrames: 0, swaps: 1 });
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(false);
    expect((await live(SERVED_VIEWS)).served).toBe(0);
    expect((await live("exact.agent({op:'logs'})")).lines.join('\n')).toContain('document: not adopted: the same');
  }, { tamper: (page) => page.replace(/data-digest="[0-9a-f]{64}"/, `data-digest="${'0'.repeat(64)}"`) });
}, 180000);

check(`a press on the document before the runtime starts is replayed once${unavailable ? ` — ${unavailable}` : ''}`, async () => {
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

check(`a press before the glue runs is captured, replays once, and a later one isn't doubled${unavailable ? ` — ${unavailable}` : ''}`, async () => {
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
    const presses = async () => (await live("exact.agent({op:'logs'})")).lines.filter((l) => /press view \d+ \(setScheme\)/.test(l)).length;
    expect(await presses()).toBe(1);
    expect((await live("exact.agent({op:'state'})")).adopted).toBe(true);
    // After adoption a press is the runtime's own: once.
    await press('scheme-light');
    await live.until("document.documentElement.style.colorScheme === 'light'", 'the live press');
    await live(settled);
    expect(await presses()).toBe(2);
  }, { glueAfter });
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

check(`the render server's page is the document, and the runtime adopts it${unavailable ? ` — ${unavailable}` : ''}`, async () => {
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
test('a TypeScript app\'s served document is adopted, with its module running under the CSP', async () => {
  const out = mkdtempSync(resolve(tmpdir(), 'exact-weatherlight-'));
  try {
    const build = spawnSync('bun', ['host/web/build.mjs', 'weatherlight'], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20,
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
      }, { origin: server.origin });
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
test('interaction documents stay readable, then replay edits and actions once', async () => {
  const html = `<!doctype html><meta charset="utf-8"><div id="exact-root">
    <div data-view="1" data-exact-on="navigate">
      <p id="reading">Public content</p><a href="#reading" id="link">Read more</a>
      <input data-view="2" data-exact-on="change submit" name="title">
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
      globalThis.__release = () => { page.hold({ops:[{op:'adopt',adopted:true}]}); page.release(() => {}); };`,
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
    await live(`(() => { const el = document.querySelector('input'); el.value = '東京';
      el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));
      el.dispatchEvent(new InputEvent('input',{bubbles:true})); el.setSelectionRange(1,1);
      el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));
      document.querySelector('button').click(); })()`);
    await live.until('!exact.documentPage.holding && __events.length === 3', 'semantic replay');
    expect(await live('__events')).toEqual([[2,1,'東京'],[2,7,''],[3,0,'']]);
    expect(await live('__logs')).toEqual([]);
    expect(await live("[document.querySelector('input').value,document.querySelector('input').selectionStart,document.activeElement.localName]")).toEqual(['東京',1,'input']);
    expect(await live('__loads')).toBe(1);
    expect(await live("document.getElementById('reading').textContent")).toBe('Public content');
  }, { html, files });
}, 60000);
