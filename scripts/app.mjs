// Where an app lives. Inside this repo an app is `apps/<name>` — its crates
// are workspace members and build into `target/`. Outside it (weird-castle:
// its own repo, its own cargo workspace, depending on this repo's crates by
// path so the two iterate together), `EXACT_APP_DIR` names the directory and
// everything else follows from it: the workspace cargo runs in, the target
// directory the artifacts land in, `app.contract`, `assets/`, `gpu/`. Every
// script that builds, serves, or drives an app resolves it here, so nothing
// else knows the difference.
//
//   node host/web/build.mjs weird-castle-web          (EXACT_APP_DIR set)
//   node host/apple/build.mjs --ios weird-castle-apple --run
//   node host/web/dev.mjs --app weird-castle
//   node scripts/agent.mjs --app weird-castle macos tree
//
// The app manifest (LLP 1030 D2; 1030.000 D7): `app.json` beside
// `app.contract` — the W3C Web App Manifest's own keys, which the web host
// copies out as `manifest.json`, plus `app` (identity: the id every platform
// derives its bundle id from, the name, the origin), `host.<platform>` (what
// bake generates each platform's host files from), and `deploy` (policy,
// never identity). Validated here against `scripts/app.schema.json` with a
// validator small enough to live beside the reader; an app without one gets
// the derived defaults it had before the manifest existed.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { tmpdir } from 'node:os';

