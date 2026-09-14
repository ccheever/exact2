#!/usr/bin/env bun
// The resident dev loop: edit app.contract → the page shows it, no cargo
// build in the loop. Usage: bun host/web/dev.mjs [--app caltrain] [--port 8765] [--loopback]
//
// Binds the LAN by default (LLP 1023 D8) so a phone on the network can boot
// the plan from this URL; --loopback (or EXACT_LOOPBACK=1) restores
// 127.0.0.1 only. The agent carrier is not here and never binds the LAN.
//
// One Rust process (the app's `dev` bin, exact_web::dev) watches the source
// and writes each baked plan to dist/app.plan; this script serves dist/
// (index.html with dev.js added), pushes each ready plan to the page over
// server-sent events, and prints edit → present against the budget row
// "Dev restart, request to present: 100ms p50" (rules/RULES.md).
//
// The Rust side too (2026-08-30): an edit under the crates the wasm is
// built from — kernel, plan, motion, runner, host/web, gpu, the app's data,
// web, and gpu crates, the vendored Taffy — runs the same warm build
// (host/web/build.mjs), restarts the resident compiler (its plans must
// match the new format), and pushes a reload: a new wasm is a new program,
// so the page reloads rather than restarts in place. A build that fails
// shows its errors in the page's overlay, as a contract that fails does,
// and the page keeps the last good wasm. TypeScript apps use a checked
// resident producer below the data seam; file watching remains Node's own.
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { canonicalBytes, classifyArtifacts, cohortReceipt } from '../../scripts/deploy.mjs';
import { filesystem } from '../../scripts/filesystem.mjs';
import { developmentInstallPage, installNetworkPage, INSTALL_FILES, LOCAL_IOS_INSTALL_ENDPOINT } from '../../scripts/install-page.mjs';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, watch } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { resolve } from 'node:path';
import { rustPackage, rustOutput, rustInputs, rustCards } from '../../scripts/rust.mjs';
import { rustPolicy, rebuildPolicy } from '../../scripts/app.mjs';
import { developmentBuildEnv, developmentCandidate, pendingBuildInputs, readBuilds, resolveApp } from '../../scripts/app.mjs';
import { phones, simulators } from '../apple/build.mjs';
import { applyStaticChange, applyStaticTreeChange, builtAppMatches, developmentOpenPage, readDevGenerationAsync, readStaticFileAsync, reflectShaderFiles, retainDevGeneration, shaderInterfaceDigests, syncStaticTree, watchStaticTrees, webContentType, webEnvelope, MODULE_FILES, moduleCards } from './serve.mjs';

const argv = process.argv.slice(2);
const arg = (name, fallback) => { const i = argv.indexOf(name); return i >= 0 ? argv[i + 1] : fallback; };
const buildEnv = {...developmentBuildEnv(),EXACT_UPDATE_TRUST:'development'};
let app = resolveApp(arg('--app', undefined));
const port = Number(arg('--port', 8765));
const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
const host = loopback ? '127.0.0.1' : '0.0.0.0';
const privateAddress = address => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(address);
const lanAddresses = Object.values(networkInterfaces()).flat()
  .filter(address => address && !address.internal && address.family === 'IPv4')
  .map(address => address.address).sort((a, b) => privateAddress(b) - privateAddress(a));
const root = resolve(new URL('../..', import.meta.url).pathname);
const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(root, 'host/web/dist'));
const source = resolve(app.dir, 'app.contract');
let typescript = existsSync(resolve(app.dir, 'app.ts'));
let portableRust = Boolean(rustPackage(app)) && rustPolicy(app.manifest, 'web') !== 'off';
let rebuildOn = rebuildPolicy(app.manifest);
let manualTypescript = null, rustChild = null, rustActive = false, rustRun = 0, rustHeartbeat = null, rustDirty = false, rustSaved = 0, rustSourceWatch = null, rustOutputWatch = null;
let rustInputFiles = new Set();
let changed = new Set(), timer=null, building=false, again=false, builds=0;
const plan = resolve(dist, 'app.plan');
const graphPath = resolve(dist, 'bake.json');
buildEnv.EXACT_DEV_BAKE = graphPath;
if (!builtAppMatches(dist, app) || !existsSync(graphPath) || JSON.parse(readFileSync(graphPath,'utf8')).version!==1 || JSON.parse(readFileSync(graphPath,'utf8')).trust!=='development') {
  const b = spawnSync(process.execPath, [resolve(root, 'host/web/build.mjs'), app.crate('web')], { cwd: root, env:buildEnv, stdio: 'inherit' });
  if (b.status !== 0) process.exit(b.status ?? 1);
}
const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';

const assetTrees = [['assets', 'assets'], ['deck', 'deck'], ['gpu/shaders', 'shaders']].map(([from, to]) => [resolve(app.dir, from), to]);
const skipped = /(^|\/)(target|dist(?:\.previous)?|\.build|node_modules)(\/|$)/;
const shaderDigests = new Map();
const toolingEnv = { ...buildEnv, CARGO_TARGET_DIR: app.target };
const reflectBin = resolve(app.target, 'debug/exact-gpu-reflect');
function reflectShaders(tree) {
  if (!existsSync(reflectBin)) {
    const built = spawnSync('cargo', ['build', '-q', '-p', 'exact-gpu-reflect'], { cwd: root, env: toolingEnv, stdio: 'inherit' });
    if (built.status !== 0) throw new Error(`exact-gpu-reflect did not build (exit ${built.status ?? built.signal})`);
  }
  return shaderInterfaceDigests(tree, reflectBin);
}
// A completed build may predate a source-tree deletion or restoration. Mirror
// every declared tree before any compiler/server process starts. A truly
// absent root removes stale output; a dangling root link is a refusal. The
// complete shader candidate reflects before the old served tree is replaced.
for (const [from, to] of assetTrees) {
  let reflected = new Map();
  const present = syncStaticTree(from, resolve(dist, to), to === 'shaders' ? (candidate) => { reflected = reflectShaders(candidate); } : null);
  if (present && to === 'shaders') for (const [name, digest] of reflected) shaderDigests.set(name, digest);
}

