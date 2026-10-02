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
// usage: bun host/web-js/conform.mjs [app …] [--synthetic] [--build] [--strict] [--linux] [--wasm-root /tmp/e3-wasm] [--out /tmp/exact-web-js-conform] [--steps 10]
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
//   --linux adds a second reference beside the wasm page: the Rust runner
//   headless on the Linux host (`agent.mjs linux`, the data app's release
//   binary, built by --build), driven by the same steps on the same plan,
//   its state and tree compared with the wasm page's (`linux` failures).
//   Layout and pixels are the Linux host's own and are not compared. The
//   only normalization is the route stack's browser location (`linuxView`);
//   a target whose app has no Linux host, or a plan that says `// linux:
//   <why>`, is reported as not compared (`// linux: state only (<why>)`
//   compares its state and not its tree), and the comparison stops at the
//   first step the Linux host has no delivery for (a pointer gesture, a
//   wheel, a list's `into`, the browser's history) or that `LINUX_APART` names.
import { spawn, spawnSync } from 'node:child_process';
import { createServer, request } from 'node:http';
import { existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { open } from '../../scripts/agent.mjs';
import { resolveApp } from '../../scripts/app.mjs';
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
function diffLists(a, b, what, other = 'js') {
  const out = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) if (a[i] !== b[i]) { out.push(`${what} #${i}: wasm «${a[i] ?? '—'}» ${other} «${b[i] ?? '—'}»`); if (out.length >= 4) { out.push(`${what}: … (${a.length} vs ${b.length} entries)`); break; } }
  return out;
}
function diffJSON(a, b, path, out, other = 'js') {
  if (out.length >= 8) return;
  if (JSON.stringify(a) === JSON.stringify(b)) return;
  if (a && b && typeof a === 'object' && typeof b === 'object' && Array.isArray(a) === Array.isArray(b)) {
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) diffJSON(a[k], b[k], `${path}.${k}`, out, other);
  } else out.push(`${path}: wasm ${JSON.stringify(a)?.slice(0, 120)} ${other} ${JSON.stringify(b)?.slice(0, 120)}`);
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

// The wasm page's route stack carries the browser's location; the Linux
// host has none. So, and only in a route stack (entries shaped { id, name,
// url, tab, params } and the stack's `next`): an entry's `url` loses the
// query parameters the agent's harness puts in the page's address (agent,
// seed, locale, timeZone, epoch); entry ids are renumbered in the order the
// state lists them (the web runner's boot adopts the page's history entry,
// allocating ids in another order and taking ids a Linux boot does not);
// and the stack's `next` id is dropped. Everything else is
// compared as is.
const HARNESS = ['agent', 'seed', 'locale', 'timeZone', 'epoch'];
const isEntry = v => v && typeof v === 'object' && !Array.isArray(v) && ['id', 'name', 'url', 'tab', 'params'].every(k => k in v);
function linuxView(state) {
  const rank = new Map();
  const walk = (v, f) => { if (v && typeof v === 'object') { f(v); for (const x of Object.values(v)) walk(x, f); } };
  walk(state, v => { if (isEntry(v) && typeof v.id === 'number' && !rank.has(v.id)) rank.set(v.id, rank.size); });
  const map = v => {
    if (!v || typeof v !== 'object') return v;
    if (Array.isArray(v)) return v.map(map);
    const o = Object.fromEntries(Object.entries(v).map(([k, x]) => [k, map(x)]));
    if (isEntry(v)) {
      if (rank.has(v.id)) o.id = rank.get(v.id);
      if (typeof v.url === 'string') { const u = new URL(v.url, 'http://x'); for (const k of HARNESS) u.searchParams.delete(k); o.url = u.pathname + u.search; }
    }
    if (Array.isArray(v.tabs) && 'next' in v) delete o.next;
    return o;
  };
  return map(state);
}

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
  // A route that paints its boot document first is checked as its server
  // serves it: a press and an edit on the boot document (below). Its data
  // app's module is not swapped under a wasm page (a paired generation).
  if (t.contract && /\bpaint=boot\b/.test(readFileSync(t.contract, 'utf8'))) return bootPress(t, report, fail, resolve(out, 'dist', t.name));
  const [ws, js] = await Promise.all([serve(t.wasm), serve(resolve(out, 'dist', t.name))]);
  await drive(t, report, fail, dir, ws, js);
  // A page rendered at build that loads its script at the first input: the
  // reader's first press or edits, before the runtime (below).
  if (/data-activate="interaction"/.test(readFileSync(resolve(out, 'dist', t.name, 'index.html'), 'utf8'))) {
    const page = await serve(resolve(out, 'dist', t.name));
    await activation(t, report, fail, page.url);
    page.close();
  }
}

