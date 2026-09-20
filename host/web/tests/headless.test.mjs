import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';

test('ownership-only binding joins live rAF without an agent or ResizeObserver', async () => {
  let ticks=1, pending=true, drives=0;
  const f=await fixture({live:true, search:'?smoke=1',gpu:{
    gpu_load:undefined, gpu_error:()=>'',
    gpu_carry:()=>{throw Error('carry is not a capability probe');},
    gpu_advance:()=>{drives++;ticks++;pending=true;return true;},
    gpu_agent:(id,text)=>{assert.notEqual(JSON.parse(text).op,'clock');return JSON.stringify({world:{tick:ticks,ready:true}});},
    gpu_published:()=>{if(!pending)return;pending=false;return JSON.stringify({ticks});},
    gpu_render:()=>{throw Error('ownership-only world rendered');},
  }});
  f.create(1);
  for (let i=1;i<=3;i++) f.paint(i*17);
  assert.equal(drives,3);assert.equal(ticks,4);
  assert.ok(f.records.at(-1).includes('"ticks":4'));
  assert.deepEqual(f.beacons,[]);assert.equal(f.exact.root.dataset.gpuMs,undefined);
});
