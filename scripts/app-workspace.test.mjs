// An app's and a game's workspaces (scripts/app.mjs `resolveApp`, game/app/shells.mjs): shells, locks, Cargo graphs, GPU and
// render ownership, and what a deploy captures (R12–R15).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, dirname, resolve, sep } from 'node:path';
import { resolveApp, buildBake, bakeTarget } from './app.mjs';
import { contractCompiler, snapshotOf, materializeSnapshot, disposeSnapshot } from './deploy.mjs';
import { pinned } from './test-workspace.mjs';

// These cases run cargo (the filesystem tool, bakes, locks). A shell whose PATH
// omits rustup's bin directory still finds it there; without cargo, say so.
const cargoBin = resolve(process.env.CARGO_HOME ?? resolve(homedir(), '.cargo'), 'bin');
if (!(process.env.PATH ?? '').split(delimiter).includes(cargoBin)) process.env.PATH = `${process.env.PATH ?? ''}${delimiter}${cargoBin}`;
if (!Bun.which('cargo', { PATH: process.env.PATH })) throw new Error(`these tests need cargo: put it on PATH or in ${cargoBin}`);
// The fixtures name their apps; a caller's EXACT_APP_DIR would redirect every one.
delete process.env.EXACT_APP_DIR;

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
    for (const path of ['scripts/app.mjs','scripts/bun.mjs','scripts/contract-diagnosis.mjs','scripts/filesystem.mjs','scripts/rust.mjs','scripts/origin.mjs','scripts/install-page.mjs','scripts/sweep.mjs','scripts/app.schema.json','host/web/stages.mjs','host/web/look.mjs','host/apple/assets.mjs','host/apple/buildinfo.mjs','game/app/shells.mjs','game/.cargo/config.toml']) {
      write(path, readFileSync(resolve(import.meta.dir,'..',path)));
    }
    const { resolveApp: localResolveApp, cargoReproducibilityFlags: flags } = await import(resolve(root,'scripts/app.mjs'));
    const {prepareGame} = await import(resolve(root,'game/app/shells.mjs'));
    write('rust-toolchain.toml', readFileSync(resolve(import.meta.dir,'../rust-toolchain.toml')));
    const deps = ['exact-game','exact-game-render','exact-game-app','exact-game-bake','exact-runner','exact-web','exact-web-capabilities','exact-apple','exact-linux','exact-windows','wasm-bindgen','wasm-bindgen-futures','web-sys'];
    write('Cargo.toml', '[workspace]\nmembers=["stub","contract"]\nresolver="2"\n'); pkg('stub','root-stub');
    pkg('contract','contract'); // This fixture has no Contract package imports.
    write('contract/src/main.rs', 'fn main() { println!("{}", r#"{\"packages\":[],\"sources\":[],\"consulted\":[]}"#); }');
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

test('shell fixture includes Apple build info', () => fixture(({root}) => {
  assert.ok(existsSync(resolve(root, 'host/apple/buildinfo.mjs')));
}), 60_000);

test('shell repair replaces half-written members before metadata', () => fixture(({app}) => {
  const before = app(), path = before.cargoPackage('gpu').manifest_path;
  rmSync(resolve(dirname(path),'src'),{recursive:true});
  assert.ok(app().cargoPackage('gpu'));
  assert.ok(existsSync(resolve(dirname(path),'src/lib.rs')));
}), 60_000);

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
}), 60_000);

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
}), 60_000);

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
}), 60_000);

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
}), 60_000);

test('resolving a surviving game prunes a deleted game shell', () => fixture(({app, game}) => {
  const gone = game('gone');
  process.env.EXACT_APP_DIR = gone;
  const shell = dirname(app('gone').cargoPackage('gpu').manifest_path);
  rmSync(gone,{recursive:true});
  process.env.EXACT_APP_DIR = resolve(dirname(gone),'foo');
  assert.ok(app().hasGpu);
  assert.ok(!existsSync(shell));
}), 60_000);

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
}), 60_000);

