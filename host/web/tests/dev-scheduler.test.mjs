import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, mkdirSync, unlinkSync, mkdtempSync, openSync, ftruncateSync, closeSync, rmSync, existsSync, readdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {open} from '../../../scripts/agent.mjs';
import {compilerPaths} from '../../../scripts/app.mjs';
const source = readFileSync(process.env.E2B_DEV_SOURCE || new URL('../dev.mjs', import.meta.url), 'utf8');
test('GPU edit profile recognizes both authored tables and generated inline TOML', () => {
  const body=source.slice(source.indexOf('async function produceGpu(files)'));
  const line=body.split('\n').find(line=>line.trimStart().startsWith('const profile ='));
  const profile=text=>new Function('readFileSync','resolve','app','Bun',`${line};return profile;`)
    (()=>text,resolve,{workspace:'.'},Bun);
  assert.equal(profile('[profile.gpu-dev]\nopt-level=1\n'),'gpu-dev');
  assert.equal(profile('[profile]\n"gpu-dev" = { inherits="dev", opt-level=1 }\n'),'gpu-dev');
  assert.equal(profile('[profile.web]\nopt-level="z"\n'),'web');
});
test('game edits omit bake-only sources but retain runtime, code generation and cross-package includes', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-game-dev-'));
  try {
    const path=(pkg,file='lib.rs')=>join(dir,pkg,file), target=join(dir,'target');
    const pkg=(id)=>({id,name:id,manifest_path:path(id,'Cargo.toml'),targets:[{kind:['lib'],src_path:path(id)}]});
    const dep=(pkg,kind=null)=>({pkg,dep_kinds:[{kind}]});
    const metadata={packages:['app-web','core','bake','game','codegen'].map(pkg),resolve:{nodes:[
      {id:'app-web',deps:[dep('core'),dep('bake','build')]},
      {id:'core',deps:[dep('codegen','build')]},{id:'bake',deps:[dep('game')]},
      {id:'game',deps:[]},{id:'codegen',deps:[]},
    ]}};
    const app={manifest:{game:{}},target,workspace:dir,crate:()=> 'app-web'};
    const deps=join(target,'wasm32-unknown-unknown/web/deps');mkdirSync(deps,{recursive:true});
    for (const id of ['app-web','core','codegen']) writeFileSync(join(deps,id+'.d'),
      `unit: ${path(id)}${id==='app-web' ? ' '+path('game','included.rs') : ''}\n`);
    const build=join(target,'web/build/core-123abc');mkdirSync(build,{recursive:true});
    writeFileSync(join(build,'output'),`cargo:rerun-if-changed=../game/schema.rs\n`);
    const start=source.indexOf('function gameRuntimeInputs('), end=source.indexOf('\nreadGpuInputs();',start);
    const select=new Function('app','buildEnv','spawnSync','cargoReproducibilityFlags','compilerPaths','resolve','existsSync','readdirSync','readFileSync',
      source.slice(start,end)+';return gameRuntimeInputs;');
    const run=spawn=>select(app,{},spawn,()=>[],compilerPaths,resolve,existsSync,readdirSync,readFileSync);
    const inputs=new Set([path('app-web'),path('core'),path('game'),path('game','included.rs'),path('game','schema.rs'),path('codegen'),path('unknown')]);
    const selectInputs=run(()=>({status:0,stdout:JSON.stringify(metadata)}));
    assert.deepEqual(selectInputs(inputs),new Set([...inputs].filter(p=>p!==path('game'))));
    unlinkSync(join(deps,'core.d'));
    assert.equal(selectInputs(inputs),inputs,'missing compiler evidence retains the full rebuild');
    assert.throws(()=>run(()=>({status:1,stderr:'metadata unavailable'}))(inputs),/metadata unavailable/);
    app.manifest={};
    assert.equal(run(()=>{throw new Error('non-game metadata must not be queried');})(inputs),inputs);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
function scheduler() {
  const legacy = !source.includes('function drainBuilds()');
  const functions = legacy ? source.slice(source.indexOf('function produceRust()'), source.indexOf('  if (!rustChild)')) + 'calls.push(["rust"]);rustActive=true;}'
    : source.slice(source.indexOf('function produceRust()'), source.indexOf('function produceRustNow()'));
  return new Function(`
    let building=false, rustActive=false, buildPending=false, rustPending=false, rustDirty=false, again=false, changed=new Set(), calls=[];
    const gpuOnly = files => files.length && files.every(f=>f==='gpu.rs');
    const produceGpu = files => { building=true; calls.push(['gpu',files]); };
    const rebuildNow = files => { building=true; calls.push(['full',files]); };
    const produceRustNow = () => { rustActive=true; calls.push(['rust']); };
    ${legacy ? 'function rebuild(){if(building){again=true;return;}const files=[...changed];changed.clear();if(gpuOnly(files))produceGpu(files);else rebuildNow(files);}function drainBuilds(){rebuild();}' : ''}
    ${functions}
    return {calls, rust:produceRust, change(file){changed.add(file);buildPending=true;drainBuilds();},
      finish(){building=false;rustActive=false;${legacy ? 'if(again||changed.size){again=false;rebuild();}' : 'drainBuilds();'}}};
  `)();
}
test('a Rust request during GPU build stays Rust, with no empty full rebuild', () => {
  const s=scheduler(); s.change('gpu.rs'); s.rust();
  assert.equal(s.calls.length,1); s.finish();
  assert.deepEqual(s.calls,[['gpu',['gpu.rs']],['rust']]); s.finish(); assert.equal(s.calls.length,2);
});
test('GPU and full builds wait for the active resident Rust producer', () => {
  const s=scheduler(); s.rust(); s.change('gpu.rs'); assert.equal(s.calls.length,1);
  s.finish(); assert.equal(s.calls[1][0],'gpu'); s.change('app.rs'); s.finish();
  assert.deepEqual(s.calls[2],['full',['app.rs']]);
});
test('compiler cleanup kills the recorded GPU cargo group too', () => {
  const cleanup=source.slice(source.indexOf('const killCompiler = () => {'),source.indexOf('\nstartCompiler();'));
  const killed=[];
  new Function('process', `let gpuBuildChild={pid:123},manualTypescript,rustSourceWatch,rustOutputWatch,rustHeartbeat,rustActive,rustChild,moduleWatch,moduleTimer,moduleRun=0,dev,moduleStage; ${cleanup}; killCompiler();`)
    ({kill:(...args)=>killed.push(args)});
  assert.deepEqual(killed,[[-123,'SIGKILL']]);
});
test('failed Cargo metadata refuses startup before choosing a dev binary', () => {
  const body=source.slice(source.indexOf('  const metadata = spawnSync(\'cargo\''),source.indexOf('  const me = dev;'));
  let spawned=false;
  assert.throws(()=>new Function('spawnSync','spawn','cargoReproducibilityFlags',`const app={workspace:'.',crate:()=> 'fixture-web'},buildEnv={},root='.',source='',plan='';let dev;${body}`)
    (()=>({status:1,stderr:'metadata fixture failure'}),()=>{spawned=true;},()=>[]),/cargo metadata failed.*metadata fixture failure/);
  assert.equal(spawned,false);
});
test('world file size is refused before reading or launching either host', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-world-cap-')), path=join(dir,'oversized.world');
  const fd=openSync(path,'w'); ftruncateSync(fd,256*1024*1024+1); closeSync(fd);
  try { for (const host of ['web','macos']) await assert.rejects(open({host,world:path}), /world carrier exceeds 256 MiB limit/); }
  finally { rmSync(dir,{recursive:true,force:true}); }
});
