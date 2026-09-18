import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';
import {reloadInputs, verifyReloadInputs, gpuArtifact, sha256} from '../reload-build.mjs';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync, existsSync, renameSync, readSync, writeSync} from 'node:fs';
import {retainDevGeneration} from '../serve.mjs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';

for (const [kind, nextGpu] of [
  ['restore', {gpu_restore:()=>false}],
  ['render', {gpu_render:()=>2}],
  ['publication', {gpu_published:()=>'{"broken"'}],
  ['handoff', {gpu_agent:()=>'{"world":{}}'}],
  ['device', {gpu_load:async()=>{throw new Error('device unavailable');}}],
]) test(`${kind} failure retains all old canvases and reports refusal`, async () => {
  const f=await fixture({nextGpu}); const a=f.create(1), b=f.create(2,'other');
  const old=[a.canvas,b.canvas];
  await assert.rejects(f.exact.gpu.swap(1), new RegExp(kind==='handoff'?'unsupported':kind==='publication'?'JSON':kind));
  assert.deepEqual([a.canvas,b.canvas],old);
  assert.ok(!f.order.includes('old destroy') && !f.order.includes('old unload'));
  assert.equal(f.order.filter(v=>v==='next unload').length,1);
  const state=f.exact.gpu.diagnostics();
  assert.equal(state.phase,'failed'); assert.equal(state.loaded.version,0); assert.equal(state.requested.version,1);
  assert.equal(state.restoreOutcome.retained,true);
});

test('failure on the second canvas disposes every staged resource without partial cutover', async()=> {
  let renders=0;
  const f=await fixture({nextGpu:{gpu_render:()=>++renders===2?2:0}}); f.create(1); f.create(2,'second');
  await assert.rejects(f.exact.gpu.swap(1),/second: render/);
  assert.equal(f.order.filter(v=>v==='next destroy').length,2);
  assert.ok(!f.order.includes('replace'));
});

test('compile failure preserves usable world, then recovery commits with explicit timing',async()=>{
  const f=await fixture(); f.create(1);
  f.exact.gpu.requested({revision:1}); f.exact.gpu.buildFailed('compile failed: speed is not defined');
  assert.equal(f.exact.gpu.agent(1,{op:'state'}).world.tick,0);
  assert.equal(f.exact.gpu.diagnostics().loaded.version,0);
  await f.exact.gpu.swap(2,{artifact:{version:2,code:'verified'},timing:{buildMs:12000}});
  const state=f.exact.gpu.decorate({op:'state'},{}).reload;
  assert.equal(state.loaded.code,'verified'); assert.equal(state.lastSuccessfulSwap.timing.buildMs,12000);
  assert.equal(state.lastSuccessfulSwap.timing.firstUsableFrameMs,null,'fixture has not presented a frame');
  assert.equal(state.native.liveGameReplacement,false);
});

test('overlapping asynchronous candidates exclude obsolete build even after it loaded',async()=>{
  let release, started;
  const loading=new Promise(r=>started=r), blocked=new Promise(r=>release=r);
  const f=await fixture({candidate:async(version,module)=>{if(version===1){started();await blocked;}return module;}}); f.create(1);
  const first=f.exact.gpu.swap(1); await loading;
  const second=f.exact.gpu.swap(2); release();
  assert.equal((await first).stale,true); await second;
  assert.equal(f.exact.gpu.version,2); assert.equal(f.order.filter(v=>v==='old unload').length,1);
  assert.equal(f.exact.gpu.diagnostics().stale,1);
});

test('an edit invalidates an already-loading candidate before a later build completes',async()=>{
  let release, started; const loading=new Promise(r=>started=r), blocked=new Promise(r=>release=r);
  const f=await fixture({candidate:async(_,m)=>{started();await blocked;return m;}}); f.create(1);
  const pending=f.exact.gpu.swap(1); await loading; f.exact.gpu.requested({revision:2}); release();
  assert.equal((await pending).stale,true); assert.equal(f.exact.gpu.version,0);
  assert.equal(f.exact.gpu.diagnostics().phase,'building');
});

test('Continue restores, Restart uses authored bindings, Restore requires the complete selected set',async()=>{
  const carries=[],bindings=[];
  const f=await fixture({nextGpu:{gpu_restore:(id,b)=>{carries.push([...b]);return true;},gpu_bind_at:(id,v)=>{bindings.push(JSON.parse(v));return true;}}});
  f.create(1); f.create(2,'second');
  await f.exact.gpu.swap(1); assert.deepEqual(carries,[[1],[1]]);
  carries.length=0;
  await f.exact.gpu.swap(2,{intent:'restart',values:new Map([[1,[42,'scene-new']]])});
  assert.equal(carries.length,0); assert.ok(bindings.some(v=>v[0]===42));
  await assert.rejects(f.exact.gpu.swap(3,{intent:'restore',checkpoints:new Map([[1,new Uint8Array([9])]])}),/checkpoint missing/);
  assert.equal(f.exact.gpu.version,2);
  await f.exact.gpu.swap(4,{intent:'restore',checkpoints:new Map([[1,new Uint8Array([8])],[2,new Uint8Array([7])]])});
  assert.deepEqual(carries.slice(-2),[[8],[7]]);
});