test('missing game entry refuses by key at resolve time', () => fixture(({app, dir, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  write('game/games/foo/app.json',JSON.stringify({...manifest,game:undefined}));
  assert.throws(app, /app.json.*game|game.*required/);
}), 60_000);

test('game.type exports are checked by Rust compilation, not app resolution', () => fixture(({app, dir, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  write('game/games/foo/app.json',JSON.stringify({...manifest,game:{...manifest.game,type:'SmallGmae'}}));
  assert.ok(app().hasGpu);
}), 60_000);

test('game.type module paths resolve before Rust checks the referenced export', () => fixture(({app, dir, write}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  write('game/games/foo/app.json',JSON.stringify({...manifest,game:{...manifest.game,type:'play::SmallGame'}}));
  assert.ok(app().hasGpu);
  write('game/games/foo/logic/src/lib.rs', 'pub mod play;');
  write('game/games/foo/logic/src/play.rs', 'pub struct SmallGame;');
  assert.ok(app().hasGpu);
}), 60_000);

test('build graph refuses a requested GPU surface that Cargo cannot find', () => fixture(({app, pkg, root, run}) => {
  pkg('game/ordinary/foo','ordinary-web');
  const resolved = app();
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  const fake = {...resolved, workspace:resolve(root,'game'), hasGpu:true, crate:kind=>`ordinary-${kind}`};
  assert.throws(()=>buildBake(fake,'web','wasm32-unknown-unknown'), /GPU.*ordinary-gpu|ordinary-gpu.*surface/);
}), 60_000);

test('deploy excludes generated shells and regenerates them from captured game source', () => fixture(({app, root, write, run, game, dir, update}) => {
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  manifest.game.render = {crate:'foo-render',hooks:'Hooks'};
  write('game/games/foo/app.json', JSON.stringify(manifest));
  write('game/games/foo/render/Cargo.toml','[package]\nname="foo-render"\nversion="0.1.0"\nedition="2021"\nworkspace="../.shells"\n');
  write('game/games/foo/render/src/lib.rs','pub struct Hooks;');
  write('game/games/foo/render/shaders/fog.wgsl','// captured shader');
  update();
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
    assert.ok(readFileSync(resolve(dirname(gpu.manifest_path),'src/lib.rs'),'utf8').includes('game_render::Hooks'));
    const hooks = metadata.packages.find(p=>p.name==='foo-render');
    assert.ok(hooks.manifest_path.startsWith(staged.sourceRoot));
    assert.equal(readFileSync(resolve(dirname(hooks.manifest_path),'shaders/fog.wgsl'),'utf8'),'// captured shader');
  } finally { disposeSnapshot(snapshot); }
}), 60000); // Three lockfiles, a commit, a capture and Cargo metadata; Windows filesystem cost is higher.

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
}), 60_000);

test('R12 the captured source lock replaces a corrupted generated lock', () => fixture(({app, dir}) => {
  app().cargoPackage('gpu');
  const lock = resolve(dir, 'Cargo.lock');
  assert.ok(existsSync(lock), 'source lock must be outside ignored shells');
  const captured = readFileSync(lock, 'utf8');
  writeFileSync(resolve(dir, '.shells/Cargo.lock'), 'invalid generated cache');
  app().cargoPackage('gpu');
  assert.equal(readFileSync(resolve(dir, '.shells/Cargo.lock'), 'utf8'), captured);
}), 60_000);

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
}), 60_000);