const clients = new Set();
let seq = 0;
const pending = new Map(); // seq -> { saved, ready }
const push = (data) => { for (const res of clients) res.write(`data: ${JSON.stringify(data)}\n\n`); };
// A revision owns its plan and complete asset namespace. Its process epoch
// makes a restarted server's seq=1 newer than the previous server's seq=N.
const epoch = randomBytes(16).toString('hex');
const localInstallToken = randomBytes(32).toString('hex');
let localInstallChild = null;
let localInstallTargets = [], localInstallTargetsAt = 0, localInstallTargetError = null;
let localInstallState = { state: 'idle', message: 'Looking for an iOS Simulator or paired device…', log: '' };
const publicTarget = target => ({ id: target.id, kind: target.kind, name: target.name, model: target.model, os: target.os, state: target.state });
const simulatorOS = runtime => (/(?:^|\.)iOS-(\d+)-(\d+)(?:-(\d+))?$/.exec(runtime)?.slice(1).filter(Boolean).join('.') ?? runtime);
function refreshLocalInstallTargets(force = false) {
  if (process.platform !== 'darwin') {
    localInstallTargets = []; localInstallTargetError = 'Local iOS builds require a Mac running this development server.';
    return;
  }
  if (!force && Date.now() - localInstallTargetsAt < 3000) return;
  localInstallTargetsAt = Date.now();
  const found = [], errors = [];
  try { found.push(...phones().filter(device => device.reachable && device.paired).map(device => ({
    id: `device:${device.id}`, value: device.id, kind: 'device', name: device.name, model: device.model, os: device.os,
  }))); } catch (error) { errors.push(error.message); }
  try { found.push(...simulators().filter(device => /SimRuntime\.iOS/.test(device.runtime) && /^(iPhone|iPad)/.test(device.name)).map(device => ({
    id: `simulator:${device.udid}`, value: device.udid, kind: 'simulator', name: device.name,
    model: 'Simulator', os: simulatorOS(device.runtime), state: device.state,
  }))); } catch (error) { errors.push(error.message); }
  localInstallTargets = found.sort((a, b) => Number(b.state === 'Booted') - Number(a.state === 'Booted') || a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name));
  localInstallTargetError = found.length ? null : errors.join(' ');
}
function localInstallStatus(forceTargets = false) {
  if (localInstallState.state !== 'building') refreshLocalInstallTargets(forceTargets);
  let message = localInstallState.message;
  if (localInstallState.state === 'idle') {
    if (localInstallTargetError) message = localInstallTargetError;
    else if (!localInstallTargets.length) message = 'No available iOS Simulator or reachable paired device was found. Add a Simulator in Xcode, or unlock and pair a physical device.';
    else message = 'Ready to build locally. The first build can take a few minutes.';
  }
  return { ...localInstallState, message, available: process.platform === 'darwin' && !localInstallTargetError,
    targets: localInstallTargets.map(publicTarget) };
}
function appendLocalInstallLog(chunk) {
  localInstallState.log = (localInstallState.log + String(chunk)).replace(/\r/g, '').slice(-16000);
}
function startLocalInstall(targetId, requestOrigin) {
  if (localInstallChild) { const error = new Error('A local iOS build is already running.'); error.status = 409; throw error; }
  refreshLocalInstallTargets(true);
  if (localInstallTargetError) { const error = new Error(localInstallTargetError); error.status = 503; throw error; }
  const target = localInstallTargets.find(candidate => candidate.id === targetId);
  if (!target) { const error = new Error('That Simulator or device is no longer available. Refresh the target list.'); error.status = 400; throw error; }
  let appURL = new URL('/', requestOrigin);
  if (target.kind === 'device' && ['127.0.0.1', 'localhost', '[::1]'].includes(appURL.hostname)) {
    if (loopback || !lanAddresses.length) { const error = new Error('A physical device cannot reach this loopback-only server. Start it on the LAN and open the LAN install URL.'); error.status = 400; throw error; }
    appURL = new URL(`http://${lanAddresses[0]}:${port}/`);
  }
  localInstallState = { state: 'building', message: `Building ${app.displayName} for ${target.name}. This can take a few minutes…`, log: '', target: publicTarget(target), startedAt: new Date().toISOString() };
  const destination = target.kind === 'simulator' ? ['--ios', app.crate('apple'), '--sim', target.value] : ['--device', app.crate('apple'), '--phone', target.value];
  const child = localInstallChild = spawn(process.execPath, [resolve(root, 'host/apple/build.mjs'), ...destination, '--run', '--url', appURL.href], {
    cwd: root, env: { ...process.env, EXACT_APP_DIR: app.dir, EXACT_UPDATE_TRUST: 'development' }, stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', appendLocalInstallLog);
  child.stderr.on('data', appendLocalInstallLog);
  child.on('error', error => {
    if (localInstallChild !== child) return;
    localInstallChild = null;
    localInstallState = { ...localInstallState, state: 'failed', message: `The local build could not start: ${error.message}`, completedAt: new Date().toISOString() };
  });
  child.on('exit', (code, signal) => {
    if (localInstallChild !== child) return;
    localInstallChild = null;
    const failed = code !== 0;
    const detail = localInstallState.log.trim().split('\n').filter(Boolean).at(-1);
    localInstallState = { ...localInstallState, state: failed ? 'failed' : 'installed',
      message: failed ? `The local build failed${detail ? `: ${detail}` : ` (${signal ?? `exit ${code}`})`}` : `${app.displayName} was installed and opened on ${target.name}.`,
      completedAt: new Date().toISOString() };
  });
  return localInstallStatus();
}
const generationCache = resolve(root, 'target/dev-generations', createHash('sha256').update(canonicalBytes({ app: app.id, path: app.dir, dist })).digest('hex'));
let current = null;
let currentModule = null;
let currentRust = null;
let currentRustId = null;
let assetsNeedRebuild = false;
// This names the actual programs already served, including optional GPU code.
// A changed program stays terminal even when its compatibility metadata agrees.
const programIdentity = () => {
  const files = ['app.wasm', 'gpu_bg.wasm'].map((name) => {
    const encoded = filesystem({ op: 'get', root: dist, path: name });
    if (encoded === null && name === 'app.wasm') throw new Error('the app wasm is missing');
    return { name, sha256: encoded === null ? null : createHash('sha256').update(Buffer.from(encoded, 'base64')).digest('hex') };
  });
  return createHash('sha256').update(canonicalBytes({ files })).digest('hex');
};
let program = programIdentity();
const announcement = () => current ? {
  epoch, program, seq: current.seq, generation: current.generation,
  digest: current.envelope.plan.sha256, envelope: current.url,
} : { epoch, ready: false };
const hello = () => JSON.stringify({ hello: true, ...announcement() });
let retentionToken = null;
function captureGeneration(reuseCurrentAssets = false) {
  // A mixed app publishes one complete candidate. A producer may finish
  // first, but neither language may reset the other's last admitted module.
  if (typescript && portableRust && (!currentModule || !currentRust)) return;
  const encodedPlan = currentModule ? null : filesystem({ op: 'get', root: dist, path: 'app.plan' });
  if (!currentModule && encodedPlan === null) throw new Error('the plan is missing');
  const planBytes = currentModule?.get('app.plan') ?? currentRust?.get('app.plan') ?? Buffer.from(encodedPlan, 'base64');
  const files = currentModule ? new Map(currentModule) : new Map([['app.plan', planBytes]]);
  const module = currentModule ? moduleCards(files, app.id) : null;
  if (currentRust) for (const [name, body] of currentRust) {
    if (name === 'app.plan') continue;
    // A Contract/TS bake can reuse the exact accepted Rust artifact. Bind
    // its receipt to the new common plan; each client validates both
    // executors together before restart with carry.
    if (currentModule && name.endsWith('/app.module.json')) {
      const receipt = JSON.parse(body);
      receipt.plan = {file:'app.plan',bytes:planBytes.length,sha256:createHash('sha256').update(planBytes).digest('hex')};
      files.set(name, Buffer.from(JSON.stringify(receipt)));
    } else files.set(name, body);
  }
  const rust = currentRust ? rustCards(files) : null;
  const assets = [];
  // Logic edits reuse the last admitted static snapshot. Asset events capture
  // through owned filesystem reads again before publishing their own revision.
  if (reuseCurrentAssets && current) {
    for (const asset of current.envelope.assets) {
      const body = current.files.get(asset.name);
      files.set(asset.name, body);
      assets.push({ name: asset.name, sha256: asset.sha256, bytes: body.length });
    }
  } else {
    for (const tree of ['assets', 'deck', 'shaders']) {
      let captured;
      try { captured = filesystem({ op: 'tree', root: resolve(dist, tree) }); }
      catch (error) { if (error.code === 'ENOENT') continue; throw error; }
      for (const [relative, encoded] of Object.entries(captured)) {
        const name = `${tree}/${relative}`, body = Buffer.from(encoded, 'base64');
        files.set(name, body);
        assets.push({ name, sha256: createHash('sha256').update(body).digest('hex'), bytes: body.length });
      }
    }
  }
  assets.sort((a, b) => Buffer.compare(Buffer.from(a.name), Buffer.from(b.name)));
  if ([planBytes, ...files.values()].some((body) => body.length > 64 * 1024 * 1024)
    || [...files.values()].reduce((sum, body) => sum + body.length, 0) > 256 * 1024 * 1024) throw new Error('generation exceeds the payload budget');
  const envelope = webEnvelope(app, planBytes, assets);
  const generation = createHash('sha256').update(canonicalBytes({
    plan: { sha256: envelope.plan.sha256, bytes: planBytes.length }, assets, ...(module ? { module } : {}), ...(rust ? { rust: Object.fromEntries(Object.entries(rust).map(([k,v]) => [k, { module:{bytes:v.module.bytes,sha256:v.module.sha256},receipt:{bytes:v.receipt.bytes,sha256:v.receipt.sha256},...(k !== "wasm" ? {target:v.target} : {}) }])) } : {}),
  })).digest('hex');
  const prefix = `/__dev/generation/${epoch}/${seq}/`;
  envelope.dev = { epoch, program, seq, generation, events: '/__dev' };
  envelope.plan.url = prefix + 'app.plan';
  if (rust) envelope.rust = Object.fromEntries(Object.entries(rust).map(([kind, variant]) => [kind, { ...variant, receipt:{...variant.receipt,url:prefix+variant.receipt.url},module:{...variant.module,url:prefix+variant.module.url} }]));
  if (module) envelope.module = Object.fromEntries(Object.entries(module).map(([key, card]) => [key, { ...card, url: prefix + MODULE_FILES[key] }]));
  for (const asset of envelope.assets) asset.url = prefix + asset.name.split('/').map(encodeURIComponent).join('/');
  const envelopeBytes = Buffer.from(JSON.stringify(envelope) + '\n');
  if (envelopeBytes.length > 64 * 1024) throw new Error('generation envelope exceeds 64 KiB');
  files.set('exact.json', envelopeBytes);
  const retained = retainDevGeneration(generationCache, epoch, seq, files, undefined, retentionToken);
  retentionToken = retained.token;
  const revision = { epoch, seq, generation, envelope, files, prefix, url: prefix + 'exact.json' };
  const classification=classifyGeneration(planBytes,assets);
  current = revision;
  console.log(classification.map(line=>`  ${line}`).join('\n'));
}

// The resident compiler — started, and started again after a Rust rebuild.
let dev = null;
let announced = false;
function startCompiler() {
  if (portableRust) startRustCompiler();
  if (typescript) { startModuleCompiler(); return; }
  if (portableRust) return;
  dev = spawn('cargo', ['run', '-q', '--release', '-p', app.crate('web'), '--bin', 'dev', '--', source, plan], { cwd: app.workspace, env: buildEnv, stdio: ['ignore', 'pipe', 'inherit'], detached: true });
  const me = dev;
  console.log(`compiler pid ${dev.pid}`);
  let buffered = '';
  let first = true;
  dev.stdout.on('data', (chunk) => {
    if (dev !== me) return;
    buffered += chunk;
    const lines = buffered.split('\n');
    buffered = lines.pop();
    for (const line of lines) {
      const [kind, ...rest] = line.split(' ');
      if (kind === 'plan') {
        if (assetsNeedRebuild) continue;
        const [bytes, saved, compile, bake, ready] = rest.map(Number);
        seq += 1;
        if (!first) pending.set(seq, { saved, ready });
        try { captureGeneration(); } catch (error) { push({ error: `generation refused: ${error.message}` }); continue; }
        // The first ready plan completes discovery. Every subscriber reconciles
        // its full generation; only later saves contribute edit timings.
        if (first) { first = false; for (const res of clients) res.write(`data: ${hello()}\n\n`); if (!announced) { announced = true; console.log(`plan ready: ${bytes} bytes (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms) — edit ${source.replace(root + '/', '')} and watch`); } continue; }
        console.log(`edit → plan ready ${(ready - saved).toFixed(0)} ms (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms, ${bytes} bytes) · pushed to ${clients.size} page${clients.size === 1 ? '' : 's'}\n  contract → candidate plan ready`);
        push({ ...announcement(), bytes });
      } else if (kind === 'error') {
        console.log(`error: ${rest.join(' ')}`);
        push({ error: rest.join(' ') });
      }
    }
  });
  dev.on('exit', (code) => { if (dev === me) { console.error(`dev compiler exited ${code}`); process.exit(code ?? 1); } });
}
// Direct file watchers avoid recursive-directory event coalescing on macOS.
// The directory watcher discovers new paths; file metadata suppresses its
// delayed duplicate events. Bounds match the producer's byte limits, with a
// finite traversal/descriptor budget for the development watcher itself.
function watchModuleSources(directory, ignore, changed) {
  const files = new Map();
  let identity = '', refusal = null, closed = false, queued = false;
  const metadata = stat => [stat.dev,stat.ino,stat.size,stat.mtimeNs,stat.ctimeNs].join(':');
  const rescan = () => {
    if (closed || queued) return;
    queued = true;
    queueMicrotask(() => { queued = false; if (!closed) scan(true); });
  };
  function scan(notify) {
    const next = new Map(), state = [];
    let entries = 0, bytes = 0;
    try {
      const rootStat = lstatSync(directory);
      if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) throw new Error('module source root is not a regular directory');
      function walk(at, prefix = '', depth = 0) {
        if (depth > 64) throw new Error('module watcher source graph is too deep');
        for (const entry of readdirSync(at, { withFileTypes: true })) {
          const name = prefix + entry.name;
          if (ignore(name)) continue;
          if (++entries > 4096) throw new Error('module watcher source graph exceeds 4096 entries');
          const path = resolve(directory, name), stat = lstatSync(path, { bigint: true });
          if (stat.isSymbolicLink()) { state.push([name,'symlink']); continue; }
          if (stat.isDirectory()) { walk(path,name+'/',depth+1); continue; }
          if (!/\.(ts|contract|json)$/.test(name)) continue;
          if (!stat.isFile()) { state.push([name,'not-regular']); continue; }
          bytes += Number(stat.size);
          if (stat.size > 16n*1024n*1024n || bytes > 64*1024*1024 || next.size >= 2048) throw new Error('module watcher source graph exceeds its file/byte budget');
          const stamp = metadata(stat);
          next.set(path,{ stamp, inode:stat.dev+':'+stat.ino });
          state.push([name,stamp]);
        }
      }
      walk(directory);
      for (const [path, old] of files) if (!next.has(path) || next.get(path).inode !== old.inode) { old.watch.close(); files.delete(path); }
      for (const [path, record] of next) {
        const old = files.get(path);
        const handle = old?.watch ?? watch(path, rescan);
        if (!old) handle.on('error',()=>{handle.close();if(files.get(path)?.watch===handle)files.delete(path);rescan();});
        files.set(path,{ ...record,watch:handle });
      }
      const value = JSON.stringify(state.sort(([a],[b])=>a<b?-1:a>b?1:0));
      const different = value !== identity || refusal !== null;
      identity = value; refusal = null;
      if (notify && different) changed(null);
    } catch (error) {
      const different = refusal?.message !== error.message;
      refusal = error;
      if (notify && different) changed(error);
    }
  }
  const rootStat = lstatSync(directory);
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) throw new Error('module source root is not a regular directory');
  scan(false);
  const directoryWatch = watch(directory,{recursive:true},(_event,name)=>{ if(!name || !ignore(String(name))) rescan(); });
  directoryWatch.on('error',error=>{refusal=error;changed(error);});
  return { get error(){return refusal;}, close(){closed=true;directoryWatch.close();for(const file of files.values())file.watch.close();files.clear();} };
}
let moduleWatch = null, moduleTimer = null, moduleRun = 0, moduleStage = null, moduleSaved = 0;
function startModuleCompiler() {
  const built = spawnSync('cargo', ['build', '-q', '--release', '-p', 'exact-js-bake'], { cwd: root, env: toolingEnv, stdio: 'inherit' });
  if (built.status !== 0) throw new Error('the module producer did not build');
  const scratch = resolve(app.target, 'module-dev');
  mkdirSync(scratch, { recursive: true });
  const child = dev = spawn(resolve(app.target, 'release/exact-js-bake'), [app.dir, '--serve'], {
    cwd: root, env: toolingEnv, detached: true, stdio: ['pipe', 'pipe', 'pipe'],
  });
  console.log(`compiler pid ${child.pid}`);
  let active = null, buffered = '', errors = '';
  const produce = () => {
    clearImmediate(moduleTimer);
    if (dev !== child || active || moduleWatch?.error) return;
    const started = Date.now(), stage = mkdtempSync(resolve(scratch, 'candidate-'));
    moduleStage = stage;
    active = { id: moduleRun, started, saved: moduleSaved || started, stage, output: resolve(stage, 'generation') };
    child.stdin.write(JSON.stringify({ id: active.id, out: active.output }) + '\n');
  };
  manualTypescript = () => { moduleRun++; moduleSaved = Date.now(); produce(); };
  child.stderr.on('data', chunk => {  errors = (errors + chunk).slice(-65536); });
  child.stdout.on('data', chunk => {
    if (dev !== child) return;
    buffered += chunk;
    if (buffered.length > 1024 * 1024) {
      console.error('module producer response exceeds 1 MiB');
      killCompiler(); process.exit(1);
    }
    const lines = buffered.split('\n'); buffered = lines.pop();
    for (const line of lines) {
      const request = active, produced = Date.now();
      try {
        const reply = JSON.parse(line);
        if (!request || reply.id !== request.id) throw new Error('unexpected module producer response');
        // An edit arriving during compilation supersedes its entire candidate.
        if (request.id !== moduleRun) continue;
        if (!reply.ok) throw new Error(reply.error || 'module producer refused the candidate');
        const candidate = new Map(['app.plan', ...Object.values(MODULE_FILES)].map(name => [name, readFileSync(resolve(request.output, name))]));
        moduleCards(candidate, app.id);
        const previous = currentModule;
        currentModule = candidate;
        seq++;
        try { captureGeneration(true); } catch (error) { currentModule = previous; throw error; }
        if (previous) pending.set(seq, { saved: request.saved, ready: Date.now() });
        console.log(`module generation ready in ${Date.now() - request.started} ms (producer ${produced-request.started} ms, publish ${Date.now()-produced} ms); restart with carry, no native rebuild`);
        push(announcement());
      } catch (error) { console.error(error.message); push({ error: error.message }); }
      finally {
        if (request) rmSync(request.stage, { recursive: true, force: true });
        active = null; moduleStage = null;
        if (request && request.id !== moduleRun) produce();
      }
    }
  });
  child.on('error', error => { console.error(`module producer: ${error.message}`); killCompiler(); process.exit(1); });
  child.stdin.on('error', error => { if (dev === child) console.error(`module producer input: ${error.message}`); });
  child.on('exit', code => {
    if (dev !== child) return;
    console.error(`module producer exited ${code}: ${errors}`);
    killCompiler(); process.exit(code || 1);
  });
  moduleWatch = watchModuleSources(app.dir, name => skipped.test(name) || /(^|\/)\./.test(name)
    || assetTrees.some(([tree]) => resolve(app.dir,name) === tree || resolve(app.dir,name).startsWith(tree+'/')), error => {
    moduleRun++; moduleSaved = Date.now(); clearImmediate(moduleTimer);
    if (error) { console.error(error.message); push({error:error.message}); return; }
    if (rebuildOn.typescript === "save") moduleTimer = setImmediate(produce);
  });
  if (moduleWatch.error) { console.error(moduleWatch.error.message); push({error:moduleWatch.error.message}); }

  produce();
}
function readRustGeneration() {
  if (building || changed.size) return;
  const directory = rustOutput(app), id = readFileSync(resolve(directory, 'current'), 'utf8');
  if (!/^[0-9a-f]{64}$/.test(id)) throw new Error('invalid Rust generation pointer');
  if (id === currentRustId && current) return;
  const candidate = new Map();
  const walk = (dir, prefix = '') => {
    for (const entry of readdirSync(dir, {withFileTypes:true})) {
      const name = prefix + entry.name;
      if (entry.isSymbolicLink()) throw new Error('Rust generation contains a link');
      if (entry.isDirectory()) walk(resolve(dir, entry.name), name + '/');
      else candidate.set(name, readFileSync(resolve(dir, entry.name)));
    }
  };
  walk(resolve(directory, id));
  if (!rustCards(candidate)) throw new Error('Rust generation contains no module');
  const previous = currentRust; currentRust = candidate; seq++;
  try { captureGeneration(true); } catch (error) { currentRust = previous; throw error; }
  currentRustId = id;
  rustInputFiles = new Set(rustInputs(app, buildEnv, {reloadOnly:true}));
  watchCompilerInputs();
  pending.set(seq, {saved:rustSaved || Date.now(),ready:Date.now()});
  push(announcement());
  console.log(`Rust generation ${id.slice(0,12)} ready; restart with carry`);
}
function produceRust() {
  if (building) { rustDirty = true; again = true; return; }
  if (changed.size) { rebuild(); return; }
  if (rustActive) { rustDirty = true; return; }
  if (!rustChild) {
    const child=rustChild=spawn(process.execPath,[resolve(root,'scripts/rust.mjs'),app.name,'--serve'],{cwd:root,env:buildEnv,detached:true,stdio:['pipe','pipe','pipe']});
    let buffer='';
    const failed=error=>{
      if(rustChild!==child)return;
      rustChild=null;rustActive=false;clearInterval(rustHeartbeat);rustHeartbeat=null;
      console.error(error.message);push({error:error.message});
      try{process.kill(-child.pid,'SIGTERM');}catch{}
    };
    child.on('error',failed);
    child.on('exit',(code,signal)=>failed(new Error(`Rust producer exited (${code??signal})`)));
    child.stdin.on('error',failed);
    child.stderr.on('data',data=>process.stderr.write(data));
    child.stdout.on('data',data=>{
      buffer+=data;
      let end;
      while((end=buffer.indexOf('\n'))>=0) {
        const line=buffer.slice(0,end);buffer=buffer.slice(end+1);
        try {
          const reply=JSON.parse(line);
          if(reply.id!==rustRun||!rustActive||typeof reply.ok!=='boolean')throw new Error('invalid Rust producer reply');
          rustActive=false;clearInterval(rustHeartbeat);rustHeartbeat=null;
          if(!reply.ok){console.error(reply.error);push({error:reply.error});}
          if(rustDirty && rebuildOn.rust==='save')produceRust();
        } catch(error){failed(error);return;}
      }
    });
  }
  rustDirty=false;rustActive=true;rustRun++;
  const started=Date.now();
  console.log(`Rust build ${rustRun} started; the current generation stays active`);
  rustHeartbeat=setInterval(()=>console.log(`Rust build ${rustRun} still running (${Math.round((Date.now()-started)/1000)} s); waiting for compiler/baker`),10000);
  rustChild.stdin.write(JSON.stringify({id:rustRun})+'\n');
}
function startRustCompiler() {
  rustInputFiles = new Set(rustInputs(app, buildEnv, {reloadOnly:true}));
  const directory = rustOutput(app); mkdirSync(directory, {recursive:true});
  rustOutputWatch = watch(directory, (_event,name) => {
    if (name !== 'current') return;
    try { readRustGeneration(); } catch (error) { console.error(error.message); push({error:error.message}); }
  });
  rustSourceWatch = watchModuleSources(app.dir, name => skipped.test(name) || /(^|\/)\./.test(name)
    || /\.(ts|json)$/.test(name)
    || assetTrees.some(([tree]) => resolve(app.dir,name).startsWith(tree+'/')), error => {
    if (error) { push({error:error.message}); return; }
    // The TS producer owns mixed Contract edits. If a Contract changes
    // during a Rust bake, its before/after guard will refuse that bake;
    // remember to build the latest snapshot once the in-flight job ends.
    if (typescript) { if (rustActive) rustDirty = true; return; }
    rustSaved = Date.now(); rustDirty = true;
    if (rebuildOn.rust === 'save') { clearTimeout(timer); timer=setTimeout(produceRust,200); }
  });
  produceRust();
}
const killCompiler = () => {
  manualTypescript = null;
  rustSourceWatch?.close(); rustSourceWatch = null; rustOutputWatch?.close(); rustOutputWatch = null;
  clearInterval(rustHeartbeat); rustHeartbeat = null; rustActive = false;
  const rust = rustChild; rustChild = null; if (rust) { try { process.kill(-rust.pid, 'SIGKILL'); } catch {} }

  moduleWatch?.close(); moduleWatch = null; clearImmediate(moduleTimer); moduleRun++;
  const d = dev; dev = null; if (d) { try { process.kill(-d.pid, 'SIGKILL'); } catch {} }
  if (moduleStage) { rmSync(moduleStage, { recursive: true, force: true }); moduleStage = null; }
};
startCompiler();
const stop = () => { killCompiler(); localInstallChild?.kill('SIGTERM'); process.exit(0); };

