#!/usr/bin/env bun
// Independently link and bake a Rust business-logic module. LLP 1029.000.
// The dev server calls the same producer; CLI use is an explicit build boundary.
import { spawn, spawnSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargoReproducibilityFlags, developmentBuildEnv, readBuilds, resolveApp, rustPolicy } from './app.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
export const rustCard = (file, bytes) => ({ file, bytes: bytes.length, sha256: hash(bytes) });
function run(app, command, args, env) {
  const r = spawnSync(command, args, { cwd: app.workspace, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (r.stderr) process.stderr.write(r.stderr);
  if (r.error || r.status !== 0) {
    const output=args.includes('--message-format=json') ? r.stdout.split('\n').flatMap(line=>{
      try {const row=JSON.parse(line);return row.reason==='compiler-message'?[row.message.rendered ?? row.message.message]:[];} catch{return [];}
    }).join('') : r.stdout;
    throw new Error(`${command} ${args.join(' ')}: ${r.error?.message ?? output ?? ''} (exit ${r.status})`);
  }
  return r.stdout;
}
export function rustPackage(app) { return app.manifest.rust?.module?.package ?? null; }
export function rustOutput(app) { return resolve(app.target, 'rust-dev', hash(app.dir).slice(0, 24)); }

/** Fetch only signed-record companion names from the publisher's blob store.
 * Cargo independently verifies the record signature and exact asset roster. */
export function prepareRustBundle(app, platform, target, env) {
  if (env.EXACT_RUST_BUNDLE) return env.EXACT_RUST_BUNDLE;
  if (!env.EXACT_UPDATE_RECEIPT) return null;
  const recordPath=resolve(env.EXACT_UPDATE_RECEIPT), record=JSON.parse(readFileSync(recordPath));
  const head=record.envelope, cards=head?.assets?.filter(a=>a.name.startsWith('rust/')) ?? [];
  if (!cards.length) return null;
  const mode=rustPolicy(app.manifest,platform,env.EXACT_UPDATE_TRUST==='production'?'prod':'dev');
  const filename=mode==='tiered'?'app.module.bin':mode==='wasm'?'app.module.wasm':target.includes('apple')?'app.module.dylib':target.includes('windows')?'app.module.dll':'app.module.so';
  if (mode==='off'||cards.length!==2||!cards.some(c=>c.name==='rust/app.module.json')||!cards.some(c=>c.name==='rust/'+filename)) throw new Error('publisher receipt has an incompatible Rust companion roster');
  mkdirSync(env.EXACT_BAKE_OUTPUT,{recursive:true});
  const destination=mkdtempSync(resolve(env.EXACT_BAKE_OUTPUT,`${platform}-${target}-rust-`));
  try {
  for(const card of cards) {
    if(!/^[0-9a-f]{64}$/.test(card.sha256)||!Number.isSafeInteger(card.bytes)||card.bytes<0||card.bytes>32*1024*1024||card.url!==`../../blobs/${card.sha256}`)throw new Error('invalid Rust companion blob card');
    let bytes;
    if(dirname(recordPath).endsWith('/releases')) {
      const blob=resolve(dirname(recordPath),'../../../blobs',card.sha256);
      if(existsSync(blob))bytes=readFileSync(blob);
    }
    if(!bytes) {
      const origin=app.manifest.deploy?.channels?.[head.stream.channel] ?? app.origin;
      if(!origin)throw new Error('Rust companion blobs unavailable locally; provide EXACT_RUST_BUNDLE or configure the signed origin');
      const url=new URL(`.exact/blobs/${card.sha256}`,origin.endsWith('/')?origin:origin+'/');
      const code=`const r=await fetch(process.argv[1],{redirect:'error',signal:AbortSignal.timeout(30000)});if(!r.ok)throw new Error('Rust blob HTTP '+r.status);const limit=Number(process.argv[2]);let n=0;for await(const b of r.body){n+=b.length;if(n>limit)throw new Error('Rust blob exceeds bound');process.stdout.write(b);}`;
      const fetched=spawnSync(process.execPath,['--input-type=module','-e',code,url.href,String(card.bytes)],{env,maxBuffer:33*1024*1024});
      if(fetched.status!==0)throw new Error('Rust companion fetch failed: '+fetched.stderr);bytes=fetched.stdout;
    }
    if(bytes.length!==card.bytes||hash(bytes)!==card.sha256)throw new Error('Rust companion bytes differ from signed receipt');
    writeFileSync(resolve(destination,card.name.slice(5)),bytes);
  }
  return destination;
  } catch(error) {rmSync(destination,{recursive:true,force:true});throw error;}
}

// `cargo metadata` answers from the manifests, the lockfile and Cargo's
// config: the last answer stands while their bytes do (a new path dependency
// is a manifest edit). A resident producer asks on every request.
const metadataCache = new Map();
function cargoMetadata(app, env) {
  const key = JSON.stringify([app.workspace, cargoReproducibilityFlags(app)]), cached = metadataCache.get(key);
  if (cached && rustInputDigest(cached.manifests) === cached.digest) return cached.metadata;
  const metadata = JSON.parse(run(app, 'cargo', ['metadata', '--format-version', '1', ...cargoReproducibilityFlags(app)], env));
  const manifests = [...new Set(['Cargo.toml', 'Cargo.lock', '.cargo/config.toml'].map(name => resolve(metadata.workspace_root, name))
    .concat(metadata.packages.filter(p => !p.source).map(p => p.manifest_path)))].sort();
  metadataCache.set(key, { metadata, manifests, digest: rustInputDigest(manifests) });
  return metadata;
}

// Include the declared Cargo closure, its build scripts and included files.
// Recheck after compilation/bake: moving inputs never become a published pair.
export function rustInputs(app, env = developmentBuildEnv(), {reloadOnly = false} = {}) {
  return rustClosure(app, env, reloadOnly).files;
}
function rustClosure(app, env, reloadOnly = false) {
  const packageName = rustPackage(app);
  if (!packageName) return { files: [], directories: [] };
  const metadata = cargoMetadata(app, env);
  const packages = new Map(metadata.packages.map(p => [p.id, p]));
  const nodes = new Map(metadata.resolve.nodes.map(p => [p.id, p]));
  const root = metadata.packages.find(p => p.name === packageName);
  if (!root || !root.targets.some(t => t.crate_types.includes('cdylib'))) throw new Error(`rust.module.package ${packageName} must declare crate-type = ["cdylib"] and export the Exact logic ABI`);
  const resident = new Set();
  const boundary = metadata.packages.filter(p=>p.name==='exact-logic-abi').map(p=>p.id);
  while(boundary.length) {
    const id=boundary.pop();if(resident.has(id))continue;resident.add(id);
    for(const dep of nodes.get(id)?.deps ?? [])boundary.push(dep.pkg);
  }
  const seen = new Set(), pending = [root.id], directories = new Set();
  while (pending.length) {
    const id = pending.pop(); if (seen.has(id)) continue; seen.add(id);
    const pkg = packages.get(id);
    if (!pkg.source && (!reloadOnly || !resident.has(id))) directories.add(dirname(pkg.manifest_path));
    for (const dep of nodes.get(id)?.deps ?? []) if (dep.dep_kinds.some(k => k.kind !== 'dev')) pending.push(dep.pkg);
  }
  const files = new Set(reloadOnly ? [resolve(app.dir,'app.contract')] : [resolve(metadata.workspace_root, 'Cargo.toml'), resolve(metadata.workspace_root, 'Cargo.lock'), resolve(app.dir, 'app.contract'), resolve(app.dir, 'app.json')]);
  const walk = (dir, contractsOnly = false) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (/^(target|node_modules|\.git|\.build|dist)$/.test(entry.name) || entry.name.startsWith('.exact-')) continue;
      const path = resolve(dir, entry.name);
      if (entry.isSymbolicLink()) {
        if (!contractsOnly || entry.name.endsWith('.contract')) throw new Error(`Rust module source links require an explicit Cargo dependency: ${path}`);
      } else if (entry.isDirectory()) walk(path, contractsOnly);
      else if (entry.isFile() && (!contractsOnly || entry.name.endsWith('.contract'))) files.add(path);
    }
  };
  for (const dir of directories) walk(dir);
  // Contract's loader confines imports to the app root. Include all of them,
  // including a newly added import, in the before/after source observation.
  walk(app.dir, true);
  return { files: [...files].filter(path=>!reloadOnly || !/\/(Cargo\.(toml|lock)|build\.rs)$/.test(path)).sort(), directories: [...directories] };
}
// A file's hash stands while its inode, size, mtime and ctime do: every write
// moves ctime. A resident producer rereads only what changed.
const fileHashes = new Map();
function fileHash(path) {
  let stat;
  try { stat = statSync(path, { bigint: true }); }
  catch (error) { if (error.code === 'ENOENT') return hash('<absent>'); throw error; }
  const stamp = `${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeNs}:${stat.ctimeNs}`, known = fileHashes.get(path);
  if (known?.stamp === stamp) return known.hash;
  const value = hash(readFileSync(path));
  fileHashes.set(path, { stamp, hash: value });
  return value;
}
export function rustInputDigest(paths, observations = null) {
  const digest = createHash('sha256');
  for (const path of paths) {
    const value = fileHash(path);
    digest.update(path); digest.update('\0'); digest.update(value); digest.update('\0');
    observations?.set(path, value);
  }
  return digest.digest('hex');
}
function compile(app, target, env, profile) {
  const packageName = rustPackage(app);
  // A module's bytes must not depend on where it was built: a cold deploy's
  // cache is a new directory each time. Code generated under OUT_DIR records
  // its path, and a Mach-O library names itself by its output path.
  const modules = resolve(app.target,'rust-modules');
  const flags = (env.CARGO_ENCODED_RUSTFLAGS?.split('\x1f') ?? env.RUSTFLAGS?.trim().split(/\s+/) ?? []).filter(Boolean);
  flags.push(`--remap-path-prefix=${modules}=/rust-modules`, ...(target.includes('apple') ? ['-Clink-arg=-Wl,-install_name,@rpath/app.module.dylib'] : []));
  const moduleEnv = {...env,CARGO_TARGET_DIR:modules,CARGO_ENCODED_RUSTFLAGS:flags.join('\x1f')};
  delete moduleEnv.RUSTFLAGS;
  // Out-of-tree apps need no custom Cargo profile just to opt in. Keep their
  // ordinary host builds untouched: these defaults apply only to this module.
  if (profile === 'logic-dev' && !/^\s*\[\s*profile\s*\.\s*["']?logic-dev["']?\s*\]/m.test(readFileSync(resolve(app.workspace,'Cargo.toml'),'utf8'))) {
    profile='dev';
    for (const [key,value] of Object.entries({OPT_LEVEL:'1',DEBUG:'0',INCREMENTAL:'true',PANIC:'abort'})) moduleEnv[`CARGO_PROFILE_DEV_${key}`] ??= value;
  }
  const messages = run(app, 'cargo', ['build', ...cargoReproducibilityFlags(app), '--lib', '-p', packageName, '--target', target, '--profile', profile, '--message-format=json'], moduleEnv)
    .split('\n').filter(Boolean).map(line => JSON.parse(line));
  for (const m of messages) if (m.reason === 'compiler-message' && m.message.rendered) process.stderr.write(m.message.rendered);
  const artifact = messages.findLast(m => m.reason === 'compiler-artifact' && m.target.crate_types.includes('cdylib') && m.target.src_path && m.filenames.some(f => /\.(wasm|dylib|so|dll)$/.test(f)));
  const path = artifact?.filenames.find(f => /\.(wasm|dylib|so|dll)$/.test(f));
  if (!path) throw new Error(`${packageName}: Cargo emitted no loadable logic artifact`);
  return path;
}
function rustGrants(compat) {
  const scope=compat?.inputs?.rustGrants, ceiling=compat?.inputs?.grantCeiling;
  if(typeof scope!=='string'||typeof ceiling!=='string')throw new Error('Rust module requires its baked source grants');
  const admitted=ceiling.split('\n').map(s=>s.trim()).filter(Boolean);
  if(scope.split('\n').map(s=>s.trim()).filter(Boolean).some(s=>!admitted.includes(s)))throw new Error('Rust source grants exceed the app ceiling');
  return scope;
}
export function rustReceipt(app, compat, plan, module, target, executor) {
  const extension = executor === 'tiered' ? 'bin' : executor === 'wasm' ? 'wasm' : target.includes('apple') ? 'dylib' : target.includes('windows') ? 'dll' : 'so';
  return { version: 1, kind: 'rust', abi: 3, appId: app.id, grants: rustGrants(compat), target, executor,
    plan: rustCard('app.plan', plan), module: rustCard(`app.module.${extension}`, module) };
}

/** One compiler/baker process per producer. Requests are private file paths;
 * only the worker performs I/O or executes a disposable Wasm candidate. */
export class RustBaker {
  constructor(app, env) { this.app=app; this.env=env; this.child=null; this.closed=false; }
  async bake(args) {
    if (this.closed) throw new Error('Rust baker is closed');
    if (this.busy) throw new Error('Rust baker already has an active request');
    this.busy=true;
    try { await this.request(args); } finally { this.busy=false; }
  }
  async request(args) {
    // Built optimized and started once per producer, like the resident Contract
    // and module compilers: it interprets the candidate Wasm, which a debug
    // build does in seconds. A compiler edit is in the web receipt's inputs, so
    // the dev server rebuilds and starts a new producer.
    if (!this.child) {
      const target=resolve(this.app.target,'rust-tools'), env={...this.env,CARGO_TARGET_DIR:target};
      run({...this.app,workspace:ROOT},'cargo',['build',...cargoReproducibilityFlags(this.app,ROOT),'-q','--release','-p','exact-logic-bake'],env);
      const child=this.child=spawn(resolve(target,'release/exact-logic-bake'),['--serve'],{cwd:this.app.workspace,env,stdio:['pipe','pipe','pipe']});
      this.ready=new Promise((accept,reject)=>{this.startup={accept,reject};});
      const fail=error=>{
        if(this.child!==child)return;
        this.startup?.reject(error);this.startup=null;
        this.reply?.reject(error);this.reply=null;
        this.child=null;
      };
      child.on('error',fail);child.on('exit',(code,signal)=>fail(new Error(`Rust baker exited (${code??signal})`)));
      child.stdin.on('error',fail);
      child.stderr.on('data',data=>process.stderr.write(data));
      const lines=createInterface({input:child.stdout});
      lines.on('line',line=>{
        if(this.child!==child)return;
        try {
          const message=JSON.parse(line);
          if(message.ready===true && this.startup) {
            process.stderr.write(`Rust baker ready (pid ${message.pid})\n`);
            this.startup.accept();this.startup=null;return;
          }
          if(!this.reply || typeof message.ok!=='boolean')throw new Error('invalid Rust baker reply');
          const reply=this.reply;this.reply=null;
          if(message.ok)reply.accept();else reply.reject(new Error(message.error??'Rust bake refused'));
        } catch(error){fail(error);child.kill('SIGTERM');}
      });
    }
    await this.ready;
    if (!this.child) throw new Error('Rust baker stopped');
    await new Promise((accept,reject)=>{
      this.reply={accept,reject};
      this.child.stdin.write(JSON.stringify(args)+'\n');
    });
  }
  stop() {
    const child=this.child;this.child=null;
    this.startup?.reject(new Error('Rust baker stopped'));this.startup=null;
    this.reply?.reject(new Error('Rust baker stopped'));this.reply=null;
    if(child){child.stdin.end();child.kill('SIGTERM');}
  }
  close() { this.closed=true;this.stop(); }
}

/** A signature carries its signing time, so signing unchanged code again makes
 * new bytes. `published`, the stream's last published module, stands for
 * `fresh`, just signed, only when its signature verifies, was made with the
 * same certificate (the leaf's SHA-1) and covers the same code (CDHash).
 * Returns its bytes, else null: then the fresh signature ships. */
export function publishedSignature(fresh, published, env = process.env) {
  if (!fresh || !published) return null;
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-rust-signature-'));
  try {
    const signed = (name, bytes) => {
      const path = resolve(dir, name, 'module.dylib');
      mkdirSync(dirname(path)); writeFileSync(path, bytes);
      const shown = spawnSync('codesign', ['-d', '-vvv', `--extract-certificates=${resolve(dir, name, 'cert')}`, path], { env, encoding: 'utf8' });
      const leaf = resolve(dir, name, 'cert0');
      return { path, cdhash: shown.status === 0 ? /^CDHash=([0-9a-f]+)$/m.exec(shown.stderr)?.[1] : null,
        leaf: existsSync(leaf) ? createHash('sha1').update(readFileSync(leaf)).digest('hex') : null };
    };
    const now = signed('fresh', fresh), then = signed('published', published);
    if (!now.cdhash || !now.leaf || now.cdhash !== then.cdhash || now.leaf !== then.leaf) return null;
    return spawnSync('codesign', ['--verify', '--strict', then.path], { env }).status === 0 ? Buffer.from(published) : null;
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

/** The last modules each app compiled in this process, with their inputs' key. */
const compiledModules = new Map();
function compileModules(app, env, profile, nativeTarget) {
  const wasm = readFileSync(compile(app, 'wasm32-unknown-unknown', env, profile));
  let native = null;
  if (nativeTarget) {
    const nativePath = compile(app, nativeTarget, env, profile);
    if (nativeTarget.includes('apple')) {
      const identity = env.EXACT_RUST_SIGN_IDENTITY ?? (env.EXACT_UPDATE_TRUST === 'production' ? null : '-');
      if (!identity) throw new Error('macOS production Rust modules require EXACT_RUST_SIGN_IDENTITY matching the host team; select rust.prod="wasm" to use interpreted updates');
      const directory=resolve(app.target,'rust-sign');mkdirSync(directory,{recursive:true});
      const stage=mkdtempSync(resolve(directory,'candidate-')),copy=resolve(stage,'module.dylib');
      try {
        writeFileSync(copy,readFileSync(nativePath));
        run(app, 'codesign', ['--force', '--sign', identity, ...(identity === '-' ? [] : ['--timestamp', '--options', 'runtime']), copy], env);
        native=readFileSync(copy);
      } finally {rmSync(stage,{recursive:true,force:true});}
    }
    native ??= readFileSync(nativePath);
  }
  return { wasm, native };
}

/** Produce complete variants in private memory; publication is a separate act.
 * Native and Wasm artifacts share one baked plan and one stable input closure. */
export async function buildRust(app, { compat, env = developmentBuildEnv(), nativeTarget = null, plan = null, profile = 'logic-dev', baker = null, sourceMap = false } = {}) {
  if (!rustPackage(app)) return null;
  if (!compat?.inputs?.grantCeiling && compat?.inputs?.grantCeiling !== '') throw new Error('Rust module build requires a completed app bake receipt');
  rustGrants(compat);
  const { files: inputs, directories } = rustClosure(app, env), observed = new Map(), before = rustInputDigest(inputs, observed);
  // Contract sources outside every Cargo package feed the bake, never Cargo: a
  // Contract-only edit reuses the modules compiled from the same Rust inputs.
  const compiledFrom = inputs.filter(path => !path.endsWith('.contract') || directories.some(dir => path.startsWith(dir + '/')));
  const key = JSON.stringify([profile, nativeTarget, env.EXACT_UPDATE_TRUST ?? null, env.EXACT_RUST_SIGN_IDENTITY ?? null,
    hash(JSON.stringify(compiledFrom.map(path => [path, observed.get(path)])))]);
  const reused = compiledModules.get(app.dir);
  const { wasm, native } = reused?.key === key ? reused : compileModules(app, env, profile, nativeTarget);
  const scratchRoot = resolve(app.target, 'rust-bake'); mkdirSync(scratchRoot, { recursive: true });
  const scratch = mkdtempSync(resolve(scratchRoot, 'candidate-'));
  let map = null;
  try {
    {
      const compatPath = resolve(scratch, 'compat.json'); writeFileSync(compatPath, JSON.stringify(compat));
      const baked = resolve(scratch, 'app.plan'), candidateModule=resolve(scratch,'app.module.wasm');
      // Cargo's shared output can change while another producer builds. Bake
      // exactly the immutable bytes used by this receipt, never that live path.
      writeFileSync(candidateModule,wasm);
      const worker=baker??new RustBaker(app,env);
      try { await worker.bake([resolve(app.dir,'app.contract'),candidateModule,compatPath,baked,...(sourceMap ? ['--map'] : [])]); }
      finally { if(!baker)worker.close(); }
      const candidatePlan = readFileSync(baked);
      if (sourceMap) {
        map = readFileSync(baked + '.map.json');
        if (JSON.parse(map).digest !== hash(candidatePlan)) throw new Error('Rust source map does not match its baked plan');
      }
      if (plan && !Buffer.from(plan).equals(candidatePlan)) throw new Error('Rust wrapper and embedded data bake different first frames; they must share the same business-logic implementation');
      plan = candidatePlan;
    }
    const after = rustInputs(app, env), latest = new Map(), afterDigest = rustInputDigest(after, latest);
    if (JSON.stringify(inputs) !== JSON.stringify(after) || before !== afterDigest) {
      const changed = [...new Set([...inputs, ...after])].filter(path => observed.get(path) !== latest.get(path));
      throw new Error(`Rust inputs changed during compilation (${changed.slice(0,5).join(', ')}); the previous generation remains active`);
    }
    // Only a build whose inputs held still vouches for its modules.
    compiledModules.set(app.dir, { key, wasm, native });
    const variants = { wasm: { receipt: rustReceipt(app, compat, plan, wasm, 'wasm32-unknown-unknown', 'wasm'), bytes: wasm } };
    if (native) {
      variants.native = { receipt: rustReceipt(app, compat, plan, native, nativeTarget, 'native'), bytes: native };
      const environment=env.EXACT_UPDATE_TRUST==='production'?'prod':'dev';
      if (['macos','linux','windows','android'].some(platform=>rustPolicy(app.manifest,platform,environment)==='tiered')) {
        const bytes=tieredBytes(wasm,native);
        variants.tiered={receipt:rustReceipt(app,compat,plan,bytes,nativeTarget,'tiered'),bytes};
      }
    }
    return { plan, variants, inputs, inputDigest: before, sourceMap: map };
  } finally { rmSync(scratch, { recursive: true, force: true }); }
}

/** One authenticated container; the two executors can never name different updates. */
export function tieredBytes(wasm,native) {
  if (!wasm.subarray(0,8).equals(Buffer.from([0,97,115,109,1,0,0,0])) || !native.length || wasm.length+native.length+12>32*1024*1024) throw new Error('invalid or oversized tiered Rust modules');
  const header=Buffer.from([69,88,76,84,1,0,0,0,0,0,0,0]);header.writeUInt32LE(wasm.length,8);
  return Buffer.concat([header,wasm,native]);
}
/** The native half of a tiered container, or null. */
export function tieredNative(bytes) {
  if (bytes.length < 12 || !bytes.subarray(0,8).equals(Buffer.from([69,88,76,84,1,0,0,0]))) return null;
  const start=12+bytes.readUInt32LE(8);
  return start<bytes.length ? bytes.subarray(start) : null;
}

export function rustFiles(built) {
  const files = new Map();
  for (const [kind, variant] of Object.entries(built.variants)) {
    const prefix = kind === 'wasm' ? 'rust/wasm/' : `rust/${kind}/${variant.receipt.target}/`;
    files.set(prefix + 'app.module.json', Buffer.from(JSON.stringify(variant.receipt)));
    files.set(prefix + variant.receipt.module.file, variant.bytes);
  }
  return files;
}
export function rustCards(files) {
  const result = {};
  for (const [name, bytes] of files) {
    if (!/^rust\/(wasm|(?:native|tiered)\/[^/]+)\/app.module.json$/.test(name)) continue;
    const receipt = JSON.parse(bytes), kind = receipt.executor;
    const moduleName = name.replace('app.module.json', receipt.module.file), module = files.get(moduleName);
    if (!module || receipt.module.sha256 !== hash(module) || receipt.module.bytes !== module.length) throw new Error('Rust module does not match its receipt');
    result[kind] = { receipt: { ...rustCard(name, bytes) }, module: { ...rustCard(moduleName, module) }, ...(kind !== 'wasm' ? { target: receipt.target } : {}) };
    delete result[kind].receipt.file; delete result[kind].module.file;
    result[kind].receipt.url = name; result[kind].module.url = moduleName;
  }
  return Object.keys(result).length ? result : null;
}

/** Attach this platform's selected Rust pair as signed bundle assets. An old
 * cohort without that executor receives an explicit binary requirement. */
export function rustBundle(app, bundle, build, built) {
  const {platforms, ...base} = bundle;
  const mode = build.compat.inputs.rustMode;
  if (!built || !['native', 'tiered', 'wasm'].includes(mode)) return {bundle:base, build};
  const variant = mode==='tiered' && built.variants.native && built.variants.wasm
    ? {receipt:{target:built.variants.native.receipt.target},bytes:tieredBytes(built.variants.wasm.bytes,built.variants.native.bytes)} : built.variants[mode];
  if (!variant) throw new Error(`no Rust ${mode} artifact for ${build.compat.target}`);
  const receipt = rustReceipt(app, build.compat, bundle.plan.bytes, variant.bytes, variant.receipt.target, mode);
  const assets = [
    {name:'rust/app.module.json',bytes:Buffer.from(JSON.stringify(receipt))},
    {name:'rust/'+receipt.module.file,bytes:variant.bytes},
  ].map(a => ({...a,sha256:hash(a.bytes)}));
  const requires = {rustMode:mode,rustAbi:3,rustTarget:receipt.target,grantCeiling:build.compat.inputs.grantCeiling,rustGrants:rustGrants(build.compat)};
  const artifacts = build.graph.artifacts.map(a => {
    if (a.name !== 'app.plan') return a;
    const {sources, ...rest} = a.requires;
    return {...a,requires:{...rest,...requires}};
  });
  artifacts.push(...assets.map(a=>({name:a.name,sha256:a.sha256,bytes:a.bytes.length,kind:'bundle',requires})));
  return {bundle:{...base,assets:[...bundle.assets,...assets]},build:{...build,graph:{...build.graph,artifacts}}};
}
let hostTriple=null;
async function produce(app, env, baker = null) {
  if (!rustPackage(app)) throw new Error('declare rust.module.package in app.json and export the logic ABI from that cdylib crate');
  const compat=readBuilds(app,env).find(r=>r.compat.inputs.platform==='web')?.compat;
  hostTriple??=/^host: (.+)$/m.exec(run(app,'rustc',['-vV'],env))?.[1];
  const host=hostTriple;
  const nativeTarget=['darwin','linux'].includes(process.platform) && ['native','tiered'].includes(rustPolicy(app.manifest,process.platform==='darwin'?'macos':'linux'))?host:null;
  const built=await buildRust(app,{compat,env,nativeTarget,baker,sourceMap:true});
  const directory=rustOutput(app);mkdirSync(directory,{recursive:true});
  const generation=hash(Buffer.concat([built.plan,...Object.values(built.variants).map(v=>v.bytes),built.sourceMap]));
  const stage=mkdtempSync(resolve(directory,'stage-'));
  for(const [name,bytes] of new Map([['app.plan',built.plan],['app.plan.map.json',built.sourceMap],...rustFiles(built)])){mkdirSync(dirname(resolve(stage,name)),{recursive:true});writeFileSync(resolve(stage,name),bytes);}
  const final=resolve(directory,generation);
  if(existsSync(final))rmSync(stage,{recursive:true,force:true});else renameSync(stage,final);
  const pointer=resolve(directory,`current-${process.pid}.tmp`);
  writeFileSync(pointer,generation);renameSync(pointer,resolve(directory,'current'));
  return {generation,directory:final};
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const app=resolveApp(process.argv[2]),env=developmentBuildEnv();
  if(process.argv.includes('--serve')) {
    const baker=new RustBaker(app,env),lines=createInterface({input:process.stdin});
    try {
      for await(const line of lines) {
        let id=null;
        try {
          const request=JSON.parse(line);id=request.id;
          if(!Number.isSafeInteger(id)||id<0)throw new Error('invalid Rust build request');
          const result=await produce(app,env,baker);
          console.log(JSON.stringify({id,ok:true,...result}));
        } catch(error){console.log(JSON.stringify({id,ok:false,error:error.message}));}
      }
    } finally {baker.close();}
  } else {
    try {const result=await produce(app,env);console.log(`Rust generation ${result.generation.slice(0,12)} ready: ${result.directory}`);}
    catch(error){console.error(error.message);process.exitCode=1;}
  }
}
