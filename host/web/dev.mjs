#!/usr/bin/env bun
// The resident dev loop: edit app.contract → the page shows it, no cargo
// build in the loop. Usage: bun host/web/dev.mjs [--app caltrain] [--port 8765] [--lan]
//
// Binds 127.0.0.1. --lan binds every interface, so a phone on the network can
// boot the plan from a printed LAN URL — and so can any peer read the plan,
// the compile errors on /__dev and the dev generations (LLP 1023 D8, amended
// 2026-09-23). Either way the server answers only to the names it printed, and
// the local iOS installer's token reaches only a page loaded over loopback.
// The agent carrier is not here and never binds the LAN.
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
import { developmentGate, developmentInstallPage, installBrowserOrigins, installNetworkPage, localInstallURL, INSTALL_FILES, LOCAL_IOS_INSTALL_ENDPOINT } from '../../scripts/install-page.mjs';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, unwatchFile, watch, watchFile } from 'node:fs';
import { resolve } from 'node:path';
import { rustPackage, rustOutput, rustInputs, rustCards } from '../../scripts/rust.mjs';
import { rustPolicy, rebuildPolicy } from '../../scripts/app.mjs';
import { cargoReproducibilityFlags, compilerPaths, developmentBuildEnv, developmentCandidate, pendingBuildInputs, readBuilds, resolveApp } from '../../scripts/app.mjs';
import { developmentLinks, phones, simulators } from '../apple/build.mjs';
import { webRequestURL } from '../../scripts/origin.mjs';
import { sendStaticBody, applyStaticChange, applyStaticTreeChange, builtAppMatches, developmentOpenPage, readDevGenerationAsync, readStaticFileAsync, readWebRequest, reflectShaderFiles, retainDevGeneration, shaderInterfaceDigests, syncStaticTree, watchStaticTrees, webContentType, webEnvelope, MODULE_FILES, moduleCards } from './serve.mjs';