import { createHash } from 'node:crypto';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** The app `nameOrCrate` names (`caltrain`, `caltrain-web`, …; `EXACT_APP_DIR`'s basename when unset): its directory, cargo workspace, target directory, crate names, and manifest. */
export function resolveApp(nameOrCrate) {
  const outside = process.env.EXACT_APP_DIR ? resolve(process.env.EXACT_APP_DIR) : null;
  const name = nameOrCrate ? String(nameOrCrate).replace(/-(web|apple|linux|gpu)$/, '') : outside ? basename(outside) : 'caltrain';
  let dir = outside ?? resolve(ROOT, 'apps', name);
  if (!existsSync(resolve(dir, 'app.contract'))) throw new Error(`no app at ${dir} (no app.contract)${outside ? '' : '; set EXACT_APP_DIR for an app outside this repo'}`);
  dir = realpathSync(dir);
  const workspace = outside ? dir : ROOT;
  const target = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(workspace, 'target');
  const manifest = readManifest(dir, name);
  return {
    name, dir, workspace, target, crate: (kind) => `${name}-${kind}`,
    /** The manifest, validated; the derived defaults when the app has none. */
    manifest,
    /** The app identity, reverse-DNS: the bundle id on every platform (`build.mjs:48–49` derived it from the crate name before the manifest). */
    id: manifest.app.id,
    /** The name people see. */
    displayName: manifest.app.name,
    /** The production origin (1023 D1's URL), or null in an app that has not declared one. */
    origin: manifest.app.origin ?? null,
    /** Whether the app declares its own manifest (false: the defaults above stand in). */
    declared: existsSync(resolve(dir, 'app.json')),
  };
}

/** `app.json` from `dir`, validated — or the defaults an app had before the manifest: `com.exact.<name>`, the name capitalized. */
export function readManifest(dir, name) {
  const path = resolve(dir, 'app.json');
  const fallback = { name: name[0].toUpperCase() + name.slice(1), app: { id: `com.exact.${name}`, name: name[0].toUpperCase() + name.slice(1) }, host: {}, deploy: {} };
  if (!existsSync(path)) return fallback;
  let parsed;
  try { parsed = JSON.parse(readFileSync(path, 'utf8')); } catch (e) { throw new Error(`${path}: ${e.message}`); }
  const problems = validate(parsed, schema(), '', schema());
  if (problems.length) throw new Error(`${path} does not conform to scripts/app.schema.json:\n  ${problems.join('\n  ')}`);
  return { host: {}, deploy: {}, ...parsed };
}

let cachedSchema = null;
function schema() {
  cachedSchema ??= JSON.parse(readFileSync(resolve(ROOT, 'scripts/app.schema.json'), 'utf8'));
  return cachedSchema;
}

/** The subset of JSON Schema the manifest's schema uses — type, required, properties, additionalProperties, items, enum, pattern, minLength, oneOf, $ref into $defs — checked by hand so the reader needs no dependency. Every problem in one pass. */
export function validate(value, node, at, root) {
  const problems = [];
  const where = at || '(root)';
  if (node.$ref) {
    const target = node.$ref.replace(/^#\//, '').split('/').reduce((o, k) => o?.[k], root);
    if (!target) return [`${where}: schema reference ${node.$ref} does not resolve`];
    return validate(value, target, at, root);
  }
  if (node.oneOf) {
    const fits = node.oneOf.filter((alt) => validate(value, alt, at, root).length === 0);
    if (fits.length !== 1) problems.push(`${where}: ${JSON.stringify(value)} matches ${fits.length} of the allowed forms (needs exactly one)`);
    return problems;
  }
  const types = node.type ? [].concat(node.type) : null;
  const actual = value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (types && !types.includes(actual)) { problems.push(`${where}: expected ${types.join(' or ')}, got ${actual}`); return problems; }
  if (node.enum && !node.enum.includes(value)) problems.push(`${where}: ${JSON.stringify(value)} is not one of ${node.enum.map((e) => JSON.stringify(e)).join(', ')}`);
  if (typeof value === 'string') {
    if (node.minLength != null && value.length < node.minLength) problems.push(`${where}: shorter than ${node.minLength}`);
    if (node.pattern && !new RegExp(node.pattern).test(value)) problems.push(`${where}: ${JSON.stringify(value)} does not match ${node.pattern}`);
  }
  if (actual === 'array' && node.items) value.forEach((v, i) => problems.push(...validate(v, node.items, `${at}[${i}]`, root)));
  if (actual === 'object') {
    for (const key of node.required ?? []) if (!(key in value)) problems.push(`${where}: missing required ${JSON.stringify(key)}`);
    for (const [key, v] of Object.entries(value)) {
      const sub = node.properties?.[key];
      const path = at ? `${at}.${key}` : key;
      if (sub) problems.push(...validate(v, sub, path, root));
      else if (node.additionalProperties && typeof node.additionalProperties === 'object') problems.push(...validate(v, node.additionalProperties, path, root));
      else if (node.additionalProperties === false) problems.push(`${path}: not a known key`);
    }
  }
  return problems;
}

/** Developer entrypoints explicitly bake unsigned-update permission. Direct Cargo/contract bakes default to production; release callers can select it here too. */
export function developmentBuildEnv() {
  return { ...process.env, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' };
}

/** The private directory receiving documents emitted by actual app build scripts. */
export function bakeOutput(app, env = process.env) {
  return env.EXACT_BAKE_OUTPUT ?? resolve(app.target, 'bake', app.id, env.EXACT_UPDATE_TRUST ?? 'production');
}

/** Read and validate the receipt emitted by the app's actual target/grants bake. */
export function readBake(app, platform, target, directory = bakeOutput(app)) {
  const receipt = JSON.parse(readFileSync(resolve(directory, `${platform}-${target}.json`), 'utf8'));
  const canonical = (v) => v === null || typeof v !== 'object' ? JSON.stringify(v) : Array.isArray(v) ? `[${v.map(canonical).join(',')}]` : `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canonical(v[k])}`).join(',')}}`;
  const id = createHash('sha256').update('exact2 compatibility id v1\n').update(canonical(receipt.inputs)).digest('hex').slice(0, 32);
  if (receipt.id !== id || receipt.inputs.app !== app.id || receipt.inputs.platform !== platform || receipt.target !== target || !receipt.embedded || !Array.isArray(receipt.embedded.assets)) throw new Error(`invalid baked receipt for ${app.id} ${platform} ${target}`);
  return receipt;
}

/** Refuse packaging bytes that differ from the binary's complete bake receipt. */
export function verifyBakeFiles(receipt, plan, assets) {
  const embedded = receipt.embedded;
  const ordered = (cards) => [...cards].sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  if (!embedded || !Array.isArray(embedded.assets)
      || embedded.plan?.sha256 !== createHash('sha256').update(plan).digest('hex')
      || embedded.plan?.bytes !== plan.length) {
    throw new Error('the packaged plan differs from the binary bake receipt');
  }
  const copied = ordered(assets), baked = ordered(embedded.assets);
  if (copied.length !== baked.length
      || copied.some((asset, i) => ['name', 'sha256', 'bytes'].some((key) => asset[key] !== baked[i][key]))) {
    throw new Error('the packaged static files differ from the binary bake receipt');
  }
}


// The compiler owns both the loaded files and the bundle requirements. This
// outer receipt is completed after Cargo succeeds; it is not embedded in the
// product whose inputs it describes. @ref LLP 1030 D3/D3a.
const canonicalBuild = (v) => v === null || typeof v !== 'object' ? JSON.stringify(v) : Array.isArray(v) ? `[${v.map(canonicalBuild).join(',')}]` : `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canonicalBuild(v[k])}`).join(',')}}`;
const buildHash = (v) => createHash('sha256').update(v).digest('hex');
const under = (root, path) => path === root || path.startsWith(root + '/');
const orderedBuild = (rows) => rows.sort((a, b) => Buffer.compare(Buffer.from(canonicalBuild(a)), Buffer.from(canonicalBuild(b))));
function buildCommand(command, args, app, env) {
  const result = spawnSync(command, args, { cwd: app.workspace, env, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  if (result.error || result.status !== 0) throw new Error(`${command} ${args.join(' ')} failed: ${result.error?.message ?? result.stderr}\n${(result.stdout ?? '').slice(-4000)}`);
  return result;
}
export function bakeTarget(platform) {
  if (platform === 'web') return 'wasm32-unknown-unknown';
  if (platform === 'ios') return 'aarch64-apple-ios';
  // The Linux host is also a supported headless executable on macOS. This
  // is the actual target Cargo will build here, never a guessed architecture.
  const result = spawnSync('rustc', ['-vV'], { encoding: 'utf8' });
  const host = /^host: (.+)$/m.exec(result.stdout ?? '')?.[1];
  if (result.status !== 0 || !host) throw new Error('rustc did not report its host target');
  return host;
}
function buildGraph(app, target, kind, env, gpu) {
  const metadata = JSON.parse(buildCommand('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', target], app, env).stdout);
  const packages = new Map(metadata.packages.map((p) => [p.id, p]));
  const nodes = new Map(metadata.resolve.nodes.map((n) => [n.id, n]));
  const root = metadata.packages.find((p) => p.name === app.crate(kind));
  const surface = gpu && metadata.packages.find((p) => p.name === app.crate('gpu'));
  if (!root) throw new Error(`Cargo has no ${app.crate(kind)} target`);
  const roles = new Map(), pending = [[root.id, target], ...(surface ? [[surface.id, target]] : [])];
  while (pending.length) {
    let [id, role] = pending.pop();
    const pkg = packages.get(id), node = nodes.get(id);
    if (!pkg || !node) throw new Error(`incomplete Cargo dependency graph: ${id}`);
    if (pkg.targets.some((t) => t.kind.includes('proc-macro'))) role = 'host';
    const known = roles.get(id) ?? new Set(); if (known.has(role)) continue;
    known.add(role); roles.set(id, known);
    for (const dep of node.deps) if (dep.dep_kinds.some((k) => k.kind === null)) pending.push([dep.pkg, role]);
  }
  return { metadata, packages, root, surface, roles };
}
function compilerPaths(text, workspace) {
  const first = text.replace(/\\\r?\n/g, '').split('\n')[0];
  const at = first.indexOf(': ');
  if (at < 0) throw new Error('rustc dep-info has no dependency rule');
  const paths = []; let word = '', escape = false;
  for (const ch of first.slice(at + 2)) {
    if (escape) { word += ch; escape = false; }
    else if (ch === '\\') escape = true;
    else if (/\s/.test(ch)) { if (word) { paths.push(resolve(workspace, word)); word = ''; } }
    else word += ch;
  }
  if (escape) throw new Error('rustc dep-info has a truncated escape');
  if (word) paths.push(resolve(workspace, word));
  return paths;
}
function unitDepInfo(message, app, named) {
  if (named) return named;
  for (const file of message.filenames) {
    const stem = basename(file).replace(/\.[^.]+$/, '').replace(/^lib/, '');
    const candidate = resolve(dirname(file), stem + '.d');
    if (!existsSync(candidate)) continue;
    if (compilerPaths(readFileSync(candidate, 'utf8'), app.workspace).includes(resolve(message.target.src_path))) return candidate;
  }
  throw new Error(`no matching rustc unit dep-info for ${message.target.name}; rebuild the stale Cargo unit or use a private target directory`);
}
function completeBuild(app, platform, target, graph, messages, roots, env) {
  const scripts = messages.filter((m) => m.reason === 'build-script-executed' && graph.roles.has(m.package_id));
  const roleOf = (path) => under(resolve(app.target, target), path) ? target : 'host';
  const generated = scripts.map((m) => ({ path: resolve(m.out_dir), pkg: graph.packages.get(m.package_id), role: roleOf(m.out_dir) })).sort((a,b) => b.path.length-a.path.length);
  const rootOutput = generated.find((g) => g.pkg.id === graph.root.id && g.role === target)?.path;
  if (!rootOutput) throw new Error('Cargo did not report the selected target bake output');
  const compat = JSON.parse(readFileSync(resolve(rootOutput, 'compat.json'), 'utf8'));
  if (compat.target !== target || compat.inputs.platform !== platform || compat.inputs.app !== app.id) throw new Error('actual bake receipt names another target, platform or app');
  const bundleGraph = JSON.parse(readFileSync(resolve(rootOutput, 'artifacts.json'), 'utf8'));
  const replaced = new Set(['app.plan','compat.json','artifacts.json'].map((n) => resolve(rootOutput,n)));
  const packages = [...graph.roles.keys()].map((id) => graph.packages.get(id));
  const locations = packages.map((p) => ({ path: dirname(p.manifest_path), name:`crate:${p.name}@${p.version}` })).sort((a,b) => b.path.length-a.path.length);
  // contract-cli includes the canonical declaration as text without linking
  // ibex2 into the compiler. It is therefore a compiler input even on targets
  // whose Cargo graph correctly omits the native runtime crate. Its bytes are
  // hashed below exactly like every other input; only this file is admitted.
  const storageTypes = resolve(ROOT, '../ibex/crates/ibex2/src/bindings/storage.d.ts');
  const nameOf = (path) => {
    path = resolve(path);
    const made = generated.find((g) => under(g.path,path));
    if (made) return `generated:${made.pkg.name}:${made.role}/${relative(made.path,path)}`;
    const pkg = locations.find((p) => under(p.path,path));
    if (pkg) return `${pkg.name}/${relative(pkg.path,path)}`;
    if (under(app.dir,path)) return `app/${relative(app.dir,path)}`;
    if (under(ROOT,path)) return `exact/${relative(ROOT,path)}`;
    if (under(graph.metadata.workspace_root,path)) return `workspace/${relative(graph.metadata.workspace_root,path)}`;
    if (path === storageTypes) return 'included:ibex2/src/bindings/storage.d.ts';
    throw new Error(`compiler input has no captured source identity: ${path}`);
  };
  const inputs = new Map(), absent = new Map(), directories = new Map();
  const add = (path, optional = false) => {
    path = resolve(path); if (replaced.has(path)) return;
    if (!existsSync(path)) { if (optional) { absent.set(nameOf(path), path); return; } throw new Error(`stale compiler dependency names missing input ${path}; rebuild that Cargo unit`); }
    const info = statSync(path);
    if (info.isDirectory()) { const names = readdirSync(path).sort(); directories.set(nameOf(path), {path,names}); for (const name of names) add(resolve(path,name)); }
    else if (info.isFile()) inputs.set(nameOf(path), {name:nameOf(path),path,sha256:buildHash(readFileSync(path))});
    else throw new Error(`unsupported compiler input ${path}`);
  };
  const normalizeEnv = ([key,value]) => [key, value == null ? null : ['OUT_DIR','CARGO_MANIFEST_DIR'].includes(key) ? nameOf(value) : buildHash(value)];
  const units = [], usedPackages = new Set();
  for (const m of messages.filter((m) => m.reason === 'compiler-artifact' && !m.target.kind.includes('custom-build') && graph.roles.has(m.package_id))) {
    const role = roleOf(m.filenames[0]); if (!graph.roles.get(m.package_id).has(role)) continue;
    usedPackages.add(m.package_id);
    const selected = roots.find((r) => r.package === m.package_id && m.target.name === r.name);
    const dep = readFileSync(unitDepInfo(m,app,selected?.dep), 'utf8');
    for (const path of compilerPaths(dep,app.workspace)) add(path);
    const environment = dep.split('\n').filter((s) => s.startsWith('# env-dep:')).map((s) => { const pair=s.slice(10),at=pair.indexOf('=');return at<0?[pair,null]:[pair.slice(0,at),pair.slice(at+1)]; }).map(normalizeEnv);
    units.push({package:graph.packages.get(m.package_id).name,role,target:m.target.name,kind:m.target.kind,features:m.features,profile:m.profile,environment});
  }
  const builders = [];
  for (const m of scripts) {
    if(!usedPackages.has(m.package_id))continue;
    const role = roleOf(m.out_dir); if (!graph.roles.get(m.package_id).has(role)) continue;
    const pkg = graph.packages.get(m.package_id), source = pkg.targets.find((t) => t.kind.includes('custom-build'));
    if (source) add(source.src_path);
    const output = readFileSync(resolve(m.out_dir,'../output'),'utf8'); const environment = [];
    for (const line of output.split('\n')) {
      const changed = /^cargo::?rerun-if-changed=(.*)$/.exec(line);
      // The app's plan/manifest/static watches are inputs of replaceable
      // artifacts. Its generated entry and typed metadata are binary inputs.
      if (changed && pkg.id !== graph.root.id) {
        const path = resolve(dirname(pkg.manifest_path), changed[1]);
        if (!(pkg.id === graph.surface?.id && under(resolve(app.dir, 'gpu/shaders'), path))) add(path, true);
      }
      const variable = /^cargo::?rerun-if-env-changed=(.*)$/.exec(line)?.[1];
      if (variable && !variable.startsWith('EXACT_') && !['OUT_DIR','CARGO_MANIFEST_DIR'].includes(variable)) environment.push(normalizeEnv([variable,env[variable]??null]));
    }
    // A linked native archive is an input too; Rust dep-info cannot name its C/ObjC bytes.
    for (const raw of m.linked_paths) {
      const dir = raw.replace(/^native=/, '');
      if (under(resolve(m.out_dir), resolve(dir))) for (const file of readdirSync(dir)) if (/\.(a|o|dylib|so)$/.test(file)) add(resolve(dir,file));
    }
    builders.push({package:pkg.name,role,cfgs:m.cfgs,environment:orderedBuild(environment),libraries:m.linked_libs,env:m.env.filter(([key])=>!key.startsWith('EXACT_')).map(normalizeEnv)});
  }
  for (const pkg of packages) if(usedPackages.has(pkg.id))add(pkg.manifest_path);
  add(resolve(graph.metadata.workspace_root,'Cargo.toml'));add(resolve(graph.metadata.workspace_root,'Cargo.lock'));
  if (platform === 'macos' || platform === 'ios') {
    const packageRoot=resolve(ROOT,'host/apple');
    const swiftEnv = {...env, EXACT_APP_COMPOSITION: compat.inputs.store.L === '0' ? 'embedded' : 'updating'}; delete swiftEnv.SDKROOT;
    const swift=JSON.parse(buildCommand('swift',['package','--package-path',packageRoot,'describe','--type','json'],app,swiftEnv).stdout);
    const pending=[platform==='ios'?'ExactIOS':'ExactMac'],seen=new Set();
    while(pending.length) { const name=pending.pop();if(seen.has(name))continue;seen.add(name);const unit=swift.targets.find((t)=>t.name===name);if(!unit)throw new Error(`Swift package has no ${name}`);
      for(const file of unit.sources??[])add(resolve(packageRoot,unit.path,file));
      for(const resource of unit.resources??[])add(resolve(packageRoot,resource.path));
      if(unit.type==='system-target')add(resolve(packageRoot,unit.path));pending.push(...(unit.target_dependencies??[]));
    }
    add(resolve(packageRoot,'Package.swift'));add(resolve(packageRoot,'webarm/WebArm.swift'));add(resolve(packageRoot,'build.mjs'));
  }
  if(platform==='web') {
    for(const path of ['host/web/glue.js','host/web/gpu-glue.js','host/web/index.html','host/web/build.mjs']) add(resolve(ROOT,path));
    if (existsSync(resolve(app.dir, 'app.ts'))) {
      // The TS producer is a build dependency, outside the runtime Cargo graph.
      // Its canonical API declaration still determines the accepted app module.
      add(storageTypes);
      for (const path of ['host/web/module-glue.js', 'js/src/prelude.js',
        'host/web/storage.js', 'host/web/storage-fs.js', 'host/web/storage-sqlite.js', 'host/web/storage-worker.js',
        'package.json', 'package-lock.json', 'node_modules/@sqlite.org/sqlite-wasm/package.json',
        'node_modules/@sqlite.org/sqlite-wasm/dist/index.mjs', 'node_modules/@sqlite.org/sqlite-wasm/dist/sqlite3.wasm']) add(resolve(ROOT, path));
    }
  }
  const metadata={app:app.manifest.app,host:app.manifest.host?.[platform]??{},icons:app.manifest.icons??[],delivery:compat.delivery,store:compat.inputs.store,keys:compat.inputs.keys};
  const configuration={target,units:orderedBuild([...new Map(units.map(u=>[canonicalBuild(u),u])).values()]),builders:orderedBuild([...new Map(builders.map(u=>[canonicalBuild(u),u])).values()]),rustc:buildCommand('rustc',['-vV'],app,env).stdout,flags:Object.fromEntries(['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','MACOSX_DEPLOYMENT_TARGET','IPHONEOS_DEPLOYMENT_TARGET'].map((k)=>[k,env[k]??null]))};
  const files=[...inputs.values()].sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0);
  const fingerprint={files:files.map(({name,sha256})=>({name,sha256})),absent:[...absent.keys()].sort(),configuration,metadata};
  const products=roots.flatMap((r)=>messages.filter((m)=>m.reason==='compiler-artifact'&&m.package_id===r.package&&m.target.name===r.name).flatMap((m)=>m.filenames)).filter((p)=>!p.endsWith('.d')).map((path)=>({path,bytes:statSync(path).size,sha256:buildHash(readFileSync(path))}));
  return {version:1,trust:env.EXACT_UPDATE_TRUST??'production',compat,graph:bundleGraph,binary:{sha256:buildHash(canonicalBuild(fingerprint)),...fingerprint,inputs:files,directories:[...directories.values()],missing:[...absent.values()]},products};
}