test('render crate isolation checks renamed transitive normal, build and inactive-target Cargo edges', () => fixture(({app, dir, root, run, write, update, pkg}) => {
  const previousDeclaration = app();
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  manifest.game.render = {crate:'foo-render',hooks:'Hooks'};
  write('game/games/foo/app.json', JSON.stringify(manifest));
  pkg('game/games/foo/render','foo-render','pub struct Hooks;');
  const renderManifest = 'game/games/foo/render/Cargo.toml';
  write(renderManifest, readFileSync(resolve(root,renderManifest),'utf8') + '\nworkspace="../.shells"\n');
  update();
  const graph = app().prepare();
  const hooks = graph.packages.find(p=>p.name==='foo-render');
  assert.ok(graph.workspace_members.includes(hooks.id));
  const original = readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8');
  pkg('game/deps/bridge','bridge');
  write('game/deps/bridge/Cargo.toml', readFileSync(resolve(root,'game/deps/bridge/Cargo.toml'),'utf8') + '\n[dependencies]\nrenamed-hook={package="foo-render",path="../../games/foo/render"}\n');
  for (const table of ['dependencies','build-dependencies', 'target.\'cfg(target_os = "haiku")\'.dependencies']) {
    write('game/games/foo/logic/Cargo.toml', `${original}\n[${table}]\nbridge-alias={package="bridge",path="../../../deps/bridge"}\n`);
    assert.throws(update, /GPU-only: foo-logic -> bridge -> foo-render/);
  }
  // Capture the otherwise valid lock as an author could, then ask for a Windows
  // bake. Filtering out Haiku before checking would incorrectly admit this graph.
  write('game/games/foo/Cargo.lock', readFileSync(resolve(dir,'.shells/Cargo.lock'),'utf8'));
  assert.throws(() => app().prepare(true,{target:'x86_64-pc-windows-msvc'}), /GPU-only: foo-logic -> bridge -> foo-render/);
  assert.throws(() => previousDeclaration.prepare(true,{target:'x86_64-pc-windows-msvc'}), /GPU-only: foo-logic -> bridge -> foo-render/);
  write('game/games/foo/logic/Cargo.toml', original);
  update();
  // Presentation is permitted to read logic resource types in the opposite direction.
  write(renderManifest, readFileSync(resolve(root,renderManifest),'utf8') + '\n[dependencies]\ngame-logic={package="foo-logic",path="../logic"}\n');
  update();
  assert.ok(app().prepare().packages.some(p=>p.id===hooks.id));
}), 60000);

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
}), 60_000);

test('R12 deploy captures initialized dependency submodules without building discovery in the snapshot', () => fixture(({app,root,write,run})=>{
  const resolved=app();resolved.cargoPackage('gpu');
  write('contract/src/main.rs', `fn main() { let target=std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into()); std::fs::write(std::path::Path::new(&target).join("source-discovery-target"), b"compiled").unwrap(); println!("{}", r#"{"packages":[],"sources":[],"consulted":[]}"#); }`);
  run('cargo',['generate-lockfile','--offline']);run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
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
  const snapshot=snapshotOf(resolved,{},root), previousPath=process.env.PATH, cargo=Bun.which('cargo'), trace=resolve(root,'target/snapshot-cargo');
  write('target/probe/cargo', `#!/bin/sh\ncase "$1" in build|run) printf '%s\\n' "$PWD" >> '${trace}';; esac\nexec '${cargo}' "$@"\n`);chmodSync(resolve(root,'target/probe/cargo'),0o755);process.env.PATH=`${root}/target/probe:${previousPath}`;
  try {
    assert.ok(snapshot.sources.some(source=>source.roles.includes('submodule')));
    const staged=materializeSnapshot(snapshot,resolve(root,'target/run'),resolved,resolve(root,'target/source-cache'));
    assert.equal(readFileSync(resolve(staged.exactRoot,'vendor/fixture-source/data.txt'),'utf8'),'captured submodule');
    assert.equal(existsSync(trace),false,'source discovery spawned Cargo in the snapshot');
    assert.equal(readFileSync(resolve(root,'target/source-discovery-target'),'utf8'),'compiled');
    assert.equal(existsSync(resolve(staged.app.target,'source-discovery-target')),false);
  } finally {process.env.PATH=previousPath;disposeSnapshot(snapshot);}
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
}), 60_000);

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
  assert.throws(()=>app('plain').cargoPackage('web'),/Cargo\.lock does not match its Cargo\.toml files.*`cargo metadata --offline --format-version 1`.*\nerror: .*--locked/s);
  assert.equal(readFileSync(resolve(root,'game/Cargo.lock'),'utf8'),before);
}), 60_000);

