import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { fixture } from './surface.mjs';

test('player restore flushes its public record before reporting success', async () => {
  const f=await fixture();f.create(1);
  f.gpu.gpu_restore=id=>{f.publish(id,{restored:true});return true;};
  await f.exact.gpu.surfaceWork('world','restore',new Uint8Array([7]),()=>true);
  assert.equal(f.records.at(-1),'world\0{"restored":true}');
});

test('surface work refuses missing and duplicate names without touching either world', async () => {
  const f=await fixture();
  let carries=0;f.gpu.gpu_carry=()=>{carries++;return new Uint8Array([1]);};
  await assert.rejects(f.exact.gpu.surfaceWork('world','capture',null,()=>true), error=>error.kind===2&&/found 0/.test(error.message));
  f.create(1);f.create(2);
  await assert.rejects(f.exact.gpu.surfaceWork('world','capture',null,()=>true), error=>error.kind===2&&/found 2/.test(error.message));
  assert.equal(carries,0);
});

test('a retired surface ticket is checked again before capture', async () => {
  const f=await fixture();f.create(1);
  let carries=0;f.gpu.gpu_carry=()=>{carries++;return new Uint8Array([1]);};
  await assert.rejects(f.exact.gpu.surfaceWork('world','capture',null,()=>false),error=>error.kind===4);
  assert.equal(carries,0);
});

test('surface replacement while host work settles retires the request before its effect', async () => {
  let started, release;
  const fetching=new Promise(resolve=>{started=resolve;});
  const response=new Promise(resolve=>{release=()=>resolve({status:404,ok:false,headers:{get:()=>null}});});
  const f=await fixture({delivery:{fetch:()=>{started();return response;}}});
  f.create(1);
  let first=true, restores=0;
  f.gpu.gpu_assets=()=>{
    const requests=first?['slow.asset']:[];first=false;
    return JSON.stringify({requests,retired:[]});
  };
  f.gpu.gpu_restore=()=>{restores++;return true;};
  const work=f.exact.gpu.surfaceWork('world','restore',new Uint8Array([7]),()=>true);
  await fetching;
  f.destroy(1);f.create(2);release();
  await assert.rejects(work,error=>error.kind===4&&/retired or surface replaced/.test(error.message));
  assert.equal(restores,0);
});

test('destroy/create publications wait for the outermost apply and drain before return', async () => {
  const f = await fixture();
  f.create(1);
  f.expectView(2);
  f.applyBatch({ ops: [() => f.destroy(1), () => f.applyBatch({ ops: [() => f.create(2)] }), f.stale] });
  assert.deepEqual(f.records, ['world\0{"value":1}', 'world', 'world\0{"value":2}']);
  assert.equal(f.hud(), 'world\0{"value":2}');
});

test('first live canvas alone publishes and clears, with one named duplicate diagnostic', async () => {
  const f = await fixture();
  f.create(1); f.create(2);
  f.exact.gpu.surface(2, 'world', []);
  assert.deepEqual(f.records, ['world\0{"value":1}']);
  assert.equal(f.diagnostics.length, 1);
  assert.match(f.diagnostics[0], /world.*duplicate/);
  f.destroy(2);
  assert.equal(f.records.length, 1);
  f.create(3); f.destroy(1);
  assert.equal(f.records.at(-1), 'world');
  f.exact.gpu.surface(3, 'world', []);
  assert.equal(f.records.at(-1), 'world', 'an ignored instance does not silently take ownership');
  f.destroy(3); f.create(4);
  assert.equal(f.records.at(-1), 'world\0{"value":4}');
});