/** One actual target build, including the optional GPU artifact. Consumers
 * classify its completed receipt; compatibility is never recomputed in JS. */
export function buildBake(app, platform, target, options = {}) {
  const kind=platform==='macos'||platform==='ios'?'apple':platform;
  const env={...process.env,...options.env};env.CARGO_TARGET_DIR=app.target;env.EXACT_BAKE_OUTPUT=options.output??bakeOutput(app,env);
  if(options.analysis && env.EXACT_UPDATE_TRUST==='production')env.EXACT_BAKE_ANALYSIS='1';else delete env.EXACT_BAKE_ANALYSIS;
  mkdirSync(env.EXACT_BAKE_OUTPUT,{recursive:true});
  const graph=buildGraph(app,target,kind,env,platform!=='linux'&&existsSync(resolve(app.dir,'gpu/Cargo.toml'))),messages=[],roots=[];
  for(const pkg of [graph.surface,graph.root].filter(Boolean)) {
    const unit=pkg.targets.find((t)=>pkg.id===graph.root.id&&kind==='linux'?t.kind.includes('bin'):t.kind.some((k)=>['cdylib','staticlib','rlib','lib'].includes(k)));
    if(!unit)throw new Error(`Cargo has no buildable target for ${pkg.name}`);
    const dep=resolve(env.EXACT_BAKE_OUTPUT,`${platform}-${target}-${pkg.name}.d`);
    const args=['rustc','--locked','--offline','-p',pkg.name,'--target',target,'--profile',options.profile??(platform==='web'?'web':'release'),...(kind==='linux'&&pkg.id===graph.root.id?['--bin',unit.name]:['--lib']),...(pkg.id===graph.surface?.id?['--config',`profile.${options.profile??(platform==='web'?'web':'release')}.strip=false`]:[]),'--message-format=json','--',`--emit=dep-info=${dep}`];
    const result=buildCommand('cargo',args,app,env);if(result.stderr)process.stderr.write(result.stderr);
    const output=result.stdout.split('\n').filter(Boolean).map((line)=>JSON.parse(line));messages.push(...output);roots.push({package:pkg.id,name:unit.name,dep});
    for(const message of output)if(message.reason==='compiler-message'&&message.message.rendered)process.stderr.write(message.message.rendered);
  }
  const receipt=completeBuild(app,platform,target,graph,messages,roots,env);
  writeFileSync(resolve(env.EXACT_BAKE_OUTPUT,`${platform}-${target}.build.json`),JSON.stringify(receipt)+'\n');return receipt;
}

