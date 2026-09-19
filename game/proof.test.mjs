import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {agreePins, webUnavailable, pinRecorder, proofStatus, facilityReport, artifactDigest, closeSessions, equal, paranoidRuns, buildInputHash, ensureBuildReceipt, proofInputFiles} from './proof.mjs';
import {checkSteadyResidency} from './proof.mjs';
import {comparePlacement} from './games/placement-fixture/proof.mjs';
import {typeArguments, typeFor, browserKey, nativeKey, render, worldView, tapRefusal, assertWebDistApp} from '../scripts/agent.mjs';

test('external app sources and assets invalidate receipts while docs and outputs stay excluded', () => {
  const directory = mkdtempSync(resolve(tmpdir(), 'external-proof-inputs-'));
  const root = resolve(directory, 'engine'), app = resolve(directory, 'my-game');
  try {
    for (const file of ['engine/game/engine/src/lib.rs', 'engine/game/README.md',
      'engine/game/engine/README.md', 'my-game/logic/src/lib.rs',
      'my-game/app.contract', 'my-game/Cargo.toml', 'my-game/Cargo.lock',
      'my-game/art/model.glb', 'my-game/assets/texture.png', 'my-game/deck/image.bin',
      'my-game/artifacts/replies.json', 'my-game/dist.previous/module.wasm',
      'my-game/.shells/host/src/lib.rs', 'my-game/target/build.rs', 'my-game/proof.mjs']) {
      const path = resolve(directory, file);
      mkdirSync(resolve(path, '..'), {recursive:true}); writeFileSync(path, file);
    }
    const files = proofInputFiles(root, app);
    expect(files).toEqual(['../my-game/Cargo.lock', '../my-game/Cargo.toml',
      '../my-game/app.contract', '../my-game/art/model.glb', '../my-game/assets/texture.png',
      '../my-game/deck/image.bin', '../my-game/logic/src/lib.rs', 'game/engine/src/lib.rs']);
    const digest = () => {
      const hash = buildInputHash('linux', 'target');
      for (const file of proofInputFiles(root, app)) hash.update(file).update(readFileSync(resolve(root, file)));
      return hash.digest('hex');
    };
    const before = digest();
    writeFileSync(resolve(app, 'logic/src/lib.rs'), 'changed game source');
    expect(digest()).not.toBe(before);
    const after = digest();
    writeFileSync(resolve(app, 'artifacts/replies.json'), 'new proof output');
    expect(digest()).toBe(after);
    writeFileSync(resolve(root, 'game/README.md'), 'changed game guide');
    expect(digest()).toBe(after);
  } finally { rmSync(directory, {recursive:true, force:true}); }
});

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
  const {checkResidency}=await import('./proof.mjs');
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

const syntheticHash = '0x' + '12345678' + '9abcdef0';
const repeatedHash = digit => '0x' + digit.repeat(16);
const candidates = (hosts = ['linux','web']) => hosts.flatMap(host => ['0','1','fresh-game'].map(mode => ({
  name:'fixture', host, mode, failures:[], pins:{ticks:{60:syntheticHash}, saves:{continuation:'a'.repeat(64)}},
})));
test('repin requires all modes and hosts to agree on every tick and save', () => {
  const rows=candidates(), old=structuredClone(rows[0].pins);
  expect(agreePins(rows, old, ['linux','web'])).toEqual({...old,hosts:['linux','web']});
  for (const section of ['ticks','saves']) {
    const bad=structuredClone(rows), key=Object.keys(bad[4].pins[section])[0];
    bad[4].pins[section][key]=section==='ticks'?repeatedHash('1'):'b'.repeat(64);
    expect(()=>agreePins(bad,old,['linux','web'])).toThrow(`web 1 ${section} ${key}`);
    expect(rows[0].pins).toEqual(old);
  }
  expect(()=>agreePins(rows.slice(1),old,['linux','web'])).toThrow('linux 0 missing');
  expect(()=>agreePins(rows,{...old,ticks:{...old.ticks,90:syntheticHash}},['linux','web'])).toThrow('did not observe ticks 90');
});
test('no-web repin records only linux and still requires three modes', () => {
  const rows=candidates(['linux']), old=rows[0].pins;
  expect(agreePins(rows,old,['linux']).hosts).toEqual(['linux']);
  expect(()=>agreePins(rows,old,['linux','web'])).toThrow('web 0 missing');
  expect(()=>agreePins(rows.slice(0,2),old,['linux'])).toThrow('linux fresh-game missing');
});
test('pin failure gives the one regeneration command; collection bypasses only old pins', () => {
  const calls=[], old={ticks:{60:syntheticHash},saves:{}};
  const normal=pinRecorder(old,'fixture',(...args)=>calls.push(args));
  normal.pin(60,{tick:60,hash:repeatedHash('1')});
  expect(calls.at(-1)).toEqual([`pin 60 differs (expected ${syntheticHash}, got ${repeatedHash('1')}); if the change is intended: bun game/prove.mjs fixture --repin`,false]);
  expect(proofStatus({failures:calls.filter(([,ok])=>!ok),expected:old,pins:normal.pins})).toBe('FAIL');
  const collecting=pinRecorder(old,'fixture',(...args)=>calls.push(args),true);
  collecting.pin(60,{tick:59,hash:repeatedHash('1')});
  expect(calls.at(-1)[1]).toBe(false);
  collecting.pin(60,{tick:60,hash:repeatedHash('2')});
  expect(calls.at(-1)).toEqual(['pin 60 repeated consistently',false]);
});
test('proof success requires both saved baselines and complete observations', () => {
  const pins = {ticks:{60:syntheticHash}, saves:{continuation:'a'.repeat(64)}};
  const checked = {failures:[], expected:structuredClone(pins), pins};
  expect(proofStatus(checked)).toBe('PASS');
  expect(proofStatus({...checked, expected:{}})).toBe('UNVERIFIED');
  for (const section of ['ticks','saves']) {
    expect(proofStatus({...checked, expected:{...pins,[section]:{}}})).toBe('UNVERIFIED');
    expect(proofStatus({...checked, pins:{...pins,[section]:{}}})).toBe('UNVERIFIED');
  }
  expect(proofStatus({...checked, collecting:true})).toBe('UNVERIFIED');
  expect(proofStatus({...checked, partial:true})).toBe('UNVERIFIED');
  expect(proofStatus({...checked, failures:['gameplay assertion']})).toBe('FAIL');
});
test('facility report connects observed stalls/refusals to unused operations', () => {
  expect(facilityReport([{method:'clock',reply:{settled:false}},{method:'tap',args:['sign'],error:'hidden behind camera'}]).join(' ')).toContain('layout unused');
  expect(facilityReport([{method:'tap',args:['sign'],error:'hidden behind camera'},{method:'layout',args:['sign'],reply:{}}]).join(' ')).not.toContain('layout unused');
  expect(facilityReport([{method:'clock',reply:{settled:true}}])).toEqual(['no recorded stalls or refusals']);
});