for (const failure of ['createFail', 'bindFail']) test(`swap ${failure} keeps the live module and canvas`, async () => {
  const f = await fixture({[failure]:true}); f.create(1);
  await assert.rejects(f.exact.gpu.swap(1), /surface world/);
  assert.ok(!f.order.includes('old destroy') && !f.order.includes('old unload') && !f.order.includes('replace'), f.order.join(', '));
  assert.equal(f.exact.gpu.agent(1, {op:'state'}).world.tick, 0);
});
test('swap stages all surfaces before destroying any old one', async () => {
  const f = await fixture(); f.create(1); f.create(2, 'other');
  await f.exact.gpu.swap(1);
  assert.ok(f.order.lastIndexOf('next bind') < f.order.indexOf('old destroy'), f.order.join(', '));
});
test('failed initial load can bootstrap a swap with pending surfaces', async () => {
  const f = await fixture({loadFail:true}); f.create(1);
  await f.exact.gpu.swap(1);
  assert.ok(f.exact.gpu.agent(1, {op:'state'}));
});
test('forwarded keyups survive focus changes; editable focus blurs; shortcuts stay local', async () => {
  const f = await fixture({input:true}), el = f.create(1), button = new f.Element('button'), input = new f.Element('input');
  const send = (name, code, target = el, extra = {}) => el.listeners[name]({target, code, timeStamp:0, preventDefault(){}, ...extra});
  send('keydown', 'Space'); send('keyup', 'Space', button);
  send('keyup', 'KeyX'); send('keydown', 'Tab'); send('keydown', 'KeyR', el, {metaKey:true}); send('keydown', 'KeyW', el, {isComposing:true});
  assert.deepEqual(f.events.map(e => [e.code, e.down]), [['Space',true],['Space',false]]);
  send('keydown', 'KeyW'); send('focusin', '', input); send('keyup', 'KeyW', input);
  assert.equal(f.events.at(-1).t, 'blur');
});
test('restore bytes survive a refusal and reach the next capable surface', async () => {
  const f = await fixture({refuse:id=>id===1}); f.exact.worldCarry = new Uint8Array([7]);
  f.create(1);
  assert.ok(f.exact.worldCarry, 'first refusal consumed the carry');
  f.create(2, 'other');
  assert.equal(f.exact.worldCarry, undefined);
  assert.match(f.exact.gpu.agent(1,{op:'state'}).world.restoreError, /refused/);
  assert.equal(f.exact.gpu.decorate({op:'state'}, {}).error, undefined);
});
test('terminal restore refusal is reported once and remains in canvas state and journal', async () => {
  const f = await fixture({refuse:()=>true}); f.exact.worldCarry = new Uint8Array([7]); f.create(1);
  assert.match(f.exact.gpu.decorate({op:'tap'}, {}).error, /refused/);
  assert.equal(f.exact.gpu.decorate({op:'tap'}, {}).error, undefined);
  assert.match(f.exact.gpu.agent(1,{op:'state'}).world.restoreError, /refused/);
  assert.ok(f.exact.gpu.decorate({op:'logs'}, {}).world.some(w=>w.lines.some(l=>l.includes('refused'))));
});
test('shader preparation refuses a changed module instead of committing into it', async () => {
  const f = await fixture(); f.create(1);
  const commit = await f.exact.gpu.prepareShaders([]);
  await f.exact.gpu.swap(1);
  assert.throws(commit, /GPU changed/);
});
test('plan preparation and commit share the swap mutation queue', async () => {
  const f = await fixture(); f.create(1);
  let release; const blocked = new Promise(r=>release=r);
  const plan = f.exact.mutate(async () => { const commit = await f.exact.gpu.prepareShaders([]); await blocked; commit(); f.order.push('plan commit'); });
  const swap = f.exact.gpu.swap(1);
  await Promise.resolve(); release(); await Promise.all([plan, swap]);
  assert.ok(f.order.indexOf('plan commit') < f.order.indexOf('old destroy'));
});
test('capture rejects an oversized world before base64 encoding', async () => {
  const f = await fixture(); f.create(1); f.gpu.gpu_carry = () => ({length:256*1024*1024+1});
  assert.throws(()=>f.exact.gpu.handle({id:1,op:'screenshot',form:'save'},null,x=>x), /world carrier exceeds 256 MiB/);
});

test('restored keydowns own keyups in the new browser, including queued keydowns', async () => {
  const f = await fixture({input:true, forwarded:['Space']}); f.exact.worldCarry = new Uint8Array([7]);
  const el = f.create(1), button = new f.Element('button');
  el.listeners.keyup({target:button,code:'Space',timeStamp:0});
  assert.deepEqual(f.events.map(e=>[e.code,e.down]), [['Space',false]]);
});