// The asset row (LLP 1030 D10; 1030.000 stage 1): an edit to an image, a
// font, a deck page, or a shader under the app's `assets/`, `deck/`, or
// `gpu/shaders/` is one digest — the file is mirrored into dist/ (what the
// page and a native client fetch), `{seq}` names the changed digests, and
// each client re-renders what referenced it, carrying state. A shader is
// classified by its interface digest (1030 D8): unchanged, it is an asset
// the client validates and swaps in; changed, it is a rebuild of the native
// host — and the wasm here, since the surfaces' Rust binds the new layout.
let assetChanges = new Map(); // dist-relative name -> { root, relative }
let assetTimer = null;
try {
  watchStaticTrees(app.dir, assetTrees, (change) => {
    if (skipped.test(change.relative) || /(^|\/)\./.test(change.relative)) return;
    if (change.tree) {
      for (const name of assetChanges.keys()) if (name === change.targetRoot || name.startsWith(`${change.targetRoot}/`)) assetChanges.delete(name);
      assetChanges.set(change.targetRoot, change);
    } else if (!assetChanges.has(change.targetRoot)) assetChanges.set(change.name, change);
    clearTimeout(assetTimer);
    assetTimer = setTimeout(pushAssets, 100);
  });
} catch (e) { console.error(`cannot watch static trees under ${app.dir}: ${e.message}`); }
function pushAssets() {
  const edits = [...assetChanges]; assetChanges = new Map();
  const rows = [];
  const carriers = [];
  let needsRebuild = false;
  for (const [name, source] of edits) {
    const target = resolve(dist, name);
    if (source.tree) {
      let nextDigests = new Map();
      try {
        const change = applyStaticTreeChange(source.root, target,
          source.targetRoot === 'shaders' ? (candidate) => { nextDigests = reflectShaders(candidate); } : null);
        for (const file of change.files) {
          const changedName = `${source.targetRoot}/${file.name}`;
          const shader = changedName.startsWith('shaders/') && changedName.endsWith('.wgsl');
          const stem = shader ? changedName.slice('shaders/'.length, -'.wgsl'.length) : null;
          if (file.removed) {
            rows.push({ name: changedName, removed: true });
            carriers.push(`asset ${changedName} removed`);
            continue;
          }
          const row = { name: changedName, sha256: createHash('sha256').update(file.bytes).digest('hex'), bytes: file.bytes.length };
          if (shader) {
            const digest = nextDigests.get(stem);
            const before = shaderDigests.get(stem);
            row.interface = digest;
            if (before === digest) carriers.push(`asset ${changedName} → live on the web, macOS, iOS (the client validates it)`);
            else { needsRebuild = true; carriers.push(`shader ${changedName}: interface ${before ?? '?'} → ${digest} — rebuild the native host; the wasm rebuilds now`); }
          } else carriers.push(`asset ${changedName} → live on the web, macOS, iOS`);
          rows.push(row);
        }
        if (source.targetRoot === 'shaders') {
          shaderDigests.clear();
          for (const [stem, digest] of nextDigests) shaderDigests.set(stem, digest);
        }
      } catch (error) {
        const reason = error.message || String(error);
        carriers.push(`asset ${name}: rejected — ${reason}; keeping the last good bytes`);
        push({ error: `${name}: ${reason}` });
      }
      continue;
    }
    const shader = name.startsWith('shaders/') && name.endsWith('.wgsl');
    let digest = null;
    let bytes;
    try {
      const change = applyStaticChange(source.root, source.relative, target, shader ? (candidate) => {
        if (!existsSync(reflectBin)) reflectShaders(resolve(app.dir, 'gpu/shaders'));
        digest = reflectShaderFiles([candidate], reflectBin).values().next().value;
      } : null);
      bytes = change.bytes;
      if (change.removed) {
        for (const suffix of change.removedFiles) {
          const removedName = suffix ? `${name}/${suffix}` : name;
          if (removedName.startsWith('shaders/') && removedName.endsWith('.wgsl')) {
            shaderDigests.delete(removedName.slice('shaders/'.length, -'.wgsl'.length));
          }
          rows.push({ name: removedName, removed: true });
          carriers.push(`asset ${removedName} removed`);
        }
        continue;
      }
    } catch (error) {
      const reason = error.message || String(error);
      carriers.push(`asset ${name}: rejected — ${reason}; keeping the last good bytes`);
      push({ error: `${name}: ${reason}` });
      continue;
    }
    const row = { name, sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length };
    if (shader) {
      const stem = name.slice('shaders/'.length, -'.wgsl'.length);
      const before = shaderDigests.get(stem);
      shaderDigests.set(stem, digest);
      row.interface = digest;
      if (before === digest) carriers.push(`asset ${name} → live on the web, macOS, iOS (the client validates it)`);
      else { needsRebuild = true; carriers.push(`shader ${name}: interface ${before ?? '?'} → ${digest} — rebuild the native host; the wasm rebuilds now`); }
    } else {
      carriers.push(`asset ${name} → live on the web, macOS, iOS`);
    }
    rows.push(row);
  }
  if (!rows.length) return;
  if (needsRebuild) {
    assetsNeedRebuild = true;
    for (const [name] of edits) changed.add(name);
    clearTimeout(timer); timer = setTimeout(rebuild, 200);
    return;
  }
  if (assetsNeedRebuild) return;
  seq += 1;
  try { captureGeneration(); } catch (error) { push({ error: `generation refused: ${error.message}` }); return; }
  console.log(`edit → assets ${rows.map((r) => r.name).join(', ')} · pushed to ${clients.size} page${clients.size === 1 ? '' : 's'}\n  ${carriers.filter(c=>c.includes('rejected')).join('\n  ')}`);
  push({ ...announcement(), changes: rows });
}