test('hidden placed-child refusal uses observed camera visibility and names layout', async () => {
  const calls=[], s={tree:async target=>target ? {entities:[{name:'sign'}]} : {nodes:[{id:7,props:{testId:'world'},world:{}}]},
    state:async name=>{calls.push(name);return {entity:{placed:{hidden:true}}};},
    layout:async name=>({entity:{visible:{behindCamera:true}}})};
  const error=await tapRefusal(s,'sign',new Error('no view matches sign'));
  expect(error.message).toContain('hidden (behind the camera): `layout world:sign` (with --json before the quoted operation) shows visibility');
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
  expect(error.message).toBe('restore refused');
  expect(queried).toBe(0);
});


test('layout CLI names behind-camera and unavailable projection without undefined coordinates', () => {
  const text=render('layout',{entity:{name:'sign',screen:{unavailable:true},visible:{behindCamera:true,inFrustum:false}}});
  expect(text).toContain('behindCamera'); expect(text).toContain('screen unavailable'); expect(text).not.toContain('undefined');
});
test('facility use must succeed and answer the relevant refusal', () => {
  const failed={method:'tap',args:['sign'],error:'sign is hidden (behind the camera)'};
  for(const call of [{method:'layout',args:['other'],reply:{}},{method:'layout',args:['sign'],error:'unavailable'}])
    expect(facilityReport([failed,call]).join(' ')).toContain('layout unused');
  expect(facilityReport([failed,{method:'layout',args:['world:sign'],reply:{entity:{}}}]).join(' ')).not.toContain('layout unused');
  expect(facilityReport([{method:'tap',args:['hitbox'],error:'restore refused'}]).join(' ')).not.toContain('layout');
  expect(facilityReport([{method:'tap',error:'assets pending'},{method:'state',args:['world:player'],reply:{}}]).join(' ')).toContain('state unused');
  expect(facilityReport([{method:'clock',reply:{settled:true}}])).toEqual(['no recorded stalls or refusals']);
});


test('stale-build repair command names the rejected web dist', () => {
  const dist=mkdtempSync(resolve(tmpdir(),'r8b-dist-'));
  try { expect(()=>assertWebDistApp(dist,{id:'com.test',dir:'/app',crate:()=> 'test-web'})).toThrow(`EXACT_WEB_DIST='${dist}'`); }
  finally { rmSync(dist,{recursive:true,force:true}); }
});
test('repin refuses manifest normalization before writing authored files', async () => {
  const {gameDefaults}=await import('./app/shells.mjs');
  const dir=mkdtempSync(resolve(tmpdir(),'r8b-manifest-'));
  const before=process.env.EXACT_PROOF_REPIN;
  try {
    mkdirSync(resolve(dir,'logic/src'),{recursive:true});
    writeFileSync(resolve(dir,'logic/src/lib.rs'),`impl Game for Test { const ID: &'static str = "fixture"; }`);
    writeFileSync(resolve(dir,'app.json'),'{}');
    process.env.EXACT_PROOF_REPIN='1';
    expect(()=>gameDefaults(dir)).toThrow('repin refused: manifest normalization would write');
    expect(readFileSync(resolve(dir,'app.json'),'utf8')).toBe('{}');
  } finally { if(before===undefined) delete process.env.EXACT_PROOF_REPIN; else process.env.EXACT_PROOF_REPIN=before; rmSync(dir,{recursive:true,force:true}); }
});


