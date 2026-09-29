// The RealWorld bench, LLP 1047.000 §1's method, for any build: headless
// Chrome, a fresh profile per run (cold cache), the mobile profile (150 ms
// RTT, 1.6 Mbps down, 750 kbps up, 4× CPU). Four measurements, medians:
//   content   — content painted at `/` (the first preview on screen), with FCP;
//   tag       — a tag tapped at `load` → its feed shown;
//   runtime   — the runtime up, after a press that needs it at `load`;
//   press     — a press that needs the runtime, at `load` → its effect
//               (RealWorld: the sign-in form's submit, on `/login`);
// and the bytes the page's origin sent before each finished (and in all).
//
//   bun host/web-js/bench.mjs <name>=<url> … [--runs 5] [--profile mobile|none] [--config targets.json] [--json out.json]
//
// A target's selectors default to RealWorld on exact (this repo's testIds);
// a `--config` file overrides them per name, for the React builds:
//   { "react-ssr": { "content": ".article-preview", "tag": "a.tag-pill", "tagDone": "…", "press": "…", "pressDone": "…", "runtime": "…" } }
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
const runs = Number(opt('--runs', 5)), profile = opt('--profile', 'mobile');
const config = opt('--config') ? JSON.parse(readFileSync(opt('--config'), 'utf8')) : {};
const targets = args.filter((a, i) => a.includes('=') && !args[i - 1]?.startsWith('--')).map(a => { const [name, ...u] = a.split('='); return { name, url: u.join('=') }; });
const CHROME = process.env.CHROME ?? '/Users/admin/.cache/chrome-for-testing/chrome/mac_arm-154.0.8037.57/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing';
const EXACT = {
  content: '[data-testid^="preview-"]',
  tag: '[data-testid="tags"] a[href^="/tag/"], a[data-testid^="tag-"]',
  tagDone: `location.pathname.startsWith('/tag/') && !!document.querySelector('[data-testid^="preview-"]') && !document.querySelector('[data-testid="feed-loading"]')`,
  // Signed out, `/`'s presses are links; the sign-in form's submit needs the
  // runtime (a mutation): pressed at `load` with the form empty, its effect
  // is the API's errors shown.
  pressPath: '/login',
  press: '[data-testid="submit"]',
  pressDone: `!!document.querySelector('[data-testid="errors"]')`,
  runtime: `!!document.getElementById('exact-root')?.dataset.bootMs`,
};
const sleep = ms => new Promise(r => setTimeout(r, ms));