// Classification follows the actual producers. Native builds remain frozen
// until rebuilt; changed compiler inputs are reported as pending, never as
// a guessed target or compatibility id. @ref LLP 1030 D3; 1030.000 D5.
let builtReceipts=readBuilds(app,buildEnv);
const nativePending=new Map();
function refreshNativePending(){nativePending.clear();for(const r of builtReceipts)if(r.compat.inputs.platform!=='web')nativePending.set(r.compat.target+'/'+r.compat.inputs.platform,pendingBuildInputs(r));}
refreshNativePending();
let previousWeb=cohortReceipt(JSON.parse(readFileSync(graphPath,'utf8')));
function classifyGeneration(planBytes, assets) {
  if (currentModule || currentRust) return ['plan/module/assets: development candidate; each client verifies its admitted module identity and grants (not signed deployment classification)'];
  const web=JSON.parse(readFileSync(graphPath,'utf8'));
  const candidate=developmentCandidate(web,{sha256:createHash('sha256').update(planBytes).digest('hex'),bytes:planBytes.length},assets,shaderDigests);
  const lines=[];
  for(const platform of ['web','macos','ios','linux']) {
    const receipts=platform==='web'?[web]:builtReceipts.filter(r=>r.compat.inputs.platform===platform);
    if(!receipts.length){lines.push(`${platform}: unbuilt; no actual target/grants receipt`);continue;}
    for(const build of receipts) {
      const cohort=platform==='web'?previousWeb:cohortReceipt(build);
      const changes=platform==='web'?[]:nativePending.get(build.compat.target+'/'+platform)??[];
      const checked=classifyArtifacts({...candidate,binary:build.binary,pendingInputs:changes},cohort);
      if(platform==='web')lines.push(`web: origin; ${checked.binary?'new program, reload':'plan/assets, restart with carry'}`);
      else lines.push(`${platform} ${build.compat.target} ${build.compat.id.slice(0,8)}: ${checked.bundle?'bundle candidate':checked.missing.join('; ')}${changes.length?`; binary inputs changed (${changes.slice(0,3).join(', ')}); rebuild to complete classification`:''}`);
      lines.push(...checked.warnings);
    }
  }
  return lines;
}
function classifyRebuild() {
  builtReceipts=readBuilds(app,buildEnv);
  refreshNativePending();
  const web=JSON.parse(readFileSync(graphPath,'utf8'));
  const check=classifyArtifacts(web,previousWeb);
  previousWeb=cohortReceipt(web);
  watchCompilerInputs();
  return [`web: actual binary inputs ${check.binary?'changed':'unchanged'}; cohort ${web.compat.id}`, ...builtReceipts.filter(r=>r.compat.inputs.platform!=='web').map(r=>`${r.compat.inputs.platform}: ${nativePending.get(r.compat.target+'/'+r.compat.inputs.platform)?.length?'binary inputs changed; rebuild the actual target':'loaded inputs unchanged'} (${r.compat.target})`)];
}
const watched=new Map();
let compilerInputFiles = new Set(), compilerInputTrees = [], compilerMissingInputs = [], swiftSourceDirectories = new Set();
function watchCompilerInputs() {
  // Watching source directories also catches newly added modules after their
  // declaring file changes. Generated output and third-party caches never
  // cause build loops; their source declarations remain in the receipt.
  const files=new Set([...builtReceipts.flatMap(r=>r.binary.inputs.map(f=>f.path)),...rustInputFiles].filter(p=>!skipped.test(p)&&!p.includes('/.cargo/')));
  files.add(resolve(app.dir,'app.json'));
  compilerInputFiles = files;
  compilerInputTrees = builtReceipts.flatMap(r=>r.binary.directories.map(d=>d.path));
  compilerMissingInputs = builtReceipts.flatMap(r=>r.binary.missing);
  swiftSourceDirectories = new Set([...files].filter(path=>path.endsWith('.swift')).map(path=>resolve(path,'..')));
  const directories=new Set([...files].map(path=>resolve(path,'..')));
  for(const receipt of builtReceipts) {
    for(const {path} of receipt.binary.directories)directories.add(path);
    for(const missing of receipt.binary.missing) {
      let dir=resolve(missing,'..');while(!existsSync(dir)&&resolve(dir,'..')!==dir)dir=resolve(dir,'..');
      directories.add(dir);
    }
  }
  for(const dir of directories) {
    if(skipped.test(dir)||dir.includes('/.cargo/')||watched.has(dir)||!existsSync(dir))continue;
    try {watched.set(dir,watch(dir,(_event,name)=>{
      if(!name||skipped.test(name)||/(^|\/)\./.test(name)||name.endsWith('dev.js')||assetTrees.some(([tree])=>resolve(dir,name).startsWith(tree+'/'))||resolve(dir,name)===source)return;
      // Parent-directory notifications include unrelated documents and output.
      // Only receipt inputs, declared trees/missing paths, and Swift's implicit
      // source discovery can invalidate the host. Rust additions are reached
      // when their declaring module or build input changes.
      const path = resolve(dir, name);
      if (!compilerInputFiles.has(path) && !compilerInputTrees.some(tree=>path===tree||path.startsWith(tree+'/'))
        && !compilerMissingInputs.some(missing=>path===missing||missing.startsWith(path+'/'))
        && !(name.endsWith('.swift') && swiftSourceDirectories.has(dir))) return;
      if (typescript && resolve(dir, name).startsWith(app.dir + '/') && /\.(ts|contract)$/.test(name)) return;
      if (portableRust && rustInputFiles.has(resolve(dir,name))) { rustSaved=Date.now();rustDirty=true;if(rebuildOn.rust==='save'){clearTimeout(timer);timer=setTimeout(produceRust,200);}return; }
      changed.add(resolve(dir,name));console.log(`edit ${resolve(dir,name)} → build pending; classification follows its receipt`);
      if(rebuildOn.rust==='save'){clearTimeout(timer);timer=setTimeout(rebuild,200);}
    }));}catch(error){console.error(`cannot watch ${dir}: ${error.message}`);}
  }
}
watchCompilerInputs();
process.stdin.setEncoding('utf8');
process.stdin.on('data', input => {
  for (const command of input.trim().split(/\s+/)) {
    if (command === 't') manualTypescript?.();
    if (command === 'r') { if (portableRust && !changed.size) produceRust(); else rebuild(); }
  }
});
console.log(`rebuild: Rust ${rebuildOn.rust}, TypeScript ${rebuildOn.typescript}; r + Enter builds Rust, t + Enter builds TypeScript`);

