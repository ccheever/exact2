// The dev loop on the JS target (LLP 1071): what `host/web/dev.mjs` runs for
// an app the JS target takes, so a developer runs the runtime the app ships.
// An edit — the Contract, the app's TypeScript or Rust data, its assets, or
// this runtime and compiler — rebuilds the app (`host/web-js/build.mjs
// --render none`, into a stage renamed over dist/, so a request sees the old
// build or the new one; what did not change is not rebuilt: module.mjs
// `fresh`) and every open page reloads: the plan is compiled ahead of time,
// so a new plan is a new program. The old page checkpoints the wasm loop's
// carried set to this server immediately before that reload; the next
// document consumes it once. A build that fails
// shows its errors in the page and the page keeps the last good build.
// `host/web/dev.mjs --wasm` is the resident wasm loop (a plan restarts in
// place in ~20 ms, state carried).
import { spawn } from 'node:child_process';
import { createServer, request } from 'node:http';
import { existsSync, readFileSync, renameSync, rmSync, statSync, watch, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { webRequestURL } from '../../scripts/origin.mjs';
import { appManifestDigest, buildFileCards, buildTreeFile, saveTrace, sendStaticBody, webContentType } from '../web/serve.mjs';
import { localInstaller } from '../web/local-install.mjs';

const CHECKPOINT_BYTES = 16 * 1024 * 1024, CHECKPOINTS = 8;

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
    const child = building = spawn(process.execPath, [resolve(root, 'host/web-js/build.mjs'), app.name, '--out', stage, '--render', 'none', '--dev'],
      { cwd: root, env: process.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    child.stdout.on('data', (d) => { log += d; });
    child.stderr.on('data', (d) => { log += d; });
    child.on('exit', (code) => {
      building = null;
      if (code !== 0) { rmSync(stage, { recursive: true, force: true }); return done({ error: log.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l)).slice(-12).join('\n') || `build exited ${code}` }); }
      const logic = readFileSync(resolve(stage, '.exact-dev-logic.json'), 'utf8').trim();
      writeFileSync(resolve(stage, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', app: { id: app.id, name: app.displayName },
        manifestSha256: appManifestDigest(app), files: buildFileCards(stage) }) + '\n');
      rmSync(`${dist}.previous`, { recursive: true, force: true });
      if (existsSync(dist)) renameSync(dist, `${dist}.previous`);
      renameSync(stage, dist);
      rmSync(`${dist}.previous`, { recursive: true, force: true });
      done({ error: null, logic });
    });
  });
}

