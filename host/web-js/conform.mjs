// The JS target's conformance harness: the same plan through the Rust web runner
// (app.wasm + glue.js) and the JavaScript runner (exact-web-js + rt.js), in the
// same Chrome, driven by the same agent operations (scripts/agent.mjs's
// `open`). After every step it compares the runner's typed state, the tree
// (preorder: depth, type, testId, text, value, label, handlers), layout boxes
// by testId and a screenshot. `app.test.contract` files run on both.
// Every failure is reported in one run; the exit code is 0 unless `--strict`,
// which exits 1 on any failure and prints each as a `FAIL <target> <step>:`
// line (the async lane's check, scripts/async.mjs).
//
// usage: bun host/web-js/conform.mjs [app …] [--synthetic] [--build] [--strict] [--wasm-root /tmp/e3-wasm] [--out /tmp/exact-web-js-conform] [--steps 10]
//   (the JS builds go to <out>/dist/<target>)
//   apps default to every app with a built wasm dist under --wasm-root
//   (`EXACT_WEB_DIST=<root>/<app> bun host/web/build.mjs <app> --wasm`);
//   --build makes each named app's wasm dist there first (and Caltrain's,
//   for --synthetic);
//   --synthetic adds host/web-js/conformance/*.contract (and */app.contract), run on
//   Caltrain's wasm dist with the plan swapped in (agent `--plan`), whose
//   data sources they may ask; the JS side loads the same Rust module. A
//   plan whose first lines say `// data: <app>` runs on that app's dist
//   instead, for its sources and the capabilities it links; one that says
//   `// agent: timeZone=<zone> epoch=<ms>` is driven with those facts.
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { open } from '../../scripts/agent.mjs';
import { decodePng, encodePng } from '../../scripts/png.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const argv = process.argv.slice(2);
const opt = (n, d) => { const i = argv.indexOf(n); return i < 0 ? d : argv[i + 1]; };
const wasmRoot = resolve(opt('--wasm-root', '/tmp/e3-wasm'));
const out = resolve(opt('--out', '/tmp/exact-web-js-conform'));
const maxSteps = Number(opt('--steps', 10));
const named = argv.includes('--urls') ? [] : argv.filter((a, i) => !a.startsWith('--') && !['--wasm-root', '--out', '--steps', '--label'].includes(argv[i - 1]));
mkdirSync(out, { recursive: true });
mkdirSync(wasmRoot, { recursive: true }); // --build renames each app's dist into it
process.env.CHROME ??= '/Users/admin/.cache/chrome-for-testing/chrome/mac_arm-154.0.8037.57/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing';

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.png': 'image/png', '.mp4': 'video/mp4', '.css': 'text/css', '.svg': 'image/svg+xml' };
function serve(dir) {
  const server = createServer((req, res) => {
    const p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    let f = resolve(dir, '.' + p);
    try { if (statSync(f).isDirectory()) f = resolve(f, 'index.html'); } catch { f = resolve(dir, 'index.html'); }
    let body; try { body = readFileSync(f); } catch { res.writeHead(404); return res.end(); }
    res.writeHead(200, { 'content-type': TYPES[extname(f)] ?? 'application/octet-stream', 'cache-control': 'no-store' }); res.end(body);
  });
  return new Promise(ok => server.listen(0, '127.0.0.1', () => ok({ url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() })));
}