test('slow loading captures at cutover, rebases without build-time ticks, suppresses candidate messages/audio',async()=>{
  let clock=0, release, started, audio=false; const events=[];
  const waiting=new Promise(r=>started=r), blocked=new Promise(r=>release=r);
  const f=await fixture({now:()=>clock, candidate:async(_,m)=>{started();await blocked;return m;},nextGpu:{
    gpu_seekable:on=>{audio=!on;events.push(['seekable',on]);},
    gpu_render:()=>{assert.equal(audio,false,'candidate audio enabled');events.push(['render',clock]);return 0;},
    gpu_messages:()=> '["durable-write"]',
    gpu_agent:(id,text)=>{const q=JSON.parse(text);if(q.reload){events.push(['rebase',q.now]);return '{"reload":{}}';}return '{"world":{"tick":120}}';},
  }}); f.create(1); f.exact.message=()=>{throw new Error('candidate message escaped');};
  // A real module drains gpu_messages; emulate its consumptive boundary.
  let message=true; f.nextGpu.gpu_messages=()=>message?(message=false,'["durable-write"]'):undefined;
  f.gpu.gpu_carry=()=>{events.push(['capture',clock]);return new Uint8Array([1]);};
  const swap=f.exact.gpu.swap(1); await waiting; clock=12000; release(); await swap;
  assert.deepEqual(events.filter(v=>['capture','rebase','render'].includes(v[0])),[['capture',12000],['rebase',12000],['render',12000],['rebase',12000]]);
});

test('repeated swaps disconnect observers and dispose previous modules once per successful commit',async()=>{
  const f=await fixture({input:true}); const host=f.create(1);
  for(let version=1;version<=5;version++) await f.exact.gpu.swap(version);
  assert.equal(f.order.filter(v=>v==='old unload').length,1);
  assert.equal(f.order.filter(v=>v==='next unload').length,4);
  assert.equal(f.order.filter(v=>v==='replace').length,5);
  const before=f.events.length;
  host.listeners.keydown({target:host,code:'KeyW',timeStamp:123,preventDefault(){}});
  assert.equal(f.events.length,before+1,'post-swap input delivered exactly once');
});

test('Contract candidate restore failure and duplicate identities leave the current world intact',async()=>{
  const f=await fixture(); const host=f.create(1);
  f.gpu.gpu_restore=()=>false;
  assert.throws(()=>f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'world',values:[]}]}),/restore refused/);
  assert.equal(host.canvas.isConnected,true);
  assert.throws(()=>f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'world',values:[]},{op:'surface',id:3,name:'world',values:[]}]}),/duplicate/);
  assert.throws(()=>f.exact.gpu.stagePlan({ops:[{op:'store'}]}),/irreversible/);
});

test('Contract staging defers publication and accepts the complete new canvas roster',async()=>{
  const f=await fixture();f.create(1); const before=f.records.length;
  const stage=f.exact.gpu.stagePlan({ops:[{op:'surface',id:3,name:'world',values:[]}]});
  assert.equal(f.records.length,before);
  f.exact.gpu.reset(true);f.create(3);f.exact.gpu.finishRestart();stage.commit();
  assert.ok(f.exact.gpu.agent(3,{op:'state'}));assert.equal(f.exact.gpu.agent(1,{op:'state'}),null);
  assert.equal(f.exact.gpu.diagnostics().lastSuccessfulSwap.intent,'continue');
});

