import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';
const flush = () => new Promise(resolve => setTimeout(resolve, 0));
const plain = {gpu_carry:()=>undefined, gpu_agent:()=>''};
test('device bootstrap retains pending readiness, shader-before-create and pending shader validation',async()=>{
  let device, shader, installed=false, creates=0, binds=0, renders=0;
  const f=await fixture({pendingLoad:true,search:'?smoke=1',devAssets:null,
    fetch:()=>new Promise(resolve=>shader=()=>resolve(new Response('sky source'))),gpu:{...plain,
    gpu_load:()=>new Promise(resolve=>device=resolve),
    gpu_load_headless:()=>{throw Error('device module used headless ABI');},
    gpu_shader_names:()=> '["sky"]', gpu_shaders_clear:()=>{installed=false;},
    gpu_shader:()=>{installed=true;return true;},
    gpu_shader_check:async()=>false,
    gpu_create:()=>{assert.ok(installed);creates++;return creates;},
    gpu_bind_at:()=>{assert.ok(installed);binds++;return true;},
    gpu_render:()=>{assert.ok(installed);renders++;return 0;},
  }});
  f.create(1,'sky');
  let settled=false, checked=false;
  const ready=f.exact.gpu.settled().then(()=>settled=true);
  const validation=f.exact.gpu.prepareShaders(new Map([["shaders/sky.wgsl",{bytes:new TextEncoder().encode('invalid')}]]))
    .then(()=>{throw Error('invalid shader accepted');},()=>{checked=true;});
  await flush();assert.equal(settled,false);assert.equal(checked,false);
  assert.equal(creates,0);assert.equal(binds,0);assert.deepEqual(f.beacons,[]);
  device();await flush();assert.equal(creates,0);assert.equal(settled,false);
  shader();await f.initialized;await ready;await validation;f.frame();
  assert.equal(creates,1);assert.equal(binds,1);assert.ok(renders>0);assert.equal(checked,true);
  assert.deepEqual(f.beacons,[`/__gpu?ms=${f.exact.root.dataset.gpuMs}`]);
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