/** Build `app` on the JS target and serve it with reload, until the process ends. */
export async function devJs({ app, dist, port, host, origins, gate, lan, allowHosts = [] }) {
  const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';
  const t0 = Date.now();
  // A first build that fails (a refusal or a compile error) serves its
  // errors in the page and builds again at the next edit.
  let built = await build(app, dist), error = built.error, logicRevision = built.logic ?? '';
  if (error) console.log(`JS target: ${app.name} does not build; its errors show in the page\n${error}`);
  else console.log(`JS target: ${app.name} built in ${Date.now() - t0} ms`);
  console.log('plan ready');
  const clients = new Set();
  const push = (m) => { for (const res of clients) res.write(`data: ${JSON.stringify(m)}\n\n`); };
  let seq = 1, saved = 0, again = false, timer = null, since = t0;
  const pending = new Map(); // seq -> the save it answers
  const rebuild = async () => {
    if (building) { again = true; return; }
    const at = saved, t = since = Date.now();
    built = await build(app, dist); error = built.error;
    if (error) { console.log(`build failed in ${Date.now() - t} ms; the page keeps the last good build\n${error}`); push({ error }); }
    else {
      logicRevision = built.logic;
      pending.set(++seq, at);
      console.log(`edit → plan ready ${Date.now() - at} ms (rebuilt in ${Date.now() - t} ms) · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading`);
      push({ reload: seq, revision: logicRevision });
    }
    if (again) { again = false; rebuild(); }
  };
  // A build reading a tree (the assets it copies) is reported too on macOS:
  // a path not modified since the last build started is no edit.
  const changed = (base) => (_, name) => {
    if (name && skipped.test(String(name))) return;
    // The declarations a build writes beside app.ts, for an editor.
    if (base === app.dir && String(name) === 'app.contract.d.ts') return;
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
  // The one TypeScript configuration app.ts is checked with (js/bake/src/typescript.mjs).
  watchers.push(watch(resolve(root, 'js/bake/src/typescript.mjs'), changed(resolve(root, 'js/bake/src'))));
  // The page's side: reload on a new build, the errors of a failed one in an
  // overlay, and a beacon when the reloaded page's runtime is up.
  const client = (n, revision) => `<script>(()=>{const seq=${n},logicRevision=${JSON.stringify(revision)},es=new EventSource('/__dev/page');let o,reloading=false;
const show=t=>{if(!t){o?.remove();o=null;return}o??=document.body.appendChild(Object.assign(document.createElement('pre'),{style:'position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647',onclick:()=>show()}));o.textContent=t+'\\n(click to dismiss)'};
const encode=v=>typeof v==='number'&&(!Number.isFinite(v)||Object.is(v,-0))?{$exactNumber:Object.is(v,-0)?'-0':String(v)}:Array.isArray(v)?v.map(encode):v;
const children=e=>[...(e?.children??[])].filter(x=>x.hasAttribute('data-carry-type')||x.hasAttribute('data-listitemkey'));
const reserved=new Set(['exactDelivery','exactViewport','exactTime','exactPage','exactSurface']);
const capture=logic=>{const x=globalThis.exact,active=document.activeElement?.closest?.('[data-carry-type]');let focus=null;if(active){const path=[];for(let el=active,p;el&&el.id!=='exact-root';el=p){p=el.parentElement;path.unshift(children(p).indexOf(el))}if(!path.includes(-1))focus={path,type:active.getAttribute('data-carry-type')}}return{time:x.clock.now,slots:x.state[0].map(s=>[s.n.devName,s.n.devType,encode(s())]),answers:logic?[]:x.resources.filter(r=>!reserved.has(r.source)&&!r.ticket&&!r.waiting&&r.settled!==undefined).map(r=>[r.name,r.source,encode(r.settled),encode(r.value),r.devType,!!r.store]),carryAnswers:!logic,focus}};
const restored=document.querySelector('script[type="application/vnd.exact.dev-checkpoint"]'),q=new URLSearchParams(location.search),devKeys=new Set(['agent','seed','locale','timeZone','epoch','storage']),admission=q.has('agent')?[...q].filter(([k])=>devKeys.has(k)):null;
if(restored){const restoredSeq=Number(restored.dataset.seq);console.info('exact dev reload: restored',restoredSeq);const t=setInterval(()=>{const b=document.getElementById('exact-root')?.dataset.bootMs;if(b!=null){clearInterval(t);Promise.resolve(globalThis.exact?.ready).then(()=>{console.info('exact dev reload: runtime up',restoredSeq);fetch('/__dev/reloaded?seq='+restoredSeq+'&boot='+b+'&at='+Date.now(),{method:'POST'})})}},2)}
const ready=()=>globalThis.exact?Promise.resolve(globalThis.exact.ready):new Promise(ok=>{const t=setInterval(()=>{if(globalThis.exact){clearInterval(t);Promise.resolve(globalThis.exact.ready).then(ok)}},2)});
es.onmessage=e=>{const m=JSON.parse(e.data);if(m.error!==undefined)show(m.error);if(m.reload>seq&&!reloading){reloading=true;if(!globalThis.exact){location.reload();return}ready().then(async()=>{const checkpoint=capture(m.revision!==logicRevision),id=crypto.getRandomValues(new Uint32Array(4)).join('-');console.info('exact dev reload: checkpoint',m.reload);const saved=await fetch('/__dev/checkpoint?id='+id+'&seq='+m.reload+'&revision='+encodeURIComponent(m.revision??''),{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(checkpoint)});if(!saved.ok)throw new Error('checkpoint handoff answered '+saved.status);const next=new URL(location.href);next.searchParams.set('__exactDev',id);if(admission)for(const [k,v]of admission)if(!next.searchParams.has(k))next.searchParams.set(k,v);location.replace(next.href)}).catch(e=>{reloading=false;console.error('exact dev reload: current page retained; checkpoint handoff failed',e)})}}})()</script>`;
  const checkpoints = new Map();
  // The install pages (LLP 1030.003 D6a) and, from a loopback page on a Mac,
  // the local iOS build (D6b): this server's, since only it sees the real
  // peer (a forwarded request's is loopback).
  const installer = localInstaller({ app: () => app, origins, port, gate, listener: { host, port } });
  const server = createServer(async (req, res) => {
    const access = gate.check(req);
    if (!access.allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    const url = webRequestURL(req.url);
    if (!url) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    if (await installer.handle(req, res, url, access)) return;
    if (url.pathname === '/__dev/page') {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      res.write(`data: ${JSON.stringify(error ? { error, reload: seq, revision: logicRevision } : { reload: seq, revision: logicRevision })}\n\n`);
      clients.add(res);
      console.log(`page connected (${clients.size})`);
      req.on('close', () => clients.delete(res));
      return;
    }
    if (url.pathname === '/__dev/checkpoint' && req.method === 'POST') {
      const id = url.searchParams.get('id') ?? '', n = Number(url.searchParams.get('seq')), revision = url.searchParams.get('revision') ?? '', chunks = [];
      if (!/^\d+(?:-\d+){3}$/.test(id) || !Number.isSafeInteger(n)) { res.writeHead(400); res.end(); return; }
      // Bounded: a checkpoint is at most the hosts' 16 MiB surface carry, and the
      // loop keeps the newest few (a page reloads one at a time).
      let size = 0, over = false;
      req.on('data', chunk => { size += chunk.length; if (size > CHECKPOINT_BYTES) over = true; else chunks.push(chunk); });
      req.on('end', () => {
        if (over) { res.writeHead(413); res.end(); return; }
        try {
          const text = Buffer.concat(chunks).toString('utf8'); JSON.parse(text);
          checkpoints.delete(id); checkpoints.set(id, { seq: n, revision, text });
          while (checkpoints.size > CHECKPOINTS) checkpoints.delete(checkpoints.keys().next().value);
          res.writeHead(204); res.end();
        } catch { res.writeHead(400); res.end(); }
      });
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
    if (url.pathname === '/__exact/trace' && req.method === 'POST') return saveTrace(req, res, { app, dist, root });
    if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405); res.end(); return; }
    // A native client opening this URL (`build.mjs --url`, `/__dev/open`,
    // `exact run`) reads the envelope, the dev generations and their event
    // stream (LLP 1023), which the resident loop's producers make: forwarded
    // to it (below), started at the first such request.
    if (url.pathname === '/exact.json' || url.pathname === '/__dev' || url.pathname === '/__dev/open' || url.pathname.startsWith('/__dev/generation/')
      || (url.pathname === '/' && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json'))) { forward(req, res); return; }
    const found = buildTreeFile(dist, url.pathname);
    // No build yet (the first failed): a blank page that shows the errors and reloads when one lands.
    if (!found && error && !url.pathname.slice(1).includes('.')) { res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' }); res.end(`<!doctype html><meta charset="utf-8"><body>${client(seq, logicRevision)}`); return; }
    if (!found) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    let body = installer.page(found.route, readFileSync(found.path), access);
    if (found.route.endsWith('.html')) {
      body = body.toString();
      const id = url.searchParams.get('__exactDev'), carried = id && checkpoints.get(id);
      if (carried) {
        checkpoints.delete(id);
        if (carried.revision !== String(logicRevision ?? '')) {
          const c = JSON.parse(carried.text);
          if (c.carryAnswers || c.answers?.length) { c.carryAnswers = false; c.answers = []; carried.text = JSON.stringify(c); }
        }
        const checkpoint = `<script type="application/vnd.exact.dev-checkpoint" data-seq="${carried.seq}">${carried.text.replaceAll('<', '\\u003c')}</script><script>const u=new URL(location.href);u.searchParams.delete('__exactDev');history.replaceState(history.state,'',u.pathname+(u.search?'?'+u.searchParams:'')+u.hash)</script>`;
        body = body.replace('<script type="module"', checkpoint + '<script type="module"');
      }
      body += client(seq, logicRevision);
    }
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
        residentChild = spawn(process.execPath, [resolve(root, 'host/web/dev.mjs'), '--app', app.name, '--wasm', '--port', String(internal), '--serve-as', String(port), ...(lan ? ['--lan'] : []), ...allowHosts.flatMap((name) => ['--allow-host', name])],
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
  const stop = async () => {
    for (const w of watchers) w.close(); building?.kill('SIGKILL'); residentChild?.kill('SIGTERM'); server.close();
    // A local iOS build in flight is stopped and waited for, as the resident loop does.
    const install = installer.child;
    if (install && install.exitCode === null && install.signalCode === null) { const exit = new Promise((ok) => install.once('exit', ok)); install.kill('SIGTERM'); await exit; }
    process.exit(0);
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  await new Promise((ok, fail) => { server.on('error', fail); server.listen(port, host, ok); })
    .catch((e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}${e.code === 'EADDRINUSE' ? ' (another dev loop? --port <n> picks another)' : ''}`); process.exit(1); });
  const urls = origins.map((o) => `${o.origin}/`);
  console.log(urls.join('\n'));
  console.log(urls.map(url => `  Open in native: ${url}__dev/open`).join('\n'));
  if (allowHosts.length) console.log(`  also answering to ${allowHosts.join(', ')} (--allow-host)`);
  console.log(`  (dev loop on the JS target: ${app.dir.replace(root + '/', '')} and host/web-js rebuild and reload the page; a native client's requests go to the resident loop's producers, started at the first; ${lan ? 'LAN bind — any peer on this network can read the app and its compile errors' : 'loopback only — --lan to serve a phone on this network'}; ctrl-c to stop)`);
  await new Promise(() => {});
}