test('immutable build receipt detects asset edits and newly discovered dependencies',()=>{
  const dir=mkdtempSync(join(tmpdir(),'exact-reload-receipt-'));
  try {
    for(const [name,body] of [['code.rs','one'],['gpu.js','loader'],['gpu_bg.wasm','wasm']]) writeFileSync(join(dir,name),body);
    const captured=reloadInputs([join(dir,'code.rs')],[dir]);
    verifyReloadInputs(captured,reloadInputs([join(dir,'code.rs')],[dir]));
    const artifact=gpuArtifact(dir,1,captured,{buildMs:10});
    assert.equal(artifact.files['gpu_bg.wasm'].sha256,sha256('wasm'));
    writeFileSync(join(dir,'texture.bin'),'changed during cargo');
    assert.throws(()=>verifyReloadInputs(captured,reloadInputs([join(dir,'code.rs')],[dir])),/texture.bin/);
    writeFileSync(join(dir,'code.rs'),'two');
    assert.throws(()=>verifyReloadInputs(captured,reloadInputs([join(dir,'code.rs')],[dir])),/code.rs/);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

for(const mismatch of ['receipt','bytes']) test(`browser refuses ${mismatch} mismatch before executing candidate loader`,async()=>{
  const exact={}, location={href:'http://fixture/',origin:'http://fixture'};
  const source=readFileSync(new URL('../dev.js',import.meta.url),'utf8');
  const js=new TextEncoder().encode('throw new Error("UNVERIFIED CODE EXECUTED")'), wasm=new Uint8Array([1]);
  const receipt={version:1,format:'no-modules',files:{'gpu.js':{bytes:js.length,sha256:sha256(js)},'gpu_bg.wasm':{bytes:1,sha256:sha256(wasm)}}};
  exact.gpuArtifacts=new Map([[1,structuredClone(receipt)]]);
  if(mismatch==='receipt')receipt.version=2;
  const global={exact,crypto:globalThis.crypto};
  const fetch=async url=>new Response(String(url).includes('gpu-artifact')?JSON.stringify(receipt):String(url).includes('gpu_bg')?wasm:mismatch==='bytes'?new Uint8Array(js.length):js);
  new Function('globalThis','EventSource','location','fetch',source)(global,undefined,location,fetch);
  await assert.rejects(global.exactDevProtocol.loadGpuModule(1),/receipt (differs|mismatch)/);
});

test('verified initial wasm-bindgen ES wrapper uses disposable scope, including async and default exports',async()=>{
  const exact={}, location={href:'http://fixture/',origin:'http://fixture'};
  const source=readFileSync(new URL('../dev.js',import.meta.url),'utf8');
  const js=new TextEncoder().encode('let bytes; export async function gpu_load() { return bytes; } async function __wbg_init(opts) { bytes = opts.module_or_path.length; } export default __wbg_init;');
  const wasm=new Uint8Array([1,2,3]);
  const receipt={version:0,format:'module',files:{'gpu.js':{bytes:js.length,sha256:sha256(js)},'gpu_bg.wasm':{bytes:3,sha256:sha256(wasm)}}};
  const global={exact,crypto:globalThis.crypto};
  const fetch=async url=>new Response(String(url).includes('gpu-artifact')?JSON.stringify(receipt):String(url).includes('gpu_bg')?wasm:js);
  new Function('globalThis','EventSource','location','fetch',source)(global,undefined,location,fetch);
  const a=await global.exactDevProtocol.loadGpuModule(0),b=await global.exactDevProtocol.loadGpuModule(0);
  assert.equal(await a.gpu_load(),3); assert.equal(await b.gpu_load(),3); assert.notEqual(a,b);
  assert.deepEqual(exact.gpuArtifacts.get(0),receipt);
});

test('retained setup and released input stay effective until a new live value; Restart uses requested scene',async()=>{
  const bindings=[];
  const f=await fixture({nextGpu:{gpu_agent:(id,json)=>JSON.parse(json).reload?'{"reload":{"values":["old-scene",0,7],"setupIndices":[0]}}':'{"world":{"tick":42}}',gpu_bind_at:(id,values)=>{bindings.push(JSON.parse(values));return true;}}});
  f.create(1);
  await f.exact.gpu.swap(1,{values:new Map([[1,['new-scene',1,7]]])});
  f.exact.gpu.surface(1,'world',['new-scene',1,7]);
  assert.deepEqual(bindings.at(-1),['old-scene',0,7]);
  f.exact.gpu.surface(1,'world',['new-scene',2,8]);
  assert.deepEqual(bindings.at(-1),['old-scene',2,8]);
  await f.exact.gpu.swap(2,{intent:'restart'});
  assert.deepEqual(bindings.at(-1),['new-scene',2,8]);
});

for (const intent of ['continue','restore','contract']) test(`a deliberate setup change after ${intent} uses all requested construction values`,async()=>{
  const bindings=[],requested=[7,9,'new-scene',1];
  const module={gpu_agent:(id,json)=>JSON.parse(json).reload?'{"reload":{"values":[7,1,"old-scene",0],"setupIndices":[0,1,2]}}':'{"world":{"tick":42}}',
    gpu_bind_at:(id,values)=>{bindings.push(JSON.parse(values));return true;}};
  const f=await fixture({gpu:module,nextGpu:module});f.create(1);
  let view=1;
  if(intent==='contract'){
    const stage=f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'world',values:requested}]});
    f.exact.gpu.reset(true);f.create(2);f.exact.gpu.finishRestart();stage.commit();view=2;
  }else await f.exact.gpu.swap(1,{intent,values:new Map([[1,requested]]),...(intent==='restore'?{checkpoints:new Map([[1,new Uint8Array([1])]])}:{})});
  f.exact.gpu.surface(view,'world',requested);
  assert.deepEqual(bindings.at(-1),[7,1,'old-scene',0],'incidental bind discarded retained construction or released input');
  f.exact.gpu.surface(view,'world',[7,9,'new-scene',2]);
  assert.deepEqual(bindings.at(-1),[7,1,'old-scene',2],'live input reconstructed the world');
  f.exact.gpu.surface(view,'world',[7,10,'new-scene',2]);
  assert.deepEqual(bindings.at(-1),[7,10,'new-scene',2],'new round must use requested authored scene');
  f.exact.gpu.surface(view,'world',[7,10,'new-scene',2]);
  assert.deepEqual(bindings.at(-1),[7,10,'new-scene',2],'incidental bind changed the restarted construction');
});

test('normal stateless GPU canvases do not opt a data-backed core app into game transactions',async()=>{
  const f=await fixture({gpu:{gpu_carry:()=>undefined,gpu_agent:()=>''}});
  f.create(1,'line-map'); assert.equal(f.exact.gpu.participates(),false);
  f.exact.gpu.reset(true); f.create(2,'line-map'); f.exact.gpu.finishRestart();
  assert.equal(f.exact.gpu.participates(),false);
});

test('candidate pipelines may block, but final rebase excludes that time from the next game frame',async()=>{
  let time=100;const rebases=[];
  const f=await fixture({now:()=>time,nextGpu:{
    gpu_agent:(id,text)=>{const q=JSON.parse(text);if(q.reload){rebases.push(q.now);return '{"reload":{}}';}return '{"world":{}}';},
    gpu_render:()=>{time+=5000;return 0;},
  }});f.create(1);await f.exact.gpu.swap(1);
  assert.deepEqual(rebases,[100,5100]);
});