// ---------------------------------------------------------------- comparisons
const norm = t => t.nodes.map(n => [n.depth ?? 0, n.type, n.props?.testId ?? '', n.props?.text ?? '', n.props?.value ?? '', n.props?.accessibilityLabel ?? '', (n.handlers ?? []).join(' ')].join('|'));
function diffLists(a, b, what) {
  const out = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) if (a[i] !== b[i]) { out.push(`${what} #${i}: wasm «${a[i] ?? '—'}» js «${b[i] ?? '—'}»`); if (out.length >= 4) { out.push(`${what}: … (${a.length} vs ${b.length} entries)`); break; } }
  return out;
}
function diffJSON(a, b, path, out) {
  if (out.length >= 8) return;
  if (JSON.stringify(a) === JSON.stringify(b)) return;
  if (a && b && typeof a === 'object' && typeof b === 'object' && Array.isArray(a) === Array.isArray(b)) {
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) diffJSON(a[k], b[k], `${path}.${k}`, out);
  } else out.push(`${path}: wasm ${JSON.stringify(a)?.slice(0, 120)} js ${JSON.stringify(b)?.slice(0, 120)}`);
}
function boxes(l) { const m = new Map(); for (const n of l.nodes) if (n.testId && !m.has(n.testId)) m.set(n.testId, n); return m; }
function diffLayout(a, b) {
  const A = boxes(a), B = boxes(b), out = [];
  for (const [t, x] of A) {
    const y = B.get(t);
    if (!y) { out.push(`layout ${t}: on screen in wasm, not in js`); continue; }
    const d = Math.max(...['x', 'y', 'w', 'h'].map(k => Math.abs(x[k] - y[k])));
    if (d > 0.5) out.push(`layout ${t}: wasm ${[x.x, x.y, x.w, x.h].map(Math.round)} js ${[y.x, y.y, y.w, y.h].map(Math.round)}`);
  }
  for (const t of B.keys()) if (!A.has(t)) out.push(`layout ${t}: on screen in js, not in wasm`);
  return out.slice(0, 8);
}
function diffPng(a, b, sideBySide, masks = []) {
  const A = decodePng(readFileSync(a)), B = decodePng(readFileSync(b));
  let n = 0;
  const masked = i => { const x = (i / 4) % A.width, y = Math.floor(i / 4 / A.width); return masks.some(m => x >= m.x && x < m.x + m.w && y >= m.y && y < m.y + m.h); };
  for (let i = 0; i < Math.min(A.data.length, B.data.length); i += 4) if (!masked(i) && Math.abs(A.data[i] - B.data[i]) + Math.abs(A.data[i + 1] - B.data[i + 1]) + Math.abs(A.data[i + 2] - B.data[i + 2]) > 24) n++;
  const share = n / (A.width * A.height);
  if (share > 0.002) {
    const W = A.width + B.width + 10, H = Math.max(A.height, B.height), data = new Uint8Array(W * H * 4).fill(255);
    const put = (img, ox) => { for (let y = 0; y < img.height; y++) data.set(img.data.subarray(y * img.width * 4, (y + 1) * img.width * 4), (y * W + ox) * 4); };
    put(A, 0); put(B, A.width + 10);
    writeFileSync(sideBySide, encodePng({ width: W, height: H, data }));
  }
  return share;
}
const STATE_KEYS = ['slots', 'derives', 'resources'];

// ---------------------------------------------------------------- one target
async function target(t, report) {
  const fail = (step, what) => report.failures.push({ target: t.name, step, what });
  const dir = resolve(out, t.name); mkdirSync(dir, { recursive: true });
  if (t.urls) {
    await drive(t, report, fail, dir, { url: t.urls[0], close() {} }, { url: t.urls[1], close() {} });
    for (const url of new Set(t.urls)) await activation(t, report, fail, url);
    return;
  }
  const build = spawnSync('bun', ['host/web-js/build.mjs', t.app, ...(t.contract ? ['--plan', t.plan, '--data', t.wasm] : ['--plan', resolve(t.wasm, 'app.plan')]), '--out', resolve(out, 'dist', t.name)], { cwd: root, encoding: 'utf8' });
  report.targets[t.name] = { jsBuild: build.status === 0, warnings: (build.stderr.match(/^warning: .*/gm) ?? []).length };
  if (build.status !== 0) return fail('js-build', (build.stderr.split('\n').find(l => /\.plan: |\.contract:|^error/.test(l)) ?? build.stderr.slice(-300)).trim().slice(0, 400));
  const [ws, js] = await Promise.all([serve(t.wasm), serve(resolve(out, 'dist', t.name))]);
  await drive(t, report, fail, dir, ws, js);
}