/** Read completed binary-producing receipts only; absent platforms remain
 * unbuilt rather than acquiring a second, guessed compatibility identity. */
export function readBuilds(app, env = process.env) {
  const directory=bakeOutput(app,env);if(!existsSync(directory))return [];
  return readdirSync(directory).filter((name)=>name.endsWith('.build.json')).map((name)=>JSON.parse(readFileSync(resolve(directory,name),'utf8'))).filter((r)=>r.version===1&&r.compat?.inputs?.app===app.id);
}

/** Frozen capabilities and the binary input identity; small enough to bind
 * inside the signed stream envelope. Absolute source paths stay private. */
export function cohortReceipt(build) {
  if (build?.version !== 1 || build.graph?.version !== 1 || !build.binary?.sha256) throw new Error('missing completed bake graph and binary receipt');
  return {version:1,compat:{id:build.compat.id,inputs:build.compat.inputs,target:build.compat.target},sources:build.graph.sources,surfaceCalls:build.graph.surfaceCalls??{},binary:build.binary.sha256};
}

/** The shared dev/deploy comparison (LLP 1030 D3). A new binary and a safe
 * bundle are independent results. Compatibility-id equality decides neither. */
export function classifyArtifacts(candidate, cohort, signingKey = null) {
  const missing=[];
  if (!candidate?.graph?.artifacts || !cohort?.sources || !cohort?.compat?.inputs) return {binary:true,bundle:false,missing:['bake graph: the installed cohort has no authenticated capability receipt'],warnings:[]};
  const have=cohort.compat.inputs;
  if(candidate.trust==='production'&&have.store?.L!=='0') {
    if(!signingKey||have.keys?.[signingKey.id]!==signingKey.public)missing.push(`envelope: signing key ${signingKey?.id??'(not selected)'} is not carried by this cohort`);
  }
  for(const artifact of candidate.graph.artifacts) for(const [key,need] of Object.entries(artifact.requires)) {
    const fail=(detail)=>missing.push(`${artifact.name}: ${key}${detail}`);
    if(key==='sources') {
      for(const [name,shape] of Object.entries(need)) if(canonicalBuild(cohort.sources[name]??null)!==canonicalBuild(shape)) fail(`.${name} (missing source or different parameter/result shape)`);
    } else if(key==='surfaceCalls') {
      for(const [name,arities] of Object.entries(need)) for(const arity of arities) if(!cohort.surfaceCalls?.[name]?.includes(arity)) fail(`.${name}/${arity} (not demanded by the installed plan)`);
    } else if(key==='gpuSurfaces') {
      for(const surface of need) if(!have.gpuSurfaces?.some(s=>canonicalBuild(s)===canonicalBuild(surface))) fail(`.${surface.name} (interface ${surface.interface})`);
    } else if(key==='executors') {
      for(const executor of need) if(!have.executors?.includes(executor)) fail(`.${executor}`);
    } else if(key==='grantCeiling') {
      const grants=new Set((have.grantCeiling??'').split('\n').filter(Boolean));
      if(need===null||have.grantCeiling===null) fail(' (unknown baked grants)');
      else for(const grant of need.split('\n').filter(Boolean)) if(!grants.has(grant)) fail(` (${grant})`);
    } else if(canonicalBuild(have[key]??null)!==canonicalBuild(need)) fail(` (requires ${canonicalBuild(need)})`);
  }
  const changed=candidate.binary.sha256!==cohort.binary || (candidate.pendingInputs?.length ?? 0)>0;
  const warnings=[];
  if(changed&&canonicalBuild(candidate.compat.inputs.dataCrate)!==canonicalBuild(have.dataCrate)&&Object.keys(candidate.graph.sources).some(n=>n!=='exactDelivery'&&cohort.sources[n])) warnings.push(`same name, same shape, new native code: cohort ${cohort.compat.id} will run the old code`);
  if(changed&&Object.keys(candidate.graph.surfaceCalls??{}).length) warnings.push(`same surface name and arity: cohort ${cohort.compat.id} retains its old GPU implementation`);
  return {binary:changed,bundle:missing.length===0&&have.store?.L!=='0',missing:have.store?.L==='0'?['store.L=0: this binary has no bundle carrier']:missing,warnings};
}

