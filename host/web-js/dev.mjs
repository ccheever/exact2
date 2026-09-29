// The dev loop on the JS target (LLP 1071): what `host/web/dev.mjs` runs for
// an app the JS target takes, so a developer runs the runtime the app ships.
// An edit — the Contract, the app's TypeScript or Rust data, its assets, or
// this runtime and compiler — rebuilds the app (`host/web/build.mjs --js`,
// into a stage renamed over dist/, so a request sees the old build or the
// new one) and every open page reloads: the plan is compiled ahead of time,
// so a new plan is a new program. Slot values are not carried across the
// reload (LLP 1071 §7, "dev reload and state carry"). A build that fails
// shows its errors in the page and the page keeps the last good build.
// `host/web/dev.mjs --wasm` is the resident wasm loop (a plan restarts in
// place in ~20 ms, state carried).
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, readFileSync, renameSync, rmSync, statSync, watch } from 'node:fs';
import { resolve } from 'node:path';
import { webRequestURL } from '../../scripts/origin.mjs';
import { buildTreeFile, sendStaticBody, webContentType } from '../web/serve.mjs';

const root = resolve(new URL('../..', import.meta.url).pathname);
const skipped = /(^|\/)(target|dist(?:\.previous)?|node_modules|conformance)(\/|$)|(^|\/)\.|\.md$/;

let building = null; // the build in flight, stopped with the server

/** One build of `app` into `dist` through a stage under the app's ignored
 * target/ (as the wasm build stages); resolves to null or the build's
 * errors (what the JS target refused, or a compile error). */
function build(app, dist) {
  const stage = resolve(app.target, 'web-js-dev-stage');
  rmSync(stage, { recursive: true, force: true });
  return new Promise((done) => {
    const child = building = spawn(process.execPath, [resolve(root, 'host/web/build.mjs'), app.crate('web'), '--js'],
      { cwd: root, env: { ...process.env, EXACT_WEB_DIST: stage }, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    child.stdout.on('data', (d) => { log += d; });
    child.stderr.on('data', (d) => { log += d; });
    child.on('exit', (code) => {
      building = null;
      if (code !== 0) { rmSync(stage, { recursive: true, force: true }); return done(log.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l)).slice(-12).join('\n') || `build exited ${code}`); }
      rmSync(`${dist}.previous`, { recursive: true, force: true });
      if (existsSync(dist)) renameSync(dist, `${dist}.previous`);
      renameSync(stage, dist);
      rmSync(`${dist}.previous`, { recursive: true, force: true });
      done(null);
    });
  });
}

/** Build `app` on the JS target and serve it with reload. Resolves to the
 * refusal when the JS target doesn't take the app (dev.mjs then runs the
 * wasm loop); otherwise serves until the process ends. */