test('bootstrap restores pending bytes before staging its first render', async () => {
  let restored = false;
  const f = await fixture({loadFail:true, nextGpu:{
    gpu_restore(){restored=true;return true;},
    gpu_agent:()=>JSON.stringify({world:{restored}}),
    gpu_render(){assert.ok(restored, 'render preceded pending restore');return 0;},
  }});
  f.exact.worldCarry = new Uint8Array([7]); f.create(1);
  await f.exact.gpu.swap(1);
  assert.equal(f.exact.worldCarry,undefined);
});
test('a failed later staged surface does not consume a pending file carry', async () => {
  let count = 0;
  const f = await fixture({loadFail:true, nextGpu:{gpu_create:()=>++count===2 ? 0 : count}});
  f.exact.worldCarry = new Uint8Array([7]); f.create(1); f.create(2,'other');
  await assert.rejects(f.exact.gpu.swap(1), /create/);
  assert.deepEqual(f.exact.worldCarry,new Uint8Array([7]));
});

test('an in-flight shader check stays bound to its original module', async () => {
  const f=await fixture(); f.create(1);
  let release; const waiting=new Promise(r=>release=r);
  f.gpu.gpu_shader_names=()=> '["sky"]'; f.gpu.gpu_shader_check=()=>waiting;
  const preparing=f.exact.gpu.prepareShaders(new Map([['shaders/sky.wgsl',{bytes:new TextEncoder().encode('shader')}]]));
  await Promise.resolve();
  await f.exact.gpu.swap(1); release(true);
  await assert.rejects(preparing,/GPU changed during shader preparation/);
});

test('a hard bind refusal during lazy load does not prevent a different initial surface', async () => {
  const f=await fixture({pendingCount:2,initialBindFail:1});
  assert.equal(f.exact.gpu.agent(1,{op:'state'}),null);
  assert.ok(f.exact.gpu.agent(2,{op:'state'}));
  assert.ok(f.diagnostics.some(line=>line.includes('surface-1: bind')));
});

test('a direct world operation also receives the refusal that created its canvas', async () => {
  const f=await fixture({refuse:()=>true}); f.exact.worldCarry=new Uint8Array([7]); f.create(1);
  assert.match(f.exact.gpu.decorate({op:'screenshot',world:true},{bytes:1}).error,/restore refused/);
  assert.equal(f.exact.gpu.decorate({op:'state'},{}).error,undefined);
});


test('device loss replaces contexts without creating, restoring or unloading surfaces', async () => {
  const f = await fixture({input:true});
  const el = f.create(1); f.create(2, 'second');
  el.listeners.keydown({target:el, code:'KeyW', key:'w', timeStamp:1});
  const ids = [];
  f.gpu.gpu_recover = async (surfaces, canvases) => {
    ids.push(...surfaces); assert.equal(canvases.length, 2);
    return JSON.stringify({status:'recovered', instances:[...surfaces]});
  };
  f.gpu.gpu_create = () => { throw new Error('recovery recreated the surface'); };
  f.gpu.gpu_restore = () => { throw new Error('recovery restored a save'); };
  f.exact.gpu.deviceLost();
  await new Promise(resolve => setTimeout(resolve, 0));
  await f.exact.gpu.settled();
  assert.deepEqual(ids, [1,2]);
  assert.equal(f.order.filter(x => x === 'replace').length, 2);
  assert.ok(!f.order.some(x => x.includes('destroy') || x.includes('unload')));
  assert.equal(f.exact.gpu.recovery.status, 'recovered');
  assert.equal(f.exact.gpu.agent(1, {op:'state'}).world.tick, 0);
  el.listeners.keyup({target:el, code:'KeyW', key:'w', timeStamp:2});
  assert.deepEqual(f.events.filter(e=>e.t==='key').map(e=>e.down), [true,false]);
});


