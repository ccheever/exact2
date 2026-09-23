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

/** The rendered page: the built shell with the document in `#exact-root`. */
function renderedPage(location) {
  const render = spawnSync('cargo', ['run', '-q', '-p', 'caltrain-web', '--bin', 'caltrain-render', '--',
    '--plan', resolve(dist, 'app.plan'), '--viewport', `${width}x${height}`, location],
  { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' } });
  if (render.status !== 0) throw new Error(`caltrain-render: ${render.stderr}${render.stdout}`);
  const page = JSON.parse(render.stdout.trim().split('\n').at(-1));
  if (page.error) throw new Error(`caltrain-render ${location}: ${page.error}`);
  const shell = readFileSync(resolve(dist, 'index.html'), 'utf8');
  if (!shell.includes('<div id="exact-root"></div>')) throw new Error('the built shell has no empty #exact-root');
  return shell.replace('<div id="exact-root"></div>', () => `<div id="exact-root">${page.root}</div>`);
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

check(`Caltrain's served document is the live host's DOM${unavailable ? ` — ${unavailable}` : ''}`, async () => {
  // The live page launches at its own URL, agent query included; the
  // document is rendered at that same location.
  const location = '/?agent=1';
  const page = renderedPage(location);
  const server = createServer((req, res) => {
    if (req.url === location || req.url === '/') { res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(page); return; }
    serveStatic(dist, req, res);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const url = `http://127.0.0.1:${server.address().port}${location}`;
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
    const tab = async (scripts) => {
      const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
      const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
      const call = (method, params) => cdp.send(method, params, sessionId);
      await call('Page.enable');
      await call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
      await call('Emulation.setScriptExecutionDisabled', { value: !scripts });
      const evaluate = async (expression) => {
        const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
        return r.result.value;
      };
      await call('Page.navigate', { url });
      return evaluate;
    };
    const until = async (evaluate, expression, what) => {
      const start = Date.now();
      while (!(await evaluate(expression).catch(() => false))) {
        if (Date.now() - start > 60000) throw new Error(`${what} never happened; ${lines.join('\n')}`);
        await Bun.sleep(20);
      }
    };
    const plain = await tab(false);
    await until(plain, "document.readyState === 'complete' && !!document.getElementById('exact-root')?.firstElementChild", 'the served document');
    const served = JSON.parse(await plain(NORMALIZED));
    const live = await tab(true);
    await until(live, "document.getElementById('exact-root')?.dataset.bootMs != null", 'the first frame');
    await live('exact.ready');
    await live('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))');
    const found = differences(served, JSON.parse(await live(NORMALIZED)));
    expect(found).toEqual([]);
    expect(served.length).toBeGreaterThan(0);
  } finally {
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
    await Promise.race([exited, Bun.sleep(2000)]);
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 180000);
