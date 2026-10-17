// Builds and bakes (scripts/app.mjs `buildBake`, `bakeTarget`): artifacts and their evidence, receipts, GPU modules and shader
// packs, the build environment and what makes a build stale.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { basename, delimiter, dirname, resolve } from 'node:path';
import { classifyArtifacts, buildBake, bakeTarget, pendingBuildInputs } from './app.mjs';
import { pinned } from './test-workspace.mjs';

// These cases run cargo (the filesystem tool, bakes, locks). A shell whose PATH
// omits rustup's bin directory still finds it there; without cargo, say so.
const cargoBin = resolve(process.env.CARGO_HOME ?? resolve(homedir(), '.cargo'), 'bin');
if (!(process.env.PATH ?? '').split(delimiter).includes(cargoBin)) process.env.PATH = `${process.env.PATH ?? ''}${delimiter}${cargoBin}`;
if (!Bun.which('cargo', { PATH: process.env.PATH })) throw new Error(`these tests need cargo: put it on PATH or in ${cargoBin}`);
// The fixtures name their apps; a caller's EXACT_APP_DIR would redirect every one.
delete process.env.EXACT_APP_DIR;

test('runner-owned sources never warn that native app code is retained', () => {
  for (const name of ['exactSurface', 'exactViewport', 'exactDelivery', 'appData']) {
    const sources = { [name]: { params: [], result: {} } };
    const candidate = { binary: { sha256: 'new' }, graph: { artifacts: [], sources }, compat: { inputs: { dataCrate: 'new' } } };
    const cohort = { binary: 'old', sources, compat: { id: 'old', inputs: { dataCrate: 'old' } } };
    assert.equal(classifyArtifacts(candidate, cohort).warnings.length, name === 'appData' ? 1 : 0, name);
  }
});