function rebuild() {
  if (building) { again = true; return; }
  building = true;
  const files = [...changed]; changed = new Set();
  const t = Date.now();
  console.log(`rust: ${files.length} file${files.length === 1 ? '' : 's'} changed (${files.slice(0, 3).join(', ')}${files.length > 3 ? ', …' : ''}) — rebuilding the wasm`);
  const b = spawn(process.execPath, [resolve(root, 'host/web/build.mjs'), app.crate('web')], { cwd: root, env:buildEnv, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  b.stdout.on('data', (d) => { out += d; });
  b.stderr.on('data', (d) => { out += d; });
  b.on('exit', (code) => {
    building = false;
    const ms = Date.now() - t;
    if (code === 0) {
      builds += 1;
      // The compiler's plans must match the wasm's format: it is built again
      // too (cargo, warm), and its first plan reaches the reloaded page.
      killCompiler();
      app = resolveApp(arg('--app', undefined));
      typescript = existsSync(resolve(app.dir, 'app.ts'));
      portableRust = Boolean(rustPackage(app)) && rustPolicy(app.manifest, 'web') !== 'off';
      rebuildOn = rebuildPolicy(app.manifest);
      current = null; currentModule = null; currentRust = null; currentRustId = null; assetsNeedRebuild = false;
      program = programIdentity();
      // The restarted producer consumes module edits; queued core edits start
      // their rebuild there. Do not re-arm a third build below on that success.
      again = false;
      startCompiler();
      console.log(`rust: rebuilt in ${(ms / 1000).toFixed(1)} s · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading\n  ${classifyRebuild().join('\n  ')}`);
      push({ rebuilt: builds });
    } else {
      const errors = out.split('\n').filter((l) => /^(error|warning: unused|\s+-->)/.test(l)).join('\n') || out.trim().split('\n').slice(-12).join('\n');
      console.log(`rust: build failed in ${(ms / 1000).toFixed(1)} s\n${errors}`);
      push({ error: `the wasm did not build:\n${errors}` });
    }
    if (again) { again = false; rebuild(); }
  });
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://x');
  const devBeacon = url.pathname === '/__dev/reloaded' || url.pathname === '/__dev/painted';
  const localInstall = url.pathname === LOCAL_IOS_INSTALL_ENDPOINT;
  if (req.method !== 'GET' && req.method !== 'HEAD' && !(req.method === 'POST' && (devBeacon || localInstall))) { res.writeHead(405); res.end(); return; }
  if (localInstall) {
    const json = (status, body) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(JSON.stringify(body) + '\n'); };
    if (req.headers['x-exact-install-token'] !== localInstallToken) { json(404, { message: 'Not found.' }); return; }
    if (req.method === 'GET' || req.method === 'HEAD') {
      const body = localInstallStatus(url.searchParams.get('refresh') === '1');
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
      res.end(req.method === 'HEAD' ? undefined : JSON.stringify(body) + '\n');
      return;
    }
    try {
      const origin = new URL(req.headers.origin);
      if (!['http:', 'https:'].includes(origin.protocol) || origin.host !== req.headers.host) { const error = new Error('The install request must come from this development server.'); error.status = 403; throw error; }
      if (!/^application\/json(?:\s*;|$)/i.test(req.headers['content-type'] ?? '')) { const error = new Error('The install request must be JSON.'); error.status = 415; throw error; }
      let size = 0, encoded = '';
      for await (const chunk of req) {
        size += chunk.length;
        if (size > 1024) { const error = new Error('The install request is too large.'); error.status = 413; throw error; }
        encoded += chunk;
      }
      const body = JSON.parse(encoded || '{}');
      if (typeof body.target !== 'string' || body.target.length > 128) { const error = new Error('Choose an available iOS Simulator or paired device.'); error.status = 400; throw error; }
      json(202, startLocalInstall(body.target, origin));
    } catch (error) {
      json(error.status ?? 400, { message: error instanceof SyntaxError ? 'The install request must be JSON.' : error.message });
    }
    return;
  }
  if (url.pathname === '/__dev/open') {
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store', 'referrer-policy': 'no-referrer' });
    res.end(req.method === 'HEAD' ? undefined : developmentOpenPage(app));
    return;
  }
  if (url.pathname.startsWith('/__dev/generation/')) {
    // These are the same immutable bytes admitted to the retained cache before
    // publication. Keep current requests on that snapshot; older generations
    // still use the verified disk reader, including after server restart.
    let retained;
    if (current && url.pathname.startsWith(current.prefix)) {
      try {
        const name = decodeURIComponent(url.pathname.slice(current.prefix.length));
        const body = current.files.get(name);
        if (body) retained = { name, body };
      } catch { /* malformed URL */ }
    } else retained = await readDevGenerationAsync(generationCache, url.pathname);
    if (!retained) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
    const { name, body } = retained;
    res.writeHead(200, { 'content-type': name === 'exact.json' ? 'application/vnd.exact.envelope+json' : webContentType('/' + name), 'cache-control': 'no-store' });
    res.end(req.method === 'HEAD' ? undefined : body);
    return;
  }
  if (url.pathname === '/__dev') {
    res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
    res.write(':\n\n');
    res.write(`data: ${hello()}\n\n`);
    clients.add(res);
    console.log(`page connected (${clients.size})`);
    req.on('close', () => clients.delete(res));
    return;
  }
  if (url.pathname === '/__dev/reloaded') {
    const n = Number(url.searchParams.get('seq'));
    const p = url.searchParams.get('epoch') === epoch ? pending.get(n) : null;
    if (p) {
      const total = Number(url.searchParams.get('dom')) - p.saved;
      console.log(`  → page: fetch ${url.searchParams.get('fetch')} ms, restart ${url.searchParams.get('boot')} ms; edit → first frame in the DOM ${total.toFixed(0)} ms (budget ${budget})${total > 100 ? '  OVER BUDGET' : ''}`);
      console.log(`reloaded seq=${n} total_ms=${total.toFixed(0)}`);
    }
    res.writeHead(204); res.end();
    return;
  }
  if (url.pathname === '/__dev/painted') {
    const p = url.searchParams.get('epoch') === epoch ? pending.get(Number(url.searchParams.get('seq'))) : null;
    if (p) console.log(`  → painted ${(Number(url.searchParams.get('paint')) - p.saved).toFixed(0)} ms after the save`);
    res.writeHead(204); res.end();
    return;
  }
  // The live envelope (LLP 1023 D2): the static exact.json in dist/ plus the
  // dev tier — seq and the events stream. Built from the plan bytes so it can
  // never go stale against what the resident compiler last wrote; the header
  // offsets are the generated encoder's (plan/build.rs, little-endian).
  // Served at /exact.json, and — Stage 2's negotiation — for a GET of the
  // app URL itself whose Accept names the envelope type: the dev-server
  // shortcut past the link rung (D1); a browser never sends it.
  const wantsEnvelope = url.pathname === '/exact.json'
    || (url.pathname === '/' && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json'));
  if (wantsEnvelope) {
    try {
      if (!current) throw new Error('no current generation');
      res.writeHead(200, { 'content-type': 'application/vnd.exact.envelope+json', vary: 'Accept', 'cache-control': 'no-store' });
      res.end(req.method === 'HEAD' ? undefined : current.files.get('exact.json'));
    } catch { res.writeHead(404); res.end(); }
    return;
  }
  const file = url.pathname === '/' ? '/index.html' : url.pathname;
  if (file === '/dev.js') { res.writeHead(200, { 'content-type': 'text/javascript', 'cache-control': 'no-store' }); res.end(readFileSync(resolve(root, 'host/web/dev.js'))); return; }
  try {
    const found = await readStaticFileAsync(dist, file);
    if (!found) { res.writeHead(404); res.end(); return; }
    let body = found.body;
    if (file === '/index.html') body = body.toString().replace('<script type="module" src="./glue.js"></script>', '<script type="module" src="./glue.js"></script>\n<script type="module" src="./dev.js"></script>');
    if (INSTALL_FILES.includes(found.route)) body = process.platform === 'darwin'
      ? developmentInstallPage(body.toString(), localInstallToken)
      : body.toString().replace('<!-- exact-serving -->Static hosting<!-- /exact-serving -->', 'Development server');
    if (INSTALL_FILES.includes(found.route)) body = installNetworkPage(body.toString(), {host,port});
    res.writeHead(200, { 'content-type': webContentType(found.route), ...(file === '/index.html' ? { vary: 'Accept' } : {}), 'cache-control': 'no-store' });
    res.end(req.method === 'HEAD' ? undefined : body);
  } catch { try { res.writeHead(404); res.end(); } catch { /* mid-write */ } }
});
server.on('error', (e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}`); killCompiler(); process.exit(1); });
await readStaticFileAsync(dist, '/index.html'); // warm the reader before advertising readiness
server.listen(port, host, () => {
  const urls = [`http://127.0.0.1:${port}/`];
  if (!loopback) {
    // Every usable IPv4, private-range first, none silently picked (D8):
    // a utun/VPN address printed alone is a silent failure on the phone.
    urls.push(...lanAddresses.map((address) => `http://${address}:${port}/`));
    if (lanAddresses.length === 0) console.log('no LAN interface found; serving loopback only in effect');
  }
  console.log(urls.join('\n'));
  console.log(`  (dev loop on ${source.replace(root + '/', '')} and the wasm's crates; ${loopback ? 'loopback only' : 'LAN bind — --loopback to keep it local; macOS may ask to allow node'}; ctrl-c to stop)`);
});
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