async function drive(t, report, fail, dir, ws, js) {
  let W, J, L;
  try {
    // A plan's `// agent: timeZone=… epoch=…` line: the drive's facts, on both.
    const facts = Object.fromEntries([...(t.contract ? /^\/\/ agent: (.*)$/m.exec(readFileSync(t.contract, 'utf8'))?.[1] ?? '' : '').matchAll(/(\w+)=(\S+)/g)].map(([, k, v]) => [k, k === 'epoch' ? Number(v) : v]));
    try { W = await open({ host: 'web', app: t.app, ...facts, ...(t.contract ? { webDist: t.wasm, plan: t.plan } : { url: ws.url }) }); }
    catch (e) { return fail('wasm-open', e.message.split('\n')[0]); }
    try { J = await open({ host: 'web', app: t.app, ...facts, url: js.url }); }
    catch (e) { return fail('js-open', e.message.split('\n')[0]); }
    const linux = t.urls ? null : linuxFor(t);
    if (linux?.why) report.steps.push({ target: t.name, step: 'linux', skipped: linux.why });
    else if (linux) {
      // An app runs its own binary's plan (the web dist's names the web's Rust module); a synthetic plan is swapped in.
      try { L = await open({ host: 'linux', app: t.app, ...facts, ...(t.contract ? { plan: t.plan } : {}) }); }
      catch (e) { fail('linux-open', e.message.split('\n').filter(l => !/^crash report/.test(l)).slice(0, 4).join(' ').slice(0, 400)); }
    }
    // The first step the Linux host cannot take ends its comparison (its state has left the wasm page's).
    const onLinux = async (step, fn) => {
      if (!L) return;
      const apart = LINUX_APART[t.name];
      if (apart && step === apart[0]) { report.steps.push({ target: t.name, step, linux: 'stopped', skipped: `linux: ${apart[1]}` }); await L.close?.().catch(() => {}); L = null; return; }
      try { await fn(L); } catch (e) { report.steps.push({ target: t.name, step, linux: 'stopped', skipped: `linux: ${e.message.split('\n')[0]}` }); await L.close?.().catch(() => {}); L = null; }
    };
    const compare = async step => {
      let st = 0;
      const [sw, sj] = await Promise.all([W.state(), J.state().catch(e => ({ error: e.message }))]);
      if (sj.error) { fail(step, `state: js ${sj.error}`); st++; }
      else { const o = []; for (const k of STATE_KEYS) diffJSON(sw[k], sj[k], k, o); o.forEach(x => fail(step, 'state ' + x)); st += o.length; }
      const [tw, tj] = await Promise.all([W.tree(), J.tree()]);
      const o2 = diffLists(norm(tw), norm(tj), 'tree'); o2.forEach(x => fail(step, x)); st += o2.length;
      await onLinux(step, async L => {
        const [sl, tl] = await Promise.all([L.state(), L.tree()]), o = [], vw = linuxView(sw), vl = linuxView(sl);
        for (const k of STATE_KEYS) diffJSON(vw[k], vl[k], k, o, 'linux');
        if (!linux.stateOnly) o.push(...diffLists(norm(tw), norm(tl), 'tree', 'linux').slice(0, 4));
        o.forEach(x => fail(step, 'linux ' + x));
        report.steps.push({ target: t.name, step, reference: 'linux', differences: o.length });
      });
      const [lw, lj] = await Promise.all([W.layout(), J.layout()]);
      const o3 = diffLayout(lw, lj); o3.forEach(x => fail(step, x)); st += o3.length;
      // Paint facts are part of parity even when boxes happen not to overlap.
      const paint = `Array.from(document.querySelectorAll('#exact-root > *, #exact-root [data-testid]'), e => { const s = getComputedStyle(e); return [e.dataset.testid ?? '$root', s.isolation, s.position]; })`;
      const [fw, fj] = await Promise.all([W, J].map(s => s.carrier.evaluate(paint)));
      const op = []; diffJSON(fw, fj, 'paint', op); op.forEach(x => fail(step, x)); st += op.length;
      const slug = step.replace(/[^a-z0-9]+/gi, '-');
      const [pw, pj] = [resolve(dir, `${slug}-wasm.png`), resolve(dir, `${slug}-js.png`)];
      // Chrome picks how a scaled image is filtered per raster (a lower
      // quality while it judges the layer busy, a higher one later, on its
      // own clock); nearest-neighbour on both leaves one filter to compare.
      const still = `document.getElementById('conform-still') || document.head.insertAdjacentHTML('beforeend', '<style id=conform-still>img{image-rendering:pixelated!important}</style>'); new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))`;
      await Promise.all([W, J].map(s => s.carrier?.evaluate(still).catch(() => {})));
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
    const settle = () => Promise.all([W.clock('settle'), J.clock('settle'), onLinux('settle', L => L.clock('settle'))]);
    await settle();
    let tree = await compare('boot');
    if (existsSync(script)) for (const line of readFileSync(script, 'utf8').split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'))) {
      const [op, target, ...rest] = line.split(/\s+/);
      const run = s => op === 'tap' ? s.tap(target) : op === 'type' ? s.type(target, rest.join(' ')) : op === 'clock' ? s.clock(target) : op === 'back' ? s.tap(target, { history: -1 }) : op === 'wheel' ? s.tap(target, { wheel: [Number(rest[1] ?? 0), Number(rest[0])] }) : op === 'into' ? s.tap(target, { into: { key: rest[0], ...(rest[1] ? { block: rest[1] } : {}) } }) : op === 'pinch' ? s.tap(target, { pinch: Number(rest[0]) }) : op === 'down' ? s.tap(target, { down: true }) : op === 'up' ? s.pointer('up') : op === 'drag' ? s.tap(target, { down: true }).then(() => s.pointer('move', { dx: Number(rest[0]), dy: Number(rest[1]), ms: Number(rest[2] ?? 200) })).then(() => s.pointer('up')) : Promise.reject(new Error(`unknown op ${op}`));
      try { await run(W); } catch (e) { report.steps.push({ target: t.name, step: line, skipped: `wasm: ${e.message.split('\n')[0]}` }); continue; }
      try { await run(J); } catch (e) { fail(line, `js: ${e.message.split('\n')[0]}`); continue; }
      await onLinux(line, L => LINUX_OPS.includes(op) ? run(L) : Promise.reject(new Error(`\`${op}\` is the page's pointer or history delivery, not the runner's`)));
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
      await onLinux(`tap ${id}`, L => L.tap(id));
      // What the press sent lands on both first (a fetch races the compare otherwise).
      await settle();
      tree = await compare(`tap ${id}`);
    }
    await Promise.all([W.clock('+60000'), J.clock('+60000'), onLinux('clock +60000', L => L.clock('+60000'))]);
    await compare('clock +60000');
  } catch (e) {
    fail('drive', e.stack?.split('\n').slice(0, 2).join(' ') ?? String(e));
  } finally {
    await W?.close?.(); await J?.close?.(); await L?.close?.(); ws.close(); js.close();
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
  let S, early = null;
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
      // early.contract's controls, edited as a reader would (the script
      // loads at the first); else the first press.
      early = await ev(`(() => { const q = id => document.querySelector('[data-testid="' + id + '"]');
        if (!q('early-state')) return document.querySelector('[data-exact-on~=press]:not(a)')?.click(), null;
        q('early-a').click(); q('early-b').click();
        const w = q('early-words'), p = q('early-pick');
        w.value = 'early'; for (const k of ['input', 'change']) w.dispatchEvent(new Event(k, { bubbles: true }));
        p.value = 'two'; for (const k of ['input', 'change']) p.dispatchEvent(new Event(k, { bubbles: true }));
        return 'on on early two'; })()`);
    } else if (!r.preload) out.push(`${r.policy}: the head does not preload ./app.js`);
    r = await until(x => x.boot != null, 10000);
    if (r.boot == null) out.push(`${r.policy}: the runtime never came up${r.policy === 'interaction' ? ' after a press' : ' without input'}`);
    else {
      if (!r.adopted) out.push(`${r.policy}: the document was built afresh, not adopted`);
      if (r.paint == null || r.boot < r.paint) out.push(`${r.policy}: runtime up at ${r.boot} ms, before first paint (${r.paint})`);
      if (r.policy === 'idle' && r.boot < r.load) out.push(`idle: runtime up at ${r.boot} ms, before load (${r.load})`);
      if (early) {
        await new Promise(z => setTimeout(z, 300));
        const got = await ev(`(() => { const q = id => document.querySelector('[data-testid="' + id + '"]');
          return [q('early-state').textContent, q('early-a').checked, q('early-b').checked, q('early-words').value, q('early-pick').value].join('|'); })()`);
        const want = `${early}|true|true|early|two`;
        if (got !== want) out.push(`early edits: ${got}, not ${want} (state|boxes|text|select)`);
      }
    }
    out.forEach(x => fail(step, x));
    report.steps.push({ target: t.name, step, policy: r.policy, paint: r.paint, boot: r.boot, differences: out.length });
  } catch (e) {
    fail(step, e.message.split('\n')[0]);
  } finally {
    await S?.close?.();
  }
}