test('first usable timing waits for a rendering opportunity and observers stay bounded over repeated swaps',async()=>{
  const f=await fixture();f.create(1);assert.equal(f.observers.size,2);
  await f.exact.gpu.swap(1,{timing:{detectedAt:Date.now()-1000,buildCompleteAt:Date.now()-50}});
  assert.equal(f.exact.gpu.diagnostics().phase,'committed');
  f.paint();assert.equal(f.exact.gpu.diagnostics().phase,'committed');
  f.paint();const state=f.exact.gpu.diagnostics();assert.equal(state.phase,'usable');
  assert.ok(state.lastSuccessfulSwap.timing.editToFirstUsableFrameMs>=1000);
  for(let version=2;version<=6;version++) {await f.exact.gpu.swap(version);assert.equal(f.observers.size,2);}
  f.destroy(1);assert.equal(f.observers.size,0);
});

test('read-only world inspection supplies neither an advancing clock nor a viewport mutation',async()=>{
  const seen=[],f=await fixture({now:()=>999});f.create(1);
  f.gpu.gpu_agent=(id,json)=>{seen.push(JSON.parse(json));return '{"world":{}}';};
  f.exact.gpu.agent(1,{op:'state'});
  assert.deepEqual(seen,[{op:'state'}]);
});

test('Contract outcome names reset, removed and initialized UI slots',async()=>{
  const f=await fixture();f.create(1);
  const stage=f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'world',values:[]}]},{count:4,removed:true,kept:7},{count:'fresh',added:false,kept:7});
  f.exact.gpu.reset(true);f.create(2);f.exact.gpu.finishRestart();stage.commit();
  assert.deepEqual(f.exact.gpu.diagnostics().restoreOutcome.ui.resetFields,[{name:'count',outcome:'reset'},{name:'removed',outcome:'removed'},{name:'added',outcome:'initialized'}]);
});

test('explicit host detach rebases live pacing; attach freezes without implicit read ownership',async()=>{
  const source=readFileSync(new URL('../glue.js',import.meta.url),'utf8');
  const clock=source.slice(source.indexOf('async function clock(request)'),source.indexOf('\nlet ticker =',source.indexOf('async function clock(request)')));
  const run=new Function(`
    let wall=100000, t0=0, agentClock=10, ticker=null, hasTimers=true, starts=new WeakMap();
    const calls=[], animation={currentTime:5,playState:'paused',play(){this.playState='running';calls.push('play');}};
    const performance={now:()=>wall},document={getAnimations:()=>[animation]};
    const now=()=>agentClock ?? wall-t0,register=at=>calls.push(['register',at]),seek=at=>calls.push(['seek',at]);
    const setInterval=()=>123,clearInterval=id=>calls.push(['clear',id]),settleGpu=async()=>{};
    const globalThis={exact:{now,ownership:()=>({owner:agentClock===null?'human':'agent',clock:agentClock===null?'live':'controlled'}),gpu:{
      handoff:(owner,at)=>{calls.push(['handoff',owner,at]);return {world:[{tick:42,hash:'same'}],releasedInput:true};},resumeClock:on=>calls.push(['resume',on])
    }}};
    ${clock}
    return {clock,calls,now,advance:ms=>wall+=ms,exact:globalThis.exact};
  `)();
  const detached=await run.clock({owner:'human'});
  assert.equal(detached.control.clock,'live');assert.equal(detached.world[0].tick,42);assert.equal(run.exact.now,undefined);
  assert.equal(run.now(),10);run.advance(16);assert.equal(run.now(),26);
  assert.match((await run.clock({to:100})).error,/clock is live/);
  assert.equal(run.now(),26,'refused stepping acquired no clock');
  const attached=await run.clock({owner:'agent'});assert.equal(attached.clock,26);
  run.advance(5000);assert.equal(run.now(),26);
  const again=await run.clock({owner:'agent'});assert.equal(again.changed,false);
  assert.equal(run.calls.filter(c=>Array.isArray(c)&&c[0]==='handoff').length,2);
  assert.match((await run.clock({owner:'nobody'})).error,/human or agent/);
  assert.match((await run.clock({owner:'human',to:10})).error,/separate/);
});

test('world handoff preflights the complete participant set and releases held input',async()=>{
  let owner='agent',supported=true;
  const f=await fixture({input:true,gpu:{gpu_agent:(id,json)=>{
    const q=JSON.parse(json);
    if(q.owner){owner=q.owner;return JSON.stringify({tick:42,hash:'same',ownership:{owner},reload:{values:[]}});}
    return JSON.stringify({world:{ownership:supported||id===1?{owner}:undefined,input:{forwarded:[]}}});
  }}});const a=f.create(1);f.create(2,'second');
  a.listeners.keydown({target:a,code:'KeyW',timeStamp:0});
  supported=false;assert.match(f.exact.gpu.handoff('human',10).error,/unsupported/);assert.equal(owner,'agent');
  supported=true;const result=f.exact.gpu.handoff('human',10);assert.equal(result.world.length,2);assert.equal(owner,'human');
  const before=f.events.length;a.listeners.keyup({target:a,code:'KeyW',timeStamp:10});assert.equal(f.events.length,before,'old held key owner survived handoff');
});

test('controlled creation declares ownership before restoring intentional checkpoint-held input',async()=>{
  const order=[];
  const f=await fixture({now:()=>9000,input:true,gpu:{gpu_agent:(id,json)=>{const q=JSON.parse(json);if(q.owner){assert.equal(q.now,9000,'ownership must establish the destination clock before restore');order.push('owner');}return JSON.stringify({world:{restored:order.includes("restore"),input:{forwarded:["Space"]}},ownership:{owner:"agent"}});},gpu_restore:()=>{order.push('restore');return true;}}});
  f.exact.worldCarry=new Uint8Array([1]);const host=f.create(1);
  assert.deepEqual(order,['owner','restore']);
  host.listeners.keyup({target:host,code:'Space',timeStamp:0});assert.equal(f.events.at(-1).code,'Space');
});

