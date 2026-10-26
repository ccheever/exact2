// Builds and bakes (scripts/app.mjs `buildBake`, `bakeTarget`): artifacts and their evidence, receipts, GPU modules and shader
// packs, the build environment and what makes a build stale.
import { test } from 'bun:test';
import { createHash } from 'node:crypto';
import { createServer } from 'node:net';
import { cargoBakeInputs } from './cargo-dep-info.mjs';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { basename, delimiter, dirname, resolve } from 'node:path';
import { classifyArtifacts, buildBake, bakeTarget, pendingBuildInputs } from './app.mjs';
import { pinned } from './test-workspace.mjs';

test('kept registry units rebuild foreign dep-info while retaining valid units', async () => {
  const { keptCrates } = await import('../host/apple/crates.mjs');
  const { unitDepInfo } = await import('./cargo-dep-info.mjs');
  const root = pinned(realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-crate-proof-'))));
  const profile = 'cache-proof', first = resolve(root, 'first'), second = resolve(root, 'second');
  const metadata = target => ({ target_directory: target, build_directory: target });
  const build = target => {
    const result = spawnSync('cargo', ['build', '--offline', '--profile', profile, '--lib', '--message-format=json-render-diagnostics'], {
      cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, encoding: 'utf8',
    });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim().split('\n').map(line => JSON.parse(line)).filter(row => row.reason === 'compiler-artifact');
  };
  try {
    mkdirSync(resolve(root, 'src'));
    writeFileSync(resolve(root, 'Cargo.toml'), `[workspace]\n[package]\nname="cache-proof"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nzeroize="=1.9.0"\ncfg-if="=1.0.4"\n[profile.${profile}]\ninherits="dev"\ndebug=false\n`);
    writeFileSync(resolve(root, 'src/lib.rs'), 'pub fn clear(bytes: &mut [u8]) { zeroize::Zeroize::zeroize(bytes); }\n');
    const original = build(first).find(row => row.target.name === 'zeroize');
    const dep = unitDepInfo(original, root, metadata(first)), before = statSync(dep);
    const contents = readFileSync(dep, 'utf8');
    assert.ok(contents.startsWith(dep + ': '));
    writeFileSync(dep, resolve(root, 'foreign', basename(dep)) + contents.slice(dep.length));
    utimesSync(dep, before.atime, before.mtime);
    assert.throws(() => unitDepInfo(original, root, metadata(first)), /no matching rustc unit dep-info for zeroize/);
    const home = resolve(root, 'cache-home');
    const cache = keptCrates({ root, profile, env: { HOME: home, RUSTUP_TOOLCHAIN: 'fixture' } });
    cache.keep(first, 'app', resolve(root, 'Cargo.lock'));
    const store = resolve(home, '.cache/exact/apple-crates', `fixture-${profile}`, 'app');
    const generation = resolve(store, readdirSync(store).find(name => name.startsWith('gen-')));
    const snapshot = () => readdirSync(generation, { recursive: true }).sort().map(file => {
      const path = resolve(generation, file), stat = statSync(path, { bigint: true });
      return [file, stat.mtimeNs, stat.isFile() ? createHash('sha256').update(readFileSync(path)).digest('hex') : null];
    });
    const kept = snapshot();
    cache.take(second, 'app');
    const rebuilt = build(second), zeroize = rebuilt.find(row => row.target.name === 'zeroize');
    assert.ok(unitDepInfo(zeroize, root, metadata(second)).startsWith(second));
    assert.equal(zeroize.fresh, false, 'invalid evidence forces Cargo to regenerate the unit');
    assert.equal(rebuilt.find(row => row.target.name === 'cfg_if').fresh, true, 'valid registry units stay reusable');
    assert.deepEqual(snapshot(), kept, 'importing and rebuilding must leave the source generation unchanged');
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 60_000);

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

async function withContractBake(use, {binding=false}={}) {
  const root=resolve(import.meta.dir,'..'), old=new Date('2000-01-01T00:00:00Z');
  const dir=realpathSync(mkdtempSync(resolve(tmpdir(),'rebake-'))), name='rebake-'+basename(dir).split('-').at(-1).toLowerCase();
  const write=(path, bytes)=>{mkdirSync(dirname(resolve(dir,path)),{recursive:true});writeFileSync(resolve(dir,path),bytes);};
  try {
    write('rust-toolchain.toml',readFileSync(resolve(root,'rust-toolchain.toml')));
    write('Cargo.lock',readFileSync(resolve(root,'Cargo.lock')));
    write('Cargo.toml',`[workspace]\nmembers=["linux"${binding?',"binding"':''}]\nresolver="2"\n[patch.crates-io]\ntaffy={path=${JSON.stringify(resolve(root,'vendor/taffy'))}}\n`);
    write('linux/Cargo.toml',`[package]\nname="${name}-linux"\nversion="0.1.0"\nedition="2021"\n[build-dependencies]\nexact-js-bake={path=${JSON.stringify(resolve(root,'js/bake'))}}\n${binding?`[dependencies]\n${name}-binding={path="../binding"}\n`:''}`);
    write('linux/build.rs',`fn main() {
      if std::path::Path::new("../.during-bake").exists() { std::fs::rename("../.during-bake", "../app.contract").unwrap(); }
      exact_js_bake::build(std::path::Path::new(".."), "linux").unwrap();
      std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("ran"), b"ran").unwrap();
    }`);
    write('linux/src/main.rs',binding?`fn main(){println!("{}", ${name.replaceAll('-','_')}_binding::SOURCE);}`:'fn main(){let _=include_bytes!(concat!(env!("OUT_DIR"),"/app.plan"));}');
    if(binding) {
      write('binding/Cargo.toml',`[package]\nname="${name}-binding"\nversion="0.1.0"\nedition="2021"\n[build-dependencies]\ncontract={path=${JSON.stringify(resolve(root,'contract/cli'))}}\n`);
      write('binding/src/lib.rs','include!(concat!(env!("OUT_DIR"),"/binding.rs"));');
      write('binding/build.rs',`fn main(){
        let path=std::path::Path::new("../app.contract");
        contract::rerun_if_changed(path);
        contract::compile_path(path).unwrap();
        let source=std::fs::read_to_string(path).unwrap();
        std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("binding.rs"),format!("pub const SOURCE: &str = {:?};",source)).unwrap();
      }`);
    }
    const id='com.exact.rebake';
    write('app.json',JSON.stringify({app:{id,name:'Rebake'},rust:false,deploy:{store:{linux:'0'}}}));
    write('app.ts',`export const appId='${id}'; export const grants=''; export const answer=()=>{throw new Error('no sources');};`);
    const source=text=>`component App\n  view\n    text "${text}"\n`;
    write('app.contract',source('ORIGINAL')); utimesSync(resolve(dir,'app.contract'),old,old);
    const lock=spawnSync('cargo',['generate-lockfile','--offline'],{cwd:dir,encoding:'utf8'}); assert.equal(lock.status,0,lock.stderr);
    const target=bakeTarget('linux'), output=resolve(dir,'target/bakes');
    const app={dir,workspace:dir,target:process.env.CARGO_TARGET_DIR??resolve(root,'target'),name,id,manifest:{app:{id,name:'Rebake'},rust:false},crate:kind=>`${name}-${kind}`};
    const bake=()=>buildBake(app,'linux',target,{profile:'dev',output,env:{EXACT_UPDATE_TRUST:'development',EXACT_RUST_BUNDLE:undefined,EXACT_UPDATE_RECEIPT:undefined}});
    const plan=()=>readFileSync(resolve(output,`linux-${target}.plan`));
    const run=()=>{const r=spawnSync(resolve(app.target,target,'debug',name+'-linux'),[],{encoding:'utf8'});assert.equal(r.status,0,r.stderr);return r.stdout;};
    const scriptDir=()=>{
      const builds=resolve(app.target,target,'debug/build');
      const unit=readdirSync(builds).find(name=>name.startsWith(app.crate('linux')+'-') && existsSync(resolve(builds,name,'out/ran')));
      assert.ok(unit, 'the bake must leave its build-script marker');
      return resolve(builds,unit);
    };
    await use({dir,write,source,bake,plan,run,scriptDir,old});
  } finally {rmSync(dir,{recursive:true,force:true});}
}

// Real Cargo and the TypeScript baker must agree even when a restore keeps an old mtime.
test.skipIf(!process.env.EXACT_BAKE_CACHE_TEST)('restored and equal-mtime Contract sources rebake and then stay quiet', async () => {
  await withContractBake(({dir, write, source, bake, plan, scriptDir, old}) => {
    const path = resolve(dir, 'app.contract'), original = resolve(dir, 'app.contract.orig');
    copyFileSync(path, original); utimesSync(original, old, old);
    bake();
    const first = plan(); assert.ok(first.includes(Buffer.from('ORIGINAL')));
    write('app.contract', source('EDITED'));
    bake(); assert.ok(plan().includes(Buffer.from('EDITED')));
    renameSync(original, path);
    bake();
    assert.deepEqual(plan(), first, 'restoring the older source must restore the original plan');
    assert.equal(statSync(path).mtimeMs, old.getTime(), 'the builder must not touch the source');
    write('app.contract', source('EQUALNEW')); utimesSync(path, old, old);
    bake(); assert.ok(plan().includes(Buffer.from('EQUALNEW')), 'changed bytes at the same mtime and size must rebake');
    const ran = resolve(scriptDir(), 'out/ran'), stamp = statSync(ran, {bigint:true}).mtimeNs;
    const expected = plan();
    bake();
    assert.deepEqual(plan(), expected);
    assert.equal(statSync(ran, {bigint:true}).mtimeNs, stamp, 'an unchanged build must not run the bake');
    write('node_modules/@fixture/ui/package.json', JSON.stringify({name:'@fixture/ui', exports:'./child.contract'}));
    write('node_modules/@fixture/ui/child.contract', 'component Child\n  view\n    text "IMPORTED"\n');
    const library = resolve(dir, 'node_modules/@fixture/ui/child.contract');
    utimesSync(library, old, old);
    write('app.contract', 'use Child from "@fixture/ui"\ncomponent App\n  view\n    Child()\n');
    bake(); assert.ok(plan().includes(Buffer.from('IMPORTED')));
    const importedStamp = statSync(ran, {bigint:true}).mtimeNs;
    bake();
    assert.equal(statSync(ran, {bigint:true}).mtimeNs, importedStamp, 'discovering an import must not dirty the next build');
    writeFileSync(library, 'component Child\n  view\n    text "CHANGED!"\n');
    utimesSync(library, old, old);
    bake(); assert.ok(plan().includes(Buffer.from('CHANGED!')), 'installed Contract imports must be watched by content');
  });
}, 300000);

for (const entry of ['socket', 'unreadable file', 'unreadable directory', 'linked unreadable directory']) {
  test.skipIf(!process.env.EXACT_BAKE_CACHE_TEST || process.platform === 'win32' || (entry !== 'socket' && process.getuid?.() === 0))(`an unused ${entry} in a watched package permits quiet bakes`, async()=>{
    await withContractBake(async({dir,write,bake,plan,scriptDir,old})=>{
      write('node_modules/@fixture/ui/package.json',JSON.stringify({name:'@fixture/ui',exports:'./child.contract'}));
      write('node_modules/@fixture/ui/child.contract','component Child\n  view\n    text "PACKAGE_OK"\n');
      write('app.contract','use Child from "@fixture/ui"\ncomponent App\n  view\n    Child()\n');
      const pkg=resolve(dir,'node_modules/@fixture/ui'), cwd=process.cwd();
      mkdirSync(resolve(pkg,'.cache'));
      let server, blocked, payload;
      process.chdir(pkg);
      try {
        if(entry === 'socket') {
          server=createServer();
          await new Promise((ok,fail)=>{server.once('error',fail);server.listen('.cache/worker.sock',ok);});
        } else {
          const file=entry === 'unreadable file';
          blocked=entry === 'linked unreadable directory' ? resolve(dir,'.private') : resolve(pkg,'.cache/private');
          if(!file)mkdirSync(blocked);
          payload=file ? blocked : resolve(blocked,'payload');
          writeFileSync(payload,'before');utimesSync(payload,old,old);
          if(entry === 'linked unreadable directory')symlinkSync(blocked,resolve(pkg,'.cache/link'),'dir');
          chmodSync(blocked,0);
          assert.throws(()=>file ? readFileSync(blocked) : readdirSync(blocked), error=>['EACCES','EPERM'].includes(error.code));
        }
        bake(); assert.ok(plan().includes(Buffer.from('PACKAGE_OK')));
        // Without the optional .tamber file, Contract watches the installed directory.
        const watches=readFileSync(resolve(scriptDir(),'output'),'utf8').split('\n');
        assert.ok(watches.includes(`cargo:rerun-if-changed=${pkg}`),'the package directory must actually be watched');
        const stamp=()=>statSync(resolve(scriptDir(),'out/ran'),{bigint:true}).mtimeNs;
        const first=stamp(), expected=plan();
        bake(); assert.deepEqual(plan(),expected);
        assert.equal(stamp(),first,'an unchanged package must not rerun the bake');
        if(blocked) {
          chmodSync(blocked,entry === 'unreadable file' ? 0o600 : 0o700);
          writeFileSync(payload,'after!');utimesSync(payload,old,old);
          bake(); assert.deepEqual(plan(),expected);
          const readable=stamp();
          assert.notEqual(readable,first,'a now-readable input must change the revision even at an old mtime');
          bake(); assert.equal(stamp(),readable,'a readable unchanged package must stay quiet');
        }
      } finally {
        if(blocked)chmodSync(blocked,0o700);
        if(server)await new Promise(ok=>server.close(ok));
        process.chdir(cwd);
      }
    });
  },300000);
}

test.skipIf(!process.env.EXACT_BAKE_CACHE_TEST)('a dependency regenerates Rust when the contract is restored',async()=>{
  await withContractBake(({dir,write,source,bake,plan,run,old})=>{
    const path=resolve(dir,'app.contract'), saved=resolve(dir,'.saved');
    copyFileSync(path,saved);utimesSync(saved,old,old);
    bake(); assert.ok(run().includes('ORIGINAL'));
    write('app.contract',source('EDITED'));
    bake(); assert.ok(run().includes('EDITED'));
    renameSync(saved,path);bake();
    assert.ok(plan().includes(Buffer.from('ORIGINAL')));
    assert.ok(run().includes('ORIGINAL'),'the compiled dependency must agree with the restored plan');
  },{binding:true});
},300000);

test.skipIf(!process.env.EXACT_BAKE_CACHE_TEST)('restoring bytes seen before an in-build save rebakes',async()=>{
  await withContractBake(({dir,write,source,bake,plan,old})=>{
    bake();
    write('app.contract',source('BEFORE'));utimesSync(resolve(dir,'app.contract'),old,old);
    write('.during-bake',source('DURING'));utimesSync(resolve(dir,'.during-bake'),old,old);
    bake();assert.ok(plan().includes(Buffer.from('DURING')));
    write('app.contract',source('BEFORE'));utimesSync(resolve(dir,'app.contract'),old,old);
    bake();assert.ok(plan().includes(Buffer.from('BEFORE')),'restored pre-build bytes must not reuse the consumed revision');
  });
},300000);

function watchFixture(use) {
  const dir=mkdtempSync(resolve(tmpdir(),'watch-'));
  try {
    const out=resolve(dir,'out'), watched=resolve(dir,'watched'), target=resolve(dir,'target');
    for(const path of [out,watched,target])mkdirSync(path);
    const pkg={id:'watch',manifest_path:resolve(dir,'Cargo.toml'),targets:[{kind:['custom-build']}]};
    const env={CARGO_TARGET_DIR:target,EXACT_BAKE_OUTPUT:resolve(dir,'bake')};
    writeFileSync(resolve(out,'../output'),`cargo:rerun-if-changed=${watched}\n`);
    const messages=[{reason:'build-script-executed',package_id:pkg.id,out_dir:out}];
    const prepare=()=>cargoBakeInputs(pkg,'target','dev',env);
    use({dir,out,watched,env,messages,prepare,pkg});
  }finally{rmSync(dir,{recursive:true,force:true});}
}

test.skipIf(process.platform === 'win32')('a watched directory follows inner directory links without cycles or output aliases',()=>watchFixture(({dir,out,watched,env,messages,prepare})=>{
  const real=resolve(dir,'real');mkdirSync(real);writeFileSync(resolve(real,'input'),'ORIGINAL');
  symlinkSync(real,resolve(watched,'link'),'dir');
  symlinkSync(watched,resolve(real,'back'),'dir');
  symlinkSync('loop-b',resolve(watched,'loop-a'));symlinkSync('loop-a',resolve(watched,'loop-b'));
  mkdirSync(env.EXACT_BAKE_OUTPUT);
  env.CARGO_BUILD_BUILD_DIR=resolve(dir,'build');mkdirSync(env.CARGO_BUILD_BUILD_DIR);
  const outputs=[env.CARGO_TARGET_DIR,env.EXACT_BAKE_OUTPUT,env.CARGO_BUILD_BUILD_DIR,out];
  for(const [i,path] of outputs.entries())symlinkSync(path,resolve(watched,`output-${i}`),'dir');
  prepare()(messages);const first=env.EXACT_BAKE_INPUTS;
  writeFileSync(resolve(real,'input'),'RESTORED');utimesSync(resolve(real,'input'),new Date(0),new Date(0));
  const record=prepare();assert.notEqual(env.EXACT_BAKE_INPUTS,first,'bytes behind the inner directory link must change the revision');
  record(messages);const changed=env.EXACT_BAKE_INPUTS;
  for(const path of outputs)writeFileSync(resolve(path,'product'),'new output');
  prepare();assert.equal(env.EXACT_BAKE_INPUTS,changed,'output aliases must not change the next revision');
}));

test('disappearance after metadata records a missing input and a stable revision',()=>watchFixture(({out,watched,env,messages,prepare,pkg})=>{
  const path=resolve(watched,'vanish');
  // A directly watched missing path must contribute a row, even without a parent watch.
  writeFileSync(resolve(out,'../output'),`cargo:rerun-if-changed=${path}\n`);
  writeFileSync(path,'content');prepare()(messages);
  const script=`import {mock} from 'bun:test'; import * as fs from 'node:fs';
    const read=fs.readFileSync, remove=fs.rmSync;
    mock.module('node:fs',()=>({...fs,readFileSync:(path,...args)=>{if(path===${JSON.stringify(path)})remove(path);return read(path,...args);}}));
    const {cargoBakeInputs}=await import(${JSON.stringify(resolve(import.meta.dir,'cargo-dep-info.mjs'))});
    const env=${JSON.stringify(env)};
    cargoBakeInputs(${JSON.stringify(pkg)},'target','dev',env)(${JSON.stringify(messages)});
    console.log(env.EXACT_BAKE_INPUTS);
  `;
  const result=spawnSync(process.execPath,['-e',script],{encoding:'utf8'});
  assert.equal(result.status,0,result.stderr);
  assert.equal(existsSync(path),false,'the read must have raced with removal');
  const records=resolve(env.CARGO_TARGET_DIR,'bake-inputs');
  const record=JSON.parse(readFileSync(resolve(records,readdirSync(records)[0]),'utf8'));
  assert.deepEqual(record.paths,[path]);
  assert.notEqual(record.digest,createHash('sha256').digest('hex'),'a missing watch must not be silently omitted');
  prepare();
  assert.equal(env.EXACT_BAKE_INPUTS,result.stdout.trim(),'a second scan with the file gone must keep the recorded revision');
}));

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

test('watch archives from nightly require matching bytes and the exact compiler unit', async () => {
  const { unitDepInfo } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-watch-dep-'));
  try {
    const directory = resolve(root, 'aarch64-apple-watchos-sim/host-dev');
    const out = resolve(directory, 'build/caltrain-apple/a11ce/out');
    mkdirSync(out, { recursive: true });
    const file = resolve(directory, 'libcaltrain_apple.a'), unit = resolve(out, 'libcaltrain_apple-a11ce.a');
    const dep = resolve(out, 'caltrain_apple-a11ce.d'), src = resolve(root, 'lib.rs');
    const metadata = { target_directory: root, build_directory: root, packages: [{ id: 'caltrain', name: 'caltrain-apple' }] };
    const message = { package_id: 'caltrain', filenames: [file], target: { name: 'caltrain_apple', src_path: src } };
    writeFileSync(file, 'selected archive'); writeFileSync(unit, 'different bytes');
    writeFileSync(dep, `${dep}: ${src}\n`);
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
    writeFileSync(unit, 'selected archive');
    assert.equal(unitDepInfo(message, root, metadata), dep);
    writeFileSync(dep, `${dep}: ${root}/wrong.rs\n`);
    assert.throws(() => unitDepInfo(message, root, metadata), /no matching rustc unit/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('rustc unit dep-info written under another spelling of the same directory still matches', async () => {
  const { unitDepInfo } = await import('./app.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'exact-spelled-dep-')), alias = root + '-alias';
  try {
    // Cargo wrote the unit through one spelling (macOS's /private/tmp); the build asks through another (/tmp).
    symlinkSync(root, alias);
    const out = resolve(root, 'out/release'), deps = resolve(out, 'deps');
    mkdirSync(deps, { recursive: true });
    const src = resolve(root, 'src/lib.rs'), artifact = resolve(out, 'libleaf.rlib');
    writeFileSync(artifact, 'unit'); writeFileSync(resolve(deps, 'libleaf-a11ce.rlib'), 'unit');
    const dep = resolve(deps, 'leaf-a11ce.d');
    writeFileSync(dep, `${dep}: ${src}\n`);
    const metadata = { target_directory: resolve(alias, 'out'), build_directory: resolve(alias, 'out') };
    const message = { filenames: [resolve(alias, 'out/release/libleaf.rlib')], executable: null, target: { name: 'leaf', src_path: resolve(alias, 'src/lib.rs') } };
    assert.equal(realpathSync(unitDepInfo(message, alias, metadata)), realpathSync(dep));
    writeFileSync(dep, `${dep}: ${resolve(root, 'src/other.rs')}\n`);
    assert.throws(() => unitDepInfo(message, alias, metadata), /no matching rustc unit dep-info for leaf; searched target directory /);
  } finally { rmSync(alias, { force: true }); rmSync(root, { recursive: true, force: true }); }
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

test('production native bakes for Apple and Android link with fat LTO; developer builds and other hosts keep the profile', async () => {
  const {wholeProgram} = await import('./app.mjs');
  const fat = ['--config', 'profile.release.lto="fat"'];
  for (const kind of ['apple', 'ios', 'macos', 'tvos', 'android']) assert.deepEqual(wholeProgram(kind, 'release'), fat, kind);
  for (const [kind, profile] of [['android', 'host-dev'], ['ios', 'host-dev'], ['android', 'size'], ['linux', 'release'], ['windows', 'release'], ['web', 'web']])
    assert.deepEqual(wholeProgram(kind, profile), [], `${kind} at ${profile}`);
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

// LLP 1012.001.000 D1: a drive's check compares every receipt input's bytes with its digest, so a file replaced with
// other bytes of the same length, its mtime put back (`cp -p`, `rsync -a`), is still found.
test('a receipt input is found changed by its bytes, whatever its size and mtime', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-pending-'));
  try {
    const path = resolve(dir, 'a.swift');
    writeFileSync(path, 'let a = 1\n');
    const at = statSync(path), build = { binary: { inputs: [{ name: 'a.swift', path, sha256: createHash('sha256').update('let a = 1\n').digest('hex') }], missing: [], directories: [] } };
    assert.deepEqual(pendingBuildInputs(build), []);
    writeFileSync(path, 'let b = 2\n');
    utimesSync(path, at.atime, at.mtime);
    assert.deepEqual(pendingBuildInputs(build), ['a.swift']);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

// A claim's holder (the Apple build's, the touch runner's device lock) is live unless shown dead: gone, or a pid
// started after the claim; a claim that does not parse, or whose holder `ps` cannot date, is live (round-2 review).
test('a claim is live unless its holder is shown gone or reused', async () => {
  const { claimLive } = await import('./app.mjs');
  const claim = (pid, started) => JSON.stringify({ pid, started });
  assert.equal(claimLive(claim(process.pid, new Date().toISOString())), true, 'this process, claimed now');
  assert.equal(claimLive(claim(process.pid, '2000-01-01T00:00:00Z')), false, 'a pid started after its claim is reused');
  assert.equal(claimLive(claim(2 ** 22 + 12345, new Date().toISOString())), false, 'no such process');
  assert.equal(claimLive('not json'), true, 'unreadable: never guess');
  assert.equal(claimLive(claim(process.pid, 'not a date')), true, 'an undated claim of a running pid');
});

// Run after a gallery build: exercise the completed receipt and the real CLI refusal.
test.skipIf(!process.env.EXACT_WIDGET_GALLERY_TEST)('a snapshot driver edit makes a gallery build stale', async () => {
  const { resolveApp, readBuilds } = await import('./app.mjs');
  const app = resolveApp('caltrain'), source = resolve(import.meta.dir, '../host/apple/Sources/ExactSnapshot/SnapshotDriver.swift');
  const receipt = readBuilds(app).find(build => build.compat.inputs.platform === 'macos' && build.binary.inputs.some(input => input.path === source));
  assert.ok(receipt, 'build Caltrain with --widget-gallery first');
  const before = readFileSync(source);
  try {
    writeFileSync(source, Buffer.concat([before, Buffer.from('\n// freshness test\n')]));
    const name = receipt.binary.inputs.find(input => input.path === source).name;
    assert.ok(pendingBuildInputs(receipt).includes(name));
    const refused = spawnSync(process.execPath, [resolve(import.meta.dir, 'agent.mjs'), 'macos', 'widget', 'next-trains', 'tree'], { encoding: 'utf8' });
    assert.equal(refused.status, 3, refused.stderr);
    assert.match(refused.stderr, /SnapshotDriver.swift/);
  } finally { writeFileSync(source, before); }
});

test('game web, release and unwired gpu-dev bakes never open registry generations', () => {
  const dir=mkdtempSync(resolve('/tmp','exact-unwired-cache-'));
  try {
    // The child isolates module mocks. Only process execution and receipt
    // collection are stubbed; every selected GPU and host build must finish.
    const script=resolve(dir,'check.mjs');
    writeFileSync(script, `import {mock} from 'bun:test';
      import assert from 'node:assert/strict';
      import {readFileSync} from 'node:fs';
      let calls=0, lints=0, receipts=0;
      const builds=[];
      mock.module(${JSON.stringify(resolve(import.meta.dir,'../game/app/crates.mjs'))},()=>({keptGameCrates(){calls++;return {take(){calls++;},keep(){calls++;}};}}));
      mock.module(${JSON.stringify(resolve(import.meta.dir,'../game/app/shells.mjs'))},()=>({gameDefaults(){},prepareGame(){},lintGame(){lints++;}}));
      const appPath=${JSON.stringify(resolve(import.meta.dir,'app.mjs'))};
      globalThis.stubBuild=(command,args)=>{
        assert.equal(command,'cargo');
        const pkg=args[args.indexOf('-p')+1]; builds.push(pkg);
        return {stdout:JSON.stringify({reason:'compiler-artifact',package_id:pkg,filenames:[dir+'/'+pkg+'.so']})};
      };
      globalThis.stubReceipt=(app,platform,target,graph,messages,roots)=>{
        assert.deepEqual(roots.map(r=>r.package),[graph.surface.id,graph.root.id]);
        assert.equal(messages.length,2); receipts++;
        return {products:messages.flatMap(m=>m.filenames)};
      };
      Bun.plugin({name:'stub-bake-execution',setup(builder){
        builder.onLoad({filter:/[\\/]scripts[\\/]app\\.mjs$/},({path})=>{
          assert.equal(path,appPath);
          const source=readFileSync(path,'utf8')
            .replace(/(function buildCommand\\([^\\n]+\\{)/,'$1 return globalThis.stubBuild(...arguments);')
            .replace(/(function completeBuild\\([^\\n]+\\{)/,'$1 return globalThis.stubReceipt(...arguments);');
          return {contents:source,loader:'js'};
        });
      }});
      const {buildBake}=await import(appPath);
      const dir=${JSON.stringify(dir)};
      for(const [platform,profile,target] of [['web','web','wasm32-unknown-unknown'],['linux','release',${JSON.stringify(bakeTarget('linux'))}],['linux','gpu-dev',${JSON.stringify(bakeTarget('linux'))}]]) {
        const name='fixture-'+platform;
        const host={id:name,name,targets:[{kind:[platform==='web'?'cdylib':'bin'],name:'fixture'}]};
        const gpu={id:'fixture-gpu',name:'fixture-gpu',targets:[{kind:['cdylib'],name:'surface'}]};
        const metadata={workspace_root:dir,packages:[host,gpu],resolve:{nodes:[{id:host.id,deps:[]},{id:gpu.id,deps:[]}]}};
        const app={dir,workspace:dir,target:dir+'/target',hasGpu:true,manifest:{app:{id:'fixture'},rust:false,game:{}},crate:kind=>'fixture-'+kind,prepare:()=>metadata};
        const before=builds.length;
        const receipt=buildBake(app,platform,target,{kind:platform,profile,output:dir+'/bakes',env:{EXACT_UPDATE_TRUST:'development'}});
        assert.deepEqual(builds.slice(before),[gpu.name,host.name]);
        assert.equal(receipt.products.length,2); assert.equal(calls,0);
      }
      assert.equal(receipts,3); assert.equal(lints,2); assert.equal(calls,0);
    `);
    const result=spawnSync(process.execPath,[script],{cwd:dir,encoding:'utf8'});
    assert.equal(result.status,0,result.stderr);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

// The cache boundary needs real Cargo: a script rerun recompiles its crate,
// even when the script leaves the generated Rust file unchanged.
test.skipIf(!process.env.EXACT_RUST_BAKE_TEST)('a linked Rust bake changes its plan without compiling its archive', async () => {
  const { bakeOutside, linkedBakeDir, linkable, recordRustBake } = await import('./linked-plan.mjs');
  const root = resolve(import.meta.dir, '..'), dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-linked-rust-')));
  const write = (name, bytes) => { mkdirSync(dirname(resolve(dir, name)), {recursive:true}); writeFileSync(resolve(dir, name), bytes); };
  try {
    write('rust-toolchain.toml', readFileSync(resolve(root, 'rust-toolchain.toml')));
    write('Cargo.lock', readFileSync(resolve(root, 'Cargo.lock')));
    write('Cargo.toml', `[package]\nname="linked-rust-fixture"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[patch.crates-io]\ntaffy={path=${JSON.stringify(resolve(root, 'vendor/taffy'))}}\n[build-dependencies]\nexact-bake={path=${JSON.stringify(resolve(root, 'bake'))}}\n`);
    write('data/Cargo.toml', '');
    write('src/lib.rs', 'include!(concat!(env!("OUT_DIR"), "/entry.rs"));');
    write('app.contract', 'first');
    write('app.json', 'nextTrains');
    write('data/src/lib.rs', `pub fn next_trains() -> &'static str { "nextTrains" }`);
    write('build.rs', `#[path="data/src/lib.rs"] mod data;
    fn main() {
      if exact_bake::take_linked(&["entry.rs"]).unwrap() { return; }
      println!("cargo:rerun-if-changed=app.contract");
      println!("cargo:rerun-if-changed=app.json");
      assert_eq!(std::fs::read_to_string("app.json").unwrap(), data::next_trains(), "unknown widget refresh");
      println!("cargo:rerun-if-env-changed=FIXTURE_BINARY");
      let out=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
      std::fs::write(out.join("app.plan"), std::fs::read("app.contract").unwrap()).unwrap();
      std::fs::write(out.join("compat.json"), "{}").unwrap();
      std::fs::write(out.join("app.hbc"), []).unwrap();
      let entry=format!("pub const VALUE: &str = {:?};", std::env::var("FIXTURE_BINARY").unwrap());
      if std::fs::read(out.join("entry.rs")).ok().as_deref()!=Some(entry.as_bytes()) {
        std::fs::write(out.join("entry.rs"), entry).unwrap();
      }
    }`);
    const app = { dir, target: resolve(root, 'target'), name: 'linked-rust-fixture' };
    const pkg = { targets: [{kind:['custom-build'],src_path:resolve(dir,'build.rs')},{kind:['lib'],src_path:resolve(dir,'src/lib.rs')}] };
    assert.equal(linkable(app, pkg), true);
    const target = spawnSync('rustc',['-vV'],{encoding:'utf8'}).stdout.match(/^host: (.*)$/m)[1];
    const env = {...process.env, EXACT_BAKE_OUTPUT:resolve(dir,'bakes'), FIXTURE_BINARY:'one'};
    env.EXACT_BAKED = linkedBakeDir(env,'macos',target);
    const cargo = (archive = false) => {
      const result=spawnSync('cargo',[...(archive ? ['rustc','--crate-type','staticlib'] : ['build']),'--offline','--manifest-path',resolve(dir,'Cargo.toml'),'--target-dir',app.target,'--message-format=json','-vv'],{cwd:dir,env,encoding:'utf8',maxBuffer:64*1024*1024});
      assert.equal(result.status,0,result.stderr);
      return {...result.stdout.split('\n').filter(line=>line.startsWith('{')).map(line=>JSON.parse(line)).find(row=>row.reason==='compiler-artifact'&&row.target.name==='linked_rust_fixture'), diagnostic: result.stderr};
    };
    const bake = () => bakeOutside(app,'macos',target,env,() => cargo());
    cargo(); // Compile and run the baker through the rlib first.
    const first=cargo(true), library=first.filenames.find(name=>name.endsWith('.a'));
    recordRustBake(env);
    const watches=JSON.parse(readFileSync(resolve(env.EXACT_BAKED,'inputs.json'),'utf8'));
    assert.ok(watches.paths.includes(resolve(dir,'app.contract')),'the first build is ready for the driver freshness check');
    assert.ok(Number.isFinite(watches.at));
    const initial=statSync(library,{bigint:true}).mtimeNs;
    write('app.contract','second');
    bake();
    assert.equal(readFileSync(resolve(env.EXACT_BAKED,'app.plan'),'utf8'),'second');
    const unchanged=cargo(true);
    assert.equal(unchanged.fresh,true,unchanged.diagnostic);
    assert.equal(statSync(library,{bigint:true}).mtimeNs,initial);
    env.FIXTURE_BINARY='two';
    bake();
    assert.equal(cargo(true).fresh,false,'a generated Rust change must still compile');
    assert.match(readFileSync(resolve(env.EXACT_BAKED,'entry.rs'),'utf8'),/two/);
    write('app.json','upcomingTrains');
    write('data/src/lib.rs', `pub fn next_trains() -> &'static str { "upcomingTrains" }`);
    bake();
    assert.equal(cargo(true).fresh,false,'the coordinated Rust edit compiles');
    recordRustBake(env);
    write('app.json','unknownRefresh');
    assert.throws(bake,/unknown widget refresh/,'a refresh absent from the current Rust implementation is rejected');
  } finally { rmSync(dir,{recursive:true,force:true}); }
},300000);