// Opt in because this exercises three real Cargo bakes, not a mocked driver.
test.skipIf(!process.env.EXACT_ASSET_BAKE_TEST)('creating optional asset roots rebakes once and then stays fresh', async () => {
  const { mkdtempSync, mkdirSync, writeFileSync, statSync, rmSync, readFileSync, readdirSync } = await import('node:fs');
  const { resolve } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const { spawnSync } = await import('node:child_process');
  const { buildBake, readManifest } = await import('./app.mjs');
  const root = resolve(import.meta.dir, '..'), dir = mkdtempSync(resolve(tmpdir(), 'exact-asset-roots-'));
  try {
    mkdirSync(resolve(dir, 'web/src'), { recursive: true });
    writeFileSync(resolve(dir, 'rust-toolchain.toml'), readFileSync(resolve(root, 'rust-toolchain.toml')));
    writeFileSync(resolve(dir, 'Cargo.toml'), `[workspace]\nmembers=["web"]\nresolver="2"\n[patch.crates-io]\ntaffy={path=${JSON.stringify(resolve(root, 'vendor/taffy'))}}\n`);
    writeFileSync(resolve(dir, 'web/Cargo.toml'), `[package]\nname="asset-roots-web"\nversion="0.1.0"\nedition="2021"\n[lib]\ncrate-type=["cdylib"]\n[dependencies]\nexact-runner={path=${JSON.stringify(resolve(root, 'runner'))}}\nexact-web={path=${JSON.stringify(resolve(root, 'host/web'))}}\n[build-dependencies]\nexact-game-app={path=${JSON.stringify(resolve(root, 'game/app'))}}\n`);
    writeFileSync(resolve(dir, 'web/build.rs'), 'fn main() { exact_game_app::bake("web", ".."); }');
    writeFileSync(resolve(dir, 'web/src/lib.rs'), 'include!(concat!(env!("OUT_DIR"), "/entry.rs"));');
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    text "assets"\n');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Asset roots', app: { id: 'com.exact.assetroots', name: 'Asset roots' }, rust: false, deploy: { store: { web: '0' } } }));
    const lock = spawnSync('cargo', ['generate-lockfile', '--offline'], { cwd: dir, encoding: 'utf8' });
    assert.equal(lock.status, 0, lock.stderr);
    const manifest = readManifest(dir, 'asset-roots');
    const app = { dir, workspace: dir, target: resolve(root, 'target'), name: 'asset-roots', id: manifest.app.id, manifest, crate: kind => `asset-roots-${kind}` };
    const output = resolve(dir, 'bakes'), target = 'wasm32-unknown-unknown';
    const bake = () => buildBake(app, 'web', target, { output, profile: 'dev', env: { EXACT_UPDATE_TRUST: 'development' } });
    const receiptPath = resolve(output, `web-${target}.json`);
    bake();
    // Packaging recopies receipts even for a cache hit; measure Cargo's actual OUT_DIR.
    const builds = resolve(app.target, target, 'debug/build');
    const unit = readdirSync(builds).find(name => name.startsWith('asset-roots-web-') && (() => {
      try { return readFileSync(resolve(builds, name, 'output'), 'utf8').includes(dir); } catch { return false; }
    })());
    assert.ok(unit);
    const bakedPath = resolve(builds, unit, 'out/compat.json');
    let before = statSync(bakedPath, { bigint: true }).mtimeNs;
    for (const [directory, prefix] of [['assets', 'assets'], ['deck', 'deck'], ['gpu/shaders', 'shaders']]) {
      mkdirSync(resolve(dir, directory), { recursive: true });
      const name=prefix==='shaders'?'x.wgsl':'x.png';
      writeFileSync(resolve(dir,directory,name),prefix==='shaders'?'@compute @workgroup_size(1) fn cs() {}':Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64'));
      bake();
      const changed = statSync(bakedPath, { bigint: true }).mtimeNs;
      assert.notEqual(changed, before, `${directory}: bake did not rerun`);
      const receipt = JSON.parse(readFileSync(receiptPath, 'utf8'));
      assert.ok(receipt.embedded.assets.some(a => a.name === `${prefix}/${name}`));
      bake();
      assert.equal(statSync(bakedPath, { bigint: true }).mtimeNs, changed, `${directory}: unchanged third build reran`);
      before = changed;
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 300000);

// Real Cargo units include named game logic for the lint and absent-art receipt.
test.skipIf(!process.env.EXACT_BAKE_CACHE_TEST)('native bakes stay fresh and retain unit source and environment evidence', () => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact native cache-')));
  const write = (path, bytes) => { mkdirSync(dirname(resolve(dir, path)), {recursive:true}); writeFileSync(resolve(dir, path), bytes); };
  try {
    write('rust-toolchain.toml', readFileSync(resolve(import.meta.dir, '../rust-toolchain.toml')));
    write('Cargo.toml', '[workspace]\nmembers=["gpu","linux"]\nresolver="2"\n');
    write('gpu/Cargo.toml', '[package]\nname="cache-gpu"\nversion="0.1.0"\nedition="2021"\n[lib]\nname="cache_module"\ncrate-type=["cdylib","staticlib","rlib"]\n');
    write('gpu/src/lib.rs', '#[no_mangle]\npub extern "C" fn fixture() -> usize { env!("CACHE_FIXTURE_VALUE").len() + include_bytes!("../payload").len() }\n');
    write('gpu/payload', 'first');
    write('linux/Cargo.toml', '[package]\nname="cache-linux"\nversion="0.1.0"\nedition="2021"\n[[bin]]\nname="cache-runner"\npath="src/main.rs"\n');
    write('linux/src/main.rs', 'fn main() { println!("{}", env!("CACHE_EXEC_VALUE").len() + include_bytes!("../payload").len()); }\n');
    write('linux/payload', 'first');
    const target = bakeTarget('linux'), id = 'com.exact.cachefixture';
    const compat = {target, inputs:{platform:'linux', app:id, store:{L:'0'}, keys:[]}};
    write('linux/build.rs', `fn main() {
      println!("cargo:rerun-if-changed=build.rs");
      let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
      std::fs::write(out.join("compat.json"), r#"${JSON.stringify(compat)}"#).unwrap();
      std::fs::write(out.join("artifacts.json"), r#"{"version":1,"artifacts":[],"sources":{}}"#).unwrap();
      std::fs::write(out.join("app.plan"), b"fixture").unwrap();
    }`);
    write('.shells/Cargo.toml', '[workspace]\n[package]\nname="cache-logic"\nversion="0.1.0"\nedition="2021"\n'); write('.shells/src/lib.rs', 'pub fn fixture() -> bool { true }\n');
    for (const cwd of [dir, resolve(dir,'.shells')]) { const lock = spawnSync('cargo', ['generate-lockfile', '--offline'], {cwd, encoding:'utf8'}); assert.equal(lock.status, 0, lock.stderr); }
    const app = {dir, workspace:dir, target:resolve(dir,'target'), id, hasGpu:true,
      manifest:{app:{id, name:'Cache fixture'}, game:{crate:'cache-logic'}, rust:false}, crate:kind=>`cache-${kind}`};
    const bake = (value, executable = value) => buildBake(app, 'linux', target, {profile:'dev', output:resolve(dir,'bakes'),
      env:{EXACT_UPDATE_TRUST:'development', CACHE_FIXTURE_VALUE:value, CACHE_EXEC_VALUE:executable, EXACT_RUST_BUNDLE:'', EXACT_UPDATE_RECEIPT:''}});
    const modified = receipt => receipt.products.filter(p=>/libcache_module\.|\/cache-runner$/.test(p.path))
      .map(p=>[p.path, statSync(p.path, {bigint:true}).mtimeNs]);
    const environment = receipt => receipt.binary.configuration.units.find(u=>u.target==='cache_module').environment;
    const first = bake('left'), stamp = modified(first);
    assert.ok(stamp.length >= 2, 'dynamic and static libraries must both be exercised');
    assert.ok(first.binary.inputs.some(f=>f.path===resolve(dir,'gpu/src/lib.rs')));
    assert.ok(first.binary.inputs.some(f=>f.path===resolve(dir,'gpu/payload')));
    assert.ok(environment(first).some(([key])=>key==='CACHE_FIXTURE_VALUE'), 'summary .d is not unit evidence');
    assert.ok(first.binary.configuration.units.find(u=>u.target==='cache-runner').environment
      .some(([key])=>key==='CACHE_EXEC_VALUE'), 'the executable retains compile-time environment evidence');
    assert.ok(first.binary.inputs.some(f=>f.path===resolve(dir,'linux/payload')));
    assert.ok(first.binary.missing.includes(resolve(dir,'art')), 'dev must observe the first art directory');
    mkdirSync(resolve(dir, 'art'));
    assert.ok(pendingBuildInputs(first).includes(resolve(dir, 'art')));
    rmSync(resolve(dir, 'art'), {recursive:true});
    const unchanged = bake('left');
    assert.deepEqual(modified(unchanged), stamp, 'unchanged libraries and executables must not relink');
    assert.equal(unchanged.binary.sha256, first.binary.sha256);
    const changed = bake('rght'); // Same length and compiled result; the environment still differs.
    assert.notDeepEqual(environment(changed), environment(first));
    assert.notEqual(changed.binary.sha256, first.binary.sha256);
    const changedStamp = modified(changed), stable = bake('rght');
    assert.deepEqual(modified(stable), changedStamp);
    assert.equal(stable.binary.sha256, changed.binary.sha256);
    write('gpu/payload', 'second');
    const edited = bake('rght');
    assert.notEqual(edited.binary.sha256, stable.binary.sha256);
    assert.notEqual(edited.binary.inputs.find(f=>f.path.endsWith('/gpu/payload')).sha256,
      stable.binary.inputs.find(f=>f.path.endsWith('/gpu/payload')).sha256);
    const executable = bake('rght', 'next');
    assert.notEqual(executable.binary.sha256, edited.binary.sha256);
    assert.notDeepEqual(executable.binary.configuration.units.find(u=>u.target==='cache-runner').environment,
      edited.binary.configuration.units.find(u=>u.target==='cache-runner').environment);
    write('linux/payload', 'second');
    const binaryEdit = bake('rght', 'next');
    assert.notEqual(binaryEdit.binary.inputs.find(f=>f.path.endsWith('/linux/payload')).sha256,
      executable.binary.inputs.find(f=>f.path.endsWith('/linux/payload')).sha256);
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 180000);

test.each(['rlib', 'staticlib', 'executable', 'windows-executable'].flatMap(kind => [null, 'intermediate', 'output/intermediate', '.'].map(split => [kind, split])))('copied %s roots with build directory %s require unique compiler dep-info', async (kind, split) => {
  const { unitDepInfo } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-unit-dep-'));
  try {
    const metadata = {target_directory:resolve(root, 'output'), build_directory:resolve(root, split ?? 'output')};
    const dir = resolve(metadata.target_directory, 'release'), deps = resolve(metadata.build_directory, 'release/deps'), src = resolve(root, 'src/main.rs');
    mkdirSync(dir, {recursive:true});
    mkdirSync(deps, {recursive:true});
    const executable = kind.endsWith('executable'), target = executable ? 'game-native' : 'game_apple';
    const suffix = kind === 'windows-executable' ? '.exe' : '';
    const extension = kind === 'staticlib' ? '.a' : '.rlib';
    const artifact = resolve(dir, executable ? target + suffix : `lib${target}${extension}`);
    writeFileSync(artifact, 'selected unit');
    const message = {filenames:[artifact],
      executable:executable ? artifact : null, target:{name:target, src_path:src}};
    writeFileSync(resolve(dir, `${target}.d`), `${artifact}: ${src}\n`);
    const unit = (hash, bytes, source = src) => {
      const name = `${target.replaceAll('-', '_')}-${hash}`, dep = resolve(deps, `${name}.d`);
      writeFileSync(resolve(deps, executable ? name + suffix : `lib${name}${extension}`), bytes);
      writeFileSync(dep, `${dep}: ${source}\n\n# env-dep:EXACT_UPDATE_TRUST=development\n`);
      return dep;
    };
    unit('deadbeef', 'another unit');
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    const expected = unit('a11ce', 'selected unit', resolve(root, 'old/main.rs'));
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    writeFileSync(expected, `${resolve(root, 'copied.d')}: ${src}\n`);
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    unit('a11ce', 'selected unit');
    assert.equal(unitDepInfo(message, root, metadata), expected);
    assert.match(readFileSync(unitDepInfo(message, root, metadata), 'utf8'), /env-dep:EXACT_UPDATE_TRUST=development/);
    unit('aabbcc', 'selected unit');
    assert.throws(() => unitDepInfo(message, root, metadata), /ambiguous rustc unit/);
  } finally { rmSync(root, {recursive:true, force:true}); }
});

test.each([null, 'intermediate', 'output/intermediate'])('copied wasm with build directory %s requires unique compiler evidence', async split => {
  const { unitDepInfo } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-wasm-dep-'));
  try {
    const metadata = {target_directory:resolve(root, 'output'), build_directory:resolve(root, split ?? 'output'),
      packages:[{id:'game-id', name:'game-gpu'}]};
    const dir = resolve(metadata.target_directory, 'wasm32-unknown-unknown/web'), src = resolve(root, 'gpu/src/lib.rs');
    mkdirSync(dir, {recursive:true});
    const artifact = resolve(dir, 'game_gpu.wasm');
    writeFileSync(artifact, 'selected wasm');
    const message = {package_id:'game-id', filenames:[artifact], target:{name:'game_gpu', src_path:src}};
    const unit = (hash, bytes, source = src) => {
      const out = resolve(metadata.build_directory, 'wasm32-unknown-unknown/web/build/game-gpu', hash, 'out');
      mkdirSync(out, {recursive:true});
      writeFileSync(resolve(out, 'game_gpu.wasm'), bytes);
      const dep = resolve(out, 'game_gpu.d');
      writeFileSync(dep, `${dep}: ${source}\n# env-dep:EXACT_GAME_PARANOID=0\n`);
      return dep;
    };
    unit('deadbeef', 'another wasm');
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    const expected = unit('a11ce', 'selected wasm', resolve(root, 'old/lib.rs'));
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    unit('a11ce', 'selected wasm');
    writeFileSync(expected, `${artifact}: ${src}\n`);
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    unit('a11ce', 'selected wasm');
    const deps = resolve(metadata.build_directory, 'wasm32-unknown-unknown/web/deps'), stale = resolve(deps, 'game_gpu.d'); // another toolchain's unit: the JS target's stable GPU build
    mkdirSync(deps, {recursive:true}); writeFileSync(resolve(deps, 'game_gpu.wasm'), 'stable wasm'); writeFileSync(stale, `${stale}: ${src}\n`);
    assert.equal(unitDepInfo(message, root, metadata), expected);
    assert.match(readFileSync(expected, 'utf8'), /env-dep:EXACT_GAME_PARANOID=0/);
    unit('aabbcc', 'selected wasm');
    assert.throws(() => unitDepInfo(message, root, metadata), /ambiguous rustc unit/);
    writeFileSync(resolve(deps, 'game_gpu.wasm'), 'selected wasm'); assert.equal(unitDepInfo(message, root, metadata), stale, 'the unit Cargo copied out of deps/');
  } finally { rmSync(root, {recursive:true, force:true}); }
});

test.each([null, 'intermediate', 'output/intermediate'])('rustc unit dep-info accepts spaces and build directory %s', async split => {
  const { unitDepInfo } = await import('./app.mjs');
  const { spawnSync } = await import('node:child_process');
  const root = mkdtempSync(resolve(tmpdir(), 'exact unit dep '));
  try {
    const metadata = {target_directory:resolve(root, 'output'), build_directory:resolve(root, split ?? 'output')};
    mkdirSync(metadata.build_directory, {recursive:true});
    const source = resolve(root, 'lib.rs'), artifact = resolve(metadata.build_directory, 'libspace_unit.rlib');
    writeFileSync(source, 'pub fn value() -> u32 { 1 }');
    const result = spawnSync('rustc', ['--crate-name', 'space_unit', '--crate-type', 'lib',
      '--emit=dep-info,link', source, '--out-dir', metadata.build_directory], {encoding:'utf8'});
    assert.equal(result.status, 0, result.stderr);
    const message = {filenames:[artifact], target:{name:'space_unit', src_path:source}};
    assert.equal(unitDepInfo(message, root, metadata), resolve(metadata.build_directory, 'space_unit.d'));
  } finally { rmSync(root, {recursive:true, force:true}); }
}, 60_000);

test.each([false, true])('ordinary buildBake with split directories=%s streams progress and retains product receipts', async split => {
  const dir = pinned(realpathSync(mkdtempSync(resolve(tmpdir(), 'r14-no-lock-'))));
  const write = (name, text) => { mkdirSync(dirname(resolve(dir,name)), {recursive:true}); writeFileSync(resolve(dir,name),text); };
  try {
    const target = bakeTarget('linux'), id = 'com.exact.plain';
    write('Cargo.toml', '[workspace]\nmembers=["linux"]\nresolver="2"\n');
    write('linux/Cargo.toml', '[package]\nname="plain-linux"\nversion="0.1.0"\nedition="2021"\n');
    write('linux/src/main.rs', 'fn main() { let unused = 1; }');
    write('app.contract', 'component App\n  view\n    text "plain"\n');
    write('linux/build.rs', `fn main() {
      let release = std::path::Path::new(${JSON.stringify(resolve(dir,'progress-received'))});
      let started = std::time::Instant::now();
      while !release.exists() {
        assert!(started.elapsed().as_secs() < 30, "Cargo progress was buffered until the build finished");
        std::thread::sleep(std::time::Duration::from_millis(10));
      }
      let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
      std::fs::write(out.join("compat.json"), r#"${JSON.stringify({target,inputs:{platform:'linux',app:id,store:{L:'0'},keys:[]}})}"#).unwrap();
      std::fs::write(out.join("artifacts.json"), r#"{"version":1,"artifacts":[],"sources":{}}"#).unwrap();
      std::fs::write(out.join("app.plan"), b"fixture").unwrap();
    }`);
    const app = {dir, workspace:dir, target:resolve(dir,'target'), name:'plain', id,
      manifest:{app:{id,name:'Plain'},rust:false}, crate:kind=>`plain-${kind}`};
    write('bake.mjs', `import {buildBake} from ${JSON.stringify(resolve(import.meta.dir,'app.mjs'))};
      const app = {...${JSON.stringify(app)},crate:kind=>'plain-'+kind};
      const receipt = buildBake(app,'linux',${JSON.stringify(target)},{profile:'dev',output:${JSON.stringify(resolve(dir,'bakes'))}});
      console.log(JSON.stringify(receipt));`);
    const bake = () => new Promise((ok, fail) => {
      const child = spawn(process.execPath,[resolve(dir,'bake.mjs')],{cwd:dir,env:{...process.env,EXACT_UPDATE_TRUST:'development',...(split ? {CARGO_BUILD_BUILD_DIR:resolve(dir,'intermediate')} : {})},stdio:['ignore','pipe','pipe']});
      let stdout='',stderr='';
      child.stdout.on('data', bytes=>{stdout+=bytes;});
      child.stderr.on('data', bytes=>{
        stderr+=bytes;
        if (stderr.includes('Compiling plain-linux')) write('progress-received','seen before completion');
      });
      child.on('error',fail);
      child.on('close',(code,signal)=>ok({code,signal,stdout,stderr}));
    });
    assert.equal(existsSync(resolve(dir,'Cargo.lock')),false);
    const built = await bake();
    assert.equal(built.code,0,built.stderr);
    assert.ok(existsSync(resolve(dir,'progress-received')));
    assert.equal((built.stderr.match(/warning: unused variable/g)??[]).length,1,built.stderr);
    const receipt = JSON.parse(built.stdout);
    assert.ok(receipt.products.some(p=>basename(p.path) === `plain-linux${process.platform === 'win32' ? '.exe' : ''}`));
    assert.ok(existsSync(resolve(dir,'Cargo.lock')));
    const receiptPath=resolve(dir,'bakes',`linux-${target}.build.json`), before=readFileSync(receiptPath,'utf8');
    write('linux/src/main.rs','fn main() { let broken: u32 = "wrong type"; }');
    const refused = await bake();
    assert.notEqual(refused.code,0);
    assert.match(refused.stderr,/mismatched types/);
    assert.match(refused.stderr,/failed: exit 101/);
    assert.equal(readFileSync(receiptPath,'utf8'),before,'failed builds cannot replace the completed receipt');
  } finally { rmSync(dir,{recursive:true,force:true}); }
}, 180000);

test('ordinary bake metadata follows intermediate directory A, B and the unset default', () => {
  const dir=pinned(realpathSync(mkdtempSync(resolve(tmpdir(),'exact metadata directories-'))));
  const previous=process.env.CARGO_BUILD_BUILD_DIR;
  delete process.env.CARGO_BUILD_BUILD_DIR;
  const write=(name,bytes)=>{mkdirSync(dirname(resolve(dir,name)),{recursive:true});writeFileSync(resolve(dir,name),bytes);};
  try {
    const platform=process.platform==='win32'?'windows':'linux', target=bakeTarget(platform), id='com.exact.metadatadirs';
    write('rust-toolchain.toml',readFileSync(resolve(import.meta.dir,'../rust-toolchain.toml')));
    write('Cargo.toml',`[workspace]\nmembers=["${platform}"]\nresolver="2"\n`);
    write(`${platform}/Cargo.toml`,`[package]\nname="metadata-${platform}"\nversion="0.1.0"\nedition="2021"\n`);
    write('app.contract','component App\n  view\n    text "Metadata"\n');
    write(`${platform}/build.rs`,`fn main(){
      println!("cargo:rerun-if-changed=build.rs");
      let out=std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
      std::fs::write(out.join("compat.json"),r#"${JSON.stringify({target,inputs:{platform,app:id,store:{L:'0'},keys:[]}})}"#).unwrap();
      std::fs::write(out.join("artifacts.json"),r#"{"version":1,"artifacts":[],"sources":{}}"#).unwrap();
      std::fs::write(out.join("app.plan"),b"fixture").unwrap();
    }`);
    write(`${platform}/src/main.rs`,'fn main(){}');
    const lock=spawnSync('cargo',['generate-lockfile','--offline'],{cwd:dir,encoding:'utf8'});
    assert.equal(lock.status,0,lock.stderr);
    const app={dir,workspace:dir,target:resolve(dir,'target'),name:'metadata',id,
      manifest:{app:{id,name:'Metadata'},rust:false},crate:kind=>`metadata-${kind}`};
    const sourcePaths=[];
    for(const lane of ['a','b',null]) {
      const input=`${platform}/${lane??'default'}.txt`, text=`input-${lane??'default'}`;
      write(input,text);
      write(`${platform}/src/main.rs`,`fn main(){println!("{}",include_str!("../${lane??'default'}.txt"));}`);
      const receipt=buildBake(app,platform,target,{profile:'dev',output:resolve(dir,'bakes'),
        env:{EXACT_UPDATE_TRUST:'development',...(lane?{CARGO_BUILD_BUILD_DIR:resolve(dir,`intermediate-${lane}`)}:{})}});
      const executable=receipt.products.find(p=>basename(p.path)===`metadata-${platform}${process.platform==='win32'?'.exe':''}`);
      assert.ok(executable,'completed receipt identifies the actual native executable');
      const ran=spawnSync(executable.path,[],{encoding:'utf8'});
      assert.equal(ran.status,0,ran.stderr); assert.equal(ran.stdout.trim(),text);
      const paths=receipt.binary.inputs.map(f=>f.path);
      assert.ok(paths.includes(resolve(dir,input)),'new compiler include is in the completed receipt');
      assert.ok(sourcePaths.every(path=>!paths.includes(path)),'prior intermediate dep-info is not reused');
      sourcePaths.push(resolve(dir,input));
      write(input,`${text}-edited-after-bake`);
      const named=receipt.binary.inputs.find(f=>f.path===resolve(dir,input)).name;
      assert.ok(pendingBuildInputs(receipt).includes(named),'new included input stays freshness-tracked');
    }
  } finally {
    if(previous===undefined)delete process.env.CARGO_BUILD_BUILD_DIR;else process.env.CARGO_BUILD_BUILD_DIR=previous;
    rmSync(dir,{recursive:true,force:true});
  }
},180000);

test('E11 partial bakes select only their graph and production retains GPU binding',async()=>{
  const {bakeSelection,bindGpuProduct}=await import('./app.mjs');
  const graph={root:{id:'host'},surface:{id:'gpu'}};
  assert.deepEqual(bakeSelection(graph,'gpu'),[graph.surface]);
  assert.deepEqual(bakeSelection(graph,'host'),[graph.root]);
  assert.deepEqual(bakeSelection(graph),[graph.surface,graph.root]);
  assert.equal(bindGpuProduct('gpu-dev','development'),false);
  assert.equal(bindGpuProduct('gpu-dev','production'),true);
  assert.equal(bindGpuProduct('release','development'),true);
});

test('D6 GPU modules bake beside the primary and each surface has one owner', async () => {
  const {bakeSelection, gpuModules, readManifest} = await import('./app.mjs');
  const {mkdtempSync, writeFileSync, rmSync} = await import('node:fs');
  const {resolve} = await import('node:path');
  const {tmpdir} = await import('node:os');
  const graph = {root:{id:'host'}, surface:{id:'gpu'}, modules:[{id:'world'}]};
  assert.deepEqual(bakeSelection(graph, 'gpu'), [graph.surface, graph.modules[0]]);
  assert.deepEqual(bakeSelection(graph), [graph.surface, graph.modules[0], graph.root]);
  assert.deepEqual(bakeSelection({...graph, surface:undefined}), [graph.modules[0], graph.root], 'a module needs no primary');
  assert.deepEqual(gpuModules({gpu:{modules:{world:['world', 'arena']}}}), [{name:'world', surfaces:['world', 'arena']}]);
  assert.deepEqual(gpuModules({}), []);
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-gpu-modules-'));
  const manifest = gpu => { writeFileSync(resolve(dir, 'app.json'), JSON.stringify({name:'M', app:{id:'com.exact.m', name:'M'}, gpu})); return () => readManifest(dir, 'm'); };
  try {
    assert.equal(manifest({modules:{world:['world']}})().gpu.modules.world[0], 'world');
    assert.throws(manifest({modules:{a:['world'], b:['world']}}), /surface world is claimed by both a and b/);
    assert.throws(manifest({modules:{World:['world']}}), /gpu\.modules\.World: a module name/);
    assert.throws(manifest({modules:{world:[]}}), /gpu\.modules\.world: names no surface/);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('declared shader packs merge, reject duplicates and links, and preserve a rejected live candidate', async () => {
  const { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, symlinkSync } = await import('node:fs');
  const { resolve } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const { shaderFiles, copyShaders } = await import('./app.mjs');
  const { applyShaderTreeChange } = await import('../host/web/serve.mjs');
  const dir=mkdtempSync(resolve(tmpdir(),'exact-shader-packs-'));
  const app={dir,manifest:{gpu:{shaderRoots:['pack']}}}, target=resolve(dir,'dist/shaders');
  try {
    mkdirSync(resolve(dir,'gpu/shaders'),{recursive:true}); mkdirSync(resolve(dir,'pack'));
    writeFileSync(resolve(dir,'gpu/shaders/a.wgsl'),'a'); writeFileSync(resolve(dir,'pack/b.wgsl'),'b');
    copyShaders(app,target); writeFileSync(resolve(target,'stale.wgsl'),'old');
    copyShaders(app,target,{replace:true}); assert.ok(!existsSync(resolve(target,'stale.wgsl'))); assert.deepEqual([...shaderFiles(app).keys()].sort(),['a.wgsl','b.wgsl']);
    writeFileSync(resolve(dir,'shared.wgsl'),'shared');
    app.manifest.gpu.shaderPreludes={b:['shared.wgsl']};
    assert.equal(shaderFiles(app).get('b.wgsl').toString(),'shared\nb');
    writeFileSync(resolve(dir,'shared.wgsl'),'updated');
    applyShaderTreeChange(app,target);
    assert.equal(readFileSync(resolve(target,'b.wgsl'),'utf8'),'updated\nb');
    delete app.manifest.gpu.shaderPreludes;
    writeFileSync(resolve(dir,'pack/a.wgsl'),'collision');
    assert.throws(()=>applyShaderTreeChange(app,target),/duplicate shader/);
    assert.equal(readFileSync(resolve(target,'a.wgsl'),'utf8'),'a');
    rmSync(resolve(dir,'pack/a.wgsl')); rmSync(resolve(dir,'pack/b.wgsl'));
    const changed=applyShaderTreeChange(app,target);
    assert.ok(changed.files.some(f=>f.name==='b.wgsl'&&f.removed));
    assert.equal(readFileSync(resolve(target,'a.wgsl'),'utf8'),'a');
    // Windows directory junctions are unprivileged reparse points; file
    // symlinks require Developer Mode or an elevated process. Both must refuse.
    symlinkSync(resolve(dir,process.platform==='win32'?'gpu/shaders':'gpu/shaders/a.wgsl'),resolve(dir,'pack/b.wgsl'),process.platform==='win32'?'junction':'file');
    assert.throws(()=>shaderFiles(app));
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

test('a build env keeps the pinned toolchain and the checked Bun ahead of ambient ones', async () => {
  const { developmentBuildEnv } = await import('./app.mjs');
  const { readFileSync } = await import('node:fs');
  const { delimiter, dirname, resolve } = await import('node:path');
  const pinned = /^channel\s*=\s*"([^"]+)"/m.exec(readFileSync(resolve(import.meta.dir, '../rust-toolchain.toml'), 'utf8'))[1];
  const previous = process.env.RUSTUP_TOOLCHAIN;
  try {
    assert.equal(process.env.HERMES_LEAN_SYS_OFFLINE, '1');
    process.env.RUSTUP_TOOLCHAIN = 'stable'; // What `mise exec` exports.
    assert.equal(developmentBuildEnv().RUSTUP_TOOLCHAIN, undefined);
    for (const same of [pinned, `${pinned}-aarch64-apple-darwin`]) {
      process.env.RUSTUP_TOOLCHAIN = same;
      assert.equal(developmentBuildEnv().RUSTUP_TOOLCHAIN, same);
    }
    delete process.env.RUSTUP_TOOLCHAIN;
    const env = developmentBuildEnv();
    assert.equal(env.RUSTUP_TOOLCHAIN, undefined);
    assert.equal(env.PATH.split(delimiter)[0], dirname(process.execPath));
    assert.ok(Bun.which('cargo', {PATH:env.PATH}), 'the build inherits Cargo after spreading Windows Path');
    assert.equal(Object.keys(env).filter(key => key.toLowerCase() === 'path').length, 1);
    assert.equal(env.EXACT_UPDATE_TRUST, process.env.EXACT_UPDATE_TRUST ?? 'development');
    assert.equal(env.HERMES_LEAN_SYS_OFFLINE, '1');
  } finally { if (previous === undefined) delete process.env.RUSTUP_TOOLCHAIN; else process.env.RUSTUP_TOOLCHAIN = previous; }
});

test('web remapping preserves game floating-point determinism on both web toolchains', async () => {
  const {wasmRemapFlags, WEB_TOOLCHAIN} = await import('./app.mjs');
  const workspace = new URL('../game/', import.meta.url).pathname;
  for (const toolchain of [null, WEB_TOOLCHAIN]) {
    const flags = manifest => JSON.parse(wasmRemapFlags({workspace,target:workspace+'target',manifest},toolchain)[1].replace(/^[^=]*=/,''));
    const game = flags({game:{}});
    assert.equal(game.filter(flag=>flag==='llvm-args=-fp-contract=off').length,1);
    assert.equal(game[game.indexOf('llvm-args=-fp-contract=off')-1],'-C');
    assert.ok(!flags({}).includes('llvm-args=-fp-contract=off'));
  }
}, 60_000);

test('setup accepts both official Binaryen release tags and package-manager version output', async () => {
  const {binaryenVersion} = await import('./exact.mjs');
  assert.equal(binaryenVersion('wasm-opt version 132 (version_132)\n'), 'version 132');
  assert.equal(binaryenVersion('wasm-opt version 132\n'), 'version 132');
  assert.notEqual(binaryenVersion('wasm-opt version 1320 (version_1320)'), 'version 132');
  assert.notEqual(binaryenVersion('missing'), 'version 132');
});

// Grow a Garden: a `git checkout` that rewrote unchanged files made the next
// drive refuse (`web build is stale`) though the proof's receipt (by content)
// was fresh. The staleness check now compares content digests.
test('rewriting an input with its own bytes does not make a web build stale', async () => {
  const { webChanges, webInputDigests } = await import('./agent-launch.mjs');
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync, utimesSync } = await import('node:fs');
  const { resolve } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'web-freshness-'));
  try {
    const app = { dir: resolve(dir, 'app'), manifest: { game: {} } };
    const dist = resolve(dir, 'dist');
    mkdirSync(resolve(app.dir, 'logic/src'), { recursive: true });
    mkdirSync(dist);
    const source = resolve(app.dir, 'logic/src/lib.rs');
    writeFileSync(source, 'fn tick() {}\n');
    const marker = resolve(dist, '.exact-build.json');
    writeFileSync(marker, JSON.stringify({ exactBuild: 1, inputs: webInputDigests(app, false) }));
    const past = new Date(Date.now() - 60_000);
    utimesSync(marker, past, past);
    writeFileSync(source, 'fn tick() {}\n');
    assert.deepEqual(webChanges(dist, app).app, []);
    writeFileSync(source, 'fn tick() { edited(); }\n');
    assert.deepEqual(webChanges(dist, app).app, [source]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60000);

// The euxy drive-speed review: a drive's check hashes an input unless its size and mtime are the build's own, so
// one replaced with `cp -p` (other bytes, an older mtime) is still found, and an untouched one is not read again.
test('a receipt input is hashed unless its size and mtime are the ones the build recorded', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-pending-'));
  try {
    const path = resolve(dir, 'a.swift');
    writeFileSync(path, 'let a = 1\n');
    const at = statSync(path), file = { name: 'a.swift', path, sha256: 'not-the-digest', size: at.size, mtimeMs: at.mtimeMs };
    const build = { binary: { inputs: [file], missing: [], directories: [] } };
    assert.deepEqual(pendingBuildInputs(build), [], 'size and mtime as built: taken as built, not read');
    writeFileSync(path, 'let b = 2\n');
    utimesSync(path, new Date(at.mtimeMs - 60000), new Date(at.mtimeMs - 60000));
    assert.deepEqual(pendingBuildInputs(build), ['a.swift'], 'other bytes with an older mtime are hashed and found');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
