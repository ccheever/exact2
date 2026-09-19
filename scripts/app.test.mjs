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
      writeFileSync(resolve(dir, directory, 'x.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64'));
      bake();
      const changed = statSync(bakedPath, { bigint: true }).mtimeNs;
      assert.notEqual(changed, before, `${directory}: bake did not rerun`);
      const receipt = JSON.parse(readFileSync(receiptPath, 'utf8'));
      assert.ok(receipt.embedded.assets.some(a => a.name === `${prefix}/x.png`));
      bake();
      assert.equal(statSync(bakedPath, { bigint: true }).mtimeNs, changed, `${directory}: unchanged third build reran`);
      before = changed;
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 300000);

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { resolveApp, buildBake, bakeTarget, pendingBuildInputs } from './app.mjs';
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
    return resolve(root, dir);
  };
  try {
    for (const path of ['scripts/app.mjs','scripts/rust.mjs','scripts/install-page.mjs','scripts/app.schema.json','game/app/shells.mjs']) {
      write(path, readFileSync(resolve(import.meta.dir,'..',path)));
    }
    const { resolveApp: localResolveApp } = await import(resolve(root,'scripts/app.mjs'));
    write('rust-toolchain.toml', readFileSync(resolve(import.meta.dir,'../rust-toolchain.toml')));
    const deps = ['exact-game','exact-game-render','exact-game-app','exact-game-bake','exact-runner','exact-web','exact-apple','exact-linux','wasm-bindgen','wasm-bindgen-futures','web-sys'];
    write('Cargo.toml', '[workspace]\nmembers=["stub"]\nresolver="2"\n'); pkg('stub','root-stub');
    write('game/Cargo.toml', '[workspace]\nmembers=["deps/*","games/*/logic","ordinary/*"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\nlicense="MIT"\n[workspace.dependencies]\n' + deps.map(n=>`${n}={path="deps/${n}"}`).join('\n'));
    for (const dep of deps) pkg(`game/deps/${dep}`, dep);
    write('game/games/.gitignore', '*/.shells/\n');
    // Cargo permits an empty glob when its containing directory exists.
    pkg('game/ordinary/stub','ordinary-stub');
    const dir = game('foo'); process.env.EXACT_APP_DIR = dir;
    body({root, dir, write, run, pkg, game, app:(name='foo')=>localResolveApp(name)});
  } finally {
    if (previous === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = previous;
    rmSync(root,{recursive:true,force:true});
  }
}

test('shell repair replaces half-written members before metadata', () => fixture(({app}) => {
  const before = app(), path = before.cargoPackage('gpu').manifest_path;
  rmSync(resolve(dirname(path),'src'),{recursive:true});
  assert.ok(app().hasGpu);
  assert.ok(existsSync(resolve(dirname(path),'src/lib.rs')));
}));