test('scene content publishes a fresh plan with the cached compiler and retains the accepted generation on refusal',()=>{
  const driver=readFileSync(new URL('../dev.mjs',import.meta.url),'utf8');
  const functions=driver.slice(driver.indexOf('function sceneContractInput('),driver.indexOf('// Optional typed scene content'));
  const dir=mkdtempSync(join(tmpdir(),'scene-plan-reload-'));
  const app={dir,target:dir,workspace:dir,manifest:{game:{}}},source=join(dir,'app.contract'),plan=join(dir,'app.plan'),planCompiler=join(dir,'exact-dev');
  mkdirSync(join(dir,'.scene'));writeFileSync(source,'use sceneContent');writeFileSync(plan,'accepted');writeFileSync(planCompiler,'cached executable');writeFileSync(join(dir,'.scene/scene.contract'),'scene bytes');
  const published=[],pushed=[],calls=[],pending=new Map(),epoch='a'.repeat(32),cache=join(dir,'generations');let failure=false,stale=false,publicationFailure=false,renameFailure=false,compiled='new scene plan';
  const inputs=()=>reloadInputs([source]);
  const run=new Function('env',`const {app,resolve,planCompiler,existsSync,mkdtempSync,reloadInputs,spawnSync,source,buildEnv,verifyReloadInputs,readFileSync,renameSync,plan,pending,push,announcement,console,rmSync}=env;let seq=3,current={generation:'accepted'};function captureGeneration(reuse,bytes){env.captureGeneration(reuse,bytes,seq);current={generation:bytes.toString(),seq};}${functions};return {publishScenePlan,sceneContractInput,state:()=>({current,seq})};`)({
    app,resolve,planCompiler,existsSync,mkdtempSync,reloadInputs,source,buildEnv:{},verifyReloadInputs,readFileSync,renameSync:(...args)=>{if(renameFailure)throw new Error('rename failed');return renameSync(...args);},plan,pending,rmSync,
    spawnSync(command,args){calls.push([command,args]);assert.equal(command,planCompiler);assert.deepEqual([args[0],args[2]],[source,'--once']);writeFileSync(args[1],compiled);if(stale)writeFileSync(source,'edited during compiler');return {status:failure?1:0,stderr:'compile failed'};},
    captureGeneration(reuse,bytes,seq){assert.equal(reuse,true);retainDevGeneration(cache,epoch,seq,new Map([['app.plan',bytes],['exact.json',Buffer.from(JSON.stringify({exact:1,dev:{epoch,seq},plan:{bytes:bytes.length,sha256:sha256(bytes)},assets:[]}))]]));if(publicationFailure)throw new Error('publication failed');published.push(bytes.toString());},push:message=>pushed.push(message),announcement:()=>({generation:'new'}),console:{log(){}},
  });
  try{
    assert.equal(run.sceneContractInput(join(dir,'.scene/scene.contract')),true);
    for(const path of [source,join(dir,'logic/src/lib.rs'),join(dir,'other/scene.contract')])assert.equal(run.sceneContractInput(path),false);
    run.publishScenePlan(Date.now(),inputs(),inputs);
    assert.deepEqual(published,['new scene plan']);assert.equal(pushed.length,1);assert.equal(readFileSync(plan,'utf8'),'new scene plan');
    const accepted=run.state();
    failure=true;assert.throws(()=>run.publishScenePlan(Date.now(),inputs(),inputs),/scene plan refused/);
    failure=false;stale=true;assert.throws(()=>run.publishScenePlan(Date.now(),inputs(),inputs),/inputs changed/);
    stale=false;publicationFailure=true;compiled='partially retained';assert.throws(()=>run.publishScenePlan(Date.now(),inputs(),inputs),/publication failed/);assert.deepEqual(run.state().current,accepted.current);assert.equal(run.state().seq,accepted.seq+1);
    publicationFailure=false;renameFailure=true;compiled='rename refused';assert.throws(()=>run.publishScenePlan(Date.now(),inputs(),inputs),/rename failed/);assert.deepEqual(run.state().current,accepted.current);assert.equal(run.state().seq,accepted.seq+2);
    assert.equal(pushed.length,1);assert.equal(readFileSync(plan,'utf8'),'new scene plan');assert.equal(calls.length,5);
    assert.equal(readFileSync(join(cache,epoch,String(accepted.seq),'app.plan'),'utf8'),'new scene plan');
    renameFailure=false;compiled='different next edit';run.publishScenePlan(Date.now(),inputs(),inputs);
    assert.equal(run.state().seq,accepted.seq+3);assert.equal(pushed.length,2);assert.equal(readFileSync(plan,'utf8'),compiled);
    assert.equal(readFileSync(join(cache,epoch,String(accepted.seq+2),'app.plan'),'utf8'),'rename refused');
    assert.equal(readFileSync(join(cache,epoch,String(accepted.seq+3),'app.plan'),'utf8'),compiled);
    assert.match(driver,/if \(sceneContractInput\(path\)\) return/);
  }finally{rmSync(dir,{recursive:true,force:true});}
});