async function drive(t, report, fail, dir, ws, js) {
  let W, J;
  try {
    // A plan's `// agent: timeZone=… epoch=…` line: the drive's facts, on both.
    const facts = Object.fromEntries([...(t.contract ? /^\/\/ agent: (.*)$/m.exec(readFileSync(t.contract, 'utf8'))?.[1] ?? '' : '').matchAll(/(\w+)=(\S+)/g)].map(([, k, v]) => [k, k === 'epoch' ? Number(v) : v]));
    try { W = await open({ host: 'web', app: t.app, ...facts, ...(t.contract ? { webDist: t.wasm, plan: t.plan } : { url: ws.url }) }); }
    catch (e) { return fail('wasm-open', e.message.split('\n')[0]); }
    try { J = await open({ host: 'web', app: t.app, ...facts, url: js.url }); }
    catch (e) { return fail('js-open', e.message.split('\n')[0]); }
    const compare = async step => {
      let st = 0;
      const [sw, sj] = await Promise.all([W.state(), J.state().catch(e => ({ error: e.message }))]);
      if (sj.error) { fail(step, `state: js ${sj.error}`); st++; }
      else { const o = []; for (const k of STATE_KEYS) diffJSON(sw[k], sj[k], k, o); o.forEach(x => fail(step, 'state ' + x)); st += o.length; }
      const [tw, tj] = await Promise.all([W.tree(), J.tree()]);
      const o2 = diffLists(norm(tw), norm(tj), 'tree'); o2.forEach(x => fail(step, x)); st += o2.length;
      const [lw, lj] = await Promise.all([W.layout(), J.layout()]);
      const o3 = diffLayout(lw, lj); o3.forEach(x => fail(step, x)); st += o3.length;
      const slug = step.replace(/[^a-z0-9]+/gi, '-');
      const [pw, pj] = [resolve(dir, `${slug}-wasm.png`), resolve(dir, `${slug}-js.png`)];
      await Promise.all([W.screenshot(pw), J.screenshot(pj)]);
      // A playing video's frames and controls are the browser's clock, not the runner's.
      const share = diffPng(pw, pj, resolve(dir, `${slug}-side-by-side.png`), lw.nodes.filter(n => n.type === 'Video'));
      if (share > 0.002) { fail(step, `screenshot: ${(share * 100).toFixed(2)}% of pixels differ (${slug}-side-by-side.png)`); st++; }
      report.steps.push({ target: t.name, step, differences: st });
      return tw;
    };
    // A scripted scenario (`conformance/<app>.steps`): one agent operation
    // a line — `tap <target>`, `type <target> <text…>`, `clock <+ms|settle>`,
    // `back` (the browser's history), `wheel <target> <dy> [dx]`, `into
    // <list> <key> [block]` (a virtualized list's row by key), `drag
    // <target> <dx> <dy> [ms]` (a finger: down, a move over ms of real time,
    // up; a pan or a swipe), `pinch <target> <scale>` (two fingers), `down
    // <target>` and `up` (a held contact: press feedback) — each compared
    // after both settle.
    const script = resolve(here, 'conformance', `${t.urls ? t.app : t.name.replace(/^synthetic-/, '')}.steps`);
    const settle = () => Promise.all([W.clock('settle'), J.clock('settle')]);
    await settle();
    let tree = await compare('boot');
    if (existsSync(script)) for (const line of readFileSync(script, 'utf8').split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'))) {
      const [op, target, ...rest] = line.split(/\s+/);
      const run = s => op === 'tap' ? s.tap(target) : op === 'type' ? s.type(target, rest.join(' ')) : op === 'clock' ? s.clock(target) : op === 'back' ? s.tap(target, { history: -1 }) : op === 'wheel' ? s.tap(target, { wheel: [Number(rest[1] ?? 0), Number(rest[0])] }) : op === 'into' ? s.tap(target, { into: { key: rest[0], ...(rest[1] ? { block: rest[1] } : {}) } }) : op === 'pinch' ? s.tap(target, { pinch: Number(rest[0]) }) : op === 'down' ? s.tap(target, { down: true }) : op === 'up' ? s.pointer('up') : op === 'drag' ? s.tap(target, { down: true }).then(() => s.pointer('move', { dx: Number(rest[0]), dy: Number(rest[1]), ms: Number(rest[2] ?? 200) })).then(() => s.pointer('up')) : Promise.reject(new Error(`unknown op ${op}`));
      try { await run(W); } catch (e) { report.steps.push({ target: t.name, step: line, skipped: `wasm: ${e.message.split('\n')[0]}` }); continue; }
      try { await run(J); } catch (e) { fail(line, `js: ${e.message.split('\n')[0]}`); continue; }
      await settle();
      tree = await compare(line);
    }
    const tapped = new Set();
    for (let i = 0; i < maxSteps; i++) {
      const next = tree.nodes.find(n => (n.handlers ?? []).includes('press') && n.props?.testId && !tapped.has(n.props.testId));
      if (!next) break;
      const id = next.props.testId; tapped.add(id);
      let ok = true;
      try { await W.tap(id); } catch (e) { ok = false; report.steps.push({ target: t.name, step: `tap ${id}`, skipped: `wasm: ${e.message.split('\n')[0]}` }); }
      if (!ok) continue;
      try { await J.tap(id); } catch (e) { fail(`tap ${id}`, `js: ${e.message.split('\n')[0]}`); continue; }
      // What the press sent lands on both first (a fetch races the compare otherwise).
      await settle();
      tree = await compare(`tap ${id}`);
    }
    await Promise.all([W.clock('+60000'), J.clock('+60000')]);
    await compare('clock +60000');
  } catch (e) {
    fail('drive', e.stack?.split('\n').slice(0, 2).join(' ') ?? String(e));
  } finally {
    await W?.close?.(); await J?.close?.(); ws.close(); js.close();
  }
  // The app's own tests, on both.
  const tests = resolve(root, 'apps', t.app, 'app.test.contract');
  if (!t.contract && !t.urls && existsSync(tests)) {
    const run = url => { const r = spawnSync('bun', ['scripts/agent.mjs', 'web', '--app', t.app, '--url', url, '--test', tests], { cwd: root, encoding: 'utf8' }); return r.stdout + r.stderr; };
    const [w2, j2] = await Promise.all([serve(t.wasm), serve(resolve(out, 'dist', t.name))]);
    const [rw, rj] = [run(w2.url), run(j2.url)];
    w2.close(); j2.close();
    const lines = s => s.split('\n').filter(l => l.startsWith('test '));
    const [lw, lj] = [lines(rw), lines(rj)];
    report.targets[t.name].tests = { wasm: rw.trim().split('\n').at(-1), js: rj.trim().split('\n').at(-1) };
    diffLists(lw, lj, 'app.test.contract').forEach(x => fail('tests', x));
  }
}