test('art adds its baker on demand and retains it until generated outputs are pruned', () => fixture(({app, dir, write}) => {
  const baked = () => {
    const gpu = app().cargoPackage('gpu');
    const dependency = gpu.dependencies.some(d=>d.name==='exact-game-bake');
    assert.equal(gpu.targets.some(t=>t.kind.includes('custom-build')), dependency);
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

test('shell identity survives a renamed logic crate and removes old packages', () => fixture(({app, dir, write, run}) => {
  const before = app().cargoPackage('gpu').manifest_path;
  const manifest = JSON.parse(readFileSync(resolve(dir,'app.json'),'utf8'));
  manifest.game.crate = 'renamed-logic';
  write('game/games/foo/app.json',JSON.stringify(manifest));
  write('game/games/foo/logic/Cargo.toml','[package]\nname="renamed-logic"\nversion="0.1.0"\nedition="2021"\n');
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

test('deploy excludes generated shells and regenerates them from captured game source', () => fixture(({app, root, write, run}) => {
  const resolved = app();
  resolved.cargoPackage('gpu');
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/games/foo/.shells/Cargo.toml']);
  run('cargo',['generate-lockfile','--offline']);
  run('cargo',['generate-lockfile','--offline','--manifest-path','game/Cargo.toml']);
  run('git',['init','-q']); run('git',['add','.']);
  run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']);
  // A generated extra byte must never enter the captured source or dirty it.
  write('game/games/foo/.shells/generated-note','not source');
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
  const source = readFileSync(new URL('../host/web/glue.js', import.meta.url), 'utf8');
  const fn = source.slice(source.indexOf('function focusAutofocus()'), source.indexOf('function setInputReady'));
  const element = () => ({exactAutofocus:true, getClientRects:()=>[{}], matches:()=>false,
    setAttribute() {}, focus() { document.activeElement = this; }});
  const document = {body:{}, activeElement:null};
  const first = element(), other = element();
  const views = new Map([[1,first]]);
  const context = {document, views, root:{querySelectorAll:()=>[...views.values()]}, inputReady:true,
    inertAncestor:()=>false, getComputedStyle:()=>({visibility:'visible'})};
  const focus = runInNewContext('const autofocusProcessed = new WeakSet();'+fn+';focusAutofocus', context);
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
  const fn = source.slice(source.indexOf('function applyProps('), source.indexOf('// @ref LLP 1020 D2 — one page listener'));
  class Element {}
  const apply = runInNewContext(fn+';applyProps', {inputReady:true, HTMLIFrameElement:Element, HTMLImageElement:Element});
  const attrs = new Map();
  const el = {setAttribute:(k,v)=>attrs.set(k,v), removeAttribute:k=>attrs.delete(k)};
  apply(el, {autofocus:'true'}, []);
  assert.equal(attrs.has('autofocus'), false);
  assert.equal(el.exactAutofocus, true);
  apply(el, {}, ['autofocus']);
  assert.equal(el.exactAutofocus, false);
});

test.each(['library', 'executable'])('copied %s roots require unique compiler dep-info', async kind => {
  const { unitDepInfo } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-unit-dep-'));
  try {
    const dir = resolve(root, 'release'), deps = resolve(dir, 'deps'), src = resolve(root, 'src/main.rs');
    mkdirSync(deps, {recursive:true});
    const executable = kind === 'executable', target = executable ? 'game-native' : 'game_apple';
    const artifact = resolve(dir, executable ? target : `lib${target}.rlib`);
    writeFileSync(artifact, 'selected unit');
    const message = {filenames:executable ? [artifact] : [resolve(dir, `lib${target}.a`), artifact],
      executable:executable ? artifact : null, target:{name:target, src_path:src}};
    writeFileSync(resolve(dir, `${target}.d`), `${artifact}: ${src}\n`);
    const unit = (hash, bytes, source = src) => {
      const name = `${target.replaceAll('-', '_')}-${hash}`, dep = resolve(deps, `${name}.d`);
      writeFileSync(resolve(deps, executable ? name : `lib${name}.rlib`), bytes);
      writeFileSync(dep, `${dep}: ${source}\n\n# env-dep:EXACT_UPDATE_TRUST=development\n`);
      return dep;
    };
    unit('deadbeef', 'another unit');
    assert.throws(() => unitDepInfo(message, root), /no matching rustc unit/);
    const expected = unit('a11ce', 'selected unit', resolve(root, 'old/main.rs'));
    assert.throws(() => unitDepInfo(message, root), /no matching rustc unit/);
    writeFileSync(expected, `${resolve(root, 'copied.d')}: ${src}\n`);
    assert.throws(() => unitDepInfo(message, root), /no matching rustc unit/);
    unit('a11ce', 'selected unit');
    assert.equal(unitDepInfo(message, root), expected);
    assert.match(readFileSync(unitDepInfo(message, root), 'utf8'), /env-dep:EXACT_UPDATE_TRUST=development/);
    unit('aabbcc', 'selected unit');
    assert.throws(() => unitDepInfo(message, root), /ambiguous rustc unit/);
  } finally { rmSync(root, {recursive:true, force:true}); }
});


test('rustc unit dep-info accepts its raw output path with spaces', async () => {
  const { unitDepInfo } = await import('./app.mjs');
  const { spawnSync } = await import('node:child_process');
  const root = mkdtempSync(resolve(tmpdir(), 'exact unit dep '));
  try {
    const source = resolve(root, 'lib.rs'), artifact = resolve(root, 'libspace_unit.rlib');
    writeFileSync(source, 'pub fn value() -> u32 { 1 }');
    const result = spawnSync('rustc', ['--crate-name', 'space_unit', '--crate-type', 'lib',
      '--emit=dep-info,link', source, '--out-dir', root], {encoding:'utf8'});
    assert.equal(result.status, 0, result.stderr);
    const message = {filenames:[artifact], target:{name:'space_unit', src_path:source}};
    assert.equal(unitDepInfo(message, root), resolve(root, 'space_unit.d'));
  } finally { rmSync(root, {recursive:true, force:true}); }
});
