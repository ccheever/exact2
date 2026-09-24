import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { classifyArtifacts } from './app.mjs';

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

import { spawn, spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { resolveApp, buildBake, bakeTarget, cargoDefaultBinary, optimizeWasm, pendingBuildInputs } from './app.mjs';
import { snapshotOf, materializeSnapshot, disposeSnapshot } from './deploy.mjs';

test('resident compilers follow Cargo default-run and single-binary selection', () => {
  const bin = name => ({name, kind:['bin']});
  assert.equal(cargoDefaultBinary({name:'reflow-web', targets:[bin('reflow-dev')]}), 'reflow-dev');
  assert.equal(cargoDefaultBinary({name:'caltrain-web', default_run:'caltrain-dev', targets:[bin('metrics'),bin('caltrain-dev')]}), 'caltrain-dev');
  assert.equal(cargoDefaultBinary({name:'external-web', targets:[bin('external-compiler')]}), 'external-compiler');
  assert.equal(cargoDefaultBinary({name:'plain-web', targets:[{name:'plain_web',kind:['cdylib']}]}), null);
  assert.throws(() => cargoDefaultBinary({name:'ambiguous',targets:[bin('one'),bin('two')]}), /default-run/);
});

test('shared input scans compare each receipt and rehash on the next scan', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-input-scan-')), path = resolve(dir, 'source');
  const digest = text => new Bun.CryptoHasher('sha256').update(text).digest('hex');
  const receipt = text => ({binary:{inputs:[{path,name:'source',sha256:digest(text)}],missing:[],directories:[]}});
  try {
    writeFileSync(path, 'first');
    const stamp = statSync(path), files = new Map();
    assert.deepEqual(pendingBuildInputs(receipt('first'), files), []);
    assert.deepEqual(pendingBuildInputs(receipt('other'), files), ['source']);
    writeFileSync(path, 'other'); utimesSync(path, stamp.atime, stamp.mtime);
    assert.deepEqual(pendingBuildInputs(receipt('first')), ['source']);
    assert.deepEqual(pendingBuildInputs(receipt('other')), []);
    rmSync(path);
    assert.deepEqual(pendingBuildInputs(receipt('other')), ['source']);
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('Wasm optimization reuses only matching inputs, tool, flags and intact output', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-wasm-opt-')), oldPath = process.env.PATH;
  const source = resolve(dir,'input'), out = resolve(dir,'output'), cache = resolve(dir,'cache');
  const tool = resolve(dir,'wasm-opt'), calls = resolve(dir,'calls');
  const script = `#!/bin/sh\nprintf x >> '${calls}'\nwhile [ "$#" -gt 0 ]; do\nif [ "$1" = -o ]; then shift; out="$1"; else input="$1"; fi\nshift\ndone\ncp "$input" "$out"\n`;
  try {
    process.env.PATH = dir + ':' + oldPath;
    writeFileSync(tool,script,{mode:0o755}); writeFileSync(source,'first');
    const run = flags => optimizeWasm(source,out,cache,flags ?? ['-Oz']);
    const count = () => readFileSync(calls,'utf8').length;
    assert.equal(run(),true);assert.equal(run(),true);assert.equal(count(),1);
    const stamp=statSync(source);writeFileSync(source,'other');utimesSync(source,stamp.atime,stamp.mtime);
    run();assert.equal(count(),2);assert.equal(readFileSync(out,'utf8'),'other');
    run(['-O2']);assert.equal(count(),3);
    writeFileSync(tool,script+'# changed tool\n');run();assert.equal(count(),4);
    for (const name of readdirSync(cache)) writeFileSync(resolve(cache,name),'corrupt');
    run();assert.equal(count(),5);assert.equal(readFileSync(out,'utf8'),'other');
    writeFileSync(tool,'#!/bin/sh\nexit 42\n');
    assert.throws(run,/wasm-opt failed/);
    assert.equal(optimizeWasm(source,out,cache,['-Oz'],true),false);
    assert.equal(readFileSync(out,'utf8'),'other');
  } finally {process.env.PATH=oldPath;rmSync(dir,{recursive:true,force:true});}
});

// Real Cargo units, no engine dependencies. Opt in with the other bake diagnostics.
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
    const lock = spawnSync('cargo', ['generate-lockfile', '--offline'], {cwd:dir, encoding:'utf8'});
    assert.equal(lock.status, 0, lock.stderr);
    const app = {dir, workspace:dir, target:resolve(dir,'target'), id, hasGpu:true,
      manifest:{app:{id, name:'Cache fixture'}, game:{}, rust:false}, crate:kind=>`cache-${kind}`};
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

async function fixture(body) {
  const scratch = resolve(import.meta.dir, '../target');
  mkdirSync(scratch, {recursive:true});
  const root = mkdtempSync(resolve(scratch, 'shell-repair-'));
  const previous = process.env.EXACT_APP_DIR;
  const write = (path, bytes) => { mkdirSync(dirname(resolve(root, path)), {recursive:true}); writeFileSync(resolve(root, path), bytes); };
  const run = (cmd, args) => {
    const r = spawnSync(cmd, args, {cwd:root, encoding:'utf8', timeout:60000});
    assert.equal(r.status, 0, r.stderr); return r.stdout;
  };
  const pkg = (path, name, source = '') => {
    write(`${path}/Cargo.toml`, `[package]\nname="${name}"\nversion="0.1.0"\nedition="2021"\n`);
    write(`${path}/src/lib.rs`, source);
  };
  const game = (name, crate = `${name}-logic`) => {
    const dir = `game/games/${name}`;
    write(`${dir}/app.contract`, 'component App\n  view\n');
    write(`${dir}/app.json`, JSON.stringify({name, app:{id:`com.exact.${name}`,name}, game:{crate,type:'SmallGame'}}));
    pkg(`${dir}/logic`, crate, 'pub struct SmallGame;');
    write(`${dir}/logic/Cargo.toml`, readFileSync(resolve(root, dir, 'logic/Cargo.toml'), 'utf8').replace('[package]', '[package]\nworkspace="../.shells"'));
    if (name !== 'foo' && existsSync(resolve(root,'game/games/foo/Cargo.lock'))) write(`${dir}/Cargo.lock`,readFileSync(resolve(root,'game/games/foo/Cargo.lock'),'utf8').replaceAll('foo',name));
    return resolve(root, dir);
  };
  try {
    for (const path of ['scripts/app.mjs','scripts/filesystem.mjs','scripts/rust.mjs','scripts/install-page.mjs','scripts/app.schema.json','game/app/shells.mjs','game/.cargo/config.toml']) {
      write(path, readFileSync(resolve(import.meta.dir,'..',path)));
    }
    const { resolveApp: localResolveApp, cargoReproducibilityFlags: flags } = await import(resolve(root,'scripts/app.mjs'));
    const {prepareGame} = await import(resolve(root,'game/app/shells.mjs'));
    write('rust-toolchain.toml', readFileSync(resolve(import.meta.dir,'../rust-toolchain.toml')));
    const deps = ['exact-game','exact-game-render','exact-game-app','exact-game-bake','exact-runner','exact-web','exact-apple','exact-linux','wasm-bindgen','wasm-bindgen-futures','web-sys'];
    write('Cargo.toml', '[workspace]\nmembers=["stub"]\nresolver="2"\n'); pkg('stub','root-stub');
    write('game/Cargo.toml', '[workspace]\nmembers=["deps/*","ordinary/*"]\nexclude=["games"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\nlicense="MIT"\n[workspace.dependencies]\n' + deps.map(n=>`${n}={path="deps/${n}"}`).join('\n'));
    for (const dep of deps) pkg(`game/deps/${dep}`, dep);
    write('game/bake/src/files.rs', 'pub fn bake_game_level<G>(_: impl AsRef<std::path::Path>) -> Result<(), String> { Ok(()) }\n');
    write('game/games/.gitignore', '*/.shells/\n');
    // Cargo permits an empty glob when its containing directory exists.
    pkg('game/ordinary/stub','ordinary-stub');
    const dir = game('foo'); process.env.EXACT_APP_DIR = dir;
    prepareGame(dir, JSON.parse(readFileSync(resolve(dir,'app.json'))).game, resolve(root,'game'), {updateLock:true});
    rmSync(resolve(dir,'.shells'),{recursive:true});
    body({root, dir, write, run, pkg, game, flags, update:()=>prepareGame(dir, JSON.parse(readFileSync(resolve(dir,'app.json'))).game, resolve(root,'game'), {updateLock:true}), app:(name='foo')=>localResolveApp(name)});
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(root,{recursive:true,force:true});
  }
}

test('shell repair replaces half-written members before metadata', () => fixture(({app}) => {
  const before = app(), path = before.cargoPackage('gpu').manifest_path;
  rmSync(resolve(dirname(path),'src'),{recursive:true});
  assert.ok(app().cargoPackage('gpu'));
  assert.ok(existsSync(resolve(dirname(path),'src/lib.rs')));
}));

test('copied app identities keep separate Cargo graphs and generated hosts', () => fixture(({app, dir, game, write}) => {
  const first = app();
  const before = ['gpu','web','apple','linux'].map(kind => first.cargoPackage(kind).manifest_path);
  const copy = game('copy');
  const manifest = JSON.parse(readFileSync(resolve(copy, 'app.json'), 'utf8'));
  manifest.app.id = first.manifest.app.id;
  write('game/games/copy/app.json', JSON.stringify(manifest));
  process.env.EXACT_APP_DIR = copy;
  const second = app('copy');
  for (const kind of ['gpu','web','apple','linux']) {
    const pkg = second.cargoPackage(kind);
    assert.equal(pkg.name, `copy-${kind}`);
    assert.ok(pkg.manifest_path.startsWith(copy + '/.shells/'));
  }
  process.env.EXACT_APP_DIR = dir;
  const reopened = app();
  assert.deepEqual(['gpu','web','apple','linux'].map(kind => reopened.cargoPackage(kind).manifest_path), before);
  for (const path of before) assert.ok(existsSync(path));
}));

test('art adds its baker on demand and retains it until generated outputs are pruned', () => fixture(({app, dir, write, update}) => {
  const baked = () => {
    update();
    const gpu = app().cargoPackage('gpu');
    const dependency = gpu.dependencies.some(d=>d.name==='exact-game-bake');
    assert.ok(gpu.targets.some(t=>t.kind.includes('custom-build')));
    assert.equal(readFileSync(resolve(dirname(gpu.manifest_path),'build.rs'),'utf8').includes('exact_game_bake::bake_art'), dependency);
    return dependency;
  };
  assert.equal(baked(), false);
  write('game/games/foo/art/example.png', 'fixture');
  assert.equal(baked(), true);
  write('game/games/foo/.baked-assets.json', '{}');
  rmSync(resolve(dir, 'art'), {recursive:true});
  assert.equal(baked(), true, 'cleanup still needs the baker');
  rmSync(resolve(dir, '.baked-assets.json'));
  assert.equal(baked(), false);
}));

test('host paths survive app identity and logic crate renames', () => fixture(({app, dir, write, run, update}) => {
  const before = app().cargoPackage('gpu').manifest_path;
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  manifest.app.id = 'org.example.renamed';
  manifest.game.crate = 'renamed-logic';
  write('game/games/foo/app.json',JSON.stringify(manifest));
  write('game/games/foo/logic/Cargo.toml','[package]\nworkspace="../.shells"\nname="renamed-logic"\nversion="0.1.0"\nedition="2021"\n');
  update();
  const after = app();
  assert.equal(after.cargoPackage('gpu').manifest_path, before);
  assert.equal(after.name, 'renamed');
  const metadata = JSON.parse(run('cargo',['metadata','--manifest-path','game/games/foo/.shells/Cargo.toml','--no-deps','--offline','--format-version','1']));
  assert.ok(!metadata.packages.some(p=>p.name==='foo-gpu'));
}));

test('resolving a surviving game prunes a deleted game shell', () => fixture(({app, game}) => {
  const gone = game('gone');
  process.env.EXACT_APP_DIR = gone;
  const shell = dirname(app('gone').cargoPackage('gpu').manifest_path);
  rmSync(gone,{recursive:true});
  process.env.EXACT_APP_DIR = resolve(dirname(gone),'foo');
  assert.ok(app().hasGpu);
  assert.ok(!existsSync(shell));
}));

test('ordinary GPU ownership requires its own manifest and matching metadata directory', () => fixture(({app, dir, pkg, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8')); delete manifest.game;
  // An ordinary app outside games/ sharing a workspace with an unrelated foo-gpu.
  process.env.EXACT_APP_DIR = resolve(dirname(dirname(dir)), 'ordinary/foo');
  pkg('game/ordinary/foo','foo-web');
  write('game/ordinary/foo/app.json',JSON.stringify(manifest)); write('game/ordinary/foo/app.contract','component App\n  view\n');
  pkg('game/ordinary/unrelated','foo-gpu');
  assert.equal(app().hasGpu,false);
  write('game/ordinary/foo/gpu/Cargo.toml','[package]\nname="some-other-gpu"\nversion="0.1.0"\n');
  assert.equal(app().hasGpu,false);
}));

test('missing game entry refuses by key at resolve time', () => fixture(({app, dir, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  write('game/games/foo/app.json',JSON.stringify({...manifest,game:undefined}));
  assert.throws(app, /app.json.*game|game.*required/);
}));

test('game.type exports are checked by Rust compilation, not app resolution', () => fixture(({app, dir, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  write('game/games/foo/app.json',JSON.stringify({...manifest,game:{...manifest.game,type:'SmallGmae'}}));
  assert.ok(app().hasGpu);
}));

test('game.type module paths resolve before Rust checks the referenced export', () => fixture(({app, dir, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  write('game/games/foo/app.json',JSON.stringify({...manifest,game:{...manifest.game,type:'play::SmallGame'}}));
  assert.ok(app().hasGpu);
  write('game/games/foo/logic/src/lib.rs', 'pub mod play;');
  write('game/games/foo/logic/src/play.rs', 'pub struct SmallGame;');
  assert.ok(app().hasGpu);
}));

test('build graph refuses a requested GPU surface that Cargo cannot find', () => fixture(({app, pkg, root, run}) => {
  pkg('game/ordinary/foo','ordinary-web');
  const resolved = app();
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  const fake = {...resolved, workspace:resolve(root,'game'), hasGpu:true, crate:kind=>`ordinary-${kind}`};
  assert.throws(()=>buildBake(fake,'web','wasm32-unknown-unknown'), /GPU.*ordinary-gpu|ordinary-gpu.*surface/);
}));

test('deploy excludes generated shells and regenerates them from captured game source', () => fixture(({app, root, write, run, game}) => {
  const resolved = app();
  resolved.cargoPackage('gpu');
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/games/foo/.shells/Cargo.toml']);
  run('cargo',['generate-lockfile','--offline']);
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  run('git',['init','-q']); run('git',['add','.']);
  run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']);
  // A generated extra byte must never enter the captured source or dirty it.
  write('game/games/foo/.shells/generated-note','not source');
  write('game/target/cached-build','not source');
  write('game/render/target/cached-build','not source');
  const snapshot = snapshotOf(resolved,{},root);
  try {
    assert.equal(snapshot.dirty,false);
    const staged = materializeSnapshot(snapshot,resolve(root,'target/run'),resolved);
    assert.ok(!existsSync(resolve(staged.app.workspace,'generated-note')));
    const metadata = JSON.parse(spawnSync('cargo',['metadata','--no-deps','--offline','--format-version','1'],{cwd:staged.app.workspace,encoding:'utf8'}).stdout);
    const gpu = metadata.packages.find(p=>p.name==='foo-gpu');
    assert.ok(gpu,'materialized source must regenerate the GPU shell');
    assert.ok(gpu.manifest_path.startsWith(staged.sourceRoot));
    assert.ok(readFileSync(resolve(dirname(gpu.manifest_path),'src/lib.rs'),'utf8').includes('SmallGame'));
  } finally { disposeSnapshot(snapshot); }
}));


test('rendered tree includes focus and the computed accessible name', async () => {
  const { render } = await import('./agent.mjs');
  const line = render('tree', {nodes:[{id:1, depth:0, type:'View', props:{testId:'play'}, focused:true, accessibleName:'Play'}]});
  assert.match(line, /\[focused\]/);
  assert.match(line, /name="Play"/);
});


test('autofocus is deferred and consumed per mounted control, preserving other UI focus', async () => {
  const { readFileSync } = await import('node:fs');
  const { runInNewContext } = await import('node:vm');
  const source = readFileSync(new URL('../host/web/navigation.js', import.meta.url), 'utf8');
  const start = source.indexOf('export function focusController');
  const fn = source.slice(start, source.indexOf('\n}\n', start) + 2).replace('export ', '');
  const element = () => ({exactAutofocus:true, getClientRects:()=>[{}], matches:()=>false,
    setAttribute() {}, focus() { document.activeElement = this; }});
  const document = {body:{}, activeElement:null};
  const first = element(), other = element();
  const views = new Map([[1,first]]);
  const context = {document, views, root:{querySelectorAll:()=>[...views.values()]}, inputReady:true,
    inertAncestor:()=>false, getComputedStyle:()=>({visibility:'visible'})};
  const focus = runInNewContext(fn+';focusController({ready:()=>inputReady,elements:()=>views.values(),inert:inertAncestor}).autofocus', context);
  document.activeElement = other;
  focus();
  assert.equal(document.activeElement, other, 'existing focus must win');
  document.activeElement = document.body;
  focus();
  assert.equal(document.activeElement, document.body, 'a refused mounted control stays consumed');
  const later = element();
  views.set(1, later);
  context.inputReady = false;
  focus();
  assert.equal(document.activeElement, document.body, 'input readiness gates autofocus');
  context.inputReady = true;
  focus();
  assert.equal(document.activeElement, later, 'a later mount takes absent focus');
  const victory = element();
  const canvas = {matches:selector=>selector === '[data-gpu-input]', contains:el=>el===victory};
  document.activeElement = canvas;
  views.set(2, victory);
  focus();
  assert.equal(document.activeElement, victory, 'a canvas yields raw input focus to its new UI control');
});


test('applying autofocus props cannot trigger browser focus during a batch', async () => {
  const { readFileSync } = await import('node:fs');
  const { runInNewContext } = await import('node:vm');
  const source = readFileSync(new URL('../host/web/glue.js', import.meta.url), 'utf8');
  const fn = source.slice(source.indexOf('function applyProps('), source.indexOf('function ensureMessageListener('));
  class Element {}
  const apply = runInNewContext(fn+';applyProps', {syncMedia:()=>{}, syncMarkup:()=>{}, inputReady:true, HTMLIFrameElement:Element, HTMLImageElement:Element, HTMLVideoElement:Element});
  const attrs = new Map();
  const el = {setAttribute:(k,v)=>attrs.set(k,v), removeAttribute:k=>attrs.delete(k)};
  apply(el, {autofocus:'true'}, []);
  assert.equal(attrs.has('autofocus'), false);
  assert.equal(el.exactAutofocus, true);
  apply(el, {}, ['autofocus']);
  assert.equal(el.exactAutofocus, false);
});

test.each(['rlib', 'staticlib', 'executable'].flatMap(kind => [null, 'intermediate', 'output/intermediate', '.'].map(split => [kind, split])))('copied %s roots with build directory %s require unique compiler dep-info', async (kind, split) => {
  const { unitDepInfo } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-unit-dep-'));
  try {
    const metadata = {target_directory:resolve(root, 'output'), build_directory:resolve(root, split ?? 'output')};
    const dir = resolve(metadata.target_directory, 'release'), deps = resolve(metadata.build_directory, 'release/deps'), src = resolve(root, 'src/main.rs');
    mkdirSync(dir, {recursive:true});
    mkdirSync(deps, {recursive:true});
    const executable = kind === 'executable', target = executable ? 'game-native' : 'game_apple';
    const extension = kind === 'staticlib' ? '.a' : '.rlib';
    const artifact = resolve(dir, executable ? target : `lib${target}${extension}`);
    writeFileSync(artifact, 'selected unit');
    const message = {filenames:[artifact],
      executable:executable ? artifact : null, target:{name:target, src_path:src}};
    writeFileSync(resolve(dir, `${target}.d`), `${artifact}: ${src}\n`);
    const unit = (hash, bytes, source = src) => {
      const name = `${target.replaceAll('-', '_')}-${hash}`, dep = resolve(deps, `${name}.d`);
      writeFileSync(resolve(deps, executable ? name : `lib${name}${extension}`), bytes);
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
});


test('R12 resolution is lazy and does not create a workspace', () => fixture(({app, dir}) => {
  const started = performance.now();
  const path=process.env.PATH;
  let resolved;
  try { process.env.PATH=resolve(dir,'no-executables'); resolved=app(); }
  finally { process.env.PATH=path; }
  assert.equal(resolved.name, 'foo');
  assert.equal(resolved.hasGpu, true);
  assert.ok(!existsSync(resolve(dir, '.shells')));
  assert.ok(performance.now() - started < 200);
}));

test('R12 the captured source lock replaces a corrupted generated lock', () => fixture(({app, dir}) => {
  app().cargoPackage('gpu');
  const lock = resolve(dir, 'Cargo.lock');
  assert.ok(existsSync(lock), 'source lock must be outside ignored shells');
  const captured = readFileSync(lock, 'utf8');
  writeFileSync(resolve(dir, '.shells/Cargo.lock'), 'invalid generated cache');
  app().cargoPackage('gpu');
  assert.equal(readFileSync(resolve(dir, '.shells/Cargo.lock'), 'utf8'), captured);
}));


test('R12 authored logic belongs only to its app workspace and locked edits refuse', () => fixture(({app, dir, run, write}) => {
  const resolved=app();resolved.cargoPackage('gpu');
  const metadata=JSON.parse(run('cargo',['metadata','--locked','--offline','--format-version','1','--manifest-path',resolve(dir,'.shells/Cargo.toml')]));
  const logic=metadata.packages.find(p=>p.name==='foo-logic');
  assert.ok(metadata.workspace_members.includes(logic.id));
  assert.equal(metadata.workspace_root,resolve(dir,'.shells'));
  const captured=readFileSync(resolve(dir,'Cargo.lock'),'utf8');
  write('game/games/foo/logic/Cargo.toml',readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8').replace('version="0.1.0"','version="0.2.0"'));
  assert.throws(()=>app().cargoPackage('gpu'),/lock|locked/);
  assert.equal(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),captured);
}));

test('game profiles drop redundant dependency overrides and keep authored optimization', () => fixture(({app, dir, root, run, write, update}) => {
  write('game/Cargo.toml', readFileSync(resolve(root,'game/Cargo.toml'),'utf8') + `
[profile.gpu-dev]
inherits="dev"
opt-level=1
[profile.gpu-dev.package."*"]
opt-level=3
[profile.gpu-dev.package.exact-game]
opt-level=3
[profile.gpu-dev.package.absent-audio]
opt-level=3
[profile.gpu-dev.package.exact-game-render]
opt-level=2
[profile.gpu-dev.package."exact-runner@0.1.0"]
opt-level=3
[profile.gpu-dev.package.foo-logic]
opt-level=3
[profile.gpu-dev.package.foo-gpu]
opt-level=3
`);
  write('game/games/foo/logic/Cargo.toml', readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8') + '\n[dependencies]\nexact-game.workspace=true\nexact-game-render.workspace=true\nexact-runner.workspace=true\n');
  update();
  app().cargoPackage('gpu');
  const packages = Bun.TOML.parse(readFileSync(resolve(dir,'.shells/Cargo.toml'),'utf8')).profile['gpu-dev'].package;
  assert.ok(!('exact-game' in packages));
  assert.ok(!('absent-audio' in packages));
  assert.equal(packages['foo-gpu']['opt-level'],3);
  assert.equal(packages['exact-runner@0.1.0']['opt-level'],3);
  const messages = run('cargo',['build','--offline','--locked','--manifest-path',resolve(dir,'.shells/Cargo.toml'),'-p','foo-logic','--profile','gpu-dev','--message-format=json']).trim().split('\n').map(JSON.parse);
  const units = new Map(messages.filter(m=>m.reason==='compiler-artifact').map(m=>[m.target.name,m.profile.opt_level]));
  assert.deepEqual(Object.fromEntries(units), {exact_game:'3',exact_game_render:'2',exact_runner:'3',foo_logic:'3'});
}), 180000);


test('game bakes resolve one fresh Cargo graph for the actual target and environment', () => fixture(({app, dir, root, run, write, pkg, update}) => {
  write('game/Cargo.toml', readFileSync(resolve(root,'game/Cargo.toml'),'utf8') + '\n[profile.gpu-dev]\ninherits="dev"\n');
  write('game/deps/exact-game/src/lib.rs', `
    pub enum Value { Number(f64), Bool(bool), Str(Box<str>), Other }
    pub trait Args: Default { const FIELDS: &'static [(&'static str, ())]; fn values(&self) -> Vec<Value>; }
    impl Args for () {
      const FIELDS: &'static [(&'static str, ())] = &[("seed", ()), ("paused", ()), ("label", ())];
      fn values(&self) -> Vec<Value> { vec![Value::Number(7.0), Value::Bool(false), Value::Str("say \\"hi\\"\\n雪".into())] }
    }
    pub trait Game { const NAME: &'static str; type Args: Args; }
  `);
  write('game/deps/exact-game-render/src/lib.rs', '#[macro_export] macro_rules! module { ($game:ty) => { #[no_mangle] pub extern "C" fn answer() -> u32 { game_logic::ANSWER } }; }');
  write('game/games/foo/logic/src/lib.rs', `pub struct SmallGame; impl SmallGame { pub const NAME: &'static str = "inherent"; } pub const ANSWER: u32 = 42; impl exact_game::Game for SmallGame { const NAME: &'static str = "world"; type Args = (); }`);
  pkg('game/deps/wasm-only', 'wasm-only');
  write('game/games/foo/logic/Cargo.toml', readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8') + '\n[dependencies]\nexact-game.workspace=true\n[target.\'cfg(target_arch = "wasm32")\'.dependencies]\nwasm-only={path="../../../deps/wasm-only"}\n');
  update();
  const info = app(), target = bakeTarget('linux'), cargo = run('rustup',['which','cargo']).trim();
  info.cargoPackage('gpu'); // A package lookup must not make a later build graph stale.
  for (const kind of ['gpu','web','apple']) assert.deepEqual(
    info.cargoPackage(kind).targets.find(t=>!t.kind.includes('custom-build')).crate_types,
    [kind === 'apple' ? 'staticlib' : 'cdylib']);
  const prepare = info.prepare;
  let graph;
  info.prepare = (...args) => graph = prepare(...args);
  const quote = value => "'" + value.replaceAll("'", "'\\''") + "'";
  const trace = resolve(root,'cargo-calls'), bin = resolve(root,'bin'), previous = process.env.PATH;
  // Calling cargo's binary directly skips rustup's proxy, so name its toolchain:
  // otherwise each rustc proxy picks one by directory and a build mixes two.
  const toolchain = /\/toolchains\/([^/]+)\/bin\/cargo$/.exec(cargo)?.[1];
  write('bin/cargo', `#!/bin/sh\nif [ "$1" = metadata ]; then printf '%s|%s\\n' "$*" "$CARGO_TARGET_DIR" >> ${quote(trace)}; fi\n${toolchain ? `RUSTUP_TOOLCHAIN=${quote(toolchain)} ` : ''}exec ${quote(cargo)} "$@"\n`);
  chmodSync(resolve(bin,'cargo'), 0o755);
  process.env.PATH = `${bin}:${previous}`;
  const bake = () => {
    writeFileSync(trace, '');
    const result = buildBake(info, 'linux', target, {profile:'gpu-dev', part:'gpu', env:{EXACT_UPDATE_TRUST:'development'}});
    const calls = readFileSync(trace,'utf8').trim().split('\n');
    assert.equal(calls.length, 1, calls.join('\n'));
    assert.ok(calls[0].includes(`--filter-platform ${target}`));
    assert.ok(calls[0].endsWith(`|${info.target}`));
    assert.equal(graph.target_directory, info.target);
    assert.equal(graph.build_directory, resolve(root,'game/target'));
    assert.ok(!graph.resolve.nodes.some(node => graph.packages.find(p=>p.id===node.id)?.name === 'wasm-only'));
    assert.ok(result.products.some(path=>/\.(so|dylib)$/.test(path)));
    assert.ok(result.products.every(path=>!path.endsWith('.rlib')));
    assert.ok(!existsSync(resolve(info.target,target,'gpu-dev/libfoo_gpu.rlib')));
  };
  try {
    bake();
    const declaration = resolve(dir, '.shells/surfaces.json');
    assert.deepEqual(JSON.parse(readFileSync(declaration, 'utf8')), {world:[
      {name:'seed', default:7}, {name:'paused', default:false}, {name:'label', default:'say "hi"\n雪'},
    ]});
    const timestamp = new Date(1234000);
    utimesSync(declaration, timestamp, timestamp);
    const web = info.prepare(true, {target:'wasm32-unknown-unknown', env:{...process.env, CARGO_TARGET_DIR:info.target}});
    assert.ok(web.resolve.nodes.some(node => web.packages.find(p=>p.id===node.id)?.name === 'wasm-only'));
    const privateBuild = resolve(dir,'private-build');
    assert.equal(info.prepare(true, {target, env:{...process.env, CARGO_BUILD_BUILD_DIR:privateBuild}}).build_directory, privateBuild);
    pkg('game/deps/new-dependency','new-dependency');
    write('game/games/foo/logic/Cargo.toml', readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8').replace('[dependencies]', '[dependencies]\nnew-dependency={path="../../../deps/new-dependency"}'));
    assert.throws(bake, /locked|lock file/);
    update();
    bake();
    assert.equal(statSync(declaration).mtimeMs, timestamp.getTime(), 'a rebuilt GPU retains an unchanged declaration');
    assert.ok(graph.resolve.nodes.some(node => graph.packages.find(p=>p.id===node.id)?.name === 'new-dependency'));
  } finally { process.env.PATH = previous; }
}), 180000);

test('R12 named in-tree game resolves without EXACT_APP_DIR', () => fixture(({app, dir}) => {
  delete process.env.EXACT_APP_DIR;
  assert.equal(app().dir,dir);
}));


test('R12 deploy captures initialized dependency submodules as source', () => fixture(({app,root,write,run})=>{
  const resolved=app();resolved.cargoPackage('gpu');
  run('cargo',['generate-lockfile','--offline']);
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  write('vendor/fixture-source/data.txt','captured submodule');
  run('git',['-C','vendor/fixture-source','init','-q']);
  run('git',['-C','vendor/fixture-source','add','data.txt']);
  run('git',['-C','vendor/fixture-source','-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','source']);
  run('git',['init','-q']);run('git',['add','.']);
  const submoduleCommit=run('git',['-C','vendor/fixture-source','rev-parse','HEAD']).trim();
  mkdirSync(resolve(root,'vendor/absent-source'));
  run('git',['update-index','--add','--cacheinfo',`160000,${submoduleCommit},vendor/absent-source`]);
  run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']);
  assert.throws(()=>snapshotOf(resolved,{},root), /vendor\/absent-source.*git submodule update --init vendor\/absent-source/);
  run('git',['rm','--cached','vendor/absent-source']);
  run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','remove absent fixture']);
  const snapshot=snapshotOf(resolved,{},root);
  try {
    assert.ok(snapshot.sources.some(source=>source.roles.includes('submodule')));
    const staged=materializeSnapshot(snapshot,resolve(root,'target/run'),resolved);
    assert.equal(readFileSync(resolve(staged.exactRoot,'vendor/fixture-source/data.txt'),'utf8'),'captured submodule');
  } finally {disposeSnapshot(snapshot);}
}));


test('R13 capture refuses tracked files under inferred game output roots',()=>fixture(({app,root,write,run})=>{
  const resolved=app();resolved.cargoPackage('gpu');
  run('cargo',['generate-lockfile','--offline']);run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  run('git',['init','-q']);run('git',['add','.']);run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']);
  for(const output of ['target','.shells','dist','dist.previous','artifacts']) {
    const path=`game/games/foo/${output}/source.rs`;write(path,'source');run('git',['add','-f',path]);
    assert.throws(()=>snapshotOf(resolved,{dirty:true},root),error=>error.message.includes(path)&&/tracked/.test(error.message));
    run('git',['rm','--cached',path]);rmSync(resolve(root,path));
  }
}),30000); // five captures, each spawning cargo and git: more than the default five seconds on a loaded Mac


test('R13 stale shell lock without a captured lock refuses and is removed',()=>fixture(({app,dir})=>{
  app().cargoPackage('gpu');rmSync(resolve(dir,'Cargo.lock'));
  assert.throws(()=>app().cargoPackage('gpu'),/stale.*Cargo.lock|Cargo.lock.*stale/);
  assert.equal(existsSync(resolve(dir,'.shells/Cargo.lock')),false);
}));
test('R13 ordinary workspace without a lock resolves metadata',()=>fixture(({app,root,pkg,write})=>{
  process.env.EXACT_APP_DIR=resolve(root,'game/ordinary/plain');
  pkg('game/ordinary/plain','plain-web');
  write('game/ordinary/plain/app.json',JSON.stringify({name:'Plain',app:{id:'com.exact.plain',name:'Plain'}}));
  write('game/ordinary/plain/app.contract','component App\n  view\n');
  rmSync(resolve(root,'game/Cargo.lock'),{force:true});
  assert.equal(app('plain').cargoPackage('web').name,'plain-web');
}));
test('R13 explicit update-lock accepts a deliberate dependency change',()=>fixture(({app,dir,write,update})=>{
  app().cargoPackage('gpu');const before=readFileSync(resolve(dir,'Cargo.lock'),'utf8');
  write('game/deps/exact-game-render/Cargo.toml',readFileSync(resolve(dir,'../../deps/exact-game-render/Cargo.toml'),'utf8').replace('version="0.1.0"','version="0.2.0"'));
  assert.throws(()=>app().cargoPackage('gpu'),/locked|lock file/);
  update();assert.ok(app().cargoPackage('gpu'));assert.notEqual(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),before);
}));

test.each([false, true])('ordinary buildBake with split directories=%s streams progress and retains product receipts', async split => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'r14-no-lock-')));
  const write = (name, text) => { mkdirSync(dirname(resolve(dir,name)), {recursive:true}); writeFileSync(resolve(dir,name),text); };
  try {
    const target = bakeTarget('linux'), id = 'com.exact.plain';
    write('Cargo.toml', '[workspace]\nmembers=["linux"]\nresolver="2"\n');
    write('linux/Cargo.toml', '[package]\nname="plain-linux"\nversion="0.1.0"\nedition="2021"\n');
    write('linux/src/main.rs', 'fn main() { let unused = 1; }');
    write('app.contract', 'component App\n  view\n');
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
    assert.ok(receipt.products.some(p=>p.path.endsWith('/plain-linux')));
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

test('R14 external game capture refuses tracked output roots',()=>fixture(({app,root,write,run})=>{
  const external = resolve(root, 'outside/foreign');
  mkdirSync(external,{recursive:true});
  for (const path of ['app.json','app.contract','Cargo.lock','logic/Cargo.toml','logic/src/lib.rs']) {
    write(`outside/foreign/${path}`,readFileSync(resolve(root,'game/games/foo',path)));
  }
  // Keep the fixture dependencies pointing at its engine from this different depth.
  const manifest = resolve(external,'logic/Cargo.toml');
  writeFileSync(manifest,readFileSync(manifest,'utf8').replaceAll('../../../deps/','../../../game/deps/'));
  write('outside/foreign/.gitignore','/.shells/\n/target/\n/dist/\n/dist.previous/\n/artifacts/\n');
  process.env.EXACT_APP_DIR=external;
  const resolved=app();resolved.cargoPackage('gpu');
  run('cargo',['generate-lockfile','--offline']);run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  run('git',['init','-q']);run('git',['add','.']);run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']);
  for(const output of ['target','.shells','dist','dist.previous','artifacts']) {
    const path=`outside/foreign/${output}/source.rs`;write(path,'source');run('git',['add','-f',path]);
    assert.throws(()=>snapshotOf(resolved,{dirty:true},root),error=>error.message.includes(path)&&/tracked/.test(error.message));
    run('git',['rm','--cached',path]);rmSync(resolve(root,path));
  }
}), 60000);

test('R15 reproducibility flags follow workspace locks and always lock game shells', async () => {
  const {cargoReproducibilityFlags} = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'r15-locks-'));
  try {
    const ordinary = {workspace:dir,manifest:{}};
    assert.deepEqual(cargoReproducibilityFlags(ordinary),[]);
    writeFileSync(resolve(dir,'Cargo.lock'), '# stray lock');
    assert.deepEqual(cargoReproducibilityFlags(ordinary),[]);
    writeFileSync(resolve(dir,'Cargo.toml'), '[workspace]\nmembers=[]\n');
    assert.deepEqual(cargoReproducibilityFlags(ordinary),['--locked','--offline']);
    const game = {workspace:resolve(dir,'.shells'),manifest:{game:{}}};
    assert.deepEqual(cargoReproducibilityFlags(game),['--locked','--offline']);
    assert.deepEqual(cargoReproducibilityFlags(game,dir),['--locked','--offline']);
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

test('R15 the root Caltrain workspace refuses a missing lock and accepts its restored lock',()=>fixture(({app,root,pkg,write,run,flags})=>{
  pkg('apps/caltrain/web','caltrain-web');
  write('Cargo.toml','[workspace]\nmembers=["apps/caltrain/web"]\nresolver="2"\n');
  write('apps/caltrain/app.contract','component App\n  view\n');
  write('apps/caltrain/app.json',JSON.stringify({name:'Caltrain',app:{id:'com.exact.caltrain',name:'Caltrain'}}));
  process.env.EXACT_APP_DIR=resolve(root,'apps/caltrain');
  const metadata=()=>spawnSync('cargo',['metadata',...flags(app('caltrain')),'--format-version','1'],{cwd:root,encoding:'utf8'});
  assert.match(metadata().stderr,/locked|lock file/);
  run('cargo',['generate-lockfile','--offline']);
  assert.equal(metadata().status,0);
}));

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

test('a worktree whose target resolves into another checkout is refused', async () => {
  const { mkdtempSync, mkdirSync, symlinkSync, rmSync, writeFileSync } = await import('node:fs');
  const { resolve } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const { spawnSync } = await import('node:child_process');
  const { assertOwnTarget } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-target-')), main = resolve(root, 'main'), lane = resolve(root, 'lane');
  try {
    const git = (...args) => assert.equal(spawnSync('git', args, { cwd: main }).status, 0, args.join(' '));
    mkdirSync(main); git('init', '-q'); writeFileSync(resolve(main, 'a'), 'a'); git('add', 'a');
    git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'a'); git('worktree', 'add', '-q', lane);
    mkdirSync(resolve(main, 'target')); symlinkSync(resolve(main, 'target'), resolve(lane, 'target'));
    assertOwnTarget(resolve(main, 'target'), main);
    assert.throws(() => assertOwnTarget(resolve(lane, 'target'), lane), /another checkout of this repository/);
    mkdirSync(resolve(root, 'private'));
    assertOwnTarget(resolve(root, 'private'), lane);
  } finally { rmSync(root, { recursive: true, force: true }); }
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
    copyShaders(app,target); assert.deepEqual([...shaderFiles(app).keys()].sort(),['a.wgsl','b.wgsl']);
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
    symlinkSync(resolve(dir,'gpu/shaders/a.wgsl'),resolve(dir,'pack/b.wgsl'));
    assert.throws(()=>shaderFiles(app));
  } finally { rmSync(dir,{recursive:true,force:true}); }
});
