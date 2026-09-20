import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';

for (const mode of ['reject','never','absent']) test(`world ticks and publishes when device is ${mode}`, async () => {
  let ticks=0, pending=true;
  const f=await fixture({gpu:{
    gpu_load: mode==='absent' ? undefined : mode==='reject' ? async()=>{throw Error('no device');} : ()=>new Promise(()=>{}),
    gpu_agent:(id,text)=>{const q=JSON.parse(text); if(q.op==='clock') {ticks+=q.now??0;pending=true;} return JSON.stringify({world:{tick:ticks,hash:`h${ticks}`,ready:true,presentation:'none'},tick:ticks});},
    gpu_published:()=>{if(!pending)return;pending=false;return JSON.stringify({ticks});},
    gpu_render:()=>{throw Error('device-free world rendered');},
  }});
  f.create(1); const initial=f.records.at(-1);
  assert.equal(f.exact.gpu.agent(1,{op:'state'}).world.ready,true);
  f.exact.gpu.agent(1,{op:'clock',now:1000});
  assert.equal(f.exact.gpu.agent(1,{op:'state'}).world.tick,1000);
  assert.notEqual(f.records.at(-1),initial,'negative control: clock must publish changed state');
  f.frame(); assert.ok(f.records.length>1);
});
test('late device attaches to the same rendered world ID without a bind or restart',async()=>{
  let resolveDevice, tick=0, binds=0, creations=0; const attached=[];
  const f=await fixture({gpu:{
    gpu_load:()=>new Promise(resolve=>resolveDevice=resolve),
    gpu_create_headless:()=>{creations++;return 42;},
    gpu_bind_at:()=>{binds++;return true;},
    gpu_agent:(id,text)=>{assert.equal(id,42);const q=JSON.parse(text);if(q.op==='clock')tick=q.now;return JSON.stringify({world:{tick,hash:`h${tick}`,ready:false,readyReasons:['no device']}});},
    gpu_attach:id=>{attached.push(id);return true;},
    gpu_render:()=>0,
  }});
  f.create(1); f.exact.gpu.agent(1,{op:'clock',now:1000});
  const before=f.exact.gpu.agent(1,{op:'state'}).world;
  assert.equal(before.ready,false);assert.deepEqual(before.readyReasons,['no device']);
  resolveDevice(); await new Promise(resolve=>setTimeout(resolve,0));
  const after=f.exact.gpu.agent(1,{op:'state'}).world;
  assert.deepEqual(after,before);assert.deepEqual(attached,[42]);assert.equal(creations,1);assert.equal(binds,1);
  f.exact.gpu.agent(1,{op:'clock',now:2000});assert.equal(f.exact.gpu.agent(1,{op:'state'}).world.tick,2000);
});