test('failed recovery removes clones and backs off instead of retrying each frame', async () => {
  const f=await fixture(); f.create(1);
  let attempts=0;
  f.gpu.gpu_render=()=>3;
  f.gpu.gpu_recover=async()=>{ attempts++; throw new Error('fail once'); };
  f.exact.gpu.deviceLost();
  await new Promise(r=>setTimeout(r,0));
  assert.equal(attempts,1);
  assert.equal(f.order.filter(x=>x==='remove clone').length,1);
  for(let i=0;i<10;i++) { f.frame(); f.exact.gpu.deviceLost(); }
  await new Promise(r=>setTimeout(r,0));
  assert.equal(attempts,1,'retry is held until the backoff expires');
  f.gpu.gpu_recover=async()=>{attempts++;f.gpu.gpu_render=()=>0;return '{"status":"recovered"}';};
  await new Promise(r=>setTimeout(r,150));
  assert.equal(attempts,2); assert.equal(f.exact.gpu.recovery.status,'recovered');
});


test('restore diagnostics wrap an already named refusal exactly once', async () => {
  const f=await fixture({refuse:()=>true,error:'restore refused: EXSIM v5 awaits declared assets'});
  f.exact.worldCarry=new Uint8Array([7]); f.create(1);
  const error=f.exact.gpu.decorate({op:'tap'},{}).error;
  assert.equal(error.match(/restore refused/g).length,1,error);
});


test('a second loss during recovery schedules the next generation', async () => {
  const f=await fixture(); f.create(1);
  let finish, attempts=0;
  f.gpu.gpu_recover=async()=>{if(++attempts===1) await new Promise(r=>finish=r);return '{"status":"recovered"}';};
  f.exact.gpu.deviceLost(); await new Promise(r=>setTimeout(r,0));
  f.exact.gpu.deviceLost(); finish();
  await new Promise(r=>setTimeout(r,0));
  assert.equal(attempts,2);
  assert.equal(f.exact.gpu.recovery.status,'recovered');
});

test('Contract controls send named local contacts, keyboard edges and blur over gpu_input', async () => {
  const f = await fixture({input:true}), canvas=f.create(1), button=new f.Element('button');
  button.parent=canvas;
  button.closest = selector => selector === '[data-gpu-input]' ? canvas : selector === 'button[data-action]' ? button : null;
  button.getAttribute = name => name === 'data-action' ? 'jump' : null;
  button.getBoundingClientRect = () => ({left:100,top:200});
  button.setPointerCapture = () => {};
  const event = {target:button,pointerId:7,clientX:130,clientY:240,timeStamp:500,preventDefault(){}};
  canvas.listeners.pointerdown(event); canvas.listeners.pointermove({...event,clientX:160}); canvas.listeners.pointerup(event);
  canvas.listeners.keydown({...event,code:'Space',key:' '}); canvas.listeners.keyup({...event,code:'Space',key:' '});
  canvas.listeners.focusout({...event,relatedTarget:null});
  await Promise.resolve();
  assert.deepEqual(f.events.slice(0,3).map(e=>[e.t,e.name,e.phase,e.id,e.x,e.y,e.at]), [
    ['control','jump','down',7,30,40,0],['control','jump','move',7,60,40,0],['control','jump','up',7,30,40,0]]);
  assert.deepEqual(f.events.slice(3).map(e=>[e.t,e.phase]), [['control','down'],['control','up'],['blur',undefined]]);
});

test('focusout without a destination releases only input that remains outside the live canvas', async () => {
  const f=await fixture({input:true}),canvas=f.create(1);
  const key=code=>({target:canvas,code,key:code,timeStamp:0,preventDefault(){}});
  canvas.listeners.keydown(key('KeyW'));
  f.document.activeElement=null;
  canvas.listeners.focusout({target:canvas,relatedTarget:null,timeStamp:0});
  f.document.activeElement=canvas;
  await Promise.resolve();
  canvas.listeners.keyup(key('KeyW'));
  assert.deepEqual(f.events.map(e=>[e.t,e.down]),[['key',true],['key',false]]);
  canvas.listeners.keydown(key('KeyD'));
  f.document.activeElement=null;
  canvas.listeners.focusout({target:canvas,relatedTarget:null,timeStamp:0});
  await Promise.resolve();
  canvas.listeners.keyup(key('KeyD'));
  assert.deepEqual(f.events.slice(2).map(e=>[e.t,e.down]),[['key',true],['blur',undefined]]);
  canvas.listeners.focusout({target:canvas,relatedTarget:null,timeStamp:0});
  f.destroy(1);f.create(1);
  await Promise.resolve();
  assert.equal(f.events.filter(e=>e.t==='blur').length,1,'retired listener must not blur a replacement');
});

