// The dev loop on the JS target (LLP 1071): what `host/web/dev.mjs` runs for
// an app the JS target takes, so a developer runs the runtime the app ships.
// An edit — the Contract, the app's TypeScript or Rust data, its assets, or
// this runtime and compiler — rebuilds the app (`host/web-js/build.mjs
// --render none`, into a stage renamed over dist/, so a request sees the old
// build or the new one; what did not change is not rebuilt: module.mjs
// `fresh`) and every open page reloads: the plan is compiled ahead of time,
// so a new plan is a new program. The old page checkpoints the wasm loop's
// carried set into sessionStorage immediately before that reload; the new
// development build consumes it once. A build that fails
// shows its errors in the page and the page keeps the last good build.
// `host/web/dev.mjs --wasm` is the resident wasm loop (a plan restarts in
// place in ~20 ms, state carried).
import { spawn } from 'node:child_process';
import { createServer, request } from 'node:http';
import { existsSync, readFileSync, renameSync, rmSync, statSync, watch, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { webRequestURL } from '../../scripts/origin.mjs';
import { appManifestDigest, buildFileCards, buildTreeFile, sendStaticBody, webContentType } from '../web/serve.mjs';

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
    // The JS target's build itself, rendering no pages (every route is the
    // shell), with the completion marker host/web/build.mjs writes.
    const child = building = spawn(process.execPath, [resolve(root, 'host/web-js/build.mjs'), app.name, '--out', stage, '--render', 'none', '--dev-reload'],
      { cwd: root, env: process.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    child.stdout.on('data', (d) => { log += d; });
    child.stderr.on('data', (d) => { log += d; });
    child.on('exit', (code) => {
      building = null;
      if (code !== 0) { rmSync(stage, { recursive: true, force: true }); return done(log.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l)).slice(-12).join('\n') || `build exited ${code}`); }
      writeFileSync(resolve(stage, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', app: { id: app.id, name: app.displayName },
        manifestSha256: appManifestDigest(app), files: buildFileCards(stage) }) + '\n');
      rmSync(`${dist}.previous`, { recursive: true, force: true });
      if (existsSync(dist)) renameSync(dist, `${dist}.previous`);
      renameSync(stage, dist);
      rmSync(`${dist}.previous`, { recursive: true, force: true });
      done(null);
    });
  });
}