// ---------------------------------------------------------------- activation (LLP 1071 D6)
// A served page as a reader opens it (no `?agent`, no input): first paint
// runs no module script; `eager` (undeclared) preloads the entry from the
// head and adopts after the first paint, `idle` after `load`; an
// `interaction` page fetches no script until a press, which it replays.
async function activation(t, report, fail, url) {
  const step = `activation ${url}`;
  let S;
  try {
    S = await open({ host: 'web', app: t.app, url });
    const ev = S.carrier.evaluate, page = new URL(url);
    page.searchParams.delete('agent');
    await ev(`location.href = ${JSON.stringify(page.href)}`).catch(() => {});
    const read = () => ev(`(() => { const c = document.querySelector('script[type="application/vnd.exact.checkpoint"]'), r = document.getElementById('exact-root');
      return c && document.readyState === 'complete' && !/[?&]agent=/.test(location.search) ? { policy: c.dataset.activate, boot: r?.dataset.bootMs == null ? null : Number(r.dataset.bootMs),
        paint: performance.getEntriesByType('paint')[0]?.startTime ?? null, load: performance.getEntriesByType('navigation')[0]?.loadEventStart ?? null,
        modules: document.querySelectorAll('script[type=module]').length, preload: !!document.head.querySelector('link[rel=modulepreload][href="./app.js"]'),
        fetched: performance.getEntriesByType('resource').filter(e => e.initiatorType !== 'fetch' && e.name.endsWith('.js')).length,
        adopted: (globalThis.exact?.journal ?? []).some(l => l.endsWith('adopted the document')) } : null; })()`).catch(() => null);
    const until = async (ok, ms) => { const end = Date.now() + ms; let r; while (!(r = await read()) || !ok(r)) { if (Date.now() > end) return r; await new Promise(z => setTimeout(z, 50)); } return r; };
    let r = await until(() => true, 10000);
    if (!r) return fail(step, 'the page never loaded');
    const out = [];
    if (r.modules) out.push(`${r.modules} module script(s) in the page: first paint must run none`);
    if (r.policy === 'interaction') {
      await new Promise(z => setTimeout(z, 1000));
      r = await read();
      if (r.boot != null || r.fetched) out.push(`interaction: runtime up (${r.boot}) or ${r.fetched} script(s) fetched before any input`);
      if (r.preload) out.push('interaction: the head preloads the entry');
      await ev(`document.querySelector('[data-exact-on~=press]:not(a)')?.click()`);
    } else if (!r.preload) out.push(`${r.policy}: the head does not preload ./app.js`);
    r = await until(x => x.boot != null, 10000);
    if (r.boot == null) out.push(`${r.policy}: the runtime never came up${r.policy === 'interaction' ? ' after a press' : ' without input'}`);
    else {
      if (!r.adopted) out.push(`${r.policy}: the document was built afresh, not adopted`);
      if (r.paint == null || r.boot < r.paint) out.push(`${r.policy}: runtime up at ${r.boot} ms, before first paint (${r.paint})`);
      if (r.policy === 'idle' && r.boot < r.load) out.push(`idle: runtime up at ${r.boot} ms, before load (${r.load})`);
    }
    out.forEach(x => fail(step, x));
    report.steps.push({ target: t.name, step, policy: r.policy, paint: r.paint, boot: r.boot, differences: out.length });
  } catch (e) {
    fail(step, e.message.split('\n')[0]);
  } finally {
    await S?.close?.();
  }
}