test('R13 explicit update-lock accepts a deliberate dependency change',()=>fixture(({app,dir,write,update})=>{
  app().cargoPackage('gpu');const before=readFileSync(resolve(dir,'Cargo.lock'),'utf8');
  write('game/deps/exact-game-render/Cargo.toml',readFileSync(resolve(dir,'../../deps/exact-game-render/Cargo.toml'),'utf8').replace('version="0.1.0"','version="0.2.0"'));
  assert.throws(()=>app().cargoPackage('gpu'),/locked|lock file/);
  update();assert.ok(app().cargoPackage('gpu'));assert.notEqual(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),before);
}), 60_000);

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
}), 60_000);

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
    assert.throws(() => contractCompiler(lane), /another checkout of this repository/);
    mkdirSync(resolve(root, 'private'));
    assertOwnTarget(resolve(root, 'private'), lane);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 60_000);

test('locked metadata fetches a missing git checkout without rewriting the lock', async () => {
  const { lockedMetadata } = await import('./app.mjs'), dir = pinned(mkdtempSync(resolve(tmpdir(), 'exact-metadata-fetch-')));
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
}, 60_000);

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
    const { receiptChanges, refuseStale } = await import('./agent-launch.mjs');
    const { utimesSync } = await import('node:fs');
    const receipt = resolve(dir, 'target/receipt.json');
    const built = new Date(Date.now() + 10000), edited = new Date(+built + 10000);
    for (const platform of ['ios', 'macos']) {
      write('target/receipt.json', JSON.stringify({target: platform === 'ios' ? 'aarch64-apple-ios-sim' : 'aarch64-apple-darwin'}));
      utimesSync(receipt, built, built);
      assert.deepEqual(receiptChanges(receipt, app), []);
      const source = resolve(dir, `${platform}/modules/Input.swift`);
      utimesSync(source, edited, edited);
      const changed = receiptChanges(receipt, app);
      assert.deepEqual(changed, [source]);
      assert.throws(() => refuseStale(platform, receipt, changed, 'rebuild'), /build is stale:.*Input.swift/);
      utimesSync(source, built, built);
    }
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

test('native resource snapshot preserves ignored scoped packages, executable bits and links', () => fixture(({app,root,dir,write,run})=>{
  const resolved=app();resolved.cargoPackage('gpu');
  run('cargo',['generate-lockfile','--offline']);
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  write('.gitignore','**/node_modules/\n');
  write('game/games/foo/server/helper','#!/bin/sh\necho helper ran\n');
  chmodSync(resolve(dir,'server/helper'),0o755);
  const {symlinkSync,readlinkSync}=require('node:fs');
  symlinkSync('helper',resolve(dir,'server/link'));
  run('git',['init','-q']);run('git',['add','.']);
  run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']);
  write('game/games/foo/server/node_modules/@scope/pkg/index.js','captured ignored dependency');
  const snapshot=snapshotOf(resolved,{dirty:true},root);
  try {
    const staged=materializeSnapshot(snapshot,resolve(root,'target/run'),resolved);
    assert.equal(readFileSync(resolve(staged.app.dir,'server/node_modules/@scope/pkg/index.js'),'utf8'),'captured ignored dependency');
    assert.equal(statSync(resolve(staged.app.dir,'server/helper')).mode&0o111,0o111);
    assert.equal(readlinkSync(resolve(staged.app.dir,'server/link')),'helper');
  } finally {disposeSnapshot(snapshot);}
}),60000);