/** Build `app` on the JS target and serve it with reload, until the process ends. */
export async function devJs({ app, dist, port, host, origins, gate, lan }) {
  const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';
  const t0 = Date.now();
  // A first build that fails (a refusal or a compile error) serves its
  // errors in the page and builds again at the next edit.
  let error = await build(app, dist);
  if (error) console.log(`JS target: ${app.name} does not build; its errors show in the page\n${error}`);
  else console.log(`JS target: ${app.name} built in ${Date.now() - t0} ms`);
  console.log('plan ready');
  const clients = new Set();
  const push = (m) => { for (const res of clients) res.write(`data: ${JSON.stringify(m)}\n\n`); };
  let seq = 1, saved = 0, again = false, timer = null, since = t0, logicChanged = false;
  const pending = new Map(); // seq -> the save it answers
  const rebuild = async () => {
    if (building) { again = true; return; }
    const at = saved, t = since = Date.now(), changedLogic = logicChanged;
    logicChanged = false;
    error = await build(app, dist);
    if (error) { logicChanged ||= changedLogic; console.log(`build failed in ${Date.now() - t} ms; the page keeps the last good build\n${error}`); push({ error }); }
    else {
      pending.set(++seq, at);
      console.log(`edit → plan ready ${Date.now() - at} ms (rebuilt in ${Date.now() - t} ms) · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading`);
      push({ reload: seq, logic: changedLogic });
    }
    if (again) { again = false; rebuild(); }
  };
  // A build reading a tree (the assets it copies) is reported too on macOS:
  // a path not modified since the last build started is no edit.
  const changed = (base) => (_, name) => {
    if (name && skipped.test(String(name))) return;
    const relative = String(name ?? '');
    if (base === app.dir && (/(^|\/)(?:data|logic|modules)(\/|$)/.test(relative) || /(?:^|\/)(?:app\.ts|build\.rs|Cargo\.toml)$/.test(relative))) logicChanged = true;
    try { if (name && statSync(resolve(base, String(name))).mtimeMs < since) return; } catch { /* removed: an edit */ }
    if (!timer) saved = Date.now();
    clearTimeout(timer);
    // An editor's save is one burst of events, well inside 5 ms; an event
    // after the build starts builds again (`again`).
    timer = setTimeout(() => { timer = null; rebuild(); }, 5);
  };
  // The app's sources, and the runtime and compiler it builds with.
  const watchers = [watch(app.dir, { recursive: true }, changed(app.dir)), watch(resolve(root, 'host/web-js'), { recursive: true }, changed(resolve(root, 'host/web-js')))];
  for (const f of ['navigation.js', 'index.html']) watchers.push(watch(resolve(root, 'host/web', f), changed(resolve(root, 'host/web'))));
  // The page's side: reload on a new build, the errors of a failed one in an
  // overlay, and a beacon when the reloaded page's runtime is up.
  const client = (n) => `<script>(()=>{const seq=${n},key='exactDevReload',es=new EventSource('/__dev/page');let o;
const show=t=>{if(!t){o?.remove();o=null;return}o??=document.body.appendChild(Object.assign(document.createElement('pre'),{style:'position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647',onclick:()=>show()}));o.textContent=t+'\\n(click to dismiss)'};
const encode=v=>typeof v==='number'&&(!Number.isFinite(v)||Object.is(v,-0))?{$exactNumber:Object.is(v,-0)?'-0':String(v)}:Array.isArray(v)?v.map(encode):v;
const children=e=>[...e.children].filter(x=>x.hasAttribute('data-carry-type'));
const capture=logic=>{const x=globalThis.exact,active=document.activeElement?.closest?.('[data-carry-type]');let focus=null;if(active){const path=[];for(let el=active,p;el&&el.id!=='exact-root';el=p){p=el.parentElement;path.unshift(children(p).indexOf(el))}if(!path.includes(-1))focus={path,type:active.getAttribute('data-carry-type')}}return{time:x.clock.now,slots:x.state[0].map(s=>[s.n.devName,s.n.devType,encode(s())]),answers:logic?[]:x.resources.filter(r=>!r.ticket&&!r.waiting&&r.settled!==undefined).map(r=>[r.name,r.source,encode(r.settled),encode(r.value),r.devType,!!r.store]),carryAnswers:!logic,focus}};
let r;try{r=JSON.parse(sessionStorage.getItem(key))}catch{}
const q=new URLSearchParams(location.search),devKeys=new Set(['agent','seed','locale','timeZone','epoch','storage']),admission=r?.agent??(q.has('agent')?[...q].filter(([k])=>devKeys.has(k)):null);
if(r){sessionStorage.removeItem(key);if(admission){for(const [k,v]of admission)if(!q.has(k))q.set(k,v);history.replaceState(history.state,'',location.pathname+'?'+q+location.hash)}if(r.checkpoint){const s=document.createElement('script');s.type='application/vnd.exact.dev-checkpoint';s.textContent=JSON.stringify(r.checkpoint);document.head.appendChild(s);console.info('exact dev reload: restored',r.seq)}const t=setInterval(()=>{const b=document.getElementById('exact-root')?.dataset.bootMs;if(b!=null){clearInterval(t);Promise.resolve(globalThis.exact?.ready).then(()=>{console.info('exact dev reload: runtime up',r.seq);fetch('/__dev/reloaded?seq='+r.seq+'&boot='+b+'&at='+Date.now(),{method:'POST'})})}},2)}
const ready=()=>globalThis.exact?Promise.resolve(globalThis.exact.ready):new Promise(ok=>{const t=setInterval(()=>{if(globalThis.exact){clearInterval(t);Promise.resolve(globalThis.exact.ready).then(ok)}},2)});
es.onmessage=e=>{const m=JSON.parse(e.data);if(m.error!==undefined)show(m.error);if(m.reload>seq)ready().then(()=>{const checkpoint=capture(m.logic);console.info('exact dev reload: checkpoint',m.reload);try{sessionStorage.setItem(key,JSON.stringify({seq:m.reload,agent:admission,checkpoint}))}catch(e){console.warn('exact dev reload: checkpoint could not be stored; reloading fresh',e)}location.reload()})}})()</script>`;
  const server = createServer((req, res) => {
    if (!gate.check(req).allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    const url = webRequestURL(req.url);
    if (!url) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    if (url.pathname === '/__dev/page') {
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
    // A native client opening this URL (`build.mjs --url`, `/__dev/open`,
    // `exact run`) reads the envelope, the dev generations and their event
    // stream (LLP 1023), which the resident loop's producers make: forwarded
    // to it (below), started at the first such request.
    if (url.pathname === '/exact.json' || url.pathname === '/__dev' || url.pathname === '/__dev/open' || url.pathname.startsWith('/__dev/generation/')
      || (url.pathname === '/' && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json'))) { forward(req, res); return; }
    const found = buildTreeFile(dist, url.pathname);
    // No build yet (the first failed): a blank page that shows the errors and reloads when one lands.
    if (!found && error && !url.pathname.slice(1).includes('.')) { res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' }); res.end(`<!doctype html><meta charset="utf-8"><body>${client(seq)}`); return; }
    if (!found) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    let body = readFileSync(found.path);
    if (found.route.endsWith('.html')) body = body.toString() + client(seq);
    sendStaticBody(req, res, body, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
  });
  // The resident loop's producers (host/web/dev.mjs `--serve-as`: the
  // resident compiler, the TypeScript and Rust module producers, the
  // envelope and generations) on a loopback port, into their own dist; a
  // native client's requests are forwarded as they came (Host included, so
  // the pages it serves name this URL). Its wasm build is internal.
  let resident = null, residentChild = null;
  const residentLoop = () => resident ??= new Promise((ok, fail) => {
    const probe = createServer().listen(0, '127.0.0.1', () => {
      const internal = probe.address().port;
      probe.close(() => {
        console.log(`native client: starting the resident loop's producers (loopback :${internal})`);
        residentChild = spawn(process.execPath, [resolve(root, 'host/web/dev.mjs'), '--app', app.name, '--wasm', '--port', String(internal), '--serve-as', String(port), ...(lan ? ['--lan'] : [])],
          { cwd: root, env: { ...process.env, EXACT_WEB_DIST: resolve(app.target, 'web-dist-resident') }, stdio: ['ignore', 'pipe', 'inherit'] });
        let buf = '';
        residentChild.stdout.on('data', (d) => {
          buf += d; const lines = buf.split('\n'); buf = lines.pop();
          for (const l of lines) { console.log(`  [resident] ${l}`); if (/^(?:plan ready|module generation ready|Rust generation \w+ ready)/.test(l)) ok(internal); }
        });
        residentChild.on('exit', (code) => { resident = null; residentChild = null; fail(new Error(`the resident loop exited ${code}`)); });
      });
    });
  });
  const forward = (req, res) => residentLoop().then((internal) => {
    const out = request({ host: '127.0.0.1', port: internal, method: req.method, path: req.url, headers: req.headers }, (answer) => { res.writeHead(answer.statusCode, answer.headers); answer.pipe(res); });
    out.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
    req.pipe(out);
  }, (e) => { res.writeHead(503, { 'content-type': 'text/plain', 'cache-control': 'no-store' }); res.end(`${e.message}\n`); });
  const stop = () => { for (const w of watchers) w.close(); building?.kill('SIGKILL'); residentChild?.kill('SIGTERM'); server.close(); process.exit(0); };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  await new Promise((ok, fail) => { server.on('error', fail); server.listen(port, host, ok); })
    .catch((e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}`); process.exit(1); });
  const urls = origins.map((o) => `${o.origin}/`);
  console.log(urls.join('\n'));
  console.log(urls.map(url => `  Open in native: ${url}__dev/open`).join('\n'));
  console.log(`  (dev loop on the JS target: ${app.dir.replace(root + '/', '')} and host/web-js rebuild and reload the page; a native client's requests go to the resident loop's producers, started at the first; ${lan ? 'LAN bind — any peer on this network can read the app and its compile errors' : 'loopback only — --lan to serve a phone on this network'}; ctrl-c to stop)`);
  await new Promise(() => {});
}