const argv = process.argv.slice(2);
const arg = (name, fallback) => { const i = argv.indexOf(name); return i >= 0 ? argv[i + 1] : fallback; };
const buildEnv = {...developmentBuildEnv(),EXACT_UPDATE_TRUST:'development'};
let app = resolveApp(arg('--app', undefined));
const port = Number(arg('--port', 8765));
const lan = argv.includes('--lan');
const host = lan ? '0.0.0.0' : '127.0.0.1';
// The addresses printed at startup are the only names requests may use.
const origins = installBrowserOrigins({ host, port });
const gate = developmentGate(origins, port);
const root = resolve(new URL('../..', import.meta.url).pathname);
const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(root, 'host/web/dist'));
const source = resolve(app.dir, 'app.contract');
let typescript = existsSync(resolve(app.dir, 'app.ts'));
let portableRust = Boolean(rustPackage(app)) && rustPolicy(app.manifest, 'web') !== 'off';
let rebuildOn = rebuildPolicy(app.manifest);
let manualTypescript = null, rustChild = null, rustActive = false, rustRun = 0, rustHeartbeat = null, rustDirty = false, rustSaved = 0, rustSourceWatch = null, rustOutputWatch = null;
let rustInputFiles = new Set();
let changed = new Set(), timer=null, building=false, buildPending=false, rustPending=false, builds=0;
const plan = resolve(dist, 'app.plan');
const graphPath = resolve(dist, 'bake.json');
buildEnv.EXACT_DEV_BAKE = graphPath;
async function currentWebBuild() {
  if (!await builtAppMatches(dist, app)) return false;
  try {
    const build = JSON.parse(readFileSync(graphPath, 'utf8'));
    return build.version === 1 && build.trust === 'development' && pendingBuildInputs(build).length === 0;
  } catch { return false; }
}
if (!await currentWebBuild()) {
  const b = spawnSync(process.execPath, [resolve(root, 'host/web/build.mjs'), app.crate('web')], { cwd: root, env:buildEnv, stdio: 'inherit' });
  if (b.status !== 0) process.exit(b.status ?? 1);
}
// The last complete cdylib links name their transitive source files, including
// path dependencies outside this workspace. Unknown/shared inputs take the full build.
let gpuInputs = new Set(), appInputs = new Set();
const gpuSideRoot = resolve(app.target, 'dev-gpu', app.name);
// A server epoch pairs function-scoped glue with its Wasm bytes.
let gpuSide = null, gpuVersion = Date.now(), gpuBuildChild = null;
const gpuTimings = new Map(), gpuVersions = new Map();
function readGpuInputs(profile = 'web') {
  const inputs = (kind, profile) => {
    const path = resolve(app.target, 'wasm32-unknown-unknown', profile, app.crate(kind).replaceAll('-', '_') + '.d');
    return existsSync(path) ? new Set(compilerPaths(readFileSync(path, 'utf8'), app.workspace)) : new Set();
  };
  if (profile === 'web') appInputs = gameRuntimeInputs(inputs('web', 'web'));
  gpuInputs = inputs('gpu', profile);
}
function gameRuntimeInputs(inputs) {
  if (!app.manifest.game || !inputs.size) return inputs;
  // A generated game's bake reads Game::NAME/Args::FIELDS; its logic is not
  // linked into the app. Cargo's summary .d includes that build-only closure.
  const result = spawnSync('cargo', ['metadata', ...cargoReproducibilityFlags(app), '--format-version', '1', '--filter-platform', 'wasm32-unknown-unknown'], {cwd:app.workspace, env:buildEnv, encoding:'utf8', maxBuffer:128*1024*1024});
  if (result.status !== 0) throw new Error(`cargo metadata failed: ${result.stderr || result.error || result.status}`);
  const metadata = JSON.parse(result.stdout), packages = new Map(metadata.packages.map(p => [p.id,p]));
  const nodes = new Map(metadata.resolve.nodes.map(n => [n.id,n]));
  const root = metadata.packages.find(p => p.name === app.crate('web'));
  if (!root) return inputs;
  const runtime = new Set(), pending = [root.id];
  while (pending.length) {
    const id = pending.pop(); if (runtime.has(id)) continue; runtime.add(id);
    // Keep build inputs of runtime dependencies: their generated Rust can be
    // part of the app. Only the known generated game bake is metadata-only.
    for (const dep of nodes.get(id)?.deps ?? [])
      if (dep.dep_kinds.some(k => k.kind === null || (id !== root.id && k.kind === 'build'))) pending.push(dep.pkg);
  }
  const sources = new Set([...runtime].flatMap(id => packages.get(id).targets
    .filter(t => t.kind.some(k => ['lib','rlib','cdylib','proc-macro'].includes(k)) || (id !== root.id && t.kind.includes('custom-build')))
    .map(t => resolve(t.src_path))));
  const included = new Set(), found = new Set();
  for (const profile of [resolve(app.target,'web'), resolve(app.target,'wasm32-unknown-unknown/web')]) {
    const build = resolve(profile,'build');
    const directories = [resolve(profile,'deps'), ...(existsSync(build) ? readdirSync(build,{withFileTypes:true}).filter(e=>e.isDirectory()).map(e=>resolve(build,e.name)) : [])];
    for (const directory of directories) if (existsSync(directory)) {
      const output=resolve(directory,'output');
      if (existsSync(output)) for (const id of runtime) {
        const pkg=packages.get(id), name=directory.slice(directory.lastIndexOf('/')+1);
        if (id===root.id || name.replace(/-[a-f0-9]+$/,'')!==pkg.name) continue;
        for (const line of readFileSync(output,'utf8').split('\n')) {
          const changed=/^cargo::?rerun-if-changed=(.+)$/.exec(line); if (!changed) continue;
          const watched=resolve(pkg.manifest_path,'..',changed[1]);
          for (const path of inputs) if (path===watched || path.startsWith(watched+'/')) included.add(path);
        }
      }
      for (const file of readdirSync(directory)) {
        if (!file.endsWith('.d')) continue;
        const paths = compilerPaths(readFileSync(resolve(directory,file),'utf8'),app.workspace);
        const roots = paths.filter(path=>sources.has(path));
        if (roots.length) { for (const path of roots) found.add(path); for (const path of paths) included.add(path); }
      }
    }
  }
  // Exact rustc unit files retain include! inputs outside their package. If
  // evidence for a compiled runtime source is missing, keep the full rebuild.
  if ([...sources].some(path=>inputs.has(path) && !found.has(path))) return inputs;
  const owners = metadata.packages.map(p=>({id:p.id,dir:resolve(p.manifest_path,'..')})).sort((a,b)=>b.dir.length-a.dir.length);
  return new Set([...inputs].filter(path=>{
    const owner = owners.find(p=>path===p.dir || path.startsWith(p.dir+'/'));
    return !owner || runtime.has(owner.id) || included.has(path);
  }));
}
readGpuInputs();
const gpuOnly = files => files.length > 0 && appInputs.size > 0
  && files.every(path => path.endsWith('.rs') && gpuInputs.has(path) && !appInputs.has(path));
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
function startLocalInstall(targetId) {
  if (localInstallChild) { const error = new Error('A local iOS build is already running.'); error.status = 409; throw error; }
  refreshLocalInstallTargets(true);
  if (localInstallTargetError) { const error = new Error(localInstallTargetError); error.status = 503; throw error; }
  const target = localInstallTargets.find(candidate => candidate.id === targetId);
  if (!target) { const error = new Error('That Simulator or device is no longer available. Refresh the target list.'); error.status = 400; throw error; }
  const appURL = new URL(localInstallURL(target.kind, origins, port));
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
  const files = ['app.wasm', 'gpu_bg.wasm', 'markup-editor.wasm'].map((name) => {
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
  // Source metadata stays beside the dev generation, never in its assets or
  // module receipt. A static/Rust bake may have replaced the plan without a map.
  const encodedMap = currentModule || currentRust ? null : filesystem({ op: 'get', root: dist, path: 'app.plan.map.json' });
  const mapBytes = currentModule?.get('app.plan.map.json') ?? currentRust?.get('app.plan.map.json') ?? (encodedMap == null ? null : Buffer.from(encodedMap, 'base64'));
  files.delete('app.plan.map.json');
  if (mapBytes && mapBytes.length <= 64 * 1024 * 1024) {
    try {
      if (JSON.parse(mapBytes.toString('utf8')).digest === createHash('sha256').update(planBytes).digest('hex')) files.set('app.plan.map.json', mapBytes);
    } catch { /* Unavailable metadata must not prevent a valid app reload. */ }
  }
  const module = currentModule ? moduleCards(files, app.id) : null;
  if (currentRust) for (const [name, body] of currentRust) {
    if (name === 'app.plan' || name === 'app.plan.map.json') continue;
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
  if (files.has('app.plan.map.json')) {
    const body = files.get('app.plan.map.json');
    envelope.dev.sourceMap = { url: prefix + 'app.plan.map.json', sha256: createHash('sha256').update(body).digest('hex'), bytes: body.length };
  }
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
  const metadata = spawnSync('cargo', ['metadata',...cargoReproducibilityFlags(app),'--no-deps','--format-version','1'], {cwd:app.workspace,env:buildEnv,encoding:'utf8'});
  if (metadata.status !== 0) throw new Error(`cargo metadata failed: ${metadata.stderr || metadata.error || metadata.status}`);
  const hasDev = JSON.parse(metadata.stdout).packages.find(p=>p.name===app.crate('web'))?.targets.some(t=>t.name==='dev'&&t.kind.includes('bin'));
  dev = spawn('cargo', ['run', '-q', '--release', '-p', hasDev ? app.crate('web') : 'exact-web', '--bin', hasDev ? 'dev' : 'exact-dev', '--', source, plan], { cwd: hasDev ? app.workspace : root, env: buildEnv, stdio: ['ignore', 'pipe', 'inherit'], detached: true });
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
        // Every refusal of the save, as one JSON string (dev.rs keeps its lines).
        let error = rest.join(' ');
        try { error = JSON.parse(error); } catch { /* a plain message */ }
        console.log(`error: ${error}`);
        push({ error });
      }
    }
  });
  dev.on('exit', (code) => { if (dev === me) { console.error(`dev compiler exited ${code}`); killCompiler(); process.exit(code ?? 1); } });
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
    // The served plan and receipt seed what the producer cannot compute: a
    // resource only Rust owns keeps its last Cargo-baked first-frame value.
    child.stdin.write(JSON.stringify({ id: active.id, out: active.output, previous: resolve(dist, 'app.plan'), receipt: resolve(dist, 'app.module.json') }) + '\n');
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
        const candidate = new Map(['app.plan', 'app.plan.map.json', ...Object.values(MODULE_FILES)].map(name => [name, readFileSync(resolve(request.output, name))]));
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
function produceRust() { rustPending = true; drainBuilds(); }
function drainBuilds() {
  if (building || rustActive) return;
  if (buildPending || changed.size) {
    buildPending = false;
    const files = [...changed]; changed = new Set();
    if (gpuOnly(files)) void produceGpu(files); else rebuildNow(files);
  } else if (rustPending) { rustPending = false; produceRustNow(); }
}
function produceRustNow() {
  if (!rustChild) {
    const child=rustChild=spawn(process.execPath,[resolve(root,'scripts/rust.mjs'),app.name,'--serve'],{cwd:root,env:buildEnv,detached:true,stdio:['pipe','pipe','pipe']});
    let buffer='';
    const failed=error=>{
      if(rustChild!==child)return;
      rustChild=null;rustActive=false;clearInterval(rustHeartbeat);rustHeartbeat=null;
      console.error(error.message);push({error:error.message});
      try{process.kill(-child.pid,'SIGTERM');}catch{}
      drainBuilds();
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
          // The reply follows the pointer's rename; the directory watch would
          // see it a filesystem event later and then finds it current.
          else try{readRustGeneration();}catch(error){console.error(error.message);push({error:error.message});}
          if(rustDirty && rebuildOn.rust==='save') rustPending = true;
          drainBuilds();
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
  rustSourceWatch = watchModuleSources(app.dir, rustWatchIgnores, error => {
    if (error) { push({error:error.message}); return; }
    // The TS producer owns mixed Contract edits. If a Contract changes
    // during a Rust bake, its before/after guard will refuse that bake;
    // remember to build the latest snapshot once the in-flight job ends.
    if (typescript) { if (rustActive) rustDirty = true; return; }
    // The watcher already coalesces a save's events; the producer serializes
    // builds and reruns once for edits that arrive during one.
    rustSaved = Date.now(); rustDirty = true;
    if (rebuildOn.rust === 'save') { clearTimeout(timer); timer=setTimeout(produceRust,0); }
  });
  produceRust();
}
// App-relative names the Rust source watcher leaves to other producers.
const rustWatchIgnores = name => skipped.test(name) || /(^|\/)\./.test(name)
  || /\.(ts|json)$/.test(name) || gpuOnly([resolve(app.dir, name)])
  || assetTrees.some(([tree]) => resolve(app.dir,name).startsWith(tree+'/'));
const killCompiler = () => {
  const gpuChild = gpuBuildChild; gpuBuildChild = null;
  if (gpuChild) { try { process.kill(-gpuChild.pid, 'SIGKILL'); } catch {} }
  manualTypescript = null;
  rustSourceWatch?.close(); rustSourceWatch = null; rustOutputWatch?.close(); rustOutputWatch = null;
  clearInterval(rustHeartbeat); rustHeartbeat = null; rustActive = false;
  const rust = rustChild; rustChild = null; if (rust) { try { process.kill(-rust.pid, 'SIGKILL'); } catch {} }

  moduleWatch?.close(); moduleWatch = null; clearImmediate(moduleTimer); moduleRun++;
  const d = dev; dev = null; if (d) { try { process.kill(-d.pid, 'SIGKILL'); } catch {} }
  if (moduleStage) { rmSync(moduleStage, { recursive: true, force: true }); moduleStage = null; }
};
startCompiler();
const stop = async () => {
  const children = [dev, rustChild, gpuBuildChild, localInstallChild, hostBuildChild].filter(Boolean);
  const exits = children.map(child => new Promise(ok => child.exitCode !== null || child.signalCode !== null ? ok() : child.once('exit', ok)));
  killCompiler();
  localInstallChild?.kill('SIGTERM');
  // A rebuild in flight would otherwise swap dist under the next dev server.
  hostBuildChild?.kill('SIGTERM');
  await Promise.all(exits); process.exit(0);
};

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
    assetTimer = setTimeout(pushAssets, 20);
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
  // Poll declared file metadata: saves and replacement survive directory-event
  // coalescing. Only open-ended source discovery needs a directory watch.
  const files=new Set([...builtReceipts.flatMap(r=>r.binary.inputs.map(f=>f.path)),...rustInputFiles,...gpuInputs,...appInputs].filter(p=>!skipped.test(p)&&!p.includes('/.cargo/')));
  for(const receipt of builtReceipts)for(const missing of receipt.binary.missing)files.add(missing);
  files.add(resolve(app.dir,'app.json'));
  compilerInputFiles = files;
  compilerInputTrees = builtReceipts.flatMap(r=>r.binary.directories.map(d=>d.path));
  compilerMissingInputs = builtReceipts.flatMap(r=>r.binary.missing);
  swiftSourceDirectories = new Set([...files].filter(path=>path.endsWith('.swift')).map(path=>resolve(path,'..')));
  const directories=new Set([...compilerInputTrees,...swiftSourceDirectories]);
  const targets=new Set([...files,...directories]);
  for(const [path,handle] of watched)if(!targets.has(path)){handle.close();watched.delete(path);}
  for(const target of targets) {
    if(skipped.test(target)||target.includes('/.cargo/')||watched.has(target))continue;
    const file=files.has(target),dir=file?resolve(target,'..'):target;
    const changedPath=name=>{
      if(file)name=target.slice(target.lastIndexOf('/')+1);
      if(!name||skipped.test(name)||/(^|\/)\./.test(name)||name.endsWith('dev.js')||assetTrees.some(([tree])=>resolve(dir,name).startsWith(tree+'/'))||resolve(dir,name)===source)return;
      // Parent-directory notifications include unrelated documents and output.
      // Only receipt inputs, declared trees/missing paths, and Swift's implicit
      // source discovery can invalidate the host. Rust additions are reached
      // when their declaring module or build input changes.
      const path = resolve(dir, name);
      if(!file&&compilerInputFiles.has(path))return;
      // The resident Contract compiler already watches generated game arguments.
      // Rebuilding the host afterward would discard the carried world.
      if (app.manifest.game && path === resolve(app.dir, '.shells/surfaces.json')) return;
      if (!compilerInputFiles.has(path) && !compilerInputTrees.some(tree=>path===tree||path.startsWith(tree+'/'))
        && !compilerMissingInputs.some(missing=>path===missing||missing.startsWith(path+'/'))
        && !(name.endsWith('.swift') && swiftSourceDirectories.has(dir))) return;
      if (typescript && resolve(dir, name).startsWith(app.dir + '/') && /\.(ts|contract)$/.test(name)) return;
      if (portableRust && rustInputFiles.has(path)) {
        // A Contract source the Rust source watcher sees has already started
        // its build (that watcher tracks .contract names, never .rs).
        if (rustSourceWatch && path.endsWith('.contract') && path.startsWith(app.dir + '/') && !rustWatchIgnores(path.slice(app.dir.length + 1))) return;
        rustSaved=Date.now();rustDirty=true;if(rebuildOn.rust==='save'){clearTimeout(timer);timer=setTimeout(produceRust,200);}return;
      }
      changed.add(resolve(dir,name));console.log(`edit ${resolve(dir,name)} → build pending; classification follows its receipt`);
      if(rebuildOn.rust==='save'){clearTimeout(timer);timer=setTimeout(rebuild,gpuOnly([...changed]) ? 20 : 200);}
    };
    try {
      if(file){
        const listener=(now,previous)=>{
          if(['dev','ino','size','mtimeNs','ctimeNs'].some(key=>now[key]!==previous[key]))changedPath();
        };
        watchFile(target,{bigint:true,interval:100},listener);
        watched.set(target,{close:()=>unwatchFile(target,listener)});
      }else if(existsSync(target))watched.set(target,watch(target,(_event,name)=>changedPath(name)));
    }catch(error){console.error(`cannot watch ${target}: ${error.message}`);}
  }
}
watchCompilerInputs();
process.stdin.setEncoding('utf8');
process.stdin.on('data', input => {
  for (const command of input.trim().split(/\s+/)) {
    if (command === 'f') push({fresh:true});
    if (command === 't') manualTypescript?.();
    if (command === 'r') { if (portableRust && !changed.size) produceRust(); else rebuild(); }
  }
});
console.log(`rebuild: Rust ${rebuildOn.rust}, TypeScript ${rebuildOn.typescript}; r + Enter builds Rust, t + Enter builds TypeScript, f + Enter starts a fresh page`);

let hostBuildChild = null;
function rebuild() { buildPending = true; drainBuilds(); }
function rebuildNow(files) {
  building = true;
  const t = Date.now();
  console.log(`rust: ${files.length} file${files.length === 1 ? '' : 's'} changed (${files.slice(0, 3).join(', ')}${files.length > 3 ? ', …' : ''}) — rebuilding the wasm`);
  const b = hostBuildChild = spawn(process.execPath, [resolve(root, 'host/web/build.mjs'), app.crate('web')], { cwd: root, env:buildEnv, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  b.stdout.on('data', (d) => { out += d; });
  b.stderr.on('data', (d) => { out += d; });
  b.on('exit', (code) => {
    building = false; hostBuildChild = null;
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
      for (const directory of gpuVersions.values()) rmSync(directory, {recursive:true,force:true});
      gpuVersions.clear(); gpuSide = null; readGpuInputs(); watchCompilerInputs();
      program = programIdentity();
      // The restarted producer consumes module edits; queued core edits start
      // their rebuild there. Do not re-arm a third build below on that success.
      rustPending = false;
      startCompiler();
      console.log(`rust: rebuilt in ${(ms / 1000).toFixed(1)} s · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading\n  ${classifyRebuild().join('\n  ')}`);
      push({ rebuilt: builds });
    } else {
      const errors = out.split('\n').filter((l) => /^(error|warning: unused|\s+-->)/.test(l)).join('\n') || out.trim().split('\n').slice(-12).join('\n');
      console.log(`rust: build failed in ${(ms / 1000).toFixed(1)} s\n${errors}`);
      push({ error: `the wasm did not build:\n${errors}` });
    }
    drainBuilds();
  });
}

async function produceGpu(files) {
  building = true;
  const start = Date.now();
  const profile = Bun.TOML.parse(readFileSync(resolve(app.workspace, 'Cargo.toml'), 'utf8')).profile?.['gpu-dev'] ? 'gpu-dev' : 'web';
  if (profile === 'web') console.log('gpu: add [profile.gpu-dev] inheriting dev, opt-level=1, no LTO, and optimized dependencies for fast module rebuilds');
  mkdirSync(gpuSideRoot, { recursive: true });
  const stage = mkdtempSync(resolve(gpuSideRoot, 'build-'));
  const run = (command, args) => new Promise((ok, fail) => {
    const child = gpuBuildChild = spawn(command, args, { cwd: app.workspace, env: {...buildEnv, CARGO_TARGET_DIR:app.target}, stdio:['ignore','pipe','pipe'], detached:true });
    let output = '';
    child.stdout.on('data', data => { output += data; });
    child.stderr.on('data', data => { output += data; });
    child.on('error', fail);
    child.on('exit', code => { gpuBuildChild = null; code === 0 ? ok() : fail(new Error(output.trim())); });
  });
  try {
    console.log(`gpu: ${files.length} source file(s) changed; building ${app.crate('gpu')} (${profile})`);
    await run('cargo', ['build',...cargoReproducibilityFlags(app),'-p',app.crate('gpu'),'--target','wasm32-unknown-unknown','--profile',profile]);
    const compiled = Date.now();
    await run('wasm-bindgen', ['--target','no-modules','--no-typescript','--out-dir',stage,'--out-name','gpu',resolve(app.target,'wasm32-unknown-unknown',profile,app.crate('gpu').replaceAll('-','_')+'.wasm')]);
    gpuSide = stage; gpuVersion++; gpuVersions.set(gpuVersion, stage);
    // Query versions pin JS and wasm together. A lagging fetch gets 404 rather
    // than silently pairing exports from one build with another build's wasm.
    while (gpuVersions.size > 3) { const [version, directory] = gpuVersions.entries().next().value; rmSync(directory, {recursive:true,force:true}); gpuVersions.delete(version); }
    readGpuInputs(profile); watchCompilerInputs();
    const ms = Date.now() - start;
    gpuTimings.set(gpuVersion, { ms, start });
    console.log(`gpu: rebuilt in ${ms} ms (cargo ${compiled-start} ms, bindgen ${Date.now()-compiled} ms); swap pushed`);
    push({gpu:gpuVersion});
  } catch (error) {
    rmSync(stage, {recursive:true,force:true});
    console.error(`gpu: build failed in ${Date.now()-start} ms\n${error.message}`);
    push({error:`GPU module did not build:\n${error.message}`,source:'gpu'});
  } finally {
    building = false;
    drainBuilds();
  }
}

const server = createServer(async (req, res) => {
  const access = gate.check(req);
  if (!access.allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
  const url = webRequestURL(req.url);
  if (!url) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
  const devBeacon = url.pathname === '/__dev/reloaded' || url.pathname === '/__dev/painted' || url.pathname === '/__dev/gpu';
  const localInstall = url.pathname === LOCAL_IOS_INSTALL_ENDPOINT;
  if (req.method !== 'GET' && req.method !== 'HEAD' && !(req.method === 'POST' && (devBeacon || localInstall))) { res.writeHead(405); res.end(); return; }
  if (url.pathname === '/__dev/gpu') {
    const timing = gpuTimings.get(Number(url.searchParams.get('g')));
    if (timing) console.log(`gpu: rebuilt in ${timing.ms} ms · swapped in ${url.searchParams.get('swap')} ms · build start → running ${Date.now()-timing.start} ms (warm budget 2000 ms)`);
    res.writeHead(204); res.end(); return;
  }
  if (gpuSide && url.searchParams.has('g') && ['/gpu.js','/gpu_bg.wasm'].includes(url.pathname)) {
    const directory = url.searchParams.has('g') ? gpuVersions.get(Number(url.searchParams.get('g'))) : gpuSide;
    if (!directory) { res.writeHead(404, {'cache-control':'no-store'}); res.end(); return; }
    res.writeHead(200, {'content-type':webContentType(url.pathname),'cache-control':'no-store'});
    res.end(req.method === 'HEAD' ? undefined : readFileSync(resolve(directory,url.pathname.slice(1)))); return;
  }
  if (localInstall) {
    const json = (status, body) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(JSON.stringify(body) + '\n'); };
    if (!access.local || req.headers['x-exact-install-token'] !== localInstallToken) { json(404, { message: 'Not found.' }); return; }
    if (req.method === 'GET' || req.method === 'HEAD') {
      const body = localInstallStatus(url.searchParams.get('refresh') === '1');
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
      res.end(req.method === 'HEAD' ? undefined : JSON.stringify(body) + '\n');
      return;
    }
    try {
      if (!gate.loopbackOrigins.includes(req.headers.origin)) { const error = new Error('The install request must come from this development server.'); error.status = 403; throw error; }
      if (!/^application\/json(?:\s*;|$)/i.test(req.headers['content-type'] ?? '')) { const error = new Error('The install request must be JSON.'); error.status = 415; throw error; }
      let size = 0, encoded = '';
      for await (const chunk of req) {
        size += chunk.length;
        if (size > 1024) { const error = new Error('The install request is too large.'); error.status = 413; throw error; }
        encoded += chunk;
      }
      const body = JSON.parse(encoded || '{}');
      if (typeof body.target !== 'string' || body.target.length > 128) { const error = new Error('Choose an available iOS Simulator or paired device.'); error.status = 400; throw error; }
      json(202, startLocalInstall(body.target));
    } catch (error) {
      json(error.status ?? 400, { message: error instanceof SyntaxError ? 'The install request must be JSON.' : error.message });
    }
    return;
  }
  if (url.pathname === '/__dev/open') {
    // The page's own address (the gate admitted its Host) and the opening
    // links this Mac's development clients admit for it (LLP 1030.000 §7).
    const page = new URL('/' + url.search, `http://${req.headers.host}`).href;
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store', 'referrer-policy': 'no-referrer' });
    res.end(req.method === 'HEAD' ? undefined : developmentOpenPage(app, developmentLinks(app, page), page));
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
    if (gpuSide) res.write(`data: ${JSON.stringify({gpu:gpuVersion})}\n\n`);
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
  // @ref LLP 1038 D7 — resolve files and extensionless locations before negotiation.
  const { found, index } = await readWebRequest(dist, url.pathname);
  const wantsEnvelope = url.pathname === '/exact.json'
    || (index && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json'));
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
    if (!found) { res.writeHead(404); res.end(); return; }
    let body = found.body;
    // The page's first boot is the current generation, named here for
    // dev.js to fetch (LLP 1007 §6), never app.wasm's older baked plan.
    const first = current ? `<meta name="exact-dev-generation" content="${JSON.stringify(announcement()).replace(/[&"<>]/g, c => `&#${c.charCodeAt(0)};`)}">\n` : '';
    if (index) body = body.toString().replace('<script type="module" src="./glue.js"></script>', `${first}<script type="module" src="./glue.js"></script>\n<script type="module" src="./dev.js"></script>`);
    if (INSTALL_FILES.includes(found.route)) body = process.platform === 'darwin' && access.local
      ? developmentInstallPage(body.toString(), localInstallToken)
      : body.toString().replace('<!-- exact-serving -->Static hosting<!-- /exact-serving -->', 'Development server');
    if (INSTALL_FILES.includes(found.route)) body = installNetworkPage(body.toString(), {host,port});
    sendStaticBody(req, res, body, { 'content-type': webContentType(found.route), ...(index ? { vary: 'Accept' } : {}), 'cache-control': 'no-store' });
  } catch { try { res.writeHead(404); res.end(); } catch { /* mid-write */ } }
});
server.on('error', (e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}`); killCompiler(); process.exit(1); });
await readStaticFileAsync(dist, '/index.html'); // warm the reader before advertising readiness
server.listen(port, host, () => {
  // Every usable IPv4 with --lan, none silently picked (D8): a utun/VPN
  // address printed alone is a silent failure on the phone.
  const urls = origins.map(o => `${o.origin}/`);
  if (lan && urls.length === 1) console.log('no LAN interface found; serving loopback only in effect');
  console.log(urls.join('\n'));
  console.log(urls.map(url => `  Open in native: ${url}__dev/open`).join('\n'));
  console.log(`  (dev loop on ${source.replace(root + '/', '')} and the wasm's crates; ${lan ? 'LAN bind — any peer on this network can read the app, its compile errors and dev generations; macOS may ask to allow bun' : 'loopback only — --lan to serve a phone on this network'}; ctrl-c to stop)`);
});
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