/** Overlay the resident compiler's plan graph with the complete static
 * candidate captured by dev. Reflected shader interfaces are required. */
export function developmentCandidate(build, plan, assets, surfaces) {
  const node=build.graph.artifacts.find(a=>a.name==='app.plan');
  if(node?.sha256!==plan.sha256||node.bytes!==plan.bytes) throw new Error('the candidate plan and its compiler graph differ');
  return {...build,graph:{...build.graph,artifacts:[node,...assets.map(asset=>{
    const stem=asset.name.startsWith('shaders/')&&asset.name.endsWith('.wgsl')?asset.name.slice(8,-5):null;
    if(stem!==null&&!surfaces.has(stem)) throw new Error(`shader ${stem} has no reflected interface`);
    return {...asset,kind:'bundle',requires:stem===null?{}:{gpuSurfaces:[{name:stem,interface:surfaces.get(stem)}]}};
  })]}};
}

/** Source staleness is provisional until the next actual bake. These paths
 * came from the previous compiler receipt, including absent watched inputs. */
export function pendingBuildInputs(build) {
  const changed=[];
  for(const file of build.binary.inputs) {
    try {if(!statSync(file.path).isFile()||buildHash(readFileSync(file.path))!==file.sha256)changed.push(file.name);}
    catch {changed.push(file.name);}
  }
  for(const path of build.binary.missing)if(existsSync(path))changed.push(path);
  for(const {path,names} of build.binary.directories) {
    try {if(!statSync(path).isDirectory()||canonicalBuild(readdirSync(path).sort())!==canonicalBuild(names))changed.push(path);}
    catch {changed.push(path);}
  }
  return changed;
}

