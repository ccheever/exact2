import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, mkdirSync, unlinkSync, mkdtempSync, openSync, ftruncateSync, closeSync, rmSync, existsSync, readdirSync, watch, renameSync, watchFile, unwatchFile, statSync, utimesSync, symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {open} from '../../../scripts/agent.mjs';
import {compilerPaths, pendingBuildInputs} from '../../../scripts/app.mjs';
import {createHash} from 'node:crypto';
import {runInNewContext} from 'node:vm';
import {watchStaticTrees} from '../serve.mjs';
const source = readFileSync(process.env.E2B_DEV_SOURCE || new URL('../dev.mjs', import.meta.url), 'utf8');
test('static watcher follows immediate creation, in-place edits, and directory replacement', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-static-edits-'));
  const assets=join(dir,'assets'),outside=join(dir,'outside'),changes=[];
  const watcher=watchStaticTrees(dir,[[assets,'assets']],change=>changes.push(change));
  const until=async predicate=>{
    for(let i=0;i<100;i++){if(predicate())return;await new Promise(r=>setTimeout(r,20));}
    assert.ok(predicate(),`static edit was not observed: ${predicate}`);
  };
  const tree=()=>changes.some(c=>c.tree&&c.root===assets&&c.name==='assets');
  const leaf=name=>changes.some(c=>!c.tree&&c.relative===name&&c.name===`assets/${name}`);
  try {
    mkdirSync(join(assets,'nested'),{recursive:true});
    const file=join(assets,'nested/live.txt');writeFileSync(file,'first');
    await until(tree);changes.length=0;
    const before=statSync(file);
    writeFileSync(file,'other');utimesSync(file,before.atime,before.mtime);
    await until(()=>leaf('nested/live.txt'));changes.length=0;
    writeFileSync(join(dir,'replacement'),'third');renameSync(join(dir,'replacement'),file);
    await until(tree);changes.length=0;
    mkdirSync(join(assets,'new'));writeFileSync(join(assets,'new/leaf.txt'),'new');
    await until(tree);changes.length=0;
    writeFileSync(join(assets,'new/leaf.txt'),'changed');
    await until(()=>leaf('new/leaf.txt'));changes.length=0;
    mkdirSync(outside);writeFileSync(join(outside,'secret.txt'),'outside');
    symlinkSync(outside,join(assets,'link'));await until(tree);changes.length=0;
    writeFileSync(join(outside,'secret.txt'),'changed outside');
    await new Promise(r=>setTimeout(r,250));
    assert.deepEqual(changes,[],'no watcher traverses the linked directory');
    rmSync(assets,{recursive:true});await until(tree);changes.length=0;
    mkdirSync(assets);writeFileSync(join(assets,'again.txt'),'again');
    await until(tree);changes.length=0;
    writeFileSync(join(assets,'again.txt'),'changed again');
    await until(()=>leaf('again.txt'));
    watcher.close();changes.length=0;
    writeFileSync(join(assets,'again.txt'),'after');
    await new Promise(r=>setTimeout(r,250));assert.deepEqual(changes,[]);
  } finally {watcher.close();rmSync(dir,{recursive:true,force:true});}
});
test('dev edits clear only their own errors while unrelated failures stay visible', async () => {
  let stream, overlay, envelope, plan, seq=0, swapErrors=[];
  const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
  const context={TextEncoder,TextDecoder,URL,URLSearchParams,AbortController,setTimeout,clearTimeout,performance,
    console:{error(){}},location:{origin:'http://localhost',href:'http://localhost/',search:''},
    EventSource:class {constructor(){stream=this;}},
    document:{body:{appendChild(el){overlay=el;return el;}},createElement(){return {remove(){overlay=null;}};}},
    navigator:{sendBeacon(){}},requestAnimationFrame:callback=>callback(),
    fetch:async url=>new Response(url.endsWith('app.plan')?plan:JSON.stringify(envelope)),
    exact:{ready:Promise.resolve(),compat:{inputs:{app:'fixture'}},reloadGeneration:async()=>true,
      gpu:{swap:async()=>({ms:0,errors:swapErrors})}},
  };
  runInNewContext(readFileSync(new URL('../dev.js',import.meta.url),'utf8'),context);
  const send=message=>stream.onmessage({data:JSON.stringify(message)});
  const settled=async()=>{for(let i=0;i<10;i++)await new Promise(setImmediate);};
  const text=()=>overlay?.textContent??'';
  const edited=async()=>{
    plan=Uint8Array.of(++seq);
    const card={bytes:plan.length,sha256:hash(plan)};
    const dev={epoch:'a'.repeat(32),seq,generation:hash(JSON.stringify({assets:[],plan:card}))};
    envelope={exact:1,app:{id:'fixture'},dev,assets:[],plan:{...card,url:'/app.plan'}};
    send({...dev,envelope:'/exact.json'});await settled();
  };
  send({error:'Rust: unknown speed',source:'gpu'});
  await edited();
  assert.match(text(),/Rust: unknown speed/,'a valid plan does not fix Rust');
  send({error:'Contract: unknown argument'});
  assert.match(text(),/Rust: unknown speed/);
  assert.match(text(),/Contract: unknown argument/);
  send({gpu:1});await settled();
  assert.match(text(),/Contract: unknown argument/,'a valid GPU module does not fix Contract');
  assert.doesNotMatch(text(),/Rust: unknown speed/);
  await edited();assert.equal(text(),'');
  send({error:'first error',source:'gpu'});send({error:'corrected error',source:'gpu'});
  assert.doesNotMatch(text(),/first error/);assert.match(text(),/corrected error/);
  overlay.onclick();assert.equal(text(),'');
  context.exact.devError('runtime refusal');
  swapErrors=['carry refusal'];send({gpu:2});await settled();
  assert.match(text(),/runtime refusal/);assert.match(text(),/carry refusal/);
  swapErrors=[];send({gpu:3});await settled();
  assert.match(text(),/runtime refusal/);assert.doesNotMatch(text(),/carry refusal/);
  await edited();assert.equal(text(),'');
});
test('dev startup rebuilds changed Rust even when the app identity still matches', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-dev-startup-'));
  try {
    const input=join(dir,'lib.rs'), graphPath=join(dir,'bake.json');
    const original='fn speed() -> u32 { 4 }';
    writeFileSync(input,original);
    const build={version:1,trust:'development',binary:{
      inputs:[{name:'logic/lib.rs',path:input,sha256:createHash('sha256').update(original).digest('hex')}],
      missing:[],directories:[],
    }};
    writeFileSync(graphPath,JSON.stringify(build));
    let identity=true;
    const start=source.indexOf('async function currentWebBuild()');
    const end=source.indexOf('\nif (!await currentWebBuild())',start);
    const current=new Function('builtAppMatches','dist','app','graphPath','readFileSync','pendingBuildInputs',
      source.slice(start,end)+';return currentWebBuild;')
      (async()=>identity,dir,{},graphPath,readFileSync,pendingBuildInputs);
    assert.equal(await current(),true);
    writeFileSync(join(dir,'notes.txt'),'unrelated edit');
    assert.equal(await current(),true);
    writeFileSync(input,'fn speed() -> u32 { 8 }');
    assert.equal(await current(),false);
    writeFileSync(input,original);
    assert.equal(await current(),true);
    identity=false;
    assert.equal(await current(),false);
    identity=true;
    writeFileSync(graphPath,JSON.stringify({...build,trust:'production'}));
    assert.equal(await current(),false);
    writeFileSync(graphPath,'{');
    assert.equal(await current(),false);
    unlinkSync(graphPath);
    assert.equal(await current(),false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('generated game arguments use the resident plan compiler without a host rebuild', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-dev-declaration-'));
  try {
    const shell=join(dir,'.shells'), logic=join(dir,'logic');
    mkdirSync(shell); mkdirSync(logic);
    const declaration=join(shell,'surfaces.json'), rust=join(logic,'lib.rs'), manifest=join(shell,'Cargo.toml');
    const swift=join(logic,'main.swift'),contract=join(dir,'app.contract'),missing=join(dir,'missing/config.json');
    const app={dir,manifest:{game:{}}}, callbacks=new Map();
    const start=source.indexOf('const watched=new Map();'), end=source.indexOf('\nwatchCompilerInputs();',start);
    const changed=new Function('app','paths','watch','resolve','existsSync','watchFile','unwatchFile',`
      const source=resolve(app.dir,'app.contract'), skipped=/(^|\\/)(target|dist)(\\/|$)/;
      const builtReceipts=[{binary:{inputs:paths.map(path=>({path})),directories:[{path:resolve(app.dir,'.shells')}],missing:[resolve(app.dir,'missing/config.json')]}}];
      const rustInputFiles=new Set(),gpuInputs=new Set(),appInputs=new Set(),assetTrees=[];
      const typescript=false,portableRust=false,rebuildOn={rust:'save'},changed=new Set();
      const console={log(){},error(error){throw new Error(error);}},gpuOnly=()=>false;
      const clearTimeout=()=>{},setTimeout=()=>0,rebuild=()=>{};let timer;
      ${source.slice(start,end)}
      watchCompilerInputs();return changed;
    `)(app,[declaration,rust,manifest,swift,contract],(dir,callback)=>{callbacks.set(dir,callback);return {};},resolve,existsSync,
      (path,options,listener)=>{callbacks.set(path,()=>listener({mtimeNs:1n},{mtimeNs:0n}));},()=>{});
    callbacks.get(declaration)();
    callbacks.get(contract)();
    assert.deepEqual([...changed],[]);
    callbacks.get(rust)();
    callbacks.get(manifest)();
    assert.deepEqual([...changed],[rust,manifest]);
    changed.clear();delete app.manifest.game;
    callbacks.get(declaration)();
    assert.deepEqual([...changed],[declaration],'ordinary hosts retain declared compiler inputs');
    changed.clear();callbacks.get(shell)('change','Cargo.toml');callbacks.get(logic)('change','main.swift');
    assert.deepEqual([...changed],[],'known files have one notification owner');
    callbacks.get(shell)('rename','new-input.json');callbacks.get(logic)('rename','extra.swift');callbacks.get(missing)();
    assert.deepEqual([...changed],[join(shell,'new-input.json'),join(logic,'extra.swift'),missing],'declared trees, Swift discovery, and missing inputs remain visible');
  } finally {rmSync(dir,{recursive:true,force:true});}
});
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
test('GPU reloads refresh their includes while retaining the current app input graph', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-dev-input-owner-'));
  try {
    const target=join(dir,'target'), workspace=join(dir,'.shells');
    const app={target,workspace,crate:kind=>'fixture-'+kind}, reads=[];
    const input=(kind,profile,paths)=>{
      const file=join(target,'wasm32-unknown-unknown',profile,'fixture_'+kind+'.d');
      mkdirSync(resolve(file,'..'),{recursive:true});writeFileSync(file,`unit: ${paths.map(p=>join(dir,p)).join(' ')}\n`);
    };
    input('web','web',['host.rs','shared.rs']);input('gpu','web',['game.rs','shared.rs']);
    input('gpu','gpu-dev',['game.rs','shared.rs','new-include.rs']);
    let classifications=0;
    const start=source.indexOf('function readGpuInputs('), end=source.indexOf('\nfunction gameRuntimeInputs(',start);
    const api=new Function('app','resolve','existsSync','compilerPaths','readFileSync','gameRuntimeInputs',`
      let appInputs=new Set(),gpuInputs=new Set();
      ${source.slice(start,end)}
      const gpuOnly=files=>files.length>0&&appInputs.size>0&&files.every(p=>p.endsWith('.rs')&&gpuInputs.has(p)&&!appInputs.has(p));
      return {refresh:readGpuInputs,gpuOnly,app:()=>[...appInputs]};
    `)(app,resolve,existsSync,compilerPaths,(path,...args)=>{reads.push(path);return readFileSync(path,...args);},inputs=>{classifications++;return inputs;});
    api.refresh();const hostInputs=api.app();reads.length=0;
    api.refresh('gpu-dev');
    assert.deepEqual(api.app(),hostInputs);
    assert.equal(api.gpuOnly([join(dir,'new-include.rs')]),true);
    for(const name of ['host.rs','shared.rs','unknown.rs','Cargo.toml'])assert.equal(api.gpuOnly([join(dir,name)]),false,name);
    assert.equal(classifications,1,'unchanged host inputs need no new metadata classification');
    assert.equal(reads.length,1,'GPU-only reload reads only its completed compiler input file');
    input('web','web',['new-host.rs','new-include.rs']);
    api.refresh();
    assert.deepEqual(api.app(),[join(dir,'new-host.rs'),join(dir,'new-include.rs')]);
    assert.equal(classifications,2,'a completed full build refreshes the host inputs');
    api.refresh('gpu-dev');assert.equal(api.gpuOnly([join(dir,'new-include.rs')]),false);
    api.refresh('web');assert.equal(classifications,3,'the web-profile fallback remains conservative');
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('declared source files report in-place edits after inclusion and replacement', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-source-watch-'));
  let api;
  try {
    const src=join(dir,'logic'), main=join(src,'lib.rs'), extra=join(src,'extra.rs');
    mkdirSync(src);mkdirSync(join(dir,'target'));writeFileSync(join(dir,'app.json'),'{}');
    writeFileSync(main,'one');writeFileSync(extra,'unused');
    const start=source.indexOf('const watched=new Map();'),end=source.indexOf('\nwatchCompilerInputs();',start);
    api=new Function('app','main','watch','resolve','existsSync','watchFile','unwatchFile',`
      const source=resolve(app.dir,'app.contract'),skipped=/(^|\\/)(target|dist)(\\/|$)/;
      const builtReceipts=[{binary:{inputs:[{path:main}],directories:[],missing:[]}}];
      const rustInputFiles=new Set(),gpuInputs=new Set(),appInputs=new Set(),assetTrees=[];
      const typescript=false,portableRust=false,rebuildOn={rust:'save'},changed=new Set();
      const console={log(){},error(message){throw Error(message);}},gpuOnly=()=>true;
      const builds=[],rebuild=()=>{builds.push([...changed]);changed.clear();};let timer;
      ${source.slice(start,end)}
      watchCompilerInputs();
      return {changed,builds,add(path){gpuInputs.add(path);watchCompilerInputs();},remove(path){gpuInputs.delete(path);watchCompilerInputs();},
        has:path=>watched.has(path),close(){clearTimeout(timer);for(const record of watched.values())record.close();}};
    `)({dir,manifest:{game:{}}},main,watch,resolve,existsSync,watchFile,unwatchFile);
    const edit=async(path,text,atomic=false)=>{
      const count=api.builds.length;
      if(text===null)unlinkSync(path);
      else if(atomic){const temp=path+'.new';writeFileSync(temp,text);renameSync(temp,path);}
      else writeFileSync(path,text);
      const deadline=Date.now()+1500;
      while(api.builds.length===count&&Date.now()<deadline)await Bun.sleep(10);
      assert.deepEqual(api.builds[count],[path],`save was not observed: ${path}`);
      await Bun.sleep(1000);
      assert.equal(api.builds.length,count+1,'one save requests one build');
    };
    api.add(extra);
    await edit(extra,'included');
    await edit(extra,'replaced',true);
    await edit(extra,'edited again');
    await edit(extra,null);
    await edit(extra,'recreated',true);
    await edit(extra,'edited after recreation');
    api.remove(extra);assert.equal(api.has(extra),false,'obsolete file watches close');
    const count=api.builds.length;writeFileSync(extra,'no longer included');
    writeFileSync(join(dir,'target/output.rs'),'output');
    writeFileSync(join(dir,'notes.txt'),'unrelated');await Bun.sleep(1000);
    assert.deepEqual([...api.changed],[],'output and unrelated files do not rebuild');
    assert.equal(api.builds.length,count,'ignored edits do not build');
  } finally {api?.close();rmSync(dir,{recursive:true,force:true});}
},15_000);
