import { test } from 'bun:test';
// These cases run cargo (the filesystem tool, bakes, locks). A shell whose PATH
// omits rustup's bin directory still finds it there; without cargo, say so.
const cargoBin = resolve(process.env.CARGO_HOME ?? resolve(homedir(), '.cargo'), 'bin');
if (!(process.env.PATH ?? '').split(delimiter).includes(cargoBin)) process.env.PATH = `${process.env.PATH ?? ''}${delimiter}${cargoBin}`;
if (!Bun.which('cargo', { PATH: process.env.PATH })) throw new Error(`these tests need cargo: put it on PATH or in ${cargoBin}`);
// The fixtures name their apps; a caller's EXACT_APP_DIR would redirect every one.
delete process.env.EXACT_APP_DIR;
import assert from 'node:assert/strict';
import { classifyArtifacts } from './app.mjs';
import { chromium } from './agent-launch.mjs';

test('Windows browser discovery accepts an installed Chrome and explicit executable paths with spaces', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact browser paths-'));
  try {
    const chrome = resolve(dir, 'Google/Chrome/Application/chrome.exe');
    mkdirSync(dirname(chrome), {recursive:true});
    writeFileSync(chrome, '');
    chmodSync(chrome, 0o755);
    assert.equal(chromium({ProgramFiles:dir}, 'win32').executable, chrome);
    assert.equal(chromium({CHROME:chrome}, 'win32').unavailable, null);
    assert.ok(chromium({CHROME:resolve(dir, 'missing.exe')}, 'win32').unavailable);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

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
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { basename, delimiter, dirname, resolve, sep } from 'node:path';
import { homedir, tmpdir } from 'node:os';
import { resolveApp, buildBake, bakeTarget, pendingBuildInputs } from './app.mjs';
import { hermesIos } from './app.mjs';
import { HERMES_IOS_ARCHIVES, provisionHermesIos, iosAssets, infoPlist, macInfoPlist, macReleaseEntitlements, useXcode, writeUsageStrings, designCompatible, COMPATIBLE_SDK } from '../host/apple/build.mjs';
import { snapshotOf, materializeSnapshot, disposeSnapshot } from './deploy.mjs';

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
    for (const path of ['scripts/app.mjs','scripts/filesystem.mjs','scripts/rust.mjs','scripts/install-page.mjs','scripts/sweep.mjs','scripts/app.schema.json','host/web/stages.mjs','game/app/shells.mjs','game/.cargo/config.toml']) {
      write(path, readFileSync(resolve(import.meta.dir,'..',path)));
    }
    const { resolveApp: localResolveApp, cargoReproducibilityFlags: flags } = await import(resolve(root,'scripts/app.mjs'));
    const {prepareGame} = await import(resolve(root,'game/app/shells.mjs'));
    write('rust-toolchain.toml', readFileSync(resolve(import.meta.dir,'../rust-toolchain.toml')));
    const deps = ['exact-game','exact-game-render','exact-game-app','exact-game-bake','exact-runner','exact-web','exact-web-capabilities','exact-apple','exact-linux','exact-windows','wasm-bindgen','wasm-bindgen-futures','web-sys'];
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

test('in-repo app members can own a separate locked workspace without EXACT_APP_DIR', () => fixture(({app, root, pkg, write, run}) => {
  delete process.env.EXACT_APP_DIR;
  write('apps/separate/app.contract', 'component App\n  view\n');
  write('apps/separate/app.json', JSON.stringify({name:'Separate',app:{id:'com.exact.separate',name:'Separate'}}));
  pkg('apps/separate/web', 'separate-web');
  write('apps/separate/web/Cargo.toml', '[package]\nworkspace="../../../optional"\nname="separate-web"\nversion="0.1.0"\nedition="2021"\n');
  write('optional/Cargo.toml', '[workspace]\nmembers=["../apps/separate/web"]\nresolver="2"\n');
  run('cargo', ['generate-lockfile', '--offline', '--manifest-path', resolve(root,'optional/Cargo.toml')]);
  const resolved = app('separate');
  assert.equal(resolved.workspace, resolve(root,'optional'));
  assert.equal(resolved.target, resolve(root,'optional/target'));
  assert.equal(resolved.cargoPackage('web').name, 'separate-web');
  pkg('apps/separate/linux', 'separate-linux');
  write('apps/separate/linux/Cargo.toml', '[package]\nworkspace="../../.."\nname="separate-linux"\nversion="0.1.0"\nedition="2021"\n');
  assert.throws(() => app('separate'), /different Cargo workspaces/);
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
    assert.ok(pkg.manifest_path.startsWith(resolve(copy, '.shells') + sep));
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

test('ordinary GPU ownership requires its own manifest and matching metadata directory', () => fixture(({app, dir, pkg, write, run}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8')); delete manifest.game;
  // An ordinary app outside games/ sharing a workspace with an unrelated foo-gpu.
  process.env.EXACT_APP_DIR = resolve(dirname(dirname(dir)), 'ordinary/foo');
  pkg('game/ordinary/foo','foo-web');
  write('game/ordinary/foo/app.json',JSON.stringify(manifest)); write('game/ordinary/foo/app.contract','component App\n  view\n');
  pkg('game/ordinary/unrelated','foo-gpu');
  run('cargo',['metadata','--offline','--format-version','1','--manifest-path','game/Cargo.toml']);
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
}), 30000); // three lockfiles, a commit, a capture and cargo metadata: 1.8 s at load 35, past five seconds on a loaded Mac


test('rendered tree keeps focus; an accessible name is tree --ax\'s (LLP 1080.002)', async () => {
  const { render } = await import('./agent.mjs');
  const line = render('tree', {nodes:[{id:1, depth:0, type:'View', props:{testId:'play'}, focused:true}]});
  assert.match(line, /\[focused\]/);
  assert.doesNotMatch(line, /name=/);
});


test('a native hangup names how the app ended, its crash reports, and only its last 20 lines', async () => {
  const { hangup } = await import('./agent.mjs');
  const hostLines = Array.from({ length: 30 }, (_, i) => `app: line ${i}`);
  const hung = hangup({ what: 'the app hung up', pid: 999999, exit: { code: null, signal: 'SIGKILL' }, reports: ['/r/ExactIOS-1.ips'], hostLines });
  assert.match(hung, /^the app hung up \(killed by SIGKILL\)\ncrash report: \/r\/ExactIOS-1\.ips\napp: line 10\n/);
  assert.doesNotMatch(hung, /line 9\n/);
  assert.match(hangup({ what: 'clock did not answer', pid: process.pid }), /^clock did not answer \(pid \d+ still running\)$/);
  assert.match(hangup({ what: 'the app hung up', pid: 999999 }), /\(pid 999999 gone\)$/);
  assert.match(hangup({ what: 'the app exited', exit: { code: 3, signal: null } }), /\(exit code 3\)$/);
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
  const apply = runInNewContext(fn+';applyProps', {syncMedia:()=>{}, syncMarkup:()=>{}, settleValue:()=>{}, inputReady:true, HTMLIFrameElement:Element, HTMLImageElement:Element, HTMLVideoElement:Element});
  const attrs = new Map();
  const el = {setAttribute:(k,v)=>attrs.set(k,v), removeAttribute:k=>attrs.delete(k)};
  apply(el, {autofocus:'true'}, []);
  assert.equal(attrs.has('autofocus'), false);
  assert.equal(el.exactAutofocus, true);
  apply(el, {}, ['autofocus']);
  assert.equal(el.exactAutofocus, false);
});

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
    assert.equal(unitDepInfo(message, root, metadata), expected);
    assert.match(readFileSync(expected, 'utf8'), /env-dep:EXACT_GAME_PARANOID=0/);
    unit('aabbcc', 'selected wasm');
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
    pub enum Value { Number(f64), Bool(bool), Text(Box<str>), Other }
    impl Value {
      pub fn is_str(&self) -> bool { matches!(self, Self::Text(_)) }
      pub fn text(&self) -> &str { if let Self::Text(s) = self { s } else { "" } }
    }
    pub trait Args: Default { const FIELDS: &'static [(&'static str, ())]; fn values(&self) -> Vec<Value>; }
    impl Args for () {
      const FIELDS: &'static [(&'static str, ())] = &[("seed", ()), ("paused", ()), ("label", ())];
      fn values(&self) -> Vec<Value> { vec![Value::Number(7.0), Value::Bool(false), Value::Text("say \\"hi\\"\\n雪".into())] }
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
    assert.equal(graph.build_directory, info.target);
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
}), 30000); // two lockfiles, a submodule, three commits and two captures: 2.6 s at load 35, past five seconds on a loaded Mac


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