// The Rust integration test supplies a live Bridge over inherited stdio. This
// avoids substituting a JSON-object check for Contract's actual type checker.
function candidateHost(op,payload='') {
    writeSync(1,`@exact ${op} ${payload}\n`);
    const byte=Buffer.alloc(1),bytes=[];
    while(readSync(0,byte,0,1,null) && byte[0]!==10) bytes.push(byte[0]);
    return JSON.parse(Buffer.from(bytes).toString());
}
test.skipIf(!process.env.EXACT_TEST_CANDIDATE_HOST)('actual candidate host refuses shapes/bindings atomically and commits the complete HUD', async()=>{
  const bridge=candidateHost;
  const glue=readFileSync(new URL('../glue.js',import.meta.url),'utf8');
  const bootSource=glue.slice(glue.indexOf('async function bootNow('),glue.indexOf('\nlet ready;',glue.indexOf('async function bootNow(')));
  for(const failure of ['shape','binding',null]) {
    const initial=bridge('initial');bridge('advance','1234');
    let next=0;const worlds=new Map(),publications=new Map(),bound=[],destroyed=[];
    const f=await fixture({input:true,now:()=>1234,gpu:{
      gpu_create(name){const id=++next;worlds.set(id,{name,values:[],tick:42,input:['KeyW']});return id;},
      gpu_bind_at(id,json){const world=worlds.get(id),values=JSON.parse(json);bound.push([id,values]);if(failure==='binding'&&world.name==='other'&&values[0]===5)return false;world.values=values;publications.set(id,JSON.stringify({count:world.name==='world'?2:3}));return true;},
      gpu_published(id){const text=publications.get(id);publications.delete(id);return text;},
      gpu_carry:id=>new TextEncoder().encode(JSON.stringify(worlds.get(id))),
      gpu_restore(id,bytes){const saved=JSON.parse(new TextDecoder().decode(bytes));Object.assign(worlds.get(id),{tick:saved.tick,input:saved.input});publications.set(id,JSON.stringify({count:saved.name==='world'?2:3}));return true;},
      gpu_agent(id,text){const q=JSON.parse(text),world=worlds.get(id);if(q.reload){if(q.releaseInput)world.input=[];return JSON.stringify({reload:{values:world.values,setupIndices:[],rebased:true,releasedInput:q.releaseInput}});}return JSON.stringify({world});},
      gpu_destroy(id){destroyed.push(id);worlds.delete(id);},
    }});
    for(const row of initial.ops.filter(op=>op.op==='surface'))f.create(row.id,row.name,row.values);
    bridge('record','world\0{"count":2}');bridge('record','other\0{"count":3}');
    const old=structuredClone([...worlds]),before=bridge('agent','{"op":"state"}');
    const elements=[...f.exact.views.values()].map(el=>el.canvas);
    const staged=[];f.exact.stageSurfaceRecord=(name,json)=>{staged.push(name);return bridge('stage',`${name}\0${json}`);};
    assert.match(bridge('stage','world\0{"count":2}').error,/requires a candidate/);
    let presented=null,commits=0,rollbacks=0;
    const memory={buffer:new ArrayBuffer(1024)},wasm={
      exact_in:()=>0,exact_plan_fonts:()=>JSON.stringify([]),exact_stage_surface_record(){},
      exact_begin_boot:()=>Number(bridge('begin')),
      exact_boot_plan:len=>JSON.stringify(bridge('boot',new TextDecoder().decode(new Uint8Array(memory.buffer,0,len)))),
      exact_finish_boot(commit){commit?commits++:rollbacks++;bridge('finish',String(commit));},
    };
    const run=new Function('env',`
      const {globalThis,wasm,memory,views,root,applyBatch,ask}=env;
      let bootAttempt=0,activeModule=null,devAssets=null,incarnation=0,ticker=null,agentClock=1234,storageRequests=null,grants=[];
      const encoder=new TextEncoder(),readOut=x=>x,prepareFonts=async()=>[],location={pathname:'/',search:''},innerWidth=390,innerHeight=844;
      const navigation={reset(){}},animations=new Map(),followedScrolls=new Map(),pendingScrolls=new Map(),messageFrames=new Map(),messageViews=new Map(),controllers=new Set(),inflight=new Map();
      const commitFonts=()=>{},activateData=()=>{},requestAnimationFrame=()=>{},loadGpuIfNeeded=()=>{},releaseAssets=()=>{};
      ${bootSource};return bootNow;
    `)({globalThis:{exact:f.exact},wasm,memory,views:f.exact.views,root:{replaceChildren(){f.order.push('DOM reset');}},ask:q=>bridge('agent',JSON.stringify(q)),
      applyBatch(batch){presented=batch;for(const row of batch.ops.filter(op=>op.op==='surface'))f.create(row.id,row.name,row.values);return {timers:batch.timers};},
    });
    if(failure) {
      await assert.rejects(run(new TextEncoder().encode(failure==='shape'?'2':'1')),failure==='shape'?/publication.*count.*String/:/publication bind/);
      assert.equal(commits,0);assert.equal(rollbacks,1);assert.equal(presented,null);
      assert.deepEqual([...worlds],old,'old world state/input mutated');
      assert.deepEqual(bridge('agent','{"op":"state"}'),before,'old Contract state/clock mutated');
      assert.ok(elements.every(el=>el.isConnected));assert.ok(!f.order.includes('DOM reset'));
      assert.ok(destroyed.every(id=>id>2));assert.deepEqual(staged,['world','other']);
    } else {
      await run(new TextEncoder().encode('1'));
      assert.equal(commits,1);assert.equal(rollbacks,0);assert.deepEqual(destroyed,[1,2]);
      assert.deepEqual([...worlds.values()].map(w=>w.values),[[3],[5]],'cross-world resulting bindings not committed');
      assert.ok([...worlds.values()].every(w=>w.tick===42&&w.input.length===0));
      assert.ok(presented.ops.some(op=>op.op==='props'&&op.set?.text==='new 2/3'),JSON.stringify(presented));
      assert.equal(presented.ops.filter(op=>op.op==='surface').length,2,'intermediate bindings escaped');
      assert.ok(presented.ops.filter(op=>op.op==='at').every(op=>op.ms===1234),'publication used the wrong carried clock');
      assert.deepEqual(bound.filter(([id])=>id>2).map(([id,values])=>[worlds.get(id).name,values]),[['world',[0]],['other',[0]],['other',[5]],['world',[3]]]);
      const state=bridge('agent','{"op":"state"}');assert.equal(state.clock,1234);assert.equal(state.resources.a.count,2);assert.equal(state.resources.b.count,3);
      assert.equal(f.stagedRecords.length,0);assert.equal(f.records.length,2,'accepted publications were replayed after commit');
    }
  }
});

