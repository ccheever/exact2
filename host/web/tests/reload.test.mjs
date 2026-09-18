import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';
import {reloadInputs, verifyReloadInputs, gpuArtifact, sha256} from '../reload-build.mjs';
import {mkdtempSync, writeFileSync, rmSync, readFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

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
  assert.deepEqual(events.filter(v=>['capture','rebase','render'].includes(v[0])),[['capture',12000],['rebase',12000],['render',12000]]);
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