test('direct placement comparison rejects two hosts passing a two-pixel oracle', () => {
  const a={initial:{tick:0,x:0,y:0,w:10,h:10},moving:{tick:60,x:5,y:5,w:10,h:10}}, b=structuredClone(a);
  expect(comparePlacement(a,b)).toBe(true);
  b.moving.x+=1.31;
  expect(()=>comparePlacement(a,b)).toThrow('placement parity moving.x');
  b.moving.x=a.moving.x+0.5;
  expect(comparePlacement(a,b)).toBe(true);
});

for (const scenario of ['report','repin', ...['ordinary','repeat','cwd','failure','UNVERIFIED','PASS'].map(command => `external-report-${command}`)]) test(`prove retains refused summaries and refuses missing requested repin hosts (${scenario})`, async () => {
  const name=`r8b-tooling-${process.pid}`;
  // A sibling checkout is external without Bun's expensive /tmp ancestor search.
  const directory = scenario.startsWith('external-report-') ? mkdtempSync(resolve(import.meta.dir, '../../prove external-')) : null;
  const app=resolve(directory ?? resolve(import.meta.dir,'games'),name);
  const pins={ticks:{1:syntheticHash},saves:{continuation:'a'.repeat(64)}};
  mkdirSync(app);
  try {
    writeFileSync(resolve(app,'pins.json'),JSON.stringify(pins));
    writeFileSync(resolve(app,'proof.mjs'),`
      import {appendFileSync,mkdirSync,writeFileSync} from 'node:fs';
      export function compare(rows) { if (rows.length !== 2) throw new Error('missing comparison rows'); console.log('COMPARE generic authored proof'); }
      if (import.meta.main) {
      const out=process.env.EXACT_PROOF_OUT, host=process.argv[2];
      appendFileSync(${JSON.stringify(resolve(app,'calls.jsonl'))},JSON.stringify({host,build:process.argv.includes('--build-only'),mode:process.env.EXACT_GAME_PARANOID})+'\\n');
      mkdirSync(out,{recursive:true});
      const failed=!process.argv.includes('--build-only') && (process.env.R8B_FAIL==='1' || host==='web' && process.env.R8B_PASS_WEB!=='1');
      const status=failed?'FAIL':process.env.R8B_UNVERIFIED==='1'||process.env.R8B_UNVERIFIED_HOST===host||process.env.EXACT_PROOF_REPIN==='1'?'UNVERIFIED':'PASS';
      const row={name:${JSON.stringify(name)},host,status,mode:process.env.EXACT_GAME_PARANOID,pins:${JSON.stringify(pins)},failures:failed?['refusal']:[],facilities:failed?['state unused; pending assets']:['no recorded stalls or refusals'],seconds:0,worlds:[{session:1,tick:1,hash:'same'}],saves:[{name:'a',sha256:'same'}]};
      writeFileSync(out+'/summary.json',JSON.stringify(row));
      if(host==='web' && process.env.R8B_PASS_WEB!=='1') console.error('web carrier unavailable: /missing/chrome: ENOENT; set CHROME');
      process.exit(failed?1:0);
      }
    `);
    const run=async(args,extra={},current=false)=>{
      writeFileSync(resolve(app,'calls.jsonl'),'');
      const p=Bun.spawn([process.execPath,resolve(import.meta.dir,'prove.mjs'),current ? '.' : directory ? app : name,...args],{cwd:current ? app : undefined,env:{...process.env,...extra},stdout:'pipe',stderr:'pipe'});
      const [code,stdout,stderr]=await Promise.all([p.exited,new Response(p.stdout).text(),new Response(p.stderr).text()]);
      const calls=readFileSync(resolve(app,'calls.jsonl'),'utf8').trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));
      return {code,text:stdout+stderr,calls};
    };
    if (scenario === 'report' || directory) {
    const selected = command => !directory || scenario === `external-report-${command}`;
    if (selected('ordinary')) {
    const ordinary=await run(['--report']);
    expect(ordinary.code).toBe(0);
    expect(ordinary.calls).toEqual([{host:'linux',build:false,mode:'0'}]);
    expect(ordinary.text).toContain('REPORT linux 0: no recorded stalls or refusals');
    expect(JSON.parse(readFileSync(resolve(app,'artifacts/prove/summary.json'),'utf8')).rows.map(row=>row.host)).toEqual(['linux']);
    }
    if (selected('repeat')) {
    const repeated=await run(['--repeat','2']);
    expect(repeated.code).toBe(0);
    expect(repeated.calls).toEqual(Array(2).fill({host:'linux',build:false,mode:'0'}));
    }
    if (directory && selected('cwd')) {
      const here=await run(['--report'],{},true);
      expect(here.code).toBe(0);
      expect(here.calls).toEqual([{host:'linux',build:false,mode:'0'}]);
    }
    if (selected('failure')) {
    const started = performance.now();
    const failed=await run(['--hosts','linux','--report'],{R8B_FAIL:'1'});
    // A fake external proof needs neither Cargo metadata nor Bun's external entrypoint search.
    if (directory) expect(performance.now() - started).toBeLessThan(1000);
    expect(failed.code).toBe(1); expect(failed.text).toContain('REPORT linux 0: state unused');
    const summary=JSON.parse(readFileSync(resolve(app,'artifacts/prove/summary.json'),'utf8'));
    expect(summary.rows.length).toBe(1); expect(summary.rows[0].failures).toEqual(['refusal']);
    expect(summary.status).toBe('FAIL');
    }
    for (const status of ['UNVERIFIED','PASS'].filter(selected)) {
      const completed=await run(['--hosts','linux','--report','--compare-saves'],{R8B_UNVERIFIED:status==='UNVERIFIED'?'1':'0'});
      expect(completed.code).toBe(status === 'PASS' ? 0 : 1);
      expect(completed.text).toContain(`| identical | ${status} |`);
      expect(completed.text).toContain(`PROOF ${status} ${name}`);
      expect(JSON.parse(readFileSync(resolve(app,'artifacts/prove/summary.json'),'utf8')).status).toBe(status);
      if (status === 'UNVERIFIED') expect(completed.text).toContain('No complete tick/save baseline was checked. Generate it with bun game/prove.mjs');
      expect(JSON.parse(readFileSync(resolve(app,'pins.json'),'utf8'))).toEqual(pins);
    }
    if (scenario === 'report') {
      const web=await run(['--hosts','web'],{R8B_PASS_WEB:'1'});
      expect(web.code).toBe(0);
      expect(web.calls).toEqual([{host:'web',build:false,mode:'0'}]);
      const compared=await run(['--compare-saves'],{R8B_PASS_WEB:'1'});
      expect(compared.code).toBe(0);
      expect(compared.text).toContain('COMPARE generic authored proof');
      expect(compared.calls.length).toBe(4);
      expect(compared.calls.filter(call=>!call.build).map(call=>call.host).sort()).toEqual(['linux','web']);
      const mixed=await run(['--hosts','linux,web','--compare-saves'],{R8B_PASS_WEB:'1',R8B_UNVERIFIED_HOST:'web'});
      expect(mixed.calls.slice(0,2)).toEqual([{host:'linux',build:true,mode:'0'},{host:'web',build:true,mode:'0'}]);
      expect(mixed.calls.slice(2).map(call=>call.host).sort()).toEqual(['linux','web']);
      expect(mixed.calls.slice(2).every(call=>!call.build)).toBe(true);
      expect(mixed.code).toBe(1);
      const summary=JSON.parse(readFileSync(resolve(app,'artifacts/prove/summary.json'),'utf8'));
      expect(summary.rows.map(row=>row.status)).toEqual(['PASS','UNVERIFIED']);
      expect(summary.status).toBe('UNVERIFIED');
      expect(mixed.text).toContain(`PROOF UNVERIFIED ${name}`);
    }
    } else {
    const refused=await run(['--repin']);
    expect(refused.code).toBe(1); expect(refused.text).toContain('repin refused');
    expect(refused.calls.map(call=>call.host)).toEqual(['linux','linux','linux','web']);
    expect(JSON.parse(readFileSync(resolve(app,'pins.json'),'utf8'))).toEqual(pins);
    const allowed=await run(['--repin','--hosts','linux']);
    expect(allowed.code).toBe(0);
    const written=JSON.parse(readFileSync(resolve(app,'pins.json'),'utf8'));
    expect(written.hosts).toEqual(['linux']); expect(written.generated).toEndWith('--hosts linux');
    const empty={ticks:{},saves:{}};
    writeFileSync(resolve(app,'pins.json'),JSON.stringify(empty));
    for (const args of [['--repin'], ['--hosts','linux']]) {
      const refused = await run(args);
      expect(refused.code).toBe(1);
      expect(refused.text).toContain(args[0] === '--repin' ? 'omit `--repin` for the first baseline' : 'first baseline requires linux and web');
      expect(refused.calls).toEqual([]);
    }
    const firstRefused=await run([]);
    expect(firstRefused.code).toBe(1);
    expect(firstRefused.calls.map(call=>call.host)).toEqual(['linux','linux','linux','web']);
    expect(JSON.parse(readFileSync(resolve(app,'pins.json'),'utf8'))).toEqual(empty);
    const first=await run([],{R8B_PASS_WEB:'1'});
    expect(first.code).toBe(0);
    expect(first.calls).toEqual([
      ...['linux','web'].flatMap(host=>['0','1','fresh-game'].map(mode=>({host,mode,build:false}))),
      {host:'web',mode:'0',build:true},
    ]);
    expect(JSON.parse(readFileSync(resolve(app,'pins.json'),'utf8')).hosts).toEqual(['linux','web']);
    }
  } finally { rmSync(directory ?? app,{recursive:true,force:true}); }
});