test.skipIf(!process.env.EXACT_TEST_CANDIDATE_HOST)('actual candidate host stages GPU-only Continue/Restart/Restore and preserves current UI identities',async()=>{
  const bridge=candidateHost,glue=readFileSync(new URL('../glue.js',import.meta.url),'utf8');
  const stageSource=glue.slice(glue.indexOf('  stageCurrent() {'),glue.indexOf('\n  get ready()',glue.indexOf('  stageCurrent() {')));
  for(const intent of ['continue','restart','restore']) for(const failure of ['shape','binding','rebase',null]) {
    const initial=bridge('initial','3');bridge('advance','1234');
    const rowBatch=bridge('record','rows\0{"items":["row"]}');
    const row=rowBatch.ops.find(op=>op.op==='create'&&op.props?.['data-testid']==='row').id;
    bridge('press',`${row} 1234`);bridge('press',`${row} 1234`);
    const state=()=>bridge('agent','{"op":"state"}'),tree=()=>bridge('agent','{"op":"tree"}');
    const rowCount=()=>tree().nodes.find(node=>node.props.testId==='row-count').props.text;
    assert.equal(rowCount(),'2');
    // A fork's row-slot Rc must be private; boot+carry and a shallow clone both
    // fail this proof (or the successful-swap row/timer checks below).
    const beforeFork=tree();assert.equal(bridge('surface-begin').error,null);
    bridge('press',`${row} 1234`);assert.equal(rowCount(),'3');bridge('finish','0');assert.deepEqual(tree(),beforeFork);
    let next=0,commits=0,aborts=0;const worlds=new Map(),publications=new Map(),destroyed=[],bound=[],presented=[],rebases=new Map();
    const module={
      gpu_create(name){const id=++next;worlds.set(id,{name,values:[],tick:id<=2?42:0,input:[]});return id;},
      gpu_bind_at(id,json){const world=worlds.get(id),values=JSON.parse(json);bound.push([id,values]);if(id>2&&failure==='binding'&&world.name==='world'&&values[0]===13)return false;world.values=values;
        publications.set(id,JSON.stringify({count:id<=2?(world.name==='world'?2:3):(world.name==='world'?8:failure==='shape'?'incompatible':13)}));return true;},
      gpu_published(id){const text=publications.get(id);publications.delete(id);return text;},
      gpu_carry:id=>new TextEncoder().encode(JSON.stringify(worlds.get(id))),
      gpu_restore(id,bytes){const saved=JSON.parse(new TextDecoder().decode(bytes));Object.assign(worlds.get(id),{tick:saved.tick,input:saved.input});return true;},
      gpu_agent(id,text){const q=JSON.parse(text),world=worlds.get(id);if(q.reload){rebases.set(id,(rebases.get(id)??0)+1);if(failure==='rebase'&&id>2&&q.releaseInput===false)return '{"error":"rebase refused"}';if(q.releaseInput)world.input=[];return JSON.stringify({reload:{values:world.values,setupIndices:[],rebased:true,releasedInput:q.releaseInput}});}return JSON.stringify({world:{...world,input:{forwarded:world.input}}});},
      gpu_input(id,text){const q=JSON.parse(text),world=worlds.get(id);if(q.t==='key')world.input=q.down?[q.code]:[];return true;},
      gpu_destroy(id){destroyed.push(id);worlds.delete(id);},
    };
    const f=await fixture({input:true,now:()=>1234,gpu:module,nextGpu:module});
    const elements=[];
    for(const op of initial.ops.filter(op=>op.op==='surface')) {
      const host=f.create(op.id,op.name,op.values);elements.push(host);
      host.listeners.keydown({target:host,code:'KeyW',timeStamp:1234});
    }
    for(const [name,count] of [['world',2],['other',3]]) {
      const batch=bridge('record',`${name}\0${JSON.stringify({count})}`);
      for(const op of batch.ops.filter(op=>op.op==='surface'))f.exact.gpu.surface(op.id,op.name,op.values);
    }
    const old=structuredClone([...worlds]),before=state(),beforeTree=tree(),beforeLogs=bridge('agent','{"op":"logs"}'),recordCount=f.records.length;
    const canvases=elements.map(el=>el.canvas),listeners=elements.map(el=>el.listeners.keyup);
    const wasm={exact_begin_surface_boot:()=>JSON.stringify(bridge('surface-begin')),exact_finish_boot(commit){commit?commits++:aborts++;bridge('finish',String(commit));}};
    f.exact.stageCurrent=new Function('wasm','readOut','applyBatch',`return ({${stageSource}}).stageCurrent;`)(wasm,x=>x,batch=>{presented.push(batch);assert.equal(commits,1,'batch escaped before commitment');});
    f.exact.stageSurfaceRecord=(name,json)=>{assert.equal(commits,0);assert.equal(destroyed.length,0);return bridge('stage',`${name}\0${json}`);};
    const checkpoints=new Map(initial.ops.filter(op=>op.op==='surface').map(op=>[op.id,new TextEncoder().encode(JSON.stringify({...old.find(([,w])=>w.name===op.name)[1],tick:99,input:['Space']}))]));
    if(failure) {
      await assert.rejects(f.exact.gpu.swap(1,{intent,checkpoints}),failure==='shape'?/publication.*count.*Number/:failure==='binding'?/publication bind/:/rebase refused/);
      assert.equal(commits,0);assert.equal(aborts,1);assert.deepEqual(presented,[]);
      assert.deepEqual([...worlds],old);assert.deepEqual(state(),before);assert.deepEqual(tree(),beforeTree);assert.deepEqual(bridge('agent','{"op":"logs"}'),beforeLogs);
      assert.deepEqual(elements.map(el=>el.canvas),canvases);assert.deepEqual(elements.map(el=>el.listeners.keyup),listeners);
      assert.ok(destroyed.every(id=>id>2));assert.equal(f.exact.gpu.version,0);assert.ok(!f.order.includes('old unload'));
      elements[0].listeners.keyup({target:elements[0],code:'KeyW',timeStamp:1234});assert.deepEqual(worlds.get(1).input,[],'old held input could not be released after refusal');
    } else {
      await f.exact.gpu.swap(1,{intent,checkpoints});
      assert.equal(commits,1);assert.equal(aborts,0);assert.equal(presented.length,1);assert.deepEqual(destroyed,[1,2]);
      assert.deepEqual([...worlds.values()].map(w=>w.values),[[13],[21]],'resulting cross-world bindings did not settle');
      assert.ok([...worlds.values()].every(w=>w.tick===(intent==='restart'?0:intent==='restore'?99:42)&&w.input.length===0));
      assert.ok(presented[0].ops.some(op=>op.op==='props'&&op.set?.text==='new 8/13'));
      assert.ok(!presented[0].ops.some(op=>['create','destroy','surface','roots'].includes(op.op)),'swap rebuilt UI or replayed already-applied bindings');
      assert.equal(state().clock,1234);assert.deepEqual(state().slots,before.slots);assert.equal(rowCount(),'2');
      assert.deepEqual(tree().nodes.map(node=>node.id),beforeTree.nodes.map(node=>node.id));
      bridge('advance','1999');assert.deepEqual(state().slots,before.slots);bridge('advance','2000');assert.equal(state().slots.n,before.slots.n+1,'timer deadline was restarted');
      bridge('press',`${row} 2000`);assert.equal(rowCount(),'3','committed row frame lost its own slot storage');
      assert.equal(f.records.length,recordCount,'validated publications escaped after commit');
    }
  }
});


