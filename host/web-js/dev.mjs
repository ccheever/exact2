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
import { existsSync, lstatSync, readFileSync, rmSync, statSync, utimesSync, watch, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { acceptsHTML, appDocumentPath, webRequestURL } from '../../scripts/origin.mjs';
import { compilerWarnings, INPUT_TREE, OUTPUT, listenOn } from '../../scripts/agent-launch.mjs';
import { appManifestDigest, buildFileCards, buildTreeFile, replaceBuild, saveTrace, sendStaticBody, watchLauncher, webContentType } from '../web/serve.mjs';
import { localInstaller } from '../web/local-install.mjs';
import { designPath, frameOnlyByTool, tamberServer } from '../../scripts/tamber-server.mjs';
import { serveCommands, serveReport } from '../../scripts/tamber-builds.mjs';
import { fresh, webCompiler } from './module.mjs';

const CHECKPOINT_BYTES = 16 * 1024 * 1024, CHECKPOINTS = 8;

const root = resolve(new URL('../..', import.meta.url).pathname);
const skipped = /(^|\/)(target|dist(?:\.previous)?|node_modules|conformance)(\/|$)|(^|\/)\.|\.md$/;

let building = null; // the build in flight, stopped with the server

/** The Contract packages a build read (LLP 1091 D10), outside the app and
 * its skipped `node_modules`: the dev loop watches them too. */
function devPackages(stage) {
  try {
    const sources = JSON.parse(readFileSync(resolve(stage, '.gen', 'dev-sources.json'), 'utf8'));
    return { trees: sources.packages ?? [], shallow: sources.shallow ?? [], sources: sources.sources ?? [] };
  } catch { return { trees: [], shallow: [], sources: [] }; }
}

/** The development build's design (`exact-web-js --dev-reload`'s
 * `dev-design.json`: structure, digest, tokens), or null. */
function devDesign(stage) {
  try { return JSON.parse(readFileSync(resolve(stage, '.gen', 'dev-design.json'), 'utf8')); } catch { return null; }
}

/** One build of `app` into `dist` through a stage under the app's ignored
 * target/ (as the wasm build stages); resolves to null or the build's
 * errors (what the JS target refused, or a compile error). `keep(design,
 * logic)`, when it says so, keeps the build in `dist` (a value-only design
 * edit, LLP 1121 stage 4: the page takes a batch instead). */
function build(app, dist, keep = () => false) {
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
      const packages = devPackages(stage);
      // The one line a build says when it rewrote the types app.ts imports, built or not (LLP 1116.001 D4.5).
      const types = /^app\.contract\.d\.ts: .*$/m.exec(log)?.[0];
      if (types) console.log(types);
      if (code !== 0) { rmSync(stage, { recursive: true, force: true }); return done({ packages, error: log.split('\n').filter((l) => l.trim() && !/^\s*(Compiling|Finished|Running|warning)/.test(l) && !compilerWarnings(l).length).slice(-12).join('\n') || `build exited ${code}` }); }
      const logic = readFileSync(resolve(stage, '.exact-dev-logic.json'), 'utf8').trim();
      const design = devDesign(stage);
      if (keep(design, logic)) { rmSync(stage, { recursive: true, force: true }); return done({ error: null, logic, packages, design, kept: true, warnings: compilerWarnings(log) }); }
      writeFileSync(resolve(stage, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', app: { id: app.id, name: app.displayName },
        manifestSha256: appManifestDigest(app), files: buildFileCards(stage) }) + '\n');
      replaceBuild(stage, dist);
      done({ error: null, logic, packages, design, warnings: compilerWarnings(log) });
    });
  });
}