test('pointer release retains action control focus and keeps feeding world keys', async () => {
  const f=await fixture({input:true}),canvas=f.create(1),button=restoredButton(f,canvas);
  button.focus=()=>{f.document.activeElement=button;};
  button.blur=()=>{f.document.activeElement=null;canvas.listeners.focusout({target:button,relatedTarget:null});};
  canvas.focus=()=>{const target=f.document.activeElement;f.document.activeElement=canvas;canvas.listeners.focusout({target,relatedTarget:canvas});};
  const event={target:button,pointerId:7,clientX:10,clientY:20,timeStamp:0,preventDefault(){}};
  canvas.listeners.pointerdown(event);canvas.listeners.pointerup(event);
  assert.equal(f.document.activeElement,button);
  assert.deepEqual(f.events.map(e=>e.phase??e.t),['down','up']);
  canvas.listeners.keydown({...event,target:canvas,code:'KeyW',key:'w'});
  canvas.listeners.focusout({target:canvas,relatedTarget:new f.Element('input')});
  canvas.listeners.keyup({...event,target:canvas,code:'KeyW',key:'w'});
  assert.deepEqual(f.events.slice(2).map(e=>[e.t,e.down]),[['key',true],['blur',undefined]]);
});

for (const change of ['rename','clear','remove','disabled','hidden','inert']) test(`held control retains its binding through ${change}`, async () => {
  const f=await fixture({input:true}), canvas=f.create(1), button=new f.Element('button');
  let name='jump'; button.parent=canvas;
  button.closest=s=>s==='[data-gpu-input]'?canvas:s==='button[data-action]'?button:s==='[inert]'&&button.inert?button:null;
  button.getAttribute=()=>name; button.getBoundingClientRect=()=>Object.create({get left(){return 100;},get top(){return 200;}});
  button.setPointerCapture=()=>{throw Error('capture unavailable');};
  let focused=false;button.focus=()=>{focused=true;};
  const e={target:button,pointerId:7,clientX:130,clientY:240,timeStamp:0,preventDefault(){}};
  assert.doesNotThrow(()=>canvas.listeners.pointerdown(e)); assert.ok(focused);
  if(change==='rename') name='light';
  if(change==='clear') name=null;
  if(change==='remove') button.isConnected=false;
  if(change==='disabled') button.disabled=true;
  if(change==='hidden') button.hidden=true;
  if(change==='inert') button.inert=true;
  canvas.listeners.pointerup({...e,target:canvas});
  assert.equal(f.events.at(-1).name,'jump');
  assert.ok(['up','cancel'].includes(f.events.at(-1).phase));
});
test('two captured pointers keep CSS coordinates and lost capture cancels just its binding', async()=>{
  const f=await fixture({input:true}), canvas=f.create(1);
  for(const id of [7,8]) {
    const b=new f.Element('button'); b.parent=canvas;
    b.closest=s=>s==='[data-gpu-input]'?canvas:s==='button[data-action]'?b:null;
    b.getAttribute=()=>id===7?'jump':'move'; b.getBoundingClientRect=()=>({left:100,top:200}); b.setPointerCapture=()=>{};
    canvas.listeners.pointerdown({target:b,pointerId:id,clientX:130,clientY:240,timeStamp:0,preventDefault(){}});
  }
  canvas.listeners.pointermove({target:canvas,pointerId:8,clientX:300,clientY:400,timeStamp:0,preventDefault(){}});
  canvas.listeners.lostpointercapture({target:canvas,pointerId:7,timeStamp:0});
  canvas.listeners.pointerup({target:canvas,pointerId:8,clientX:300,clientY:400,timeStamp:0,preventDefault(){}});
  assert.deepEqual(f.events.map(e=>[e.name,e.phase,e.id]),[['jump','down',7],['move','down',8],['move','move',8],['jump','cancel',7],['move','up',8]]);
  assert.equal(f.events[2].x,200);
});
for(const status of ['healthy','no device']) test(`recovery ${status} leaves canvases intact`,async()=>{
 const f=await fixture();f.create(1);f.gpu.gpu_recover=async()=>JSON.stringify({status});
 f.exact.gpu.deviceLost();await new Promise(r=>setTimeout(r,0));
 assert.ok(!f.order.includes('replace'));assert.equal(f.exact.gpu.recovery.status,status);
});
test('second cutover failure preserves both live canvases and their input',async()=>{
 const f=await fixture({input:true}), first=f.create(1), second=f.create(2,'other');
 const replace=second.canvas.replaceWith;second.canvas.replaceWith=()=>{throw Error('second install');};
 f.gpu.gpu_recover=async()=>'{"status":"recovered"}';
 f.exact.gpu.deviceLost();await new Promise(r=>setTimeout(r,0));
 assert.ok(first.canvas.isConnected);assert.ok(second.canvas.isConnected);
 first.listeners.keydown({target:first,code:'KeyW',timeStamp:0});assert.equal(f.events.at(-1).code,'KeyW');
 second.canvas.replaceWith=replace;
 await new Promise(r=>setTimeout(r,150));
});

 test('fresh host restores exact keyboard control ownership without a new press',async()=>{
 const f=await fixture({input:true,controlContacts:[{id:4294967294,action:'jump',origin:[0,0],position:[0,0]}]});f.exact.worldCarry=new Uint8Array([7]);const canvas=f.create(1);
 canvas.listeners.keyup({target:canvas,code:'Space',timeStamp:0,preventDefault(){}});
 assert.deepEqual(f.events.map(e=>[e.t,e.name,e.id,e.phase]),[['control','jump',4294967294,'up']]);
 });

