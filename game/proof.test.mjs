import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync, existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {PassThrough} from 'node:stream';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {artifactDigest, closeSessions, equal} from './proof.mjs';
import {typeArguments, typeFor, browserKey, nativeKey, render, worldView, nativeControl, detachNativeTransport, jsonLines} from '../scripts/agent.mjs';

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
    ['type','arena',{key:'KeyW',phase:'up'}], ['clock','+100'], ['clock','settle'],
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
    expect(source).toContain('near_xz::<Beacon>');
  }
  expect(main).not.toContain('Call it at the\nend of `setup`');
  expect(engine).not.toContain('stepped explicitly by `scene::follow`');
  expect(greybox).not.toContain('math::ease');
  const proof = read('games/greybox/proof.mjs');
  for (const pin of ['0x7df5e5a89b4d0207', '0x0f14b8b231091d12', '[0, 0.9, -5.3666644]']) {
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

test('exact tick helper uses restored epochs and never silently retries a mismatch', async () => {
  const {exactTicks} = await import('../scripts/agent.mjs');
  let tick=301, host=81234, us=5016670, calls=0;
  const session={controlled:true, state:async(target,options)=>{expect(target).toBe('world');expect(options).toEqual({world:true});return {clock:host,world:{tick,hz:60,paused:false,clockState:{hostMicros:host*1000,worldMicros:us}}};},
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


test('world source navigation reports procedural metadata without inventing a line', async () => {
  const calls=[];
  const session={state:async(target,options)=>{calls.push([target,options]);return target==='world' ? {world:{resources:{SceneIdentity:{digest:'sha'}}}} : {entity:{components:{GeneratedBy:{generator:'trees',parameters:{seed:'7'}}}}};}};
  expect(await worldView(session,'world').source('tree-4')).toEqual({generated:{generator:'trees',parameters:{seed:'7'}}});
  expect(calls).toEqual([['world',{world:true}],['world:tree-4',undefined]]);
  expect(await worldView({state:async()=>({world:{resources:{}}})},'world').source('crate')).toEqual({unavailable:'world has no authored scene identity'});
});

const ownershipAck = owner => ({ownership:{owner,clock:owner === 'agent' ? 'controlled' : 'live',scope:'session'},clock:123,releasedInput:true});

test('Apple ownership requires matching host ACKs and preserves routing; ordinary handoff still cleans up', async () => {
  for (const host of ['macos', 'host', 'ios', 'host-ios']) {
    const requests=[], actions=[];
    const control=nativeControl({host, ask:async (req, label) => {requests.push([req,label]); return req.op === 'state' ? {clock:0} : ownershipAck(req.owner);},
      close:async()=>actions.push('close'),detach:async()=>actions.push('detach')});
    await control.ask({op:'state'},'b'); // Observing does not acquire ownership.
    for (const owner of ['human','agent','human']) expect(await control.ask({op:'clock',owner},'b')).toEqual(ownershipAck(owner));
    expect(requests.map(([req,label])=>[req.op,req.owner,label])).toEqual([['state',undefined,'b'],['clock','human','b'],['clock','agent','b'],['clock','human','b']]);
    expect(actions).toEqual([]);
    await control.close(); await control.close();
    expect(actions).toEqual(['close']);
  }
});

test('native detach waits for ACK even when finally closes concurrently, then never kills or quits', async () => {
  let acknowledge; const requests=[],actions=[];
  const control=nativeControl({host:'macos',ask:req=>{requests.push(req);return new Promise(r=>acknowledge=r);},
    close:async()=>actions.push('kill'),detach:async()=>actions.push('eof')});
  const pending=control.ask({op:'clock',owner:'human',detach:true});
  await expect(control.ask({op:'state'})).rejects.toThrow('handoff is in progress');
  const closing=control.close();
  await Promise.resolve(); expect(actions).toEqual([]);
  const reply={...ownershipAck('human'),detached:true,sessions:[{...ownershipAck('human'),session:'a'},{...ownershipAck('human'),session:'b'}]};
  acknowledge(reply);
  expect(await pending).toEqual(reply);
  await closing; await control.close();
  expect(actions).toEqual(['eof']);
  expect(requests).toEqual([{op:'clock',owner:'human',detach:true}]);
  await expect(control.ask({op:'clock',owner:'agent'})).rejects.toThrow('closed');
});

test('native refusals and incomplete ACKs never detach or perform unsolicited cleanup', async () => {
  for (const reply of [
    {error:'world refused',detached:false}, {}, null, ownershipAck('human'),
    {...ownershipAck('agent'),detached:true},
    {ownership:{owner:'human',clock:'controlled'},detached:true},
    {...ownershipAck('human'),detached:true,sessions:{}},
    {...ownershipAck('human'),detached:true,worldHandoff:{unavailable:true,reason:'old GPU'}},
    {...ownershipAck('human'),detached:true,sessions:[ownershipAck('agent')]},
  ]) {
    const actions=[];
    const control=nativeControl({host:'host',ask:async()=>reply,close:async()=>actions.push('close'),detach:async()=>actions.push('detach')});
    let failure;
    try {await control.ask({op:'clock',owner:'human',detach:true});} catch(error) {failure=error;}
    expect(failure.reply).toBe(reply); expect(actions).toEqual([]);
    await control.close(); expect(actions).toEqual(['close']);
  }
  for (const options of [{host:'linux'},{host:'ios',device:true}]) {
    const actions=[],control=nativeControl({...options,ask:async()=>actions.push('request'),close:async()=>actions.push('close'),detach:async()=>actions.push('detach')});
    for (const owner of ['human','agent']) await expect(control.ask({op:'clock',owner})).rejects.toThrow('unavailable');
    await expect(control.ask({op:'clock',owner:'human',detach:true})).rejects.toThrow('unavailable');
    expect(actions).toEqual([]);
  }
});

test('native detach refuses invalid intent, lost transport, timeout, and an unreleased Simulator desktop contact', async () => {
  const actions=[], input=new PassThrough(), output=new PassThrough(), lines=jsonLines(output,input,[]);
  const control=nativeControl({host:'ios',ask:lines.ask,close:async()=>actions.push('close'),detach:async()=>actions.push('detach')});
  for (const req of [{detach:true},{owner:'agent',detach:true},{owner:'human',detach:'yes'},{owner:'other'}]) {
    await expect(control.ask({op:'clock',...req})).rejects.toThrow();
    expect(input.read()).toBeNull();
  }
  const pending=control.ask({op:'clock',owner:'human',detach:true});
  output.end();
  await expect(pending).rejects.toThrow('transport ended');
  expect(actions).toEqual([]); await control.close(); expect(actions).toEqual(['close']);
  input.destroy(); output.destroy();
  const stalled=nativeControl({host:'macos',ask:()=>new Promise(()=>{}),close:async()=>actions.push('close'),detach:async()=>actions.push('detach'),timeoutMs:20});
  await expect(stalled.ask({op:'clock',owner:'human',detach:true})).rejects.toThrow('did not answer');
  await expect(stalled.ask({op:'state'})).rejects.toThrow('acknowledgement was lost');
  await stalled.close(); expect(actions).toEqual(['close','close']);
  const held=nativeControl({host:'ios',beforeHandoff:()=>{throw new Error('release desktop contact');},ask:async()=>actions.push('request'),close:async()=>{},detach:async()=>actions.push('detach')});
  await expect(held.ask({op:'clock',owner:'human',detach:true})).rejects.toThrow('release desktop contact');
  expect(actions).toEqual(['close','close']);
});

test('detach releases every parent handle without destroying output or signalling a child', () => {
  for (const socket of [false,true]) {
    const actions=[], handle=name=>({end:()=>actions.push(name+' EOF'),unref:()=>actions.push(name+' unref'),destroy:()=>actions.push(name+' DESTROY')});
    const child={stdin:handle('stdin'),stdout:handle('stdout'),stderr:handle('stderr'),unref:()=>actions.push('child unref'),kill:()=>actions.push('KILL')};
    detachNativeTransport(child,socket ? handle('socket') : undefined);
    expect(actions).toEqual([`${socket?'socket':'stdin'} EOF`,`${socket?'socket':'stdin'} unref`,'stdout unref','stderr unref','child unref']);
  }
});

// Protocol fixtures exercise the real driver and OS process lifetime, without
// building or launching an Apple app, simulator, phone, or user-owned process.
function nativeFixture(body) {
  const dir=mkdtempSync(resolve(tmpdir(),'native-driver-')), binary=resolve(dir,'fixture'), events=resolve(dir,'events');
  const driver=resolve(import.meta.dir,'../scripts/agent.mjs');
  const source=`
import {appendFileSync,writeFileSync} from 'node:fs';
import {createInterface} from 'node:readline';
const record=value=>appendFileSync(process.env.FIXTURE_EVENTS,JSON.stringify(value)+'\\n');
let detached=false,owner='agent',clock=0;
setInterval(()=>writeFileSync(process.env.FIXTURE_EVENTS+'.tick',String(Date.now())),20);
process.on('SIGTERM',()=>{record({event:'signal'});process.exit(0);});
const reply=value=>process.stdout.write(JSON.stringify(value)+'\\n');
reply({ready:true,boot:1,sessions:['a','b']});
createInterface({input:process.stdin}).on('line',line=>{
  const req=JSON.parse(line);record(req);
  if(req.op==='quit') process.exit(0);
  if(req.op==='clock' && req.owner){
    owner=req.owner;detached=req.detach===true;
    const ack={ownership:{owner,clock:owner==='agent'?'controlled':'live'},clock:++clock,releasedInput:true};
    if(detached && process.env.FIXTURE_MODE!=='missing-ack') ack.detached=true;
    setTimeout(()=>reply(ack),30);
  } else reply({clock,ownership:{owner,clock:owner==='agent'?'controlled':'live'}});
}).on('close',()=>{record({event:'eof',detached});if(!detached)process.exit(0);});
`;
  const quote=value=>"'"+value.replaceAll("'","'\\''")+"'";
  // Record the spawned PID before interpreter startup, including startup failures.
  writeFileSync(binary,`#!/bin/sh\n# {"id":"00000000000000000000000000000000","inputs":{"app":"com.exact.caltrain"}}\nprintf '{"event":"launch","pid":%s}\\n' "$$" >> "$FIXTURE_EVENTS"\nexec ${quote(process.execPath)} --eval ${quote(source)}\n`,{mode:0o755});
  const env={...process.env,EXACT_APP_DIR:resolve(import.meta.dir,'../apps/caltrain'),EXACT_MAC_BIN:binary,EXACT_LINUX_BIN:binary,FIXTURE_EVENTS:events};
  const read=()=>existsSync(events)?readFileSync(events,'utf8').trim().split('\n').map(JSON.parse):[];
  try {return body({dir,driver,events,env,read});}
  finally {
    // The only PID signalled is the fixture's own launch receipt.
    for (const row of read()) if(row.event==='launch') {try {process.kill(row.pid,'SIGTERM');} catch {}}
    rmSync(dir,{recursive:true,force:true});
  }
}

test('native CLI refuses unsupported ownership/detach before any launch or device tooling', () => nativeFixture(({dir,driver,env,read}) => {
  const cases=[['linux','clock owner human'],['linux','clock owner human detach'],['ios','--device','clock owner agent'],['ios','--device','clock owner human detach'],['web','clock owner human detach'],['macos','clock owner agent detach'],['macos','clock owner human detach','state']];
  for(const args of cases) {
    const result=spawnSync(process.execPath,[driver,...args],{env:{...env,PATH:dir},encoding:'utf8',timeout:4000});
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/unavailable|requires owner human|must be the last/);
  }
  expect(read()).toEqual([]);
}));

test('real stdio handoff/detach keeps the fixture playing after natural parent exit and finally close', () => nativeFixture(({driver,events,env,read}) => {
  const result=spawnSync(process.execPath,['--eval',`
    import {open} from ${JSON.stringify(driver)};
    const s=await open({host:'host',app:'caltrain'});
    try {
      const handoff=s.clock({owner:'human'}); s.session='b'; await handoff;
      if(!s.controlled) throw new Error('ACK applied to a different session');
      s.session='a'; if(s.controlled) throw new Error('default session ownership lost');
      s.session='b'; if(!s.controlled) throw new Error('ownership leaked across sessions');
      await s.clock({owner:'agent'}); if(!s.controlled) throw new Error('unacknowledged agent');
      s.contact={x:1,y:2}; await s.detach(); if(s.controlled || s.contact) throw new Error('handoff retained contact');
    } finally {await s.close();}
  `],{env,encoding:'utf8',timeout:4000});
  expect(result.stderr).toBe(''); expect(result.status).toBe(0);
  const rows=read(),pid=rows.find(r=>r.event==='launch').pid;
  expect(rows.filter(r=>r.op)).toEqual([
    {op:'clock',owner:'human'}, {op:'clock',owner:'agent',session:'b'},
    {op:'clock',owner:'human',detach:true,session:'b'},
  ]);
  expect(rows).toContainEqual({event:'eof',detached:true});
  expect(()=>process.kill(pid,0)).not.toThrow();
  const before=readFileSync(events+'.tick','utf8');
  // Longer than the old carrier.close force-kill deadline; only this fixture runs.
  spawnSync(process.execPath,['--eval','await Bun.sleep(2200)'],{timeout:3000});
  expect(()=>process.kill(pid,0)).not.toThrow();
  expect(readFileSync(events+'.tick','utf8')).not.toBe(before);
  expect(read().some(r=>r.event==='signal'||r.op==='quit')).toBe(false);
}),10000);

test('ordinary native close still ends the isolated app, including a missing detach ACK', () => nativeFixture(({driver,env,read}) => {
  for(const mode of ['normal','missing-ack']) {
    const result=spawnSync(process.execPath,['--eval',`
      import {open} from ${JSON.stringify(driver)};
      const s=await open({host:'macos',app:'caltrain'});
      try {
        if(process.env.FIXTURE_MODE==='missing-ack') {
          try {await s.detach();throw new Error('accepted missing ACK');}
          catch(error) {if(!error.message.includes('did not acknowledge'))throw error;}
          if(!s.controlled) throw new Error('inferred ownership from request');
        } else await s.clock({owner:'human'});
      } finally {await s.close();}
    `],{env:{...env,FIXTURE_MODE:mode},encoding:'utf8',timeout:4000});
    expect(result.stderr).toBe(''); expect(result.status).toBe(0);
    const rows=read(),pid=rows.filter(r=>r.event==='launch').at(-1).pid;
    expect(()=>process.kill(pid,0)).toThrow();
    expect(rows.some(r=>r.event==='eof' && r.detached===(mode==='missing-ack'))).toBe(true);
  }
}),10000);