// ---------------------------------------------------------------- the run
const report = { at: new Date().toISOString(), targets: {}, steps: [], failures: [] };
// `--urls <app> <a> <b>`: two served pages of one app, compared the same way
// (a fresh JavaScript render against an adopted one, one renderer against another).
const urls = argv.indexOf('--urls');
const apps = urls >= 0 ? [] : named.length ? named : readdirSync(wasmRoot).filter(a => existsSync(resolve(wasmRoot, a, 'app.plan')));
const sdir = resolve(here, 'conformance');
// A plan with its own files (`strings/`) is a directory holding `app.contract`.
const synthetic = argv.includes('--synthetic') ? readdirSync(sdir).flatMap(f => f.endsWith('.contract') ? [f] : existsSync(resolve(sdir, f, 'app.contract')) ? [`${f}/app.contract`] : []).map(f => ({ f, data: /^\/\/ data: (\S+)/m.exec(readFileSync(resolve(sdir, f), 'utf8'))?.[1] ?? 'caltrain' })) : [];
if (argv.includes('--build')) mkdirSync(wasmRoot, { recursive: true });
if (argv.includes('--build')) for (const a of new Set([...apps, ...synthetic.map(s => s.data)])) {
  const b = spawnSync('bun', ['host/web/build.mjs', `${a}-web`, '--wasm'], { cwd: root, encoding: 'utf8', maxBuffer: 64 << 20, env: { ...process.env, EXACT_WEB_DIST: resolve(wasmRoot, a) } });
  if (b.status !== 0) report.failures.push({ target: a, step: 'wasm-build', what: b.stderr.trim().split('\n').slice(-3).join(' ').slice(0, 300) });
}
const targets = apps.map(a => ({ name: a, app: a, wasm: resolve(wasmRoot, a) }));
if (urls >= 0) targets.push({ name: `${argv[urls + 1]}-${opt('--label', 'urls')}`, app: argv[urls + 1], urls: [argv[urls + 2], argv[urls + 3]] });
if (argv.includes('--synthetic')) {
  for (const { f, data } of synthetic) {
    const name = 'synthetic-' + (f.endsWith('/app.contract') ? dirname(f) : basename(f, '.contract')), contract = resolve(sdir, f), plan = resolve(out, name + '.plan');
    const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
    if (c.status !== 0) { report.failures.push({ target: name, step: 'contract-build', what: c.stderr.trim().slice(0, 300) }); continue; }
    // Synthetic plans ask their data app's sources (Caltrain's stations, nearest, search): its wasm links them.
    targets.push({ name, app: data, wasm: realpathSync(resolve(wasmRoot, data)), contract, plan });
  }
}
for (const t of targets) {
  const before = report.failures.length;
  process.stderr.write(`${t.name}: `);
  try { await target(t, report); } catch (e) { report.failures.push({ target: t.name, step: 'harness', what: String(e.stack ?? e).slice(0, 300) }); }
  process.stderr.write(`${report.failures.length - before} failures\n`);
}
writeFileSync(resolve(out, 'report.json'), JSON.stringify(report, null, 1));
const byTarget = {};
for (const f of report.failures) (byTarget[f.target] ??= []).push(f);
const lines = [`# JS target conformance — ${report.at}`, '', '| target | JS build | steps compared | steps equal | failures | app tests (wasm / js) |', '|---|---|---|---|---|---|'];
for (const t of targets) {
  const s = report.steps.filter(x => x.target === t.name && x.differences != null), info = report.targets[t.name] ?? {};
  lines.push(`| ${t.name} | ${info.jsBuild === false ? 'refused' : info.jsBuild ? 'ok' : '—'} | ${s.length} | ${s.filter(x => x.differences === 0).length} | ${(byTarget[t.name] ?? []).length} | ${info.tests ? `${info.tests.wasm} / ${info.tests.js}` : '—'} |`);
}
lines.push('', '## Failures', '');
for (const [t, fs] of Object.entries(byTarget)) { lines.push(`### ${t}`); for (const f of fs) lines.push(`- **${f.step}** — ${f.what}`); lines.push(''); }
writeFileSync(resolve(out, 'report.md'), lines.join('\n'));
console.log(lines.slice(0, targets.length + 4).join('\n'));
console.log(`\n${report.failures.length} failures across ${targets.length} targets; ${resolve(out, 'report.md')}`);
if (argv.includes('--strict') && report.failures.length) {
  for (const f of report.failures) console.log(`FAIL ${f.target} ${f.step}: ${f.what.split('\n')[0].slice(0, 200)}`);
  process.exit(1);
}