/** Diagnostics that edit inputs run on the deploy snapshot's closed source
 * graph. Build outputs and child process state belong to this invocation. */
export async function withAppFixture(app, use) {
  const started = Date.now();
  const { snapshotOf, materializeSnapshot, disposeSnapshot } = await import('./deploy.mjs');
  const run = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-diagnostic-')));
  let snapshot;
  try {
    snapshot = snapshotOf(app, { dirty: true }, ROOT);
    // materializeSnapshot honors the caller's target override. Scope this
    // synchronous call to the diagnostic's private target, then restore it.
    const previousTarget = process.env.CARGO_TARGET_DIR;
    let fixture;
    try {
      process.env.CARGO_TARGET_DIR = resolve(run, 'target');
      fixture = materializeSnapshot(snapshot, run, app);
    } finally {
      if (previousTarget === undefined) delete process.env.CARGO_TARGET_DIR;
      else process.env.CARGO_TARGET_DIR = previousTarget;
    }
    const env = { ...process.env, EXACT_APP_DIR: fixture.app.dir, EXACT2: fixture.exactRoot,
      CARGO_TARGET_DIR: resolve(run, 'target'), EXACT_WEB_DIST: resolve(fixture.exactRoot, 'host/web/dist'),
      EXACT_BAKE_OUTPUT: resolve(run, 'bake'), EXACT_UPDATE_DIR: resolve(run, 'update'),
      EXACT_DIAGNOSTIC_ROOT: fixture.exactRoot, GIT_CEILING_DIRECTORIES: dirname(fixture.sourceRoot),
      GIT_DISCOVERY_ACROSS_FILESYSTEM: '0', EXACT_DIAGNOSTIC_SOURCE: JSON.stringify({
        app: { name: app.name, id: app.id, dir: app.dir }, snapshot: snapshot.id, started,
        sources: snapshot.sources.map(({ repo, roles, commit, workingSha256 }) => ({ repo, roles, commit, workingSha256 })),
      }) };
    for (const name of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE',
      'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'EXACT_DEPLOY_CAPSULE',
      'EXACT_UPDATE_RECEIPT', 'EXACT_UPDATE_GENESIS', 'EXACT_UPDATE_ORIGIN', 'EXACT_GPU_DYLIB',
      'EXACT_DEV_PLAN', 'EXACT_PLAN', 'EXACT_ASSETS']) delete env[name];
    const barrier = resolve(fixture.sourceRoot, '.git');
    if (readFileSync(barrier, 'utf8') !== 'exact deploy source boundary\n') throw new Error('unexpected diagnostic Git boundary');
    rmSync(barrier);
    const git = (args) => {
      const result = spawnSync('git', ['-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
        '-c', 'user.name=Exact diagnostic', '-c', 'user.email=diagnostic@exact.invalid', ...args],
      { cwd: fixture.sourceRoot, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
      if (result.status !== 0) throw new Error(`diagnostic git ${args[0]}: ${result.stderr || result.error?.message}`);
      return result.stdout;
    };
    for (const args of [['init', '-q', '-b', 'main'], ['add', '-f', '-A'], ['commit', '-qm', 'Captured diagnostic source']]) git(args);
    const manifest = readManifest(fixture.app.dir, app.name);
    return await use({ ...fixture, run, env, git, snapshot, app: { ...app, ...fixture.app,
      target: env.CARGO_TARGET_DIR, manifest, id: manifest.app.id, displayName: manifest.app.name,
      origin: manifest.app.origin ?? null } });
  } finally {
    try { if (snapshot) disposeSnapshot(snapshot); }
    finally { rmSync(run, { recursive: true, force: true }); }
  }
}