test('candidate declarations and textures finish before restore validation and cutover', async()=>{
  const delivered=[]; let requested=0;
  const f=await fixture({nextGpu:{
    gpu_assets:()=>JSON.stringify([['crate.model'],['crate/0.tex'],[]][Math.min(requested++,2)]),
    gpu_asset:(id,name,bytes)=>{delivered.push([name,...bytes]);return true;},
    gpu_agent:(id,json)=>{
      if(JSON.parse(json).reload) {
        assert.equal(delivered.length,2,'no reload clock before texture readiness');
        return JSON.stringify({reload:{values:[],setupIndices:[]}});
      }
      return JSON.stringify({world:{tick:30,restored:delivered.length===2}});
    },
  }});
  f.create(1);
  f.exact.devAssets=new Map([['assets/crate.model',{bytes:new Uint8Array([1])}],['assets/crate/0.tex',{bytes:new Uint8Array([2])}]]);
  await f.exact.gpu.swap(1);
  assert.deepEqual(delivered,[['crate.model',1],['crate/0.tex',2]]);
  assert.equal(f.exact.gpu.diagnostics().phase,'committed');
});

test('candidate asset refusal and excessive dependency rounds retain every old canvas', async()=>{
  for(const mode of ['missing','failed','rounds']) {
    let deliveries=0;
    const f=await fixture({nextGpu:{gpu_assets:()=> '["a.tex"]',gpu_asset:()=>{deliveries++;return mode!=='failed';}}});
    const a=f.create(1), old=a.canvas;
    f.exact.devAssets=new Map(mode==='missing'?[]:[['assets/a.tex',{bytes:new Uint8Array([1])}]]);
    await assert.rejects(f.exact.gpu.swap(1),mode==='rounds'?/16 delivery rounds/:/a.tex/);
    assert.equal(deliveries,mode==='rounds'?16:mode==='failed'?1:0);
    assert.equal(a.canvas,old);
    assert.ok(!f.order.includes('old destroy'));
  }
});