test('clock settle diagnostic names busy, held input, and logs on a real unsettled reply', async () => {
  const source=readFileSync(resolve(import.meta.dir,'../scripts/agent.mjs'),'utf8');
  const a=source.indexOf("    async clock(spec = 'settle') {"), b=source.indexOf('\n    /** Pixels as PNG',a);
  const s={now:0,op:async req=>{expect(req).toEqual({op:'clock',settle:true});return {clock:100,settled:false,world:{changing:['player']}};}};
  const clock=new Function('s',`return ({${source.slice(a,b)}}).clock;`)(s);
  const reply=await clock();
  expect(reply.diagnostic).toContain('clock settle did not reach quiescence');
  expect(reply.diagnostic).toContain('state world:* busy');
  expect(reply.diagnostic).toContain('state shows held input');
  expect(reply.diagnostic).toContain('logs shows reload/refusals');
});

function pinLiterals(path, text) {
  const evidence=/(^|\/)(artifacts|diaries)\/|^game\/bench\/results\/|^(llp|issues|vendor)\//;
  if(/(^|\/)(Cargo\.lock|bun\.lock)$/.test(path) || path.endsWith('/pins.json') || evidence.test(path) || path==='scripts/fixtures/fonts/SOURCE.md') return [];
  const generated = path === '.llp/skills-receipt.json' || path.endsWith('/.baked-assets.json') || ['update/tests/fixtures/publisher/canonical.bin','update/tests/fixtures/publisher/exact.json'].includes(path);
  if (generated) return [];
  const samples = new Set(['b510eca2e2ef33f62f9ed57d6e7ce2d10'+'ebb2bdebc4a8e59d347719ba81abdf4', 'a1e3b04de97b11de564ce6e53b95f02954'+'a297f0008183ac63a4f5974f6b32d8', 'd97044e701822bac5a62696459b27d7b3'+'75aada5de8574ed4362edbba94771f7']);
  const constants=new Set(['9e3779b1'+'85ebca87','c2b2ae3d'+'27d4eb4f'].map(v=>'0x'+v));
  return [...text.matchAll(/0x[0-9a-fA-F]{16}|(?<![0-9a-fA-F])[0-9a-fA-F]{64}(?![0-9a-fA-F])/g)].map(m=>m[0])
    .filter(value=>!(path==='game/render/src/world/upload.rs' && constants.has(value)) && !(path==='game/bake/tests/samples.rs' && samples.has(value)));
}
test('pin scan includes authored benchmark tests and ordinary digest strings', () => {
  const digest='abcdef01'.repeat(8), hash='0x'+'12345678'.repeat(2);
  expect(pinLiterals('game/bench/feel.test.mjs',hash)).toEqual([hash]);
  for(const quote of ['"',"'",'`','']) expect(pinLiterals('game/engine/tests/foo.rs',quote+digest+quote).length).toBe(1);
  expect(pinLiterals('game/bench/results/receipt.md',digest)).toEqual([]);
  expect(pinLiterals('Cargo.lock',digest)).toEqual([]);
});
test('game pin literals stay in fixture pins across the tracked tree', () => {
  const root=resolve(import.meta.dir,'..');
  const tracked=Bun.spawnSync(['git','ls-files','-z'],{cwd:root}); expect(tracked.exitCode).toBe(0);
  const violations=[];
  for(const path of tracked.stdout.toString().split('\0').filter(Boolean)) {
    let source;
    try { source = readFileSync(resolve(root,path),'utf8'); }
    catch (error) { if (error.code === 'ENOENT') continue; throw error; }
    for(const value of pinLiterals(path,source)) violations.push(`${path}: ${value}`);
  }
  expect(violations).toEqual([]);
});
test('moving placement oracle rejects the old 1.31 pixel discrepancy', () => {
  const source=readFileSync(resolve(import.meta.dir,'games/placement-fixture/proof.mjs'),'utf8');
  const line=source.split('\n').find(s=>s.includes("check('moving displayed sign"));
  const projected={x:492.88443,y:285.10403,w:93.112885,h:38.104492};
  let accepted;
  new Function('check','projected','after',line)((_,ok)=>accepted=ok,projected,{...projected,x:projected.x+1.31});
  expect(accepted).toBe(false);
});
test('paranoid placement pins the reconstructed endpoint with the same half pixel limit', () => {
  const source=readFileSync(resolve(import.meta.dir,'games/placement-fixture/proof.mjs'),'utf8');
  const declaration=source.split('\n').find(s=>s.includes('const projected='));
  const checkLine=source.split('\n').find(s=>s.includes("check('moving displayed sign"));
  const run=new Function('check','projected','after',checkLine);
  for(const mode of ['0','1','fresh-game']) {
    const projected=new Function('process',`${declaration};return projected;`)({env:{EXACT_GAME_PARANOID:mode}});
    expect(projected.x).toBe(mode==='0'?492.88443:494.20934);
    let accepted;
    run((_,ok)=>accepted=ok,projected,{...projected,x:projected.x+0.49});expect(accepted).toBe(true);
    run((_,ok)=>accepted=ok,projected,{...projected,x:projected.x+0.51});expect(accepted).toBe(false);
  }
});