for(const refusal of ['disabled','hidden','inert','missing']) test(`control down refuses ${refusal}`,async()=>{
 const f=await fixture({input:true}),canvas=f.create(1),b=new f.Element('button');b.parent=canvas;
 b.closest=s=>s==='[data-gpu-input]'?canvas:s==='button[data-action]'?b:s.includes('[inert]')&&refusal==='inert'?b:null;
 b.getAttribute=()=>refusal==='missing'?null:'jump';b.disabled=refusal==='disabled';b.hidden=refusal==='hidden';
 canvas.listeners.pointerdown({target:b,pointerId:1,clientX:0,clientY:0,timeStamp:0,preventDefault(){}});
 assert.deepEqual(f.events,[]);
});
test('fresh host can release a saved pointer contact by its original identity',async()=>{
 const f=await fixture({input:true,controlContacts:[{id:7,action:'move',origin:[20,30],position:[80,30]}]});f.exact.worldCarry=new Uint8Array([7]);f.create(1);
 const reply=f.exact.gpu.handle({op:'tap',id:1,contact:7,phase:'cancel'},null,x=>x);
 assert.equal(reply.delivery,'recognized');assert.deepEqual(f.events.map(e=>[e.name,e.id,e.phase,e.x,e.y]),[['move',7,'cancel',80,30]]);
});
test('attachment exception rolls back the published map and keeps both inputs live',async()=>{
 const f=await fixture({input:true}),a=f.create(1),b=f.create(2,'other');let fail=true;
 f.gpu.gpu_recover=async()=>'{"status":"recovered"}';
 f.gpu.gpu_wants_input=id=>{if(id===2&&fail){fail=false;throw Error('attach');}return true;};
 f.exact.gpu.deviceLost();await new Promise(r=>setTimeout(r,0));
 assert.ok(a.canvas.isConnected&&b.canvas.isConnected);
 for(const el of [a,b]) el.listeners.keydown({target:el,code:'KeyW',timeStamp:0});
 assert.equal(f.events.length,2);await new Promise(r=>setTimeout(r,150));
 assert.equal(f.exact.gpu.recovery.status,'recovered');
});

