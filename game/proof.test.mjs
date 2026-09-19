import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {agreePins, webUnavailable, pinRecorder, facilityReport, artifactDigest, closeSessions, equal, paranoidRuns, buildInputHash, ensureBuildReceipt} from './proof.mjs';
import {checkSteadyResidency} from './games/asset-fixture/residency.mjs';
import {typeArguments, typeFor, browserKey, nativeKey, render, worldView, tapRefusal} from '../scripts/agent.mjs';

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
  expect(await w.local_position('player')).toEqual([0,0.9,0]);
  expect(await w.get('player','Transform')).toEqual({position:[0,0.9,0]});
  expect(await w.local_position('missing')).toBeUndefined();
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


 test('world get translates only the named missing-entity refusal', async () => {
   for (const error of ['no view matches arena', 'no entity named `other`', 'device lost']) {
     const w = worldView({state: async () => {throw Object.assign(new Error(`state: ${error}`), {reply:{tick:0,error}});}}, 'arena');
     await expect(w.get('missing', 'Transform')).rejects.toThrow(error);
   }
   const failure = new Error('no entity named `missing`');
   await expect(worldView({state: async () => {throw failure;}}, 'arena').get('missing', 'Transform')).rejects.toBe(failure);
 });

for (const failure of ['none', 'save', 'fresh-throw', 'off-before-receipt']) test(`paranoid receipt lifecycle: ${failure}`, async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'r5-receipt-')), dist = resolve(dir, 'dist');
  const receipt = resolve(dir, 'build-web.sha256'), modes = [];
  mkdirSync(dist);
  const inputs = mode => buildInputHash('web', 'target', mode).update('source bytes').digest('hex');
  let bakes = 0;
  const ordinary = (mode, die = false) => ensureBuildReceipt({receipt, inputs:inputs(mode),
    artifact:() => artifactDigest('web', dist), build:async () => {
      bakes++;
      writeFileSync(resolve(dist, 'exact.json'), '{"module":"gpu_bg.wasm"}');
      writeFileSync(resolve(dist, 'gpu_bg.wasm'), `wasm compiled with ${mode}`);
      if (die) throw new Error('child died before receipt');
    }});
  try {
    await ordinary('fresh-game');
    expect(await ordinary('0')).toBe(true); // Ordinary rejects a real paranoid receipt.
    expect(bakes).toBe(2);
    const failed = await paranoidRuns(async mode => {
      modes.push(mode);
      await ordinary(mode);
      if (failure === 'fresh-throw' && mode === 'fresh-game') throw new Error('proof child threw');
      return failure === 'save' && mode === '1' ? 1 : 0;
    }, async () => { await ordinary('0', failure === 'off-before-receipt'); return 0; });
    expect(modes).toEqual(['0', '1', 'fresh-game']);
    expect(failed).toBe(failure !== 'none');
    const stamp = JSON.parse(readFileSync(receipt, 'utf8'));
    expect(stamp.inputs).toBe(inputs(failure === 'off-before-receipt' ? 'fresh-game' : '0'));
    const before = bakes;
    expect(await ordinary('0')).toBe(failure === 'off-before-receipt');
    expect(bakes - before).toBe(failure === 'off-before-receipt' ? 1 : 0);
    expect(JSON.parse(readFileSync(receipt, 'utf8'))).toEqual({inputs:inputs('0'), artifact:artifactDigest('web', dist)});
    expect(readFileSync(resolve(dist, 'gpu_bg.wasm'), 'utf8')).toBe('wasm compiled with 0');
    expect(buildInputHash('linux', 'target', '0').digest('hex'))
      .toBe(buildInputHash('linux', 'target', 'fresh-game').digest('hex'));
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('steady residency skips no-device worlds and asserts only device-backed work', () => {
  const checks = [], lines = [], check = (...args) => checks.push(args), say = line => lines.push(line);
  const gpu = {afterReady:{textureUploads:0, meshUploads:0, pipelineCreations:0, modelSkinBufferReallocations:0}};
  checkSteadyResidency({device:false, ready:false, gpu}, check, say, "linux");
  expect(checks).toEqual([]);
  expect(lines).toEqual(['SKIP: no device — after-ready GPU residency']);
  checkSteadyResidency({device:true, ready:true, gpu}, check, say);
  expect(checks.at(-1)[1]).toBe(true);
  checkSteadyResidency({device:true, ready:false, gpu}, check, say);
  expect(checks.at(-1)[1]).toBe(false);
  checkSteadyResidency({device:true, ready:true, gpu:{afterReady:{textureUploads:1}}}, check, say);
  expect(checks.at(-1)[1]).toBe(false);
});

 test('inventory parses both lstart day widths and ignores zombies', async () => {
  const {parseInventoryLine} = await import('./proof.mjs');
  for (const stamp of ['Tue Sep  8 12:34:56 2026','Fri Sep 18 12:34:56 2026']) {
    expect(parseInventoryLine(` 123 45 S ${stamp} /path/app --flag`)).toEqual({pid:123,parent:45,stamp,command:'/path/app --flag'});
    expect(parseInventoryLine(` 123 45 Z+ ${stamp} <defunct>`)).toBeNull();
  }
});
 test('only linux may skip missing GPU residency', () => {
  for (const host of ['web','macos','ios']) for (const device of [false,undefined]) {
    const checks=[], lines=[];
    checkSteadyResidency({device,ready:false,gpu:{afterReady:{}}},(...args)=>checks.push(args),line=>lines.push(line),host);
    expect(lines).toEqual([]);
    expect(checks[0][1]).toBe(false);
  }
});

test('explicit focus in a commit precedes autofocus and its authored side effects', () => {
  const source=readFileSync(resolve(import.meta.dir,'../host/web/glue.js'),'utf8');
  const code=source.slice(source.indexOf('  // Newly mounted autofocus'),source.indexOf('  positionContexts();\n  return batch.timers;'));
  const calls=[], explicit={id:'chosen',isConnected:true,matches:()=>false,getClientRects:()=>[{}],focus:()=>{calls.push('explicit');document.activeElement=explicit;}};
  const document={activeElement:null};
  new Function('focusAutofocus','focusCommands','inputReady','root','inertAncestor','getComputedStyle','log','document',code)(
    ()=>{if(!document.activeElement) calls.push('autofocus side effect');},[{args:['chosen']}],true,{querySelectorAll:()=>[explicit]},()=>false,()=>({visibility:'visible'}),()=>{},document);
  expect(calls).toEqual(['explicit']);
});

test('KeyP forbids texture uploads and pipeline creation as well as requiring new geometry', async () => {
  const {checkResidency}=await import('./games/asset-fixture/residency.mjs');
  const state=(textureUploads=0,meshUploads=0,pipelineCreations=0)=>({ready:true,gpu:{afterReady:{textureUploads,meshUploads,pipelineCreations,modelSkinBufferReallocations:0}}});
  for (const error of ['none','texture','pipeline']) {
    const checks=[], responses=[{before:state(),after:state()}, {before:state(),after:state(1)}, {before:state(1),after:state(1)},
      {before:state(1),after:state(error==='texture'?2:1,1,error==='pipeline'?1:0)}];
    await checkResidency({run:async()=>responses.shift()}, {}, (name,ok)=>checks.push([name,ok]), ()=>{});
    expect(checks.find(([name])=>name==='new model name reuses textures and pipelines')[1]).toBe(error==='none');
  }
});


test('local and global position helpers preserve parent-space distinction', async () => {
  const w = worldView({
    async state() { return {entity:{components:{Transform:{position:[1,2,3]}}}}; },
    async layout() { return {entity:{world:{position:[11,2,3]}}}; },
  }, 'arena');
  expect(await w.local_position('child')).toEqual([1,2,3]);
  expect(await w.global_position('child')).toEqual([11,2,3]);
  expect(w.position).toBeUndefined();
});

const candidates = (hosts = ['linux','web']) => hosts.flatMap(host => ['0','1','fresh-game'].map(mode => ({
  name:'fixture', host, mode, failures:[], pins:{ticks:{60:'0x123456789abcdef0'}, saves:{continuation:'a'.repeat(64)}},
})));
test('repin requires all modes and hosts to agree on every tick and save', () => {
  const rows=candidates(), old=structuredClone(rows[0].pins);
  expect(agreePins(rows, old, ['linux','web'])).toEqual({...old,hosts:['linux','web']});
  for (const section of ['ticks','saves']) {
    const bad=structuredClone(rows), key=Object.keys(bad[4].pins[section])[0];
    bad[4].pins[section][key]=section==='ticks'?'0x1111111111111111':'b'.repeat(64);
    expect(()=>agreePins(bad,old,['linux','web'])).toThrow(`web 1 ${section} ${key}`);
    expect(rows[0].pins).toEqual(old);
  }
  expect(()=>agreePins(rows.slice(1),old,['linux','web'])).toThrow('linux 0 missing');
  expect(()=>agreePins(rows,{...old,ticks:{...old.ticks,90:'0x123456789abcdef0'}},['linux','web'])).toThrow('did not observe ticks 90');
});
test('no-web repin records only linux and still requires three modes', () => {
  const rows=candidates(['linux']), old=rows[0].pins;
  expect(agreePins(rows,old,['linux']).hosts).toEqual(['linux']);
  expect(()=>agreePins(rows,old,['linux','web'])).toThrow('web 0 missing');
  expect(()=>agreePins(rows.slice(0,2),old,['linux'])).toThrow('linux fresh-game missing');
});
test('pin failure gives the one regeneration command; collection bypasses only old pins', () => {
  const calls=[], old={ticks:{60:'0x123456789abcdef0'},saves:{}};
  const normal=pinRecorder(old,'fixture',(...args)=>calls.push(args));
  normal.pin(60,{tick:60,hash:'0x1111111111111111'});
  expect(calls.at(-1)).toEqual(['pin 60 differs (expected 0x123456789abcdef0, got 0x1111111111111111); if the change is intended: bun game/prove.mjs fixture --repin',false]);
  const collecting=pinRecorder(old,'fixture',(...args)=>calls.push(args),true);
  collecting.pin(60,{tick:59,hash:'0x1111111111111111'});
  expect(calls.at(-1)[1]).toBe(false);
  collecting.pin(60,{tick:60,hash:'0x2222222222222222'});
  expect(calls.at(-1)).toEqual(['pin 60 repeated consistently',false]);
});
test('facility report connects observed stalls/refusals to unused operations', () => {
  expect(facilityReport([{method:'clock',reply:{settled:false}},{method:'tap',error:'hidden behind camera'}]).join(' ')).toContain('layout unused');
  expect(facilityReport([{method:'layout'},{method:'tap',error:'hidden behind camera'}]).join(' ')).not.toContain('layout unused');
  expect(facilityReport([{method:'clock',reply:{settled:true}}])).toEqual([]);
});

test('hidden placed-child refusal uses observed camera visibility and names layout', async () => {
  const calls=[], s={tree:async target=>target ? {entities:[{name:'sign'}]} : {nodes:[{id:7,props:{testId:'world'},world:{}}]},
    state:async name=>{calls.push(name);return {entity:{placed:{hidden:true}}};},
    layout:async name=>({entity:{visible:{behindCamera:true}}})};
  const error=await tapRefusal(s,'sign',new Error('no view matches sign'));
  expect(error.message).toContain('hidden (behind the camera): layout world:sign shows the placed box');
  expect(calls).toEqual(['world:sign']);
});

test('automatic no-web fallback is only a missing configured browser, never a failed proof', () => {
  expect(webUnavailable('web carrier unavailable: /missing/chrome: ENOENT; set CHROME to an installed browser')).toBe(true);
  for (const log of ['asset ENOENT', 'Chrome exited (1)', 'web carrier unavailable: chrome: EACCES;', 'FAIL web pixel assertion']) expect(webUnavailable(log)).toBe(false);
});

test('tap diagnostics never manufacture a missing-entity refusal for a UI-only target', async () => {
  let queried=0;
  const s={tree:async target=>target?{entities:[{name:'sign'}]}:{nodes:[{id:7,world:{}}]},
    state:async()=>{queried++;throw new Error('must not query an unobserved name');}};
  const error=await tapRefusal(s,'play',new Error('restore refused'));
  expect(error.message).toStartWith('restore refused; layout play');
  expect(queried).toBe(0);
});