test('without its own lock a game resolves only against the SDK lock, never a leftover shell lock',()=>fixture(({app,dir,write})=>{
  app().cargoPackage('gpu');const captured=readFileSync(resolve(dir,'Cargo.lock'),'utf8');rmSync(resolve(dir,'Cargo.lock'));
  assert.throws(()=>app().cargoPackage('gpu'),/no captured lock and no SDK lock/);
  write('game/app/shells.lock',captured);
  assert.ok(app().cargoPackage('gpu'));
  assert.equal(existsSync(resolve(dir,'Cargo.lock')),false,'the SDK lock writes no lock into the game');
}));
test('ordinary resolution refuses missing and stale locks without writing them',()=>fixture(({app,root,pkg,write,run})=>{
  process.env.EXACT_APP_DIR=resolve(root,'game/ordinary/plain');
  pkg('game/ordinary/plain','plain-web');
  write('game/ordinary/plain/app.json',JSON.stringify({name:'Plain',app:{id:'com.exact.plain',name:'Plain'}}));
  write('game/ordinary/plain/app.contract','component App\n  view\n');
  rmSync(resolve(root,'game/Cargo.lock'),{force:true});
  assert.throws(()=>app('plain').cargoPackage('web'),/update the lock explicitly.*cargo metadata --offline/s);
  assert.equal(existsSync(resolve(root,'game/Cargo.lock')),false);
  run('cargo',['metadata','--offline','--format-version','1','--manifest-path','game/Cargo.toml']);
  const before=readFileSync(resolve(root,'game/Cargo.lock'),'utf8');
  assert.equal(app('plain').cargoPackage('web').name,'plain-web');
  const path='game/ordinary/plain/Cargo.toml';
  write(path,readFileSync(resolve(root,path),'utf8').replace('version="0.1.0"','version="0.2.0"'));
  assert.throws(()=>app('plain').cargoPackage('web'),/update the lock explicitly.*cargo metadata --offline/s);
  assert.equal(readFileSync(resolve(root,'game/Cargo.lock'),'utf8'),before);
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

test('a build env keeps the pinned toolchain and the checked Bun ahead of ambient ones', async () => {
  const { developmentBuildEnv } = await import('./app.mjs');
  const { readFileSync } = await import('node:fs');
  const { delimiter, dirname, resolve } = await import('node:path');
  const pinned = /^channel\s*=\s*"([^"]+)"/m.exec(readFileSync(resolve(import.meta.dir, '../rust-toolchain.toml'), 'utf8'))[1];
  const previous = process.env.RUSTUP_TOOLCHAIN;
  try {
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
  } finally { if (previous === undefined) delete process.env.RUSTUP_TOOLCHAIN; else process.env.RUSTUP_TOOLCHAIN = previous; }
});

// @ref LLP 1036.001 D5 — no CMake here: the refusals and the no-op paths.
test('lean iOS Hermes provisions into its per-pin cache, only from the pinned pristine source', () => {
  const home = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-hermes-home-')));
  try {
    const env = { ...process.env, HOME: home }; delete env.EXACT_HERMES_IOS_DIR;
    const { pin, root, cached } = hermesIos(env);
    assert.equal(cached, true);
    assert.equal(root, resolve(home, '.cache/exact/hermes', `${pin.slice(0, 12)}-lean-ios`));
    assert.throws(() => provisionHermesIos('ios-simulator', env), /no Hermes source at .*build-hermes\.sh --vanilla/);
    const source = resolve(home, '.cache/exact/hermes/hermes-src');
    spawnSync('git', ['init', '-q', source]);
    spawnSync('git', ['-C', source, '-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '--allow-empty', '-m', 'other']);
    assert.throws(() => provisionHermesIos('ios-simulator', env), new RegExp(`is at [0-9a-f]{40}; js/build.rs pins ${pin}`));
    // Complete archives are used as they are; the source is not consulted.
    for (const archive of HERMES_IOS_ARCHIVES) { mkdirSync(dirname(resolve(root, 'ios', archive)), { recursive: true }); writeFileSync(resolve(root, 'ios', archive), ''); }
    provisionHermesIos('ios', env);
    // Archives an override names are provisioned elsewhere; js/build.rs refuses missing ones.
    const elsewhere = resolve(home, 'elsewhere');
    assert.deepEqual(hermesIos({ ...env, EXACT_HERMES_IOS_DIR: elsewhere }), { pin, root: elsewhere, cached: false });
    provisionHermesIos('ios-simulator', { ...env, EXACT_HERMES_IOS_DIR: elsewhere });
    assert.equal(existsSync(elsewhere), false);
  } finally { rmSync(home, { recursive: true, force: true }); }
});

// Real actool: separate compiles into the same bundle silently replace Assets.car.
test.skipIf(process.platform !== 'darwin')('iOS distribution assets retain the icon and both launch appearances', () => {
  useXcode();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ios-assets-'));
  try {
    writeFileSync(resolve(dir, 'icon.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64'));
    const bundle = resolve(dir, 'Fixture.app');
    mkdirSync(bundle);
    const app = { dir, name: 'fixture', manifest: { icons: [{ src: 'icon.png', sizes: '1024x1024' }], background_color: '#fff', background_color_dark: '#123456' } };
    const keys = iosAssets(app, bundle, true, { catalog: true });
    assert.equal(keys.CFBundleIcons.CFBundlePrimaryIcon.CFBundleIconName, 'AppIcon');
    assert.equal(keys.UILaunchScreen.UIColorName, 'ExactLaunch');
    const result = spawnSync('xcrun', ['assetutil', '--info', resolve(bundle, 'Assets.car')], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const assets = JSON.parse(result.stdout);
    assert.ok(assets.some(asset => asset.Name === 'AppIcon'), 'Assets.car must retain AppIcon');
    assert.equal(assets.filter(asset => asset.Name === 'ExactLaunch').length, 2);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60000);

test('manifest colours follow the web and retired launch and alias keys are refused', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-manifest-'));
  const manifest = { name: 'Manifest', app: { id: 'test.manifest', name: 'Manifest' }, background_color: '#fff', background_color_dark: '#123456' };
  const read = value => { writeFileSync(resolve(dir, 'app.json'), JSON.stringify(value)); return readManifest(dir, 'fixture'); };
  try {
    assert.equal(read(manifest).background_color_dark, '#123456');
    assert.throws(() => read({ ...manifest, launch: { background: '#fff' } }), /launch/);
    assert.throws(() => read({ ...manifest, typescript: { aliases: { '@/*': './*' } } }), /aliases/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('device grants derive the plists, their translations and the release entitlements (LLP 1069.008)', async () => {
  const { readManifest } = await import('./app.mjs');
  // The bake's `reach` for `device.microphone purpose.mic` and
  // `device.speech-recognition purpose.speech`, with en and fr tables.
  const reach = { base: 'en', locales: ['en', 'fr'], rows: [], entitlements: ['com.apple.security.device.audio-input'],
    usage: { NSMicrophoneUsageDescription: { en: 'Records "takes".', fr: 'Enregistre.' }, NSSpeechRecognitionUsageDescription: { en: 'Transcribes.', fr: 'Transcrit.' } } };
  const app = { id: 'com.example.fixture', displayName: 'Fixture', manifest: { host: {} } };
  assert.equal(macReleaseEntitlements(null), null);
  assert.equal(macReleaseEntitlements({ reach: { ...reach, entitlements: [] } }), null);
  const entitled = macReleaseEntitlements({ reach });
  assert.deepEqual([...entitled.matchAll(/<key>([^<]+)<\/key><true\/>/g)].map((m) => m[1]), ['com.apple.security.device.audio-input']);
  for (const plist of [infoPlist(app, false, { reach }), macInfoPlist(app, { reach })]) {
    assert.match(plist, /<key>NSMicrophoneUsageDescription<\/key><string>Records "takes".<\/string>/);
    assert.match(plist, /<key>NSSpeechRecognitionUsageDescription<\/key><string>Transcribes.<\/string>/);
    assert.match(plist, /<key>CFBundleLocalizations<\/key><array><string>en<\/string><string>fr<\/string><\/array>/);
  }
  // No device grant, no key: the plists are what they were.
  for (const plist of [infoPlist(app, false), macInfoPlist(app, { reach: { ...reach, usage: {} } })]) assert.doesNotMatch(plist, /UsageDescription|CFBundleLocalizations/);
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-usage-'));
  try {
    writeUsageStrings(reach, dir);
    const fr = readFileSync(resolve(dir, 'fr.lproj/InfoPlist.strings'), 'utf8');
    assert.match(fr, /<key>NSMicrophoneUsageDescription<\/key><string>Enregistre.<\/string>/);
    assert.ok(existsSync(resolve(dir, 'en.lproj/InfoPlist.strings')));
    if (process.platform === 'darwin') {
      writeFileSync(resolve(dir, 'entitlements.plist'), entitled);
      for (const file of ['entitlements.plist', 'fr.lproj/InfoPlist.strings']) assert.equal(spawnSync('plutil', ['-lint', resolve(dir, file)]).status, 0, file);
    }
    // The hand-written keys are gone, and the refusal names the grant form.
    for (const platform of ['ios', 'macos']) {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ app: { id: 'com.example.fixture', name: 'Fixture' }, host: { [platform]: { permissions: { NSMicrophoneUsageDescription: 'x' } } } }));
      assert.throws(() => readManifest(dir, 'fixture'), new RegExp(`host\\.${platform}\\.permissions: deleted \\(LLP 1069\\.008\\); declare the device .*device\\.microphone purpose\\.microphone`));
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});


test('locked metadata fetches a missing git checkout without rewriting the lock', async () => {
  const { lockedMetadata } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-metadata-fetch-'));
  const git = resolve(dir, 'source'), app = resolve(dir, 'app');
  const run = (cwd, cmd, args, env = process.env) => {
    const r = spawnSync(cmd, args, {cwd, env, encoding:'utf8'});
    assert.equal(r.status, 0, r.stderr); return r.stdout;
  };
  try {
    for (const path of [git, app]) mkdirSync(resolve(path, 'src'), {recursive:true});
    writeFileSync(resolve(git, 'Cargo.toml'), '[package]\nname="local-source"\nversion="0.1.0"\nedition="2021"\n');
    writeFileSync(resolve(git, 'src/lib.rs'), 'pub fn value() {}');
    run(git, 'git', ['init', '-q']); run(git, 'git', ['add', '.']);
    run(git, 'git', ['-c', 'user.name=Test', '-c', 'user.email=test@example.test', 'commit', '-qm', 'source']);
    writeFileSync(resolve(app, 'Cargo.toml'), `[package]\nname="metadata-app"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nlocal-source={git="file://${git}"}\n`);
    writeFileSync(resolve(app, 'src/lib.rs'), '');
    run(app, 'cargo', ['generate-lockfile'], {...process.env, CARGO_HOME:resolve(dir, 'warm')});
    const lock = readFileSync(resolve(app, 'Cargo.lock'), 'utf8');
    const env = {...process.env, CARGO_HOME:resolve(dir, 'cold')};
    const missing = spawnSync('cargo', ['metadata', '--locked', '--offline', '--format-version', '1'], {cwd:app, env, encoding:'utf8'});
    assert.notEqual(missing.status, 0);
    const result = lockedMetadata(app, false, env);
    assert.equal(result.status, 0, result.stderr);
    assert.ok(JSON.parse(result.stdout).packages.some(p => p.name === 'local-source'));
    assert.equal(readFileSync(resolve(app, 'Cargo.lock'), 'utf8'), lock);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('the launch handler bakes `ExactLaunchMode` with or without documents (LLP 1069.010 D4)', () => {
  const app = (manifest) => ({ id: 'com.example.fixture', displayName: 'Fixture', name: 'fixture', manifest: { host: {}, ...manifest } });
  // An app that opens nothing still gets File ▸ New Window from `navigate-new`.
  const windows = macInfoPlist(app({ launch_handler: { client_mode: 'navigate-new' } }));
  assert.match(windows, /<key>ExactLaunchMode<\/key><string>navigate-new<\/string>/);
  assert.doesNotMatch(windows, /CFBundleDocumentTypes/);
  // With documents, both keys; the W3C list's first mode after `auto` wins.
  const documents = macInfoPlist(app({ file_handlers: [{ action: '/', accept: { 'text/markdown': ['.md'] } }], launch_handler: { client_mode: ['auto', 'navigate-new'] } }));
  assert.match(documents, /<key>CFBundleDocumentTypes<\/key>/);
  assert.match(documents, /<key>ExactLaunchMode<\/key><string>navigate-new<\/string>/);
  // No launch handler: the mode is still written, as the host's default `navigate-existing`; no document types.
  const plain = macInfoPlist(app({}));
  assert.match(plain, /<key>ExactLaunchMode<\/key><string>navigate-existing<\/string>/);
  assert.doesNotMatch(plain, /CFBundleDocumentTypes/);
});

test('an Apple app records the SDK it is built with unless its manifest keeps the design before 26', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-design-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Design', app: { id: 'com.example.design', name: 'Design' }, host: { macos: { minimumOS: '14.0', designRequiresCompatibility: true }, ios: { designRequiresCompatibility: true } } }));
    const manifest = readManifest(dir, 'design');
    const app = (host) => ({ id: 'com.example.design', manifest: { host } });
    assert.equal(designCompatible({ id: 'com.example.design', manifest }, 'macos'), true, 'the manifest field is read');
    assert.equal(designCompatible({ id: 'com.example.design', manifest }, 'ios'), true);
    assert.equal(designCompatible(app({ macos: { minimumOS: '14.0' } }), 'macos'), false, 'absent: the SDK the app is built with');
    assert.equal(designCompatible(app({ macos: { designRequiresCompatibility: true } }), 'ios'), false, 'per platform');
    assert.deepEqual(COMPATIBLE_SDK, { ios: '18.0', macos: '15.0' }, 'the last SDKs before the 26 design');
    // An app that needs 26 has no earlier design to keep.
    assert.throws(() => designCompatible(app({ macos: { minimumOS: '26.0', designRequiresCompatibility: true } }), 'macos'), /no earlier design/);
    assert.throws(() => designCompatible(app({ ios: { minimumOS: '26.0', designRequiresCompatibility: true } }), 'ios'), /no earlier design/);
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Design', app: { id: 'com.example.design', name: 'Design' }, host: { macos: { designRequiresCompatibility: 'yes' } } }));
    assert.throws(() => readManifest(dir, 'design'), /designRequiresCompatibility/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
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
});

test('simulator signing gives each app and embedded host a distinct Keychain identity', async () => {
  const {entitlements} = await import('../host/apple/build.mjs');
  const app = {id:'com.exact.test',manifest:{host:{ios:{}}}};
  const sim = entitlements(app), host = entitlements({...app,id:app.id+'.host'});
  assert.match(sim, /<key>application-identifier<\/key><string>com.exact.test<\/string>/);
  assert.match(host, /<key>application-identifier<\/key><string>com.exact.test.host<\/string>/);
  assert.doesNotMatch(sim, /com.apple.developer.team-identifier/);
  const device = entitlements(app, 'TEAM', false);
  assert.match(device, /TEAM.com.exact.test/);
  assert.match(device, /<key>get-task-allow<\/key><false\/>/);
});

test('setup accepts both official Binaryen release tags and package-manager version output', async () => {
  const {binaryenVersion} = await import('./exact.mjs');
  assert.equal(binaryenVersion('wasm-opt version 132 (version_132)\n'), 'version 132');
  assert.equal(binaryenVersion('wasm-opt version 132\n'), 'version 132');
  assert.notEqual(binaryenVersion('wasm-opt version 1320 (version_1320)'), 'version 132');
  assert.notEqual(binaryenVersion('missing'), 'version 132');
});

test.each(['notes', 'planner'])('OS-specific folders resolve for %s without app-specific rules', async name => {
  const { moduleDirectory } = await import('./app.mjs');
  const dir = (await import('node:fs')).realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-platform-folders-')));
  const previous = process.env.EXACT_APP_DIR;
  const write = (p, text = '') => { mkdirSync(dirname(resolve(dir, p)), {recursive:true}); writeFileSync(resolve(dir, p), text); };
  try {
    process.env.EXACT_APP_DIR = dir;
    write('src/app.contract', 'component App\n  view\n    text "test"\n');
    assert.throws(() => resolveApp(name), /no app.contract/);
    write('app.contract', 'use App from "./src/app.contract"\n');
    write('app.json', JSON.stringify({name,app:{id:`com.exact.${name}`,name}}));
    for (const p of ['ios','macos','web']) write(`${p}/Cargo.toml`, `[package]\nname="${name}-${p}"\nversion="0.1.0"\n`);
    write('ios/modules/Input.swift'); write('macos/modules/Input.swift'); write('web/modules/index.js');
    const app = resolveApp(`${name}-ios`);
    assert.equal(app.crate('ios'), `${name}-ios`); assert.equal(app.crate('macos'), `${name}-macos`);
    assert.deepEqual(app.modulesFor('ios').apple, [resolve(dir, 'ios/modules/Input.swift')]);
    assert.deepEqual(app.modulesFor('macos').apple, [resolve(dir, 'macos/modules/Input.swift')]);
    assert.equal(app.modules.web, resolve(dir, 'web/modules/index.js'));
    rmSync(resolve(dir, 'ios'), {recursive:true}); rmSync(resolve(dir, 'macos'), {recursive:true});
    write('modules/apple/Input.swift');
    const shared = resolveApp(name);
    assert.equal(shared.crate('ios'), `${name}-apple`); assert.equal(shared.crate('macos'), `${name}-apple`);
    assert.equal(moduleDirectory(dir, 'ios'), resolve(dir, 'modules/apple'));
    assert.deepEqual(shared.modulesFor('macos').apple, [resolve(dir, 'modules/apple/Input.swift')]);
    assert.deepEqual(shared.modules.apple, [resolve(dir, 'modules/apple/Input.swift')]);
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(dir, {recursive:true,force:true});
  }
});
