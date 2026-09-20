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

// The baked ownership ABI drives the real Sim through the production staging code.
import {existsSync,readFileSync} from 'node:fs';
const tallyJS=new URL('../../../game/games/tally/dist/gpu.js',import.meta.url);
const realWorld=existsSync(tallyJS)?test:test.skip;
async function tallyModule() {
  const module=await import(tallyJS.href);
  await module.default({module_or_path:readFileSync(new URL('gpu_bg.wasm',tallyJS))});
  assert.equal(module.gpu_load,undefined);
  module.gpu_unload();
  return {...module,gpu_load:undefined};
}
realWorld('baked Tally joins live frames and identifies smoke as ownership-only',async()=>{
  const module=await tallyModule();
  const f=await fixture({live:true,gpu:module,search:'?smoke=1'});
  try {
    f.create(1,'world',{seed:7});
    for(let i=1;i<=3;i++)f.paint(i*17);
    const state=f.exact.gpu.decorate({op:'state'},{});
    assert.equal(state.surfaceModule,'ownership-only');
    assert.ok(state.world[0].tick>=3,'negative control: autonomous live ticks');
    assert.ok(f.records.at(-1).includes('"ticks":3'));
    assert.deepEqual(f.beacons,[]);
  } finally {module.gpu_unload();}
});
realWorld('baked Tally reload stages plain values and releases a held Sim key',async()=>{
  const module=await tallyModule();let now=0;
  const f=await fixture({gpu:module,now:()=>now});
  const key=()=>assert.ok(module.gpu_input(f.exact.entries()[0].id,JSON.stringify({t:'key',code:'KeyD',key:'d',down:true,repeat:false,at:now})));
  try {
    f.create(1,'world',{seed:7});
    key();now=17;await f.exact.gpu.settled();
    const before=f.exact.gpu.agent(1,{op:'state'}).world.published.pile_count;
    const stage=f.exact.gpu.stagePlan({ops:[{op:'surface',id:2,name:'world',values:{seed:7}}]});
    f.exact.gpu.reset(true);f.create(2,'world',{seed:7});f.exact.gpu.finishRestart();stage.commit();
    assert.deepEqual(f.exact.entries()[0].values,{seed:7});
    assert.deepEqual(f.exact.entries()[0].setupKeys,['seed']);
    key();now=34;await f.exact.gpu.settled();
    assert.equal(f.exact.gpu.agent(2,{op:'state'}).world.published.pile_count,before-1);
  } finally {module.gpu_unload();}
});

test('a resize before the first paced frame never advances ownership time', async () => {
  const stamps=[];
  const f=await fixture({live:true,gpu:{gpu_load:undefined,gpu_error:()=>'',gpu_advance:at=>{stamps.push(at);return true;}}});
  f.create(1);
  f.mutation();
  assert.deepEqual(stamps,[], 'observer callbacks are layout, not simulation steps');
  f.paint(17);
  assert.equal(stamps.length,1, 'negative control: the paced frame still drives');
});

const failureJS=new URL('../../../game/tests/world-failure/dist/gpu.js',import.meta.url);
(existsSync(failureJS)?test:test.skip)('baked tick-one failure stays bound through the web carrier and restarts',async()=>{
  const module=await import(failureJS.href);
  await module.default({module_or_path:readFileSync(new URL('gpu_bg.wasm',failureJS))});
  module.gpu_unload();
  const f=await fixture({gpu:{...module,gpu_load:undefined}});
  try {
    f.create(1,'world',{first_tick:true,restart:false});
    const first=f.exact.gpu.agent(1,{op:'state'});
    assert.equal(first?.world.failed,true,'creation must retain the initialized failed world');
    assert.match(first.world.error,/tick 1.*fixture tick refused/);
    assert.equal(first.world.tick,0);
    assert.ok(f.exact.gpu.agent(1,{op:'tree'}).entities);
    assert.match(f.exact.gpu.agent(1,{op:'clock',ticks:1}).error,/fixture tick refused/);
    assert.equal(module.gpu_messages(f.exact.entries()[0].id),undefined);
    assert.equal(module.gpu_published(f.exact.entries()[0].id),undefined);
    f.exact.gpu.surface(1,'world',{first_tick:false,restart:true});
    assert.equal(f.exact.gpu.agent(1,{op:'state'}).world.failed,false);
    assert.equal(f.exact.gpu.agent(1,{op:'state'}).world.tick,1);
  } finally {module.gpu_unload();}
});
// Execute both real revisions under exactly the same device, clock and resource inputs.
import {execFileSync} from 'node:child_process';
test('device state and DOM dataset are byte-identical to f418821',async()=>{
  const before=execFileSync('git',['show','f418821:host/web/gpu-glue.js'],{cwd:import.meta.dir,encoding:'utf8'});
  const performance={now:()=>100,getEntriesByName:()=>[{startTime:12}],getEntriesByType:()=>[
    {name:'http://fixture/gpu_bg.wasm',startTime:10,responseEnd:20,decodedBodySize:123,duration:10,transferSize:150,encodedBodySize:100},
  ]};
  const run=async gpuSource=>{
    const f=await fixture({gpuSource,performance,gpu:{gpu_error:()=>''}});
    f.create(1);f.paint(17);
    return {state:JSON.stringify(f.exact.gpu.decorate({op:'state'},{})),dataset:JSON.stringify(f.exact.root.dataset)};
  };
  const expected=await run(before),actual=await run();
  assert.equal(actual.state,expected.state);
  assert.equal(actual.dataset,expected.dataset);
});
