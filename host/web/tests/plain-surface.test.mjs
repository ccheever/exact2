import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';
const flush = () => new Promise(resolve => setTimeout(resolve, 0));
const plain = {gpu_carry:()=>undefined, gpu_agent:()=>''};
const stamps = ['boundMs','firstTickMs','firstPublicationMs','firstFrameSubmittedMs','firstFrameMs','inputMs'];

for (const mode of ['reject','never','absent']) test(`no device beacon or gpuMs when device is ${mode}`, async()=>{
  const f=await fixture({search:'?smoke=1',gpu:{...plain,
    gpu_load:mode==='absent'?undefined:mode==='reject'?async()=>{throw Error('no adapter');}:()=>new Promise(()=>{}),
  }});
  f.create(1,'sky'); f.frame(); await f.exact.gpu.settled();
  assert.deepEqual(f.beacons,[]); assert.equal(f.exact.root.dataset.gpuMs,undefined);
  assert.ok(Number.isFinite(Number(f.exact.root.dataset.worldModuleMs)));
});

test('plain surface waits for device AND shaders, emits one beacon, never drives a world clock or stamps world timing',async()=>{
  let device, shader, installed=false, renders=0, clocks=0, attaches=0;
  const f=await fixture({search:'?smoke=1',devAssets:null,fetch:()=>new Promise(resolve=>shader=()=>resolve(new Response('sky source'))),gpu:{...plain,
    gpu_load:()=>new Promise(resolve=>device=resolve),
    gpu_shader_names:()=> '["sky"]', gpu_shaders_clear:()=>{installed=false;},
    gpu_shader:(name,text)=>{assert.equal(name,'sky');assert.equal(text,'sky source');installed=true;return true;},
    gpu_agent:(id,text)=>{if(JSON.parse(text).op==='clock')clocks++;return '';},
    gpu_attach:()=>{assert.ok(installed);attaches++;return true;},
    gpu_render:()=>{assert.ok(installed,'first render must see installed shaders');renders++;return 0;},
  }});
  f.create(1,'sky'); f.frame(); await f.exact.gpu.settled();
  assert.equal(renders,0); assert.deepEqual(f.beacons,[]);
  assert.throws(()=>f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'sky',values:[]}]}),/device is not ready/);
  device(); await flush(); f.frame();
  assert.equal(renders,0); assert.equal(attaches,0); assert.deepEqual(f.beacons,[]);
  shader(); await flush(); f.frame(); f.paint(); await f.exact.gpu.settled();
  assert.ok(renders>0,'negative control: successful device load must render'); assert.equal(attaches,1);
  assert.deepEqual(f.beacons,[`/__gpu?ms=${f.exact.root.dataset.gpuMs}`]);
  assert.ok(Number.isFinite(Number(f.exact.root.dataset.gpuMs)));
  assert.equal(clocks,0);
  for(const key of stamps)assert.equal(f.exact.entries()[0][key],undefined,key);
  f.exact.devAssets=new Map([["shaders/sky.wgsl",{bytes:new TextEncoder().encode("sky source")}]]);
  await f.exact.gpu.swap(1); f.frame();
  assert.equal(f.beacons.length,1,'swap must not repeat startup beacon');
});

test('plain device recovery retains IDs and bindings and replaces only attached canvases',async()=>{
  let lost=false, binds=0, recovered;
  const f=await fixture({gpu:{...plain,gpu_bind_at:()=>{binds++;return true;},gpu_render:()=>lost?3:0,gpu_dirty:()=>lost,
    gpu_recover:async ids=>{recovered=[...ids];lost=false;return '{"status":"recovered"}';},
  }});
  const a=f.create(1,'sky'), b=f.create(2,'map'), old=[a.canvas,b.canvas];
  const ids=f.exact.entries().map(e=>e.id); lost=true; f.frame(); await flush();
  assert.deepEqual(recovered,ids);assert.deepEqual(f.exact.entries().map(e=>e.id),ids);
  assert.equal(binds,2);assert.notEqual(a.canvas,old[0]);assert.notEqual(b.canvas,old[1]);
  assert.equal(f.exact.gpu.recovery.status,'recovered');
});

for (const path of ['plan','swap']) test(`plain ${path} validates render and refuses a failed candidate without replacing live canvases`,async()=>{
  let renders=0;
  const f=await fixture({gpu:plain,nextGpu:{...plain,gpu_render:()=>{renders++;return 2;}}});
  const a=f.create(1,'sky'), old=a.canvas;
  if(path==='plan') {
    f.gpu.gpu_render=()=>{renders++;return 2;};
    assert.throws(()=>f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'sky',values:[]}]}),/render/);
  } else await assert.rejects(f.exact.gpu.swap(1),/render/);
  assert.equal(renders,1);assert.equal(a.canvas,old);assert.ok(!f.order.includes('old unload'));
});

test('plain plan restart and module swap still render candidates and commit, requiring the device ABI',async()=>{
  let renders=0;
  const f=await fixture({gpu:{...plain,gpu_render:()=>{renders++;return 0;}},nextGpu:plain});
  f.create(1,'sky');const before=renders;
  const stage=f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'sky',values:[]}]});
  assert.ok(renders>before);f.exact.gpu.reset(true);f.create(2,'sky');f.exact.gpu.finishRestart();stage.commit();
  await f.exact.gpu.swap(1);assert.equal(f.exact.gpu.version,1);
  f.nextGpu.gpu_render=undefined;
  await assert.rejects(f.exact.gpu.swap(2),/ABI missing gpu_render/);
  assert.equal(f.exact.gpu.version,1);
});