async function browser() {
  const profileDir = mkdtempSync(join(tmpdir(), 'e3-bench-'));
  const child = spawn(CHROME, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profileDir}`, '--no-first-run', '--no-default-browser-check', '--disable-extensions',
    '--disable-background-networking', '--disable-component-update', '--use-mock-keychain', '--password-store=basic', '--window-size=412,915', 'about:blank'], { stdio: 'ignore', detached: true });
  let port;
  for (let i = 0; i < 400 && !port; i++) { await sleep(25); const f = join(profileDir, 'DevToolsActivePort'); if (existsSync(f)) port = readFileSync(f, 'utf8').split('\n')[0]; }
  let list = [];
  for (let i = 0; i < 80 && !list.some(t => t.type === 'page'); i++) { await sleep(50); list = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); }
  const ws = new WebSocket(list.find(t => t.type === 'page').webSocketDebuggerUrl);
  await new Promise(r => ws.onopen = r);
  let id = 0; const wait = new Map(), events = [];
  ws.onmessage = m => { const d = JSON.parse(m.data); if (d.id && wait.has(d.id)) { wait.get(d.id)(d); wait.delete(d.id); } else if (d.method) events.push(d); };
  const send = (method, params = {}) => new Promise(r => { const i = ++id; wait.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
  const close = async () => { ws.close(); try { process.kill(-child.pid, 'SIGKILL'); } catch {} await sleep(150); rmSync(profileDir, { recursive: true, force: true }); };
  return { send, events, close };
}

/** One cold run of `url`, pressing `click` at `load` (or nothing), until `done`. */
async function run(t, sel, click, done, path = '') {
  const b = await browser();
  const { send, events } = b;
  await send('Network.enable'); await send('Page.enable'); await send('Runtime.enable');
  // The profile is fresh (a cold cache); disabling the cache would send
  // `Cache-Control: no-cache`, which makes a render server render again.
  await send('Emulation.setDeviceMetricsOverride', { width: 412, height: 915, deviceScaleFactor: 1, mobile: true });
  if (profile === 'mobile') {
    await send('Emulation.setCPUThrottlingRate', { rate: 4 });
    await send('Network.emulateNetworkConditions', { offline: false, latency: 150, downloadThroughput: 1.6e6 / 8, uploadThroughput: 750e3 / 8 });
  }
  // On the first document only: content painted (the frame after the first
  // preview exists), and the press at `load`.
  await send('Page.addScriptToEvaluateOnNewDocument', { source: `(() => {
    if (sessionStorage.getItem('e3-bench')) return; sessionStorage.setItem('e3-bench', '1');
    const content = ${JSON.stringify(sel.content)}, click = ${JSON.stringify(click ?? null)};
    const seen = () => { if (window.__e3content == null && document.querySelector(content)) { window.__e3content = -1; requestAnimationFrame(() => { window.__e3content = performance.now(); }); } };
    new MutationObserver(seen).observe(document, { childList: true, subtree: true });
    if (click) addEventListener('load', () => { window.__e3loaded = performance.now(); document.querySelector(click)?.click(); }, { once: true });
  })();` });
  const origin = new URL(t.url).origin;
  await send('Page.navigate', { url: new URL(path || '/', t.url).href });
  let t0 = null, first = null, doneAt = null, runtimeAt = null;
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    await sleep(20);
    const r = (await send('Runtime.evaluate', { expression: `JSON.stringify({ origin: performance.timeOrigin, done: !!(${done}), runtime: !!(${sel.runtime}), content: window.__e3content ?? null, fcp: performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? null, loaded: window.__e3loaded ?? null })`, returnByValue: true })).result?.result?.value;
    if (!r) continue;
    const v = JSON.parse(r), now = Date.now();
    if (t0 == null) { t0 = v.origin; first = v; }
    if (v.origin === t0) first = v;
    if (v.runtime && runtimeAt == null) runtimeAt = now - t0;
    if (v.done && doneAt == null) doneAt = now - t0;
    if (doneAt != null && (runtimeAt != null || !click) && (click || (first.content > 0 && first.fcp != null))) break;
  }
  // Bytes from the page's origin (the app), finished before `done`, and in all.
  const reqs = new Map();
  for (const e of events) {
    if (e.method === 'Network.requestWillBeSent') reqs.set(e.params.requestId, { url: e.params.request.url, wall: e.params.wallTime * 1000, ts: e.params.timestamp });
    if (e.method === 'Network.loadingFinished' && reqs.has(e.params.requestId)) Object.assign(reqs.get(e.params.requestId), { end: e.params.timestamp, bytes: e.params.encodedDataLength });
  }
  let before = 0, all = 0; const files = [];
  for (const q of reqs.values()) {
    if (q.bytes == null || !q.url.startsWith(origin) || /\.(mp4|png|jpe?g|svg|ico)(\?|$)/.test(q.url)) continue;
    all += q.bytes;
    const endMs = q.wall + (q.end - q.ts) * 1000 - t0;
    if (doneAt == null || endMs <= doneAt) { before += q.bytes; files.push(`${q.url.slice(origin.length)} ${q.bytes}`); }
  }
  await b.close();
  // Content can't be painted before the first paint: the later of the two.
  const content = first?.content > 0 ? Math.max(first.content, first.fcp ?? 0) : null;
  return { done: doneAt, runtime: runtimeAt, content, fcp: first?.fcp ?? null, before, all, files };
}

const median = xs => { const v = xs.filter(x => x != null).sort((a, b) => a - b); return v.length ? Math.round(v[Math.floor(v.length / 2)]) : null; };
const results = {};
for (const t of targets) {
  const sel = { ...EXACT, ...(config[t.name] ?? {}) };
  // A warm origin: a cached page renders once, and dist files are compressed once.
  // The tag pressed is the first tag link `/` shows; its page is warmed too.
  const home = await fetch(t.url).then(r => r.text()).catch(() => '');
  const tag = /href="(\/tag\/[^"]+)"/.exec(home)?.[1];
  for (const p of ['/', sel.pressPath ?? '/', ...(tag ? [tag] : [])]) for (let k = 0; k < 2; k++) await fetch(new URL(p, t.url), { headers: { 'accept-encoding': 'br' } }).then(r => r.text()).catch(() => {});
  const out = { content: [], tag: [], press: [] };
  for (let i = 0; i < runs; i++) {
    out.content.push(await run(t, sel, null, `!!document.querySelector(${JSON.stringify(sel.content)})`));
    out.tag.push(await run(t, sel, sel.tag, sel.tagDone));
    out.press.push(await run(t, sel, sel.press, sel.pressDone, sel.pressPath));
  }
  results[t.name] = {
    fcp: median(out.content.map(r => r.fcp)), content: median(out.content.map(r => r.content)),
    tag: median(out.tag.map(r => r.done)), runtime: median(out.press.map(r => r.runtime)), press: median(out.press.map(r => r.done)),
    bytesContent: median(out.content.map(r => r.before)), bytesPress: median(out.press.map(r => r.before)), bytesAll: median(out.press.map(r => r.all)),
    files: out.press[0].files, raw: out,
  };
  process.stderr.write(`${t.name}: ${JSON.stringify({ ...results[t.name], raw: undefined, files: undefined })}\n`);
}
const rows = [['', ...targets.map(t => t.name)],
  ['FCP, `/` (ms)', ...targets.map(t => results[t.name].fcp)],
  ['Content painted, `/` (ms)', ...targets.map(t => results[t.name].content)],
  ['Tag tapped at `load` → feed (ms)', ...targets.map(t => results[t.name].tag)],
  ['Runtime up, after a press at `load` (ms)', ...targets.map(t => results[t.name].runtime)],
  ['A press that needs the runtime, at `load` → effect (ms)', ...targets.map(t => results[t.name].press)],
  ['Origin bytes before content (B)', ...targets.map(t => results[t.name].bytesContent)],
  ['Origin bytes before the press answered (B)', ...targets.map(t => results[t.name].bytesPress)],
  ['Origin bytes in all (B)', ...targets.map(t => results[t.name].bytesAll)]];
console.log(`profile: ${profile}, ${runs} cold runs each, medians\n`);
console.log(rows.map((r, i) => `| ${r.join(' | ')} |${i === 0 ? '\n|' + r.map(() => '---').join('|') + '|' : ''}`).join('\n'));
if (opt('--json')) writeFileSync(opt('--json'), JSON.stringify(results, null, 1));