test('forty-child capture does not mislabel painter timing as CPU cost', async () => {
  const source=readFileSync(resolve(import.meta.dir,'games/placement-fixture/proof.mjs'),'utf8');
  const a=source.indexOf("  if(process.argv.includes('--capture40')) {"),b=source.indexOf('  const start=',a);
  const messages=[];
  const session={tap:async()=>{},clock:async()=>{},screenshot:async()=>{},logs:async()=>[],close:async()=>{},
    state:async()=>{throw new Error('paint.ms does not establish CPU cost');},world:()=>({snapshot:async()=>({entities:Array.from({length:40},()=>({components:{Placed:{}}}))})})};
  const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor;
  await new AsyncFunction('process','open','resolve','out','host','say','check',source.slice(a,b))(
    {argv:['--capture40']},async()=>session,resolve,'/tmp','linux',s=>messages.push(s),(name,ok)=>expect(ok).toBe(true));
  expect(messages.join(' ')).toContain('no CPU-cost claim');
});


test('report does not count diagnostics from another session or infer geometry from asset names', () => {
  expect(facilityReport([{session:1,method:'tap',args:['play'],error:'restore refused: asset hidden.model pending'}]).join(' ')).not.toContain('layout');
  expect(facilityReport([{session:1,method:'tap',args:['sign'],error:'sign is hidden'}, {session:2,method:'layout',args:['sign'],reply:{}}]).join(' ')).toContain('layout unused');
  expect(facilityReport([{session:1,method:'clock',reply:{settled:false}}, {session:1,method:'state',args:['world:*',null,false,true],reply:{busy:[]}}]).join(' ')).not.toContain('state unused');
});