export async function devJs({ app, dist, port, host, origins, gate, lan }) {
  const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';
  const t0 = Date.now();
  const refused = await build(app, dist);
  if (refused) return refused;
  console.log(`JS target: ${app.name} built in ${Date.now() - t0} ms`);
  console.log('plan ready');
  const clients = new Set();
  const push = (m) => { for (const res of clients) res.write(`data: ${JSON.stringify(m)}\n\n`); };
  let seq = 1, error = null, saved = 0, again = false, timer = null, since = t0;
  const pending = new Map(); // seq -> the save it answers
  const rebuild = async () => {
    if (building) { again = true; return; }
    const at = saved, t = since = Date.now();
    error = await build(app, dist);
    if (error) { console.log(`build failed in ${Date.now() - t} ms; the page keeps the last good build\n${error}`); push({ error }); }
    else {
      pending.set(++seq, at);
      console.log(`edit → plan ready ${Date.now() - at} ms (rebuilt in ${Date.now() - t} ms) · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading`);
      push({ reload: seq });
    }
    if (again) { again = false; rebuild(); }
  };
  // A build reading a tree (the assets it copies) is reported too on macOS:
  // a path not modified since the last build started is no edit.
  const changed = (base) => (_, name) => {
    if (name && skipped.test(String(name))) return;
    try { if (name && statSync(resolve(base, String(name))).mtimeMs < since) return; } catch { /* removed: an edit */ }
    if (!timer) saved = Date.now();
    clearTimeout(timer);
    timer = setTimeout(() => { timer = null; rebuild(); }, 30);
  };
  // The app's sources, and the runtime and compiler it builds with.
  const watchers = [watch(app.dir, { recursive: true }, changed(app.dir)), watch(resolve(root, 'host/web-js'), { recursive: true }, changed(resolve(root, 'host/web-js')))];
  for (const f of ['navigation.js', 'index.html']) watchers.push(watch(resolve(root, 'host/web', f), changed(resolve(root, 'host/web'))));
  // The page's side: reload on a new build, the errors of a failed one in an
  // overlay, and a beacon when the reloaded page's runtime is up.
  const client = (n) => `<script>(()=>{const seq=${n},es=new EventSource('/__dev');let o;
const show=t=>{if(!t){o?.remove();o=null;return}o??=document.body.appendChild(Object.assign(document.createElement('pre'),{style:'position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647',onclick:()=>show()}));o.textContent=t+'\\n(click to dismiss)'};
es.onmessage=e=>{const m=JSON.parse(e.data);if(m.error!==undefined)show(m.error);if(m.reload>seq){sessionStorage.exactDevReload=m.reload;location.reload()}};
const r=sessionStorage.exactDevReload;if(r){delete sessionStorage.exactDevReload;const t=setInterval(()=>{const b=document.getElementById('exact-root')?.dataset.bootMs;if(b!=null){clearInterval(t);fetch('/__dev/reloaded?seq='+r+'&boot='+b+'&at='+Date.now(),{method:'POST'})}},2)}})()</script>`;
  const server = createServer((req, res) => {
    if (!gate.check(req).allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    const url = webRequestURL(req.url);
    if (!url) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    if (url.pathname === '/__dev') {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      res.write(`data: ${JSON.stringify(error ? { error, reload: seq } : { reload: seq })}\n\n`);
      clients.add(res);
      console.log(`page connected (${clients.size})`);
      req.on('close', () => clients.delete(res));
      return;
    }
    if (url.pathname === '/__dev/reloaded') {
      const n = Number(url.searchParams.get('seq')), at = pending.get(n);
      if (at) {
        const total = Number(url.searchParams.get('at')) - at;
        console.log(`  → page: runtime up ${url.searchParams.get('boot')} ms after its script; edit → first frame in the DOM ${total} ms (budget ${budget}: a rebuild and a reload)`);
        console.log(`reloaded seq=${n} total_ms=${total}`);
      }
      res.writeHead(204); res.end(); return;
    }
    if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405); res.end(); return; }
    // A native client opening this URL reads the envelope and dev generations
    // the wasm loop serves (LLP 1023); this loop has neither.
    if (url.pathname === '/exact.json' || url.pathname.startsWith('/__dev/') || (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json')) {
      res.writeHead(404, { 'content-type': 'text/plain', 'cache-control': 'no-store' });
      res.end(`${app.name} is on the JS target's dev loop, which serves browsers only; a native client opens the wasm loop: bun host/web/dev.mjs --app ${app.name} --wasm\n`);
      return;
    }
    const found = buildTreeFile(dist, url.pathname);
    if (!found) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    let body = readFileSync(found.path);
    if (found.route.endsWith('.html')) body = body.toString() + client(seq);
    sendStaticBody(req, res, body, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
  });
  const stop = () => { for (const w of watchers) w.close(); building?.kill('SIGKILL'); server.close(); process.exit(0); };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  await new Promise((ok, fail) => { server.on('error', fail); server.listen(port, host, ok); })
    .catch((e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}`); process.exit(1); });
  const urls = origins.map((o) => `${o.origin}/`);
  console.log(urls.join('\n'));
  console.log(`  (dev loop on the JS target: ${app.dir.replace(root + '/', '')} and host/web-js rebuild and reload the page; --wasm for the resident wasm loop; ${lan ? 'LAN bind — any peer on this network can read the app and its compile errors' : 'loopback only — --lan to serve a phone on this network'}; ctrl-c to stop)`);
  await new Promise(() => {});
}