// ---------------------------------------------------------------- the boot document (LLP 1048.005)
// A route with `paint=boot`, served by its data app's render entry: the
// reader presses a button and types in a field on the boot document, before
// the settled page arrives (a pass-through holds everything after the boot
// document for HOLD ms); once the runtime has adopted the settled page,
// state holds both, the field its text, and the page one root.
const HOLD = 1500;
async function bootPress(t, report, fail, dist) {
  const step = 'boot press';
  const bin = `${t.app}-render`;
  const at = [['linux', `${t.app}-linux`], ['web', `${t.app}-web`]].find(([dir]) => existsSync(resolve(root, 'apps', t.app, dir, 'src/bin', `${bin}.rs`)));
  if (!at) return fail(step, `${t.app} has no ${bin} entry to serve the page`);
  const b = spawnSync('cargo', ['build', '--release', '-q', '-p', at[1], '--bin', bin], { cwd: root, encoding: 'utf8', maxBuffer: 64 << 20 });
  if (b.status !== 0) return fail(step, `${bin}: ${b.stderr.trim().split('\n').slice(-3).join(' ').slice(0, 300)}`);
  const server = spawn(resolve(process.env.CARGO_TARGET_DIR ?? resolve(root, 'target'), 'release', bin), ['--serve', dist, '--port', '0'], { cwd: root, stdio: ['ignore', 'pipe', 'inherit'] });
  let proxy, S;
  try {
    const inner = await new Promise((ok, no) => {
      let seen = '';
      server.stdout.on('data', d => { seen += d; const m = /serving http:\/\/127\.0\.0\.1:(\d+)\//.exec(seen); if (m) ok(Number(m[1])); });
      server.on('exit', code => no(new Error(`${bin} exited ${code}`)));
    });
    // Everything from the style that hides the boot document waits HOLD ms.
    const MARK = '<style>#exact-root[data-boot]';
    proxy = createServer((req, res) => {
      const up = request({ host: '127.0.0.1', port: inner, path: req.url, method: req.method, headers: { ...req.headers, 'accept-encoding': 'identity' } }, r => {
        const headers = { ...r.headers }; delete headers['content-length']; delete headers['transfer-encoding']; delete headers.connection;
        res.writeHead(r.statusCode, headers);
        let seen = Buffer.alloc(0), sent = 0, held = null;
        r.on('data', d => {
          if (held) return held.push(d);
          seen = Buffer.concat([seen, d]);
          const at = seen.indexOf(MARK);
          if (at < 0) { sent = seen.length; return res.write(d); }
          res.write(seen.subarray(sent, at));
          held = [seen.subarray(at)];
          setTimeout(() => { for (const c of held) res.write(c); held.done = true; if (held.ended) res.end(); }, HOLD);
        });
        r.on('end', () => { if (!held || held.done) res.end(); else held.ended = true; });
      });
      up.on('error', () => res.destroy());
      req.pipe(up);
    });
    const port = await new Promise(ok => proxy.listen(0, '127.0.0.1', () => ok(proxy.address().port)));
    const url = `http://127.0.0.1:${port}/`;
    S = await open({ host: 'web', app: t.app, url });
    const ev = S.carrier.evaluate;
    const q = id => `document.querySelector('#exact-root[data-boot] [data-testid="${id}"]')`;
    // Twice: the field edited without focus (the page's focus on <body>),
    // then focused, which the settled field takes over.
    for (const focus of [false, true]) {
      const pass = `${step}${focus ? ', focused' : ''}`;
      await ev(`location.href = ${JSON.stringify(url)}`).catch(() => {});
      let pressed = null;
      for (const end = Date.now() + 15000; !pressed && Date.now() < end; await new Promise(z => setTimeout(z, 25))) {
        pressed = await ev(`(() => { const b = ${q('boot-bump')}, w = ${q('boot-words')};
          if (!b || !w || location.search) return null;
          if (document.querySelector('#exact-root:not([data-boot])')) return 'late';
          b.click(); ${focus ? 'w.focus();' : ''} w.value = 'early'; for (const k of ['input', 'change']) w.dispatchEvent(new Event(k, { bubbles: true }));
          return 'pressed'; })()`).catch(() => null);
      }
      if (pressed !== 'pressed') { fail(pass, pressed === 'late' ? 'the settled page arrived before the boot document could be pressed' : 'no boot document with boot-bump and boot-words'); continue; }
      let got = null;
      const want = `1 early|early|${focus ? 'boot-words' : ''}|1|`;
      for (const end = Date.now() + 15000; Date.now() < end; await new Promise(z => setTimeout(z, 50))) {
        got = await ev(`(() => { const r = document.querySelectorAll('#exact-root'), s = document.querySelector('[data-testid="boot-state"]');
          return r.length === 1 && r[0].dataset.bootMs != null && s ? [s.textContent, document.querySelector('[data-testid="boot-words"]').value, document.activeElement?.dataset.testid ?? '', r.length, document.title].join('|') : null; })()`).catch(() => null);
        if (got?.startsWith(want)) break;
      }
      if (!got?.startsWith(want)) fail(pass, `after adoption: ${got}, not ${want}… (state|field|focus|roots|title)`);
      report.steps.push({ target: t.name, step: pass, differences: got?.startsWith(want) ? 0 : 1 });
    }
  } catch (e) {
    fail(step, e.message.split('\n')[0]);
  } finally {
    await S?.close?.();
    proxy?.close();
    server.kill();
  }
}

// ---------------------------------------------------------------- the Linux reference
const linuxRef = argv.includes('--linux');
const LINUX_OPS = ['tap', 'type', 'clock'];
// Where an app's drive reaches what only one host has, the Linux comparison
// stops before that step (null: from the start), saying why (each is a host
// difference, not the runner's).
const LINUX_APART = {
  caltrain: ['tap open-deck', 'its deck screen is an iframe, whose load and message only a browser delivers'],
  'markdown-stress': ['tap toggle-single', "its editor's selection report (formats, links) is the web's markup editor's, which the Linux host's text field does not make"],
  'native-fixture': [null, 'its views are native modules (LLP 1024), which the web and Apple hosts load and the Linux host does not'],
  'photo-editor': [null, 'its editor is a native module (LLP 1024), which the web and Apple hosts load and the Linux host does not'],
  messages: ['tap conversation-maya', 'its data sources write drafts and reads to storage on the Linux host, where the page refuses storage in agent mode without --storage (QUEUE)'],
};
const linuxCrate = app => { const f = resolve(root, 'apps', app, 'linux', 'Cargo.toml'); return existsSync(f) ? /^name\s*=\s*"([^"]+)"/m.exec(readFileSync(f, 'utf8'))?.[1] : null; };
function linuxFor(t) {
  if (!linuxRef) return null;
  const why = t.contract && /^\/\/ linux: (.*)$/m.exec(readFileSync(t.contract, 'utf8'))?.[1];
  if (why?.startsWith('state only')) return { stateOnly: true };
  if (why) return { why: `not compared on Linux: ${why}` };
  const crate = linuxCrate(t.app);
  if (!crate) return { why: `not compared on Linux: ${t.app} has no Linux host` };
  if (LINUX_APART[t.name]?.[0] === null) return { why: `not compared on Linux: ${LINUX_APART[t.name][1]}` };
  // agent.mjs runs the crate's own binary; a crate that builds only other bins (a render server) has none.
  if (!existsSync(resolve(resolveApp(t.app).target, 'release', crate))) return { why: `not compared on Linux: ${t.app}'s Linux crate has no ${crate} binary built` };
  return {};
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
if (linuxRef && argv.includes('--build')) {
  // Each data app's binary where agent.mjs runs it (`resolveApp(app).target`):
  // the root workspace's crates in one build; an app in a workspace of its own
  // (Messages, snapback4's) from its manifest into that workspace's target.
  const own = [], rooted = [];
  for (const a of new Set([...apps, ...synthetic.map(s => s.data)])) {
    const crate = linuxCrate(a); if (!crate) continue;
    const target = resolveApp(a).target;
    if (target === resolveApp('caltrain').target) rooted.push(crate); else own.push({ a, crate, target });
  }
  const builds = [...(rooted.length ? [{ what: rooted.join(' '), args: rooted.flatMap(c => ['-p', c]), env: {} }] : []),
    ...own.map(o => ({ what: o.crate, args: ['--manifest-path', resolve(root, 'apps', o.a, 'linux', 'Cargo.toml')], env: { CARGO_TARGET_DIR: o.target } }))];
  for (const { what, args, env } of builds) {
    const b = spawnSync('cargo', ['build', '-q', '--release', ...args], { cwd: root, encoding: 'utf8', maxBuffer: 64 << 20, env: { ...process.env, ...env } });
    if (b.status !== 0) report.failures.push({ target: 'linux', step: `linux-build ${what}`, what: b.stderr.trim().split('\n').slice(-3).join(' ').slice(0, 300) });
  }
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
const lines = [`# JS target conformance — ${report.at}`, '', `| target | JS build | steps compared | steps equal | failures | app tests (wasm / js) |${linuxRef ? ' Linux reference (equal / compared) |' : ''}`, `|---|---|---|---|---|---|${linuxRef ? '---|' : ''}`];
for (const t of targets) {
  const mine = report.steps.filter(x => x.target === t.name), s = mine.filter(x => x.differences != null && !x.reference), info = report.targets[t.name] ?? {};
  const lx = mine.filter(x => x.reference === 'linux'), lskip = mine.find(x => x.step === 'linux' || x.linux === 'stopped');
  const lcell = lskip && !lx.length ? lskip.skipped.replace(/^not compared on Linux: /, 'not compared: ') : `${lx.filter(x => x.differences === 0).length} / ${lx.length}${lskip ? ` (stopped at ${lskip.step})` : ''}`;
  lines.push(`| ${t.name} | ${info.jsBuild === false ? 'refused' : info.jsBuild ? 'ok' : '—'} | ${s.length} | ${s.filter(x => x.differences === 0).length} | ${(byTarget[t.name] ?? []).length} | ${info.tests ? `${info.tests.wasm} / ${info.tests.js}` : '—'} |${linuxRef ? ` ${lcell} |` : ''}`);
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