/** Build `app` on the JS target and serve it with reload, until the process ends. */
export async function devJs({ app, dist, port, host, origins, gate, lan, allowHosts = [], residentPort = 0 }) {
  const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';
  const t0 = Date.now();
  // A first build that fails (a refusal or a compile error) serves its
  // errors in the page and builds again at the next edit.
  let built = await build(app, dist), error = built.error, logicRevision = built.logic ?? '';
  // The compiler's warnings (LLP 1116.004 D7) when they change, not again at each save that leaves them.
  let warned = '';
  const warn = (warnings) => {
    if (!warnings || warnings.join('\n') === warned) return;
    if (warnings.length) console.log(warnings.join('\n'));
    else console.log('the compiler warns of nothing now');
    warned = warnings.join('\n');
  };
  if (error) console.log(`JS target: ${app.name} does not build; its errors show in the page\n${error}`);
  else console.log(`JS target: ${app.name} built in ${Date.now() - t0} ms`);
  warn(built.warnings);
  console.log('plan ready');
  // The design server's wire, once made below (a rebuild may come first).
  let tamber = null;
  const clients = new Set();
  const push = (m) => { for (const res of clients) res.write(`data: ${JSON.stringify(m)}\n\n`); };
  // The live table (LLP 1121 stage 4): the design the page in dist was built
  // with, and the one the pages run after value-only edits since.
  let design = built.design ?? null, snapshot = null;
  const notice = (s) => ({ kind: 'tamber-batch', structure: s.structure, base: s.design, design: s.design, tokens: [] });
  // A batch larger than this, in UTF-8 bytes, goes as the snapshot's notice
  // instead, as the resident loop sends it (host/web/dev.mjs DESIGN_FRAME):
  // a native host's event frames are bounded (PlanURL.swift's 64 KiB).
  const framed = (batch) => (Buffer.byteLength(JSON.stringify(batch)) > 32 * 1024 ? notice(snapshot) : batch);
  const same = (a, b) => a.value === b.value && a.slices.length === b.slices.length && a.slices.every((s, i) => s === b.slices[i]);
  // A scrub (LLP 1121 stage 4, commit 6): the design the pages run while it
  // previews, and the snapshot it put aside; the next build, or its end,
  // sends them on from it.
  // `released`: that scrub is being written; the build of what it writes
  // goes from it, and ends it.
  let scrubbed = null, aside = null, released = false;
  // A design (`exact-web-js design`), read from the compiler with no build:
  // a value-only edit's and a scrub's. Optimised (host-dev), built as the
  // loop starts and again after any edit that is not a design's, so it is
  // the compiler the builds use; until it is, the debug compiler answers.
  const tool = { path: resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'host-dev/exact-web-js'), ready: false, child: null, reading: null };
  // Cargo and the compilers it runs, as a group (stopped together).
  const stopGroup = (child) => { if (!child) return; try { process.kill(-child.pid, 'SIGTERM'); } catch { child.kill(); } };
  const buildTool = () => {
    tool.ready = false;
    stopGroup(tool.child);
    const started = new Date();
    const child = tool.child = spawn('cargo', ['build', '-q', '--profile', 'host-dev', '-p', 'exact-web-js'], { cwd: root, stdio: ['ignore', 'ignore', 'pipe'], detached: true });
    let err = '';
    child.stderr.on('data', (d) => { err = (err + d).slice(-2000); });
    child.on('error', (e) => { if (tool.child === child) { tool.child = null; console.log(`the design reader did not build (design edits take the full build): ${e.message}`); } });
    child.on('exit', (code, signal) => {
      if (tool.child !== child) return;
      tool.child = null;
      tool.ready = code === 0 && existsSync(tool.path);
      // Dated to the build's start, as Cargo dates its fingerprints: a
      // source saved while it compiled makes it stale (`fresh`).
      if (tool.ready) { try { utimesSync(tool.path, started, started); } catch { tool.ready = false; } }
      if (!tool.ready && !signal) console.log(`the design reader did not build (design edits take the full build):\n${err.trim().split('\n').slice(-4).join('\n')}`);
    });
  };
  // Ready, and built from the compiler's sources as they are (a compiler
  // edit the loop does not watch, under contract/ or kernel/, rebuilds it).
  const toolFresh = () => {
    if (!tool.ready) return false;
    if (fresh(tool.path, `${tool.path}.d`)) return true;
    buildTool();
    return false;
  };
  buildTool();
  const designOf = (text) => new Promise((done) => {
    const compiler = toolFresh() ? { cmd: tool.path, pre: [] } : webCompiler();
    // A group: the debug fallback is a `cargo run`, stopped with its compilers.
    const child = tool.reading = spawn(compiler.cmd, [...compiler.pre, 'design', resolve(app.dir, 'app.contract'), '-'], { cwd: root, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
    child.on('error', (e) => done({ error: e.message }));
    child.stdin.on('error', () => {});
    child.stdin.end(text);
    let out = '', err = '';
    child.stdout.on('data', (d) => { out += d; });
    child.stderr.on('data', (d) => { err += d; });
    // `close`: its output read whole.
    child.on('close', (code) => {
      if (tool.reading === child) tool.reading = null;
      try { done(code === 0 ? JSON.parse(out) : { error: err.trim().split('\n').slice(-6).join('\n') || `the compiler exited ${code}` }); } catch (e) { done({ error: e.message }); }
    });
  });
  // The design's shape and the plan's structure (a profile colour's value
  // is in it) as a kept build requires, and each token's accent.
  const sameShape = (from, next) => from && next && from.shape === next.shape && from.structure === next.structure && from.tokens.length === next.tokens.length
    && next.tokens.every((t, i) => t.name === from.tokens[i].name && String(t.accent) === String(from.tokens[i].accent));
  // A fast batch is checked by the full build after it (`verifying`): the
  // design's module inputs and its compile; a value-only edit during that
  // build is sent at once and checked again after (`fastSeq`).
  let designing = false, verifying = false, fastSeq = 0, stopping = false;
  // The files edited since the last build, by path: only design files may
  // be a value-only edit.
  const edited = new Set(), appeared = new Set();
  let seq = 1, saved = 0, again = false, timer = null, since = t0;
  const pending = new Map(); // seq -> the save it answers
  const rebuild = async () => {
    const at = saved, t = Date.now();
    // A path that existed during the save and is gone by now (an editor's
    // or sed's temporary file) edited nothing; one already gone when its
    // event came (a deleted asset) is an edit.
    const designs = [...edited].filter((path) => path.endsWith('.tamber'));
    const designOnly = designs.length > 0 && [...edited].every((path) => path.endsWith('.tamber') || (!existsSync(path) && appeared.has(path)));
    // A value-only design edit (LLP 1121 stage 4): its design from the
    // compiler, no build, sent as a batch; the page in dist stays, as a kept
    // build's does; the full build after it checks what the read cannot (the
    // modules' inputs, the whole compile). One that moves the design's shape
    // (or fails) builds. Only the app's own design: another .tamber may be
    // any build's input.
    const file = designPath(app.dir);
    const fast = designOnly && design && designs.every((path) => path === file);
    if (designing || (building && !(verifying && fast))) { again = true; return; }
    // Kept until a build lands: a failed one's edits are the next build's too.
    const pendingEdits = [...edited], pendingAppeared = [...appeared];
    edited.clear(); appeared.clear();
    if (fast && toolFresh()) {
      designing = true;
      let next;
      try { next = existsSync(file) ? await designOf(readFileSync(file, 'utf8')) : { error: 'no design' }; } catch (e) { next = { error: e.message }; } finally { designing = false; }
      // The loop stopped while it read: no build follows.
      if (stopping) return;
      if (!next.error && sameShape(design, next)) {
        const from = scrubbed ?? design;
        design = next;
        const committed = { kind: 'tamber-snapshot', structure: design.shape, base: null, design: design.digest, tokens: design.tokens };
        if (scrubbed && released) { scrubbed = null; aside = null; released = false; }
        if (scrubbed) aside = committed; else snapshot = committed;
        const tokens = design.tokens.filter((tk, i) => !same(tk, from.tokens[i]));
        if (tokens.length && !scrubbed) {
          console.log(`edit → batch ready ${Date.now() - at} ms (design read in ${Date.now() - t} ms, no build) · ${tokens.length} token${tokens.length === 1 ? '' : 's'} to ${clients.size} page${clients.size === 1 ? '' : 's'}, no reload`);
          const batch = framed({ kind: 'tamber-batch', structure: design.shape, base: from.digest, design: design.digest, tokens });
          push(batch);
          tamber?.channel.broadcast(batch);
        }
        fastSeq++;
        // A check already under way is done again after it.
        if (building) { again = true; return; }
      }
    }
    if (building) { for (const p of pendingEdits) edited.add(p); for (const p of pendingAppeared) appeared.add(p); again = true; return; }
    // Anything else may move the compiler: the design tool is built again.
    if (!designOnly) buildTool();
    // What a full build reads from disk is what changed since it began.
    since = t;
    verifying = fast;
    const seqAtStart = fastSeq, hadError = error;
    const keep = (next, logic) => designOnly && design && next && logic === logicRevision
      && next.structure === design.structure && next.tokens.length === design.tokens.length
      && next.tokens.every((t, i) => t.name === design.tokens[i].name && String(t.accent) === String(design.tokens[i].accent));
    built = await build(app, dist, keep);
    verifying = false;
    // A value-only edit was sent while this built: what it built is behind
    // the pages, so it changes nothing and builds again.
    if (fast && fastSeq !== seqAtStart) {
      watchPackages(built.packages);
      // What it put in dist is published, though: new module code reloads
      // the pages, which take the design from the snapshot.
      if (!built.error && !built.kept) {
        logicRevision = built.logic;
        pending.set(++seq, at);
        push({ reload: seq, revision: logicRevision });
      }
      edited.add(file);
      again = false;
      rebuild();
      return;
    }
    error = built.error;
    // The design the pages run now (a scrub's preview, or its end, may have
    // come while it built).
    const from = scrubbed ?? design;
    watchPackages(built.packages);
    // A repair clears the page's error.
    if (!error && hadError) push({ error: null });
    if (error) {
      for (const p of pendingEdits) edited.add(p);
      for (const p of pendingAppeared) appeared.add(p);
      console.log(`build failed in ${Date.now() - t} ms; the page keeps the last good build\n${error}`); push({ error });
    }
    else if (built.kept) {
      const tokens = built.design.tokens.filter((t, i) => !same(t, from.tokens[i]));
      design = built.design;
      const committed = { kind: 'tamber-snapshot', structure: design.shape, base: null, design: design.digest, tokens: design.tokens };
      // While a scrub previews the pages stay on it: this changes only what
      // its end restores. A released scrub's build goes from it, and ends it.
      if (scrubbed && released) { scrubbed = null; aside = null; released = false; }
      if (scrubbed) aside = committed; else snapshot = committed;
      if (tokens.length && !scrubbed) {
        console.log(`edit → batch ready ${Date.now() - at} ms (built in ${Date.now() - t} ms) · ${tokens.length} token${tokens.length === 1 ? '' : 's'} to ${clients.size} page${clients.size === 1 ? '' : 's'}, no reload`);
        const batch = framed({ kind: 'tamber-batch', structure: design.shape, base: from.digest, design: design.digest, tokens });
        push(batch);
        // The one source of batches: the resident loop's devices and Linux
        // take them through the channel's polled streams.
        tamber?.channel.broadcast(batch);
      }
    }
    else {
      // A reload ends a scrub's preview: the pages boot the committed design.
      scrubbed = null; aside = null; released = false;
      const next = built.design ?? null;
      // The page reloads; a native build on the resident producer, whose
      // plan this edit may leave as it was (an accent's companion rules are
      // the JS target's own), takes the values as a batch when the design's
      // shape held, and the snapshot stays its to fetch.
      const shaped = from && next && from.shape === next.shape && from.tokens.length === next.tokens.length && next.tokens.every((tk, i) => tk.name === from.tokens[i].name);
      design = next;
      snapshot = shaped ? { kind: 'tamber-snapshot', structure: next.shape, base: null, design: next.digest, tokens: next.tokens } : null;
      const changedTokens = shaped ? next.tokens.filter((tk, i) => !same(tk, from.tokens[i])) : [];
      if (changedTokens.length) tamber?.channel.broadcast(framed({ kind: 'tamber-batch', structure: next.shape, base: from.digest, design: next.digest, tokens: changedTokens }));
      logicRevision = built.logic;
      pending.set(++seq, at);
      console.log(`edit → plan ready ${Date.now() - at} ms (rebuilt in ${Date.now() - t} ms) · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading`);
      warn(built.warnings);
      push({ reload: seq, revision: logicRevision });
    }
    if (again) { again = false; rebuild(); }
  };
  // A build reading a tree (the assets it copies) is reported too on macOS:
  // a path not modified since the last build started is no edit.
  const changed = (base, skip = skipped, fresh = true) => (_, name) => {
    // A Contract source the compile read is an input even where the app's
    // watcher skips (a dot directory).
    if (name && skip.test(String(name)) && !contractInput(resolve(base, String(name)))) return;
    // The declarations a build writes beside app.ts, for an editor.
    if (base === app.dir && String(name) === 'app.contract.d.ts') return;
    // A screenshot, film, log or note written into the app folder, outside the input trees and not named by
    // app.json, does not reload a page under a test (authoring bench).
    if (base === app.dir && OUTPUT.test(String(name)) && !INPUT_TREE.test(String(name)) && !JSON.stringify(app.manifest ?? {}).includes(String(name))) return;
    // A package's events are all edits: a link replaced by one to an older
    // file is newer than nothing, and its target's time says nothing of it.
    // The entry's own time (lstat): a link replaced by one to an older file
    // is new, though its target is not.
    try { if (fresh && name && lstatSync(resolve(base, String(name))).mtimeMs < since) return; } catch { /* removed: an edit */ }
    if (!timer) saved = Date.now();
    const path = resolve(base, String(name ?? ''));
    edited.add(path);
    if (existsSync(path)) appeared.add(path);
    clearTimeout(timer);
    // An editor's save is one burst of events, well inside 5 ms; an event
    // after the build starts builds again (`again`).
    timer = setTimeout(() => { timer = null; rebuild(); }, 5);
  };
  // The app's sources, and the runtime and compiler it builds with.
  const watchers = [watch(app.dir, { recursive: true }, changed(app.dir)), watch(resolve(root, 'host/web-js'), { recursive: true }, changed(resolve(root, 'host/web-js')))];
  for (const f of ['navigation.js', 'index.html', 'neutral.css', 'look.mjs']) watchers.push(watch(resolve(root, 'host/web', f), changed(resolve(root, 'host/web'))));
  // Each Contract package the app uses, wherever it is installed or linked.
  const packages = new Set();
  const contractSources = new Set();
  // A source, or a directory holding one (its event names the directory).
  const contractInput = path => contractSources.has(path) || [...contractSources].some(s => s.startsWith(path + '/') || s.startsWith(path + '\\'));
  const watchPackages = ({ trees = [], shallow = [], sources = [] } = {}) => {
    for (const source of sources) contractSources.add(source);
    for (const dir of trees) {
      if (packages.has(dir) || !existsSync(dir)) continue;
      packages.add(dir);
      // A package's dot directories are its sources too; only its own
      // node_modules is not.
      watchers.push(watch(dir, { recursive: true }, changed(dir, /(^|\/)node_modules(\/|$)/, false)));
    }
    // One entry of a directory: a node_modules not made yet, or an install
    // that is a link. Its making, or its retargeting, is an edit.
    for (const [dir, entry] of shallow) {
      const key = `shallow:${dir}:${entry}`;
      if (packages.has(key) || !existsSync(dir)) continue;
      packages.add(key);
      watchers.push(watch(dir, (_, name) => { if (String(name) === entry) changed(dir, /^$/, false)(_, name); }));
    }
  };
  watchPackages(built.packages);
  // The one TypeScript configuration app.ts is checked with (js/bake/src/typescript.mjs).
  watchers.push(watch(resolve(root, 'js/bake/src/typescript.mjs'), changed(resolve(root, 'js/bake/src'))));
  // The page's side: reload on a new build, the errors of a failed one in an
  // overlay, and a beacon when the reloaded page's runtime is up. Its build
  // channel (as host/web/dev.js's): a pin's scheme is the system's, a rule
  // beneath the app's `setScheme`; its status is what paints, sent again
  // once a turn when the app's own commands move it.
  const client = (n, revision) => `<script>(()=>{const seq=${n},logicRevision=${JSON.stringify(revision)},es=new EventSource('/__dev/page');let o,reloading=false;
const show=t=>{if(!t){o?.remove();o=null;return}o??=document.body.appendChild(Object.assign(document.createElement('pre'),{style:'position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647',onclick:()=>show()}));o.textContent=t+'\\n(click to dismiss)'};
const encode=v=>typeof v==='number'&&(!Number.isFinite(v)||Object.is(v,-0))?{$exactNumber:Object.is(v,-0)?'-0':String(v)}:Array.isArray(v)?v.map(encode):v;
const children=e=>[...(e?.children??[])].filter(x=>x.hasAttribute('data-carry-type')||x.hasAttribute('data-listitemkey'));
const capture=logic=>{const x=globalThis.exact,active=document.activeElement?.closest?.('[data-carry-type]');let focus=null;if(active){const path=[];for(let el=active,p;el&&el.id!=='exact-root';el=p){p=el.parentElement;path.unshift(children(p).indexOf(el))}if(!path.includes(-1))focus={path,type:active.getAttribute('data-carry-type')}}return{time:x.clock.now,slots:x.state[0].map(s=>[s.n.devName,s.n.devType,encode(s())]),answers:logic?[]:x.resources.filter(r=>!r.owned&&!r.failed&&!r.ticket&&!r.waiting&&!r.ov&&r.settled!==undefined).map(r=>[r.name,r.source,encode(r.settled),encode(r.value),r.devType,!!r.store]),carryAnswers:!logic,focus}};
const restored=document.querySelector('script[type="application/vnd.exact.dev-checkpoint"]'),q=new URLSearchParams(location.search),devKeys=new Set(['agent','seed','locale','timeZone','epoch','storage']),admission=q.has('agent')?[...q].filter(([k])=>devKeys.has(k)):null;
if(restored){const restoredSeq=Number(restored.dataset.seq);console.info('exact dev reload: restored',restoredSeq);const t=setInterval(()=>{const b=document.getElementById('exact-root')?.dataset.bootMs;if(b!=null){clearInterval(t);Promise.resolve(globalThis.exact?.ready).then(()=>{console.info('exact dev reload: runtime up',restoredSeq);fetch('/__dev/reloaded?seq='+restoredSeq+'&boot='+b+'&at='+Date.now(),{method:'POST'})})}},2)}
const ready=()=>globalThis.exact?Promise.resolve(globalThis.exact.ready):new Promise(ok=>{const t=setInterval(()=>{if(globalThis.exact){clearInterval(t);Promise.resolve(globalThis.exact.ready).then(ok)}},2)});
let liveQ=Promise.resolve();const live=m=>{liveQ=liveQ.then(async()=>{const x=await ready().then(()=>globalThis.exact),was=x?.designDigest;const out=x?.design?.(m);if(out?.live==='stale'){const r=await fetch('/__dev/design',{cache:'no-store'});if(r.status===200)x.design(await r.json())}if(x?.designDigest!==was&&ch.nonce)status()}).catch(e=>console.error('exact dev:',e))};
const ch={nonce:null,registered:null,revision:null,before:null,told:null},bid=(async()=>{const fresh=()=>'web-js-'+crypto.getRandomValues(new Uint32Array(2)).join('');let i;try{i=sessionStorage.getItem('exact.dev.build')}catch{return fresh()}const t=typeof BroadcastChannel==='function'?new BroadcastChannel('exact.dev.build'):null;if(i&&t){const mine=i;if(await new Promise(done=>{t.onmessage=e=>{if(e.data==='taken:'+mine)done(true)};t.postMessage('who:'+mine);setTimeout(()=>done(false),60)}))i=null}i??=fresh();if(t){const own=i;t.onmessage=e=>{if(e.data==='who:'+own)t.postMessage('taken:'+own)}}try{sessionStorage.setItem('exact.dev.build',i)}catch{}return i})(),post=(n,b)=>fetch('/__dev/report',{method:'POST',body:JSON.stringify({nonce:n,...b})}),rep=async b=>{const{nonce:n,registered:r}=ch;if(!n||!(await r)||ch.nonce!==n||ch.registered!==r)return;await post(n,b).catch(()=>{})};
const facts=()=>{const s=getComputedStyle(document.documentElement),cs=s.colorScheme,x=globalThis.exact;return{kind:'status',scheme:/dark/.test(cs)&&!/light/.test(cs)?'dark':/light/.test(cs)&&!/dark/.test(cs)?'light':x?.systemScheme??(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'),contrast:matchMedia('(prefers-contrast: more)').matches?'more':matchMedia('(prefers-contrast: less)').matches?'less':'no-preference',mode:x?.modes?.read?.()??null,rootFontSize:parseFloat(s.fontSize),plan:null,design:x?.designDigest??null,revision:ch.revision}},status=()=>{const b=facts();ch.told=JSON.stringify(b);rep(b)};for(const q of['(prefers-color-scheme: dark)','(prefers-contrast: more)','(prefers-contrast: less)'])matchMedia(q).addEventListener?.('change',()=>{if(ch.nonce)status()});
let due=false;const moved=()=>{if(due||!ch.nonce)return;due=true;setTimeout(()=>{due=false;const b=facts();if(JSON.stringify(b)!==ch.told){ch.told=JSON.stringify(b);rep(b)}},0)},seen=new MutationObserver(moved);seen.observe(document.documentElement,{attributes:true,attributeFilter:['style','data-x-modes']});seen.observe(document.head,{childList:true,subtree:true,characterData:true});
const pin=m=>{liveQ=liveQ.then(async()=>{await ready();const r=document.documentElement.style,p=m.pin,x=globalThis.exact;ch.before??={fontSize:r.fontSize,hostFont:r.getPropertyValue('--exact-host-font-size'),mode:x?.modes?.read?.()??{}};const sh=document.getElementById('exact-dev-pin')??document.head.appendChild(Object.assign(document.createElement('style'),{id:'exact-dev-pin'}));sh.textContent=p?.scheme?':root{color-scheme:'+p.scheme+'}':'';if((x.systemScheme??null)!==(p?.scheme??null)){x.systemScheme=p?.scheme??null;dispatchEvent(new Event('resize'))}r.fontSize=p?.rootFontSize?p.rootFontSize+'px':ch.before.fontSize;if(p?.rootFontSize)r.setProperty('--exact-host-font-size',r.fontSize);else if(ch.before.hostFont)r.setProperty('--exact-host-font-size',ch.before.hostFont);else r.removeProperty('--exact-host-font-size');const mode=Object.entries({...ch.before.mode,...(p?.mode??{})});if(mode.length&&x?.modes?.set){const why=x.modes.set(mode);if(why){rep({kind:'unsupported',what:'pin.mode',detail:why});x.modes.set(Object.entries(ch.before.mode))}}if(p?.contrast)rep({kind:'unsupported',what:'pin.contrast',detail:"a web page cannot pin the browser's contrast"});if(!p)ch.before=null;ch.revision=m.revision;status()}).catch(e=>console.error('exact dev:',e))};
es.onmessage=e=>{const m=JSON.parse(e.data);if(m.kind==='tamber-batch'||m.kind==='tamber-snapshot'){live(m);return}if(m.kind==='pin'){pin(m);return}if(m.kind==='inspect'||m.kind==='outline'||m.kind==='hold'){rep({kind:'unsupported',what:m.kind});return}if(m.nonce&&m.nonce!==ch.nonce){const n=ch.nonce=m.nonce;ch.registered=ready().then(async()=>(await post(n,{kind:'register',id:await bid,host:'web-js',capabilities:['pin'],plan:null,design:globalThis.exact?.designDigest??null})).ok).catch(()=>false);status()}if(m.error!==undefined)show(m.error);if(m.reload>seq&&!reloading){reloading=true;if(!globalThis.exact){location.reload();return}ready().then(async()=>{const checkpoint=capture(m.revision!==logicRevision),id=crypto.getRandomValues(new Uint32Array(4)).join('-');console.info('exact dev reload: checkpoint',m.reload);const saved=await fetch('/__dev/checkpoint?id='+id+'&seq='+m.reload+'&revision='+encodeURIComponent(m.revision??''),{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(checkpoint)});if(!saved.ok)throw new Error('checkpoint handoff answered '+saved.status);const next=new URL(location.href);next.searchParams.set('__exactDev',id);if(admission)for(const [k,v]of admission)if(!next.searchParams.has(k))next.searchParams.set(k,v);location.replace(next.href)}).catch(e=>{reloading=false;console.error('exact dev reload: current page retained; checkpoint handoff failed',e)})}}})()</script>`;
  const checkpoints = new Map();
  /** A scrub's design: the compiler's, with its text in place of the design file's; written nowhere. */
  const scrubDesign = designOf;
  /** Preview a scrub's texts: a batch from the design the pages run, as a
   * value-only edit sends one, and its snapshot for a page that asks. */
  async function scrubPreview(texts) {
    const file = relative(app.dir, designPath(app.dir));
    // Texts that leave the design file as it is: the committed design.
    if (!(file in texts)) { scrubRestore(); return {}; }
    const next = await scrubDesign(texts[file]);
    if (next.error) return next;
    const from = scrubbed ?? design;
    if (!from || from.shape !== next.shape || from.tokens.length !== next.tokens.length
      || next.tokens.some((t, i) => t.name !== from.tokens[i].name || String(t.accent) !== String(from.tokens[i].accent))) return { error: 'it changes the design\'s shape: a rebuild applies on release' };
    if (!scrubbed) aside = snapshot;
    // A new preview is no released scrub's.
    scrubbed = next;
    released = false;
    snapshot = { kind: 'tamber-snapshot', structure: next.shape, base: null, design: next.digest, tokens: next.tokens };
    const tokens = next.tokens.filter((t, i) => !same(t, from.tokens[i]));
    if (tokens.length) {
      const batch = framed({ kind: 'tamber-batch', structure: next.shape, base: from.digest, design: next.digest, tokens });
      push(batch);
      tamber?.channel.broadcast(batch);
    }
    return {};
  }
  /** A scrub ended without its write: the pages back on the committed design. */
  function scrubRestore() {
    released = false;
    if (!scrubbed || !design) return;
    const from = scrubbed;
    scrubbed = null;
    // The committed design's snapshot (a page fetching it applies nothing new).
    snapshot = aside ?? { kind: 'tamber-snapshot', structure: design.shape, base: null, design: design.digest, tokens: design.tokens };
    aside = null;
    const tokens = design.tokens.filter((t, i) => !same(t, from.tokens[i]));
    if (!tokens.length) return;
    const batch = framed({ kind: 'tamber-batch', structure: design.shape, base: from.digest, design: design.digest, tokens });
    push(batch);
    tamber?.channel.broadcast(batch);
  }
  // The install pages (LLP 1030.003 D6a) and, from a loopback page on a Mac,
  // the local iOS build (D6b): this server's, since only it sees the real
  // peer (a forwarded request's is loopback).
  const installer = localInstaller({ app: () => app, origins, port, gate, listener: { host, port } });
  // The design server's wire (LLP 1121 D13), when the app has a design.
  // `page`: this loop's own origin, which the design tool frames (LLP 1122 T2).
  tamber = designPath(app.dir) ? tamberServer({ appDir: app.dir, live: true, page: `http://127.0.0.1:${port}`, scrub: { preview: scrubPreview, restore: scrubRestore, released: () => { released = scrubbed != null; } } }) : null;
  // A polled stream (Linux, the resident loop's relay) is told the design to run.
  tamber?.channel.greet(() => snapshot && notice(snapshot));
  const server = createServer(async (req, res) => {
    // Every HTML answer is framed by the design tool only (LLP 1122 T2).
    frameOnlyByTool(res);
    const access = gate.check(req);
    if (!access.allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    const url = webRequestURL(req.url);
    if (!url) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    if (tamber && await tamber.handle(req, res, url, access)) return;
    if (await installer.handle(req, res, url, access)) return;
    // The build channel (LLP 1122 D4): the page's stream gives it a nonce;
    // reports come up by POST; a build with no stream here (the resident
    // loop's devices, Linux) opens a polled one.
    if (url.pathname === '/__dev/report' && req.method === 'POST') {
      if (!tamber) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
      await serveReport(tamber.channel, req, res);
      return;
    }
    if ((url.pathname === '/__dev/hello' || url.pathname === '/__dev/commands') && req.method === 'GET') {
      if (!tamber) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
      serveCommands(tamber.channel, url, res);
      return;
    }
    if (url.pathname === '/__dev/page') {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      const opened = tamber?.channel.open(frame => res.write(`data: ${JSON.stringify(frame)}\n\n`));
      req.on('close', () => opened?.close());
      res.write(`data: ${JSON.stringify({ ...(error ? { error } : {}), reload: seq, revision: logicRevision, ...(opened ? { nonce: opened.nonce } : {}) })}\n\n`);
      // The design the pages run, after value-only edits since the build.
      if (snapshot) res.write(`data: ${JSON.stringify(notice(snapshot))}\n\n`);
      clients.add(res);
      console.log(`page connected (${clients.size})`);
      req.on('close', () => clients.delete(res));
      return;
    }
    if (url.pathname === '/__dev/design' && (req.method === 'GET' || req.method === 'HEAD')) {
      if (!snapshot) { res.writeHead(204, { 'cache-control': 'no-store' }); res.end(); return; }
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
      res.end(req.method === 'HEAD' ? undefined : JSON.stringify(snapshot));
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
    const found = buildTreeFile(dist, url.pathname, req.headers.accept);
    // No build yet (the first failed): a blank page that shows the errors and reloads when one lands.
    if (!found && error && (url.pathname === '/' || appDocumentPath(url.pathname, acceptsHTML(req.headers.accept)))) { res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' }); res.end(`<!doctype html><meta charset="utf-8"><body>${client(seq, logicRevision)}`); return; }
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
    // A named port's next one (an assigned range holds both), else a free one.
    const probe = createServer();
    listenOn(probe, residentPort).then(() => {
      const internal = probe.address().port;
      probe.close(() => {
        console.log(`native client: starting the resident loop's producers (loopback :${internal})`);
        residentChild = spawn(process.execPath, [resolve(root, 'host/web/dev.mjs'), '--app', app.name, '--wasm', '--port', String(internal), '--serve-as', String(port), ...(lan ? ['--lan'] : []), ...allowHosts.flatMap((name) => ['--allow-host', name])],
          { cwd: root, env: { ...process.env, EXACT_WEB_DIST: resolve(app.target, 'web-dist-resident'), EXACT_LAUNCHER_PID: String(process.pid) }, stdio: ['ignore', 'pipe', 'inherit'] });
        let buf = '';
        residentChild.stdout.on('data', (d) => {
          buf += d; const lines = buf.split('\n'); buf = lines.pop();
          for (const l of lines) { console.log(`  [resident] ${l}`); if (/^(?:plan ready|module generation ready|Rust generation \w+ ready)/.test(l)) ok(internal); }
        });
        residentChild.on('exit', (code) => { resident = null; residentChild = null; fail(new Error(`the resident loop exited ${code}`)); });
      });
    }).catch(error => { resident = null; fail(error); });
  });
  const forward = (req, res) => {
    // A device gone ends its forwarded stream too (the resident's relay
    // stops), and one gone while the resident loop starts opens none.
    let out = null, gone = false;
    res.on('close', () => { gone = true; out?.destroy(); });
    residentLoop().then((internal) => {
      if (gone || res.destroyed) return;
      out = request({ host: '127.0.0.1', port: internal, method: req.method, path: req.url, headers: req.headers }, (answer) => { res.writeHead(answer.statusCode, answer.headers); answer.pipe(res); });
      out.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
      req.pipe(out);
    }, (e) => { if (gone) return; res.writeHead(503, { 'content-type': 'text/plain', 'cache-control': 'no-store' }); res.end(`${e.message}\n`); });
  };
  const stop = async () => {
    stopping = true;
    for (const w of watchers) w.close(); building?.kill('SIGKILL'); stopGroup(tool.child); stopGroup(tool.reading); residentChild?.kill('SIGTERM'); server.close();
    // The design server lets go of its lock before this exits.
    await tamber?.close();
    // A local iOS build in flight is stopped and waited for, as the resident loop does.
    const install = installer.child;
    if (install && install.exitCode === null && install.signalCode === null) { const exit = new Promise((ok) => install.once('exit', ok)); install.kill('SIGTERM'); await exit; }
    process.exit(0);
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  process.on('SIGHUP', stop);
  watchLauncher(stop);
  await new Promise((ok, fail) => { server.on('error', fail); server.listen(port, host, ok); })
    .catch((e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}${e.code === 'EADDRINUSE' ? ' (another dev loop? --port <n> picks another)' : ''}`); stopGroup(tool.child); process.exit(1); });
  const urls = origins.map((o) => `${o.origin}/`);
  console.log(urls.join('\n'));
  console.log(urls.map(url => `  Open in native: ${url}__dev/open`).join('\n'));
  if (allowHosts.length) console.log(`  also answering to ${allowHosts.join(', ')} (--allow-host)`);
  console.log(`  (dev loop on the JS target: ${app.dir.replace(root + '/', '')} and host/web-js rebuild and reload the page; a native client's requests go to the resident loop's producers, started at the first; ${lan ? 'LAN bind — any peer on this network can read the app and its compile errors' : 'loopback only — --lan to serve a phone on this network'}; ctrl-c to stop)`);
  await new Promise(() => {});
}
