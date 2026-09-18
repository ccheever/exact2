import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {artifactDigest, closeSessions, equal} from './proof.mjs';
import {typeArguments, typeFor, browserKey, nativeKey, render, worldView} from '../scripts/agent.mjs';

test('held keys release the original carrier and retain partial failure steps', async () => {
  const calls = [], node = {id:17};
  let clockRan = false;
  const carrier = {input:async (id, kind, options) => {
    calls.push([id, kind, options.phase]);
    if (options.phase === 'up') expect(clockRan).toBe(true);
    return {delivery:'platform'};
  }};
  let failure;
  try {
    await typeFor({node, target:'world', options:{key:'KeyW',for:1500}, carrier,
      clock:async () => { clockRan = true; throw new Error('world disappeared'); }, tagged:r => r});
  } catch (error) { failure = error; }
  expect(calls).toEqual([[17,'key','down'],[17,'key','up']]);
  expect(failure.message).toBe('world disappeared');
  expect(failure.steps.map(s => s.op)).toEqual(['type','clock','type']);
  expect(render('type', {steps:failure.steps})).toContain('ERROR world disappeared');
});
test('a failed down still releases; a failed up keeps the original error and both failures', async () => {
  const calls = [];
  const carrier = {input:async (_, __, {phase}) => {calls.push(phase); throw new Error(phase);}};
  let failure;
  try { await typeFor({node:{id:1},target:'world',options:{key:'KeyW',for:1},carrier,clock:()=>{throw new Error('unexpected clock');},tagged:r=>r}); }
  catch(error) {failure=error;}
  expect(calls).toEqual(['down','up']);
  expect(failure.message).toBe('down');
  expect(failure.steps.map(s=>s.error)).toEqual(['down','up']);
});
test('CLI held key syntax cannot capture an ordinary text suffix', () => {
  expect(typeArguments(['world','key','KeyW','for','1500'])).toEqual(['world',{key:'KeyW',for:1500}]);
  expect(typeArguments(['editor','hello','for','100'])).toEqual(['editor','hello for 100']);
  expect(typeArguments(['world','KeyW','for','100'])).toEqual(['world','KeyW for 100']);
});
test('proof receipts bind the web manifest and the actual app executable', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'g1b-receipt-')), bundle=resolve(dir,'Game.app');
  try {
    expect(artifactDigest('web',dir)).toBe(null);
    writeFileSync(resolve(dir,'exact.json'),'one');
    const web=artifactDigest('web',dir);
    writeFileSync(resolve(dir,'exact.json'),'two');
    expect(artifactDigest('web',dir)).not.toBe(web);
    writeFileSync(resolve(dir,'built-macos'),'');
    expect(artifactDigest('macos',dir,{bundle})).toBe(null);
    mkdirSync(resolve(bundle,'Contents/MacOS'),{recursive:true});
    const exe=resolve(bundle,'Contents/MacOS/ExactMac');
    writeFileSync(exe,'native one'); const mac=artifactDigest('macos',dir,{bundle});
    writeFileSync(exe,'native two'); expect(artifactDigest('macos',dir,{bundle})).not.toBe(mac);
    const binary=resolve(dir,'standalone');
    writeFileSync(binary,'carrier one'); const carrier=artifactDigest('macos',dir,{bundle,binary});
    writeFileSync(binary,'carrier two'); expect(artifactDigest('macos',dir,{bundle,binary})).not.toBe(carrier);
    rmSync(binary); expect(artifactDigest('macos',dir,{bundle,binary})).toBe(null);
    rmSync(bundle,{recursive:true}); expect(artifactDigest('macos',dir,{bundle})).toBe(null);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('inventory failure clears the timer before unconditional session cleanup', async () => {
  let polls=0; const monitor=setInterval(()=>polls++,1), calls=[];
  await closeSessions(monitor,()=>{throw new Error('ps failed');},[
    {close:async()=>{calls.push('first');throw new Error('close failed');}},
    {close:async()=>{await new Promise(r=>setTimeout(r,15));calls.push('second');}},
  ],(...args)=>calls.push(args[0]));
  expect(polls).toBe(0);
  expect(calls).toEqual(['first','session cleanup','second']);
});

for (const fails of [false, true]) test(`browser held key release survives canvas removal (clock failure=${fails})`, async () => {
  let canvas = true, down = false;
  const carrier = {input: async (id, _, opts) => browserKey({id, opts,
    evaluate: async () => { if (!canvas) throw new Error('canvas removed'); return true; },
    ask: async () => { if (!canvas) throw new Error('canvas removed'); return {ok:true}; },
    call: async (_, event) => { down=event.type==='keyDown'; }, frame: async () => {},
  })};
  let failure;
  try { await typeFor({node:{id:17},target:'world',options:{key:'KeyW',for:10},carrier,
    clock:async () => {canvas=false; if (fails) throw new Error('clock failed'); return {};},tagged:async r=>r}); }
  catch(error) {failure=error;}
  expect(down).toBe(false);
  if (fails) expect(failure.message).toBe('clock failed');
  else expect(failure).toBeUndefined();
});

test('native receipt changes with game dylibs and embedded plan/assets', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'g1c-receipt-')), bundle=resolve(dir,'Game.app');
  try {
    mkdirSync(resolve(bundle,'Contents/MacOS'),{recursive:true});
    mkdirSync(resolve(bundle,'Contents/Resources'),{recursive:true});
    writeFileSync(resolve(bundle,'Contents/MacOS/ExactMac'),'executable');
    for (const file of ['Contents/MacOS/libexact_gpu.dylib','Contents/MacOS/libexact_web.dylib','Contents/Resources/app.plan','Contents/Resources/texture.bin']) {
      writeFileSync(resolve(bundle,file),'before'); const before=artifactDigest('macos',dir,{bundle});
      writeFileSync(resolve(bundle,file),'after'); expect(artifactDigest('macos',dir,{bundle})).not.toBe(before);
    }
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('held key receipts await asynchronous tags on success and failure', async () => {
  for (const fails of [false, true]) {
    const carrier = {input: async (_, __, {phase}) => ({phase, delivery:'recognized'})};
    const tagged = async reply => { await Promise.resolve(); return {...reply, epoch:7}; };
    let result;
    try {
      result = await typeFor({node:{id:17},target:'world',options:{key:'KeyW',for:10},carrier,tagged,
        clock:async () => {if (fails) throw new Error('clock failed'); return {now:10};}});
    } catch (error) { result = error; }
    const steps = JSON.parse(JSON.stringify(result.steps));
    expect(steps[0].reply).toMatchObject({epoch:7,phase:'down',delivery:'recognized'});
    expect(steps[2].reply).toMatchObject({epoch:7,phase:'up',delivery:'recognized'});
    if (!fails) expect(result.delivery).toBe('recognized');
  }
});
test('web receipts bind every dist path and byte in deterministic order', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'g1d-web-'));
  try {
    writeFileSync(resolve(dir,'exact.json'),'manifest');
    mkdirSync(resolve(dir,'assets'));
    writeFileSync(resolve(dir,'assets/texture.bin'),'one');
    const original=artifactDigest('web',dir);
    rmSync(resolve(dir,'exact.json'));
    writeFileSync(resolve(dir,'exact.json'),'manifest'); // Opposite creation order, identical manifest.
    expect(artifactDigest('web',dir)).toBe(original);
    writeFileSync(resolve(dir,'assets/texture.bin'),'two');
    expect(artifactDigest('web',dir)).not.toBe(original);
    writeFileSync(resolve(dir,'assets/texture.bin'),'one');
    expect(artifactDigest('web',dir)).toBe(original);
    writeFileSync(resolve(dir,'gpu_bg.wasm'),'module');
    expect(artifactDigest('web',dir)).not.toBe(original);
    rmSync(resolve(dir,'gpu_bg.wasm'));
    expect(artifactDigest('web',dir)).toBe(original);
    rmSync(resolve(dir,'assets/texture.bin'));
    writeFileSync(resolve(dir,'assets/renamed.bin'),'one');
    expect(artifactDigest('web',dir)).not.toBe(original);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

for (const fails of [false, true]) test(`native held key owns release after canvas removal (clock failure=${fails})`, async () => {
  let canvas=true, down=false, focused=0;
  const releases=new Map();
  const ask=async request => {
    if (request.releaseKey && request.phase === 'up') {
      const release=releases.get(request.releaseKey);
      expect(release).toBeDefined();
      releases.delete(request.releaseKey);
      release();
      return {phase:'up',delivery:'recognized'};
    }
    if (!canvas) return {error:'canvas removed'};
    focused++;
    down=request.phase === 'down';
    if (request.releaseKey) releases.set(request.releaseKey, () => {down=false;});
    return {phase:request.phase,delivery:'recognized'};
  };
  const carrier={input:async (id, _, opts) => nativeKey({id,opts,ask})};
  let failure;
  try { await typeFor({node:{id:17},target:'world',options:{key:'KeyW',for:10},carrier,tagged:async r=>r,
    clock:async () => {canvas=false; if (fails) throw new Error('clock failed'); return {};}}); }
  catch(error) {failure=error;}
  expect(down).toBe(false);
  expect(focused).toBe(1);
  expect(releases.size).toBe(0);
  if (fails) expect(failure.message).toBe('clock failed');
  else expect(failure).toBeUndefined();
});

test('native receipt cache misses when only the standalone game dylib changes', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'g1d-native-')), bundle=resolve(dir,'Game.app'), products=resolve(dir,'products');
  try {
    mkdirSync(resolve(bundle,'Contents/MacOS'),{recursive:true});
    mkdirSync(products);
    writeFileSync(resolve(bundle,'Contents/MacOS/ExactMac'),'bundled executable');
    const binary=resolve(products,'ExactMac'), dylib=resolve(products,'libgreybox_gpu.dylib');
    writeFileSync(binary,'standalone executable');
    writeFileSync(dylib,'game before');
    const artifacts={bundle,binary,products};
    const stamp=() => JSON.stringify({inputs:'unchanged sources',artifact:artifactDigest('macos',dir,artifacts)});
    const cached=stamp();
    expect(stamp()).toBe(cached);
    writeFileSync(dylib,'game after');
    expect(stamp()).not.toBe(cached);
    rmSync(dylib);
    expect(stamp()).not.toBe(cached);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('proof equality compares complete nested objects independently of key order', () => {
  expect(equal({Mesh:{Capsule:{radius:0.4,height:1.8}}, rows:[{a:1,b:null},2]},
    {rows:[{b:null,a:1},2], Mesh:{Capsule:{height:1.8,radius:0.4}}})).toBe(true);
  for (const [a,b] of [
    [{Capsule:{radius:0.4,height:1.8}}, {Capsule:{radius:0.4,height:1.8,extra:0}}],
    [{a:null}, {}], [[1,2], [2,1]], [[1], [1,2]], [[], {}], [null, {}], [1, '1'],
  ]) expect(equal(a,b)).toBe(false);
  expect(equal([null,true,{a:[]}], [null,true,{a:[]}])).toBe(true);
});


test('world convenience keeps simulation fields only and dispatches the existing operations', async () => {
  const calls = [], entities = [{name:'player', components:{Transform:{position:[0,0.9,0]}}}];
  let clock=0, epoch=1, incarnation=1;
  const raw = {
    world(name) { return worldView(this, name); },
    async clock(value) { return {settled:value === 'settle'}; },
    async state(target) { if (target === 'arena:missing') throw Object.assign(new Error('state: no entity named `missing`'), {reply:{tick:90,error:'no entity named `missing`'}}); return {entity: target === 'arena:player' ? entities[0] : undefined, tick:90, hash:'0x123', entities, truncated:false, clock, epoch, incarnation}; },
    async type(...args) { return {clock, epoch, incarnation, args}; },
    async screenshot(...args) { return {clock, epoch, incarnation, args}; },
  };
  // Like proof's proxy, every underlying operation is recorded with its full reply.
  const session = new Proxy(raw, {get(target, method) {
    if (method === 'world') return target[method];
    return async (...args) => { try { const reply=await target[method](...args); calls.push({method,args,reply}); return reply; } catch (error) { calls.push({method,args,reply:error.reply}); throw error; } };
  }});
  const w=session.world('arena'), before=await w.snapshot();
  clock+=2000;
  expect(await w.snapshot()).toEqual(before);
  clock=0; epoch++; incarnation++;
  expect(await w.snapshot()).toEqual(before);
  expect(Object.keys(before)).toEqual(['tick','hash','entities','truncated']);
  expect(before).toEqual({tick:90,hash:'0x123',entities,truncated:false});
  await w.state('player'); await w.save('checkpoint.world');
  await w.key_down('KeyW'); await w.tap('KeyE'); await w.hold('KeyW',1500);
  await w.key_up('KeyW'); await w.run(100);
  expect(await w.settle()).toBe(true);
  expect(await w.position('player')).toEqual([0,0.9,0]);
  expect(await w.get('player','Transform')).toEqual({position:[0,0.9,0]});
  expect(await w.position('missing')).toBeUndefined();
  expect(await w.get('player','Missing')).toBeUndefined();
  for (const ms of [-1, NaN, Infinity]) expect(() => w.run(ms)).toThrow();
  expect(calls.map(c => [c.method,...c.args])).toEqual([
    ['state','arena:*'], ['state','arena:*'], ['state','arena:*'], ['state','arena:player'],
    ['screenshot','checkpoint.world','arena','save'], ['type','arena',{key:'KeyW',phase:'down'}],
    ['type','arena',{key:'KeyE'}], ['type','arena',{key:'KeyW',for:1500}],
    ['type','arena',{key:'KeyW',phase:'up'}], ['clock','+0'], ['clock','+100'], ['clock','settle'],
    ['state','arena:player'], ['state','arena:player'], ['state','arena:missing'], ['state','arena:player'],
  ]);
  expect(calls[1].reply).toMatchObject({clock:2000,epoch:1,incarnation:1});
  expect(calls[2].reply).toMatchObject({clock:0,epoch:2,incarnation:2});
});


test('author examples and documentation describe current motion, placement and parity pins', () => {
  const read = path => readFileSync(resolve(import.meta.dir, path), 'utf8');
  const main = read('README.md'), engine = read('engine/README.md'), greybox = read('games/greybox/README.md');
  for (const source of [main, read('new/logic/src/lib.rs')]) {
    const setup = source.slice(source.indexOf('fn setup'), source.indexOf('fn paused'));
    expect(setup).not.toContain('scene::follow');
    expect(source).toContain('Character');
    expect(source).toContain('nearest_xz::<Beacon>');
  }
  expect(main).not.toContain('Call it at the\nend of `setup`');
  expect(engine).not.toContain('stepped explicitly by `scene::follow`');
  expect(greybox).not.toContain('math::ease');
  const proof = read('games/greybox/proof.mjs');
  for (const pin of ['0x7544ef30a82fdcdc', '0xa655423c9a442bce', '[0, 0.9, -5.3666644]']) {
    expect(proof).toContain(pin);
    expect(greybox).toContain(pin);
  }
});

 test('world get translates only the named missing-entity refusal', async () => {
   for (const error of ['no view matches arena', 'no entity named `other`', 'device lost']) {
     const w = worldView({state: async () => {throw Object.assign(new Error(`state: ${error}`), {reply:{tick:0,error}});}}, 'arena');
     await expect(w.get('missing', 'Transform')).rejects.toThrow(error);
   }
   const failure = new Error('no entity named `missing`');
   await expect(worldView({state: async () => {throw failure;}}, 'arena').get('missing', 'Transform')).rejects.toBe(failure);
 });

test('queue no longer lists the repaired Beacons designed-defaults fixture', () => {
  expect(readFileSync(resolve(import.meta.dir, '../QUEUE.md'), 'utf8')).not.toContain('`render/tests/world.rs::beacons_designed_defaults` still expects');
});