for (const change of ['clear','remove']) test(`DOM ${change} cancels a held control without another pointer event`,async()=>{
  const f=await fixture({input:true}), canvas=f.create(1), b=new f.Element('button');
  b.parent=canvas; let name='jump';
  b.closest=s=>s==='[data-gpu-input]'?canvas:s==='button[data-action]'?b:null;
  b.getAttribute=()=>name; b.getBoundingClientRect=()=>({left:10,top:20});
  canvas.listeners.pointerdown({target:b,pointerId:7,clientX:20,clientY:30,timeStamp:0,preventDefault(){}});
  if(change==='clear')name=null;else b.isConnected=false;
  f.mutation();
  assert.deepEqual(f.events.map(e=>[e.name,e.phase,e.id]),[['jump','down',7],['jump','cancel',7]]);
});

for (const status of ['healthy','no device']) test(`recovery ${status} preserves a pending asset flight`, async () => {
  let resolveFetch, signal, delivered = 0;
  const f = await fixture({delivery:{fetch:(_url, options) => { signal=options.signal; return new Promise(r=>resolveFetch=r); }}});
  let pending = true;
  f.gpu.gpu_assets = () => JSON.stringify({requests: pending ? (pending=false, ['fox.model']) : [], retired:[]});
  f.gpu.gpu_asset = () => { delivered++; return true; };
  f.create(1); f.gpu.gpu_recover=async()=>JSON.stringify({status});
  f.exact.gpu.deviceLost(); await new Promise(r=>setTimeout(r,0));
  const aborted = signal.aborted;
  resolveFetch(new Response(new Uint8Array([1]))); await f.exact.gpu.settled();
  assert.equal(aborted,false); assert.equal(delivered,1);
});
for (const site of ['before swap','after swap','attach']) test(`first canvas failure ${site} restores each listener exactly once`, async () => {
  const f=await fixture({input:true}), a=f.create(1), b=f.create(2,'other');
  const replace=a.canvas.replaceWith; let fail=true;
  if(site==='attach') f.gpu.gpu_wants_input=id=>{if(id===1&&fail){fail=false;throw Error('attach');}return true;};
  else a.canvas.replaceWith=function(el){if(site==='after swap')replace.call(this,el);if(fail){fail=false;throw Error('install');} if(site==='before swap')replace.call(this,el);};
  f.gpu.gpu_recover=async()=>' {"status":"recovered"}';
  f.exact.gpu.deviceLost(); await new Promise(r=>setTimeout(r,0));
  assert.ok(a.canvas.isConnected && b.canvas.isConnected);
  for(const el of [a,b]) el.listeners.keydown({target:el,code:'KeyW',timeStamp:0});
  const count=f.events.length;
  await new Promise(r=>setTimeout(r,150));
  assert.equal(count,2); assert.equal(f.exact.gpu.recovery.status,'recovered');
});
function restoredButton(f, canvas) {
  const b=new f.Element('button'); b.parent=canvas; canvas.buttons=[b];
  b.closest=s=>s==='[data-gpu-input]'?canvas:s==='button[data-action]'?b:null;
  b.getAttribute=()=>b.action; b.action='jump'; b.getBoundingClientRect=()=>({left:10,top:20}); return b;
}
test('control pointer down preserves an active editor', async () => {
  const f=await fixture({input:true}), canvas=f.create(1), b=restoredButton(f,canvas);
  const input=f.document.activeElement=new f.Element('input'); b.focus=()=>{f.document.activeElement=b;};
  canvas.listeners.pointerdown({target:b,pointerId:1,clientX:10,clientY:20,timeStamp:0,preventDefault(){}});
  assert.equal(f.document.activeElement,input); assert.equal(f.events[0].phase,'down');
});
for (const change of ['clear','remove']) test(`restored contact cancels on ${change}`, async () => {
  const options={input:true,controlContacts:[{id:7,action:'jump',position:[1,2]}]};
  const f=await fixture(options); f.exact.worldCarry=new Uint8Array([7]); const canvas=f.create(1), b=restoredButton(f,canvas);
  f.exact.finishRestore(1);
  if(change==='clear')b.action=null;else b.isConnected=false;
  f.mutation(); assert.deepEqual(f.events.map(e=>[e.id,e.phase]),[[7,'cancel']]);
});
test('a second restore replaces host ownership immediately', async () => {
  const options={input:true,controlContacts:[{id:7,action:'jump',position:[1,2]}]};
  const f=await fixture(options); f.exact.worldCarry=new Uint8Array([7]); const canvas=f.create(1);
  options.controlContacts=[]; f.exact.finishRestore(1);
  canvas.listeners.pointerup({target:canvas,pointerId:7,clientX:0,clientY:0,timeStamp:0,preventDefault(){}});
  assert.ok(f.events.every(e=>e.t!=='control'));
});