test('report requires relevant diagnostics after the failure and at its clock or later',()=>{
  for(const [method,args,error] of [['layout',['sign'],'sign is hidden'],['state',[],'restore asset refused'],['logs',[],'refused'],['state',['world:*',null,false,true],null]]) {
    const diagnostic={session:1,method,args,reply:{},clock:10};
    const failure={session:1,method:error?'tap':'clock',args:['sign'],...(error?{error}:{reply:{settled:false}}),clock:20};
    const hint=method==='state'?'state unused':`${method} unused`;
    expect(facilityReport([diagnostic,failure]).join(' ')).toContain(hint);
    expect(facilityReport([failure,diagnostic]).join(' ')).toContain(hint);
    expect(facilityReport([failure,{...diagnostic,clock:20}]).join(' ')).not.toContain(hint);
  }
});
test('direct placement comparison refuses different simulation ticks',()=>{
  const sample={x:1,y:2,w:3,h:4,tick:0};
  expect(()=>comparePlacement({initial:sample,moving:{...sample,tick:60}},{initial:sample,moving:{...sample,tick:61}})).toThrow(/tick/);
});

test('refusal advice executes as real driver CLI operations with a global JSON flag', async()=>{
  const s={tree:async target=>target?{entities:[{name:'sign'}]}:{nodes:[{id:7,props:{testId:'world'},world:{}}]},state:async()=>({entity:{placed:{hidden:true}}}),layout:async()=>({entity:{visible:{behindCamera:true}}})};
  const refusal=await tapRefusal(s,'sign',Error('no view matches sign'));
  const advised=[...refusal.message.matchAll(/`([^`]+)`/g)].map(m=>m[1]);
  expect(advised).toEqual(['layout world:sign']);
  const source=readFileSync(new URL('../scripts/agent.mjs',import.meta.url),'utf8');
  const body=source.slice(source.indexOf('async function main(argv)'),source.lastIndexOf('\nif (process.argv[1]'));
  const calls=[], output=[];
  const cli=new Function('open','resolve','render','console',`${body}; return main;`)(async()=>({layout:async target=>{calls.push(['layout',target]);return {visible:true};},state:async()=>{calls.push(['state']);return {world:[{loading:['crate.model'],assets:[]}]};},close:async()=>{}}),x=>x,()=>{throw Error('global --json was ignored');},{log:x=>output.push(JSON.parse(x)),error:()=>{}});
  for(const op of [...advised,'state']) expect(await cli(['web','--json',op])).toBe(0);
  expect(calls).toEqual([['layout','world:sign'],['state']]);
  expect(output[1].world[0].loading).toEqual(['crate.model']);
});

test('Fox predicate rejects a 95 percent white crop retaining orange pixels', async () => {
  const {foxPixels}=await import('./games/skinned-fixture/proof.mjs');
  const data=new Uint8Array(160*90*4).fill(255);
  for(let y=0;y<90;y++) for(let x=0;x<160;x++) if((y*160+x)%20===0) data.set([180,90,30,255],(y*160+x)*4);
  expect(foxPixels({width:160,height:90,data},{x:0,y:0,w:160,h:90},[160,90]).ok).toBe(false);
});

test('Fox predicate crops reported bounds and requires varied fur at the declared aspect',async()=>{
  const {foxPixels}=await import('./games/skinned-fixture/proof.mjs');
  const image={width:160,height:90,data:new Uint8Array(160*90*4).fill(255)};
  for(let y=10;y<30;y++) for(let x=10;x<50;x++) image.data.set([150+(x%10)*8,70+(y%4)*4,20,255],(y*160+x)*4);
  expect(foxPixels(image,{x:10,y:10,w:40,h:20},[160,90]).ok).toBe(true);
  expect(foxPixels(image,{x:80,y:10,w:40,h:20},[160,90]).ok).toBe(false);
  expect(foxPixels(image,{x:10,y:10,w:40,h:20},[90,160]).ok).toBe(false);
});


test('R12 proof inputs from game include host code and exclude other games', async () => {
  const {proofInputFiles}=await import('./proof.mjs');
  const files=proofInputFiles(import.meta.dir,resolve(import.meta.dir,'games/beacons'));
  expect(files).toContain('../host/web/gpu-glue.js');
  expect(files.some(f=>f.startsWith('games/greybox/'))).toBe(false);
  expect(files.some(f=>f.startsWith('bench/'))).toBe(false);
});

test('R12 Fox crop accepts approximately 16:9 real screenshot coordinates', async () => {
  const {foxPixels}=await import('./games/skinned-fixture/proof.mjs');
  const width=321,height=180,data=new Uint8Array(width*height*4).fill(255);
  for(let y=40;y<120;y++) for(let x=240;x<300;x++) data.set([160+(x%12)*8,70,25,255],(y*width+x)*4);
  const image={width,height,data}, screen={x:240,y:40,w:60,h:80};
  expect(foxPixels(image,screen,[width,height]).ok).toBe(true);
});

test('reused Chrome clears IndexedDB, history and held keys/contacts between stages', async () => {
  const {open} = await import('../scripts/agent.mjs');
  let generation = 0;
  const released = [];
  const server = Bun.serve({port:0, fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === '/released') { released.push(url.searchParams.get('event')); return new Response('ok'); }
    return new Response(`<div id="exact-root" data-boot-ms="${++generation}"><button id="button">Input</button></div><script>
      const button = document.getElementById('button');
      for (const event of ['keyup','touchcancel']) addEventListener(event, e => fetch('/released?event='+event+(e.code || '')));
      const database = () => new Promise((resolve,reject) => { const r=indexedDB.open('stage',1); r.onupgradeneeded=()=>r.result.createObjectStore('data'); r.onsuccess=()=>resolve(r.result); r.onerror=()=>reject(r.error); });
      window.exact={ready:Promise.resolve(), views:new Map([[1,button]]), agent:async request=>{
        if(request.op==='layout') return {nodes:[{id:1,x:0,y:0,w:100,h:40}]};
        const db=await database();
        if(request.op==='write') { const tx=db.transaction('data','readwrite'); tx.objectStore('data').put('secret','key'); await new Promise(r=>tx.oncomplete=r); history.pushState({},'', '/one'); history.pushState({},'', '/two'); db.close(); return {}; }
        const tx=db.transaction('data'); const r=tx.objectStore('data').get('key'); const value=await new Promise(ok=>r.onsuccess=()=>ok(r.result??null)); db.close(); return {value,history:history.length};
      }};
    </script>`, {headers:{'content-type':'text/html'}});
  }});
  let first, second;
  try {
    first = await open({host:'web',url:server.url.href});
    await first.carrier.ask({op:'write'});
    await first.carrier.input(1,'key',{key:'Shift',phase:'down'});
    await first.carrier.input(1,'down',{});
    second = await open({host:'web',url:server.url.href,reuse:first.carrier});
    expect(second.carrier).toBe(first.carrier);
    expect(await second.carrier.ask({op:'state'})).toEqual({value:null,history:1});
    expect(released).toContain('keyupShiftLeft');
    expect(released).toContain('touchcancel');
    expect(second.boot).toBeGreaterThan(first.boot);
  } finally { await (second ?? first)?.close(); server.stop(true); }
}, 30000);

test('paranoid traversal restores the ordinary artifact only on web', async () => {
  const restored = [];
  for (const host of ['linux','web']) {
    const modes = [];
    expect(await paranoidRuns(async mode => {modes.push(mode); return 0;}, async () => {restored.push(host); return 0;}, host)).toBe(false);
    expect(modes).toEqual(['0','1','fresh-game']);
  }
  expect(restored).toEqual(['web']);
});

test('shared residency probe takes the authored replacement model name', async () => {
  const {residencyProbe} = await import('./proof.mjs');
  expect(residencyProbe('sample.model', 'texture.tex', 'reload.model').source).toContain("encode('reload.model')");
  expect(() => residencyProbe('sample.model', 'texture.tex', 'longer.model-name')).toThrow('same byte length');
});


test('R13 output names nested in logic remain proof inputs and change the hash', async () => {
  const {proofInputExcluded, proofInputFiles}=await import('./proof.mjs');
  for(const name of ['target','.shells','dist','dist.previous','artifacts']) {
    expect(proofInputExcluded(`game/games/beacons/logic/src/${name}/mod.rs`,'beacons')).toBe(false);
    expect(proofInputExcluded(`game/games/beacons/${name}/mod.rs`,'beacons')).toBe(true);
  }
  const dir=mkdtempSync(resolve(tmpdir(),'r13-proof-'));
  try {
    const path=resolve(dir,'logic/src/target/mod.rs');mkdirSync(dirname(path),{recursive:true});writeFileSync(path,'before');
    expect(proofInputFiles(dir,dir)).toContain('logic/src/target/mod.rs');
    const hash=()=>{const value=buildInputHash('linux','target');for(const file of proofInputFiles(dir,dir)) value.update(file).update(readFileSync(resolve(dir,file)));return value.digest('hex');};
    const before=hash();writeFileSync(path,'after');expect(hash()).not.toBe(before);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('R13 Fox screenshot reply scales logical bounds at DPR 2 and 3', async () => {
  const {foxScreenshotPixels}=await import('./games/skinned-fixture/proof.mjs');
  for(const scale of [2,3]) {
    const w=160,h=90,width=w*scale,height=h*scale,data=new Uint8Array(width*height*4).fill(255);
    for(let y=20*scale;y<40*scale;y++) for(let x=90*scale;x<120*scale;x++) data.set([150+(x%10)*8,70,20,255],(y*width+x)*4);
    const result=foxScreenshotPixels({width,height,data},{x:90,y:20,w:30,h:20},{w,h,scale});
    expect(result.ok).toBe(true);expect(result.crop).toEqual([90*scale,20*scale,120*scale,40*scale]);
  }
});

test('exact tick helper uses restored epochs and never silently retries a mismatch', async () => {
  const {exactTicks} = await import('../scripts/agent.mjs');
  let tick=301, host=81234, us=5016670, calls=0;
  const session={controlled:true, state:async(target,options)=>{expect(target).toBe('world');expect(options).toEqual({world:true,clockState:true});return {clock:host,world:{tick,hz:60,paused:false,clockState:{hostMicros:host*1000,worldMicros:us}}};},
    clock:async to=>{calls++;us+=(to-host)*1000;host=to;tick=Math.floor(us*60/1000000);return {clock:to};}};
  for(let i=0;i<120;i++) expect((await exactTicks(session,'world',1)).actualTick).toBe(302+i);
  expect(calls).toBe(120);
  session.clock=async()=>{calls++;return {clock:host};};
  await expect(exactTicks(session,'world',1)).rejects.toThrow('no retry');
  expect(calls).toBe(121);
  session.state=async()=>({world:{tick,hz:60,paused:true}});
  await expect(exactTicks(session,'world',1)).rejects.toThrow('paused');
  expect(calls).toBe(121);
});

test('capture replay validates actual artifacts and never executes imported script text', async () => {
  const {captureTools}=await import('./proof.mjs');
  const dir=mkdtempSync(resolve(tmpdir(),'capture-proof-')), file=resolve(dir,'capture.json');
  const identity={digest:'sha256:actual',files:[['game.wasm','code'],['app.plan','plan']]};
  let opened=0,closed=0;
  const requests=[];
  const fake={loadedArtifact:identity.digest,controlled:true,input:{delivery:()=> 'platform'},
    world(name){return worldView(this,name);},
    state:async(target,options)=>{requests.push({op:'state',target,...options});expect(target).toBe('world');expect(options.world).toBe(true);
      if(options.capture==='replay')return {isolated:true,replay:{world:{hash:'hash',tick:4}}};
      if(options.capture)return {capture:{complete:true,records:2,lastReliableTick:4,hash:'hash'},data:'aabb'};
      return {world:{hash:'hash',tick:4,resources:{SceneIdentity:{digest:'loaded-scene'}}}};},
    logs:async()=>({lines:[]}),close:async()=>closed++};
  const kit=captureTools({identity:()=>identity,host:'web',open:async()=>{opened++;return fake;}});
  try {
    const capture=await kit.capture(fake,'world',{script:'throw new Error("must never execute")',failure:'crate stuck'});
    await capture.finish(file);
    expect(JSON.parse(readFileSync(file,'utf8')).metadata.scene).toBe('loaded-scene');
    expect((await kit.replay(file)).isolated).toBe(true);
    expect(requests.map(request=>request.capture??'inspect')).toEqual(['start','stop','inspect','replay']);
    expect(opened).toBe(1);expect(closed).toBe(1);
    const bundle=JSON.parse(readFileSync(file,'utf8'));bundle.artifacts.digest='other';writeFileSync(file,JSON.stringify(bundle));
    await expect(kit.replay(file)).rejects.toThrow('actual local artifact');
    expect(opened).toBe(1);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