test('R12 reparent cancels the original canvas contact',async()=>{
  const f=await fixture({input:true}),a=f.create(1),b=f.create(2,'other'),button=restoredButton(f,a);
  let canvas=a; button.closest=s=>s==='[data-gpu-input]'?canvas:s==='button[data-action]'?button:null;
  a.listeners.pointerdown({target:button,pointerId:7,clientX:10,clientY:20,timeStamp:0,preventDefault(){}});
  canvas=b;button.parent=b;f.mutation();
  assert.deepEqual(f.events.map(e=>e.phase),['down','cancel']);
});
test('R12 duplicate restored actions have no guessed node owner',async()=>{
  const f=await fixture({input:true,controlContacts:[{id:7,action:'jump',position:[1,2]}]});
  f.exact.worldCarry=new Uint8Array([7]);const canvas=f.create(1),a=restoredButton(f,canvas),b=restoredButton(f,canvas);canvas.buttons=[a,b];
  f.exact.finishRestore(1);a.isConnected=false;f.mutation();
  assert.deepEqual(f.events,[]);
  b.isConnected=false;canvas.buttons=[];f.mutation();
  assert.equal(f.events.filter(e=>e.t==='control'&&e.phase==='cancel'&&e.id===7).length,1);
});
test('R12 pressed control routes Space from an editor without focusing',async()=>{
  const f=await fixture({input:true}),canvas=f.create(1),button=restoredButton(f,canvas),editor=f.document.activeElement=new f.Element('input');
  canvas.listeners.pointerdown({target:button,pointerId:7,clientX:10,clientY:20,timeStamp:0,preventDefault(){}});
  const event=new Event('keydown',{cancelable:true});Object.defineProperties(event,{target:{value:editor},code:{value:'Space'}});f.window.dispatchEvent(event);
  assert.deepEqual(f.events.map(e=>[e.phase,e.id]),[['down',7],['down',4294967294]]);
  assert.equal(f.document.activeElement,editor);assert.equal(event.defaultPrevented,true);
});
test('R12 blur cancels restored bindings before the engine blur',async()=>{
  const f=await fixture({input:true,controlContacts:[{id:7,action:'jump',position:[1,2]}]});f.exact.worldCarry=new Uint8Array([7]);f.create(1);
  f.window.dispatchEvent(new Event('blur'));
  assert.deepEqual(f.events.map(e=>e.phase??e.t),['cancel','blur']);
});


test('R13 named and empty argument objects survive the web batch and GPU binding path',async()=>{
  const f=await fixture(), seen=[];
  f.gpu.gpu_bind_at=(id,json)=>{seen.push(JSON.parse(json));return true;};
  f.create(1);
  for(const values of [{restart:false,seed:7,paused:true},{}]) {
    await f.applyBatch({ops:[{op:'surface',id:1,name:'world',values}],timers:false,motion:false});
    assert.deepEqual(seen.at(-1),values);
  }
});

test('E10 focused Contract buttons consume activation keys before the world', async () => {
  const f = await fixture({input:true}), canvas=f.create(1), pause=new f.Element('button');
  let prevented=0; pause.click=()=>{};
  for (const code of ['Space','Enter','NumpadEnter']) {
    canvas.listeners.keydown({target:pause,code,timeStamp:0,preventDefault(){prevented++;}});
    canvas.listeners.keyup({target:pause,code,timeStamp:0,preventDefault(){}});
  }
  assert.equal(prevented,3);
  assert.equal(f.events.length,0);
  canvas.listeners.keydown({target:canvas,code:'KeyW',timeStamp:0});
  assert.equal(f.events.at(-1).code,'KeyW');
});
