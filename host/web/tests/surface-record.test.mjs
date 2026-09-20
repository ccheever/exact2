import {assetDelivery, assetName} from '../gpu-assets.js';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Run the production lazy module and applyBatch with a deterministic GPU and
// presenter. The runner's returned batch addresses the *final* outer tree.
export async function fixture(options = {}) {
  const views = new Map(), records = [], stagedRecords = [], diagnostics = [];
  let next = 0, hud = null, expectedView = null;
  const checkpointMessages = [], storage = options.storage ?? new Map();
  const changed = new Map(), events = [], order = [], restored = new Set();
  const beacons = [];
  const frames = new Map(), observers = new Set(); let frameId=0;
  class Observer { constructor(callback) { this.callback = callback; } observe() { observers.add(this); } disconnect() { observers.delete(this); } }
  let mutations = Promise.resolve(), frame;
  const window = new EventTarget();
  const document = { createElement: kind => new Element(kind), head: { append() {} }, activeElement:{}, hidden:false, baseURI:"http://fixture/", addEventListener() {} };
  const exact = { compat:{inputs:{app:options.app ?? "fixture.app"}}, message: (host,text) => checkpointMessages.push([host,text]), mutate: fn => { const p = mutations.then(fn); mutations = p.catch(() => {}); return p; }, views, root: { dataset: {} }, now: options.now ?? (() => 0), devAssets: options.devAssets === undefined ? [] : options.devAssets,
    stageCurrent: options.stageCurrent ?? (()=>({batch:{ops:[]},commit(){order.push('host commit');},abort(){order.push('host abort');},present(){}})),
    stageSurfaceRecord(name, json) { stagedRecords.push([name, json]); return options.stageSurfaceRecord?.(name, json) ?? {ops:[]}; },
    writeIn: text => text, wasm: { exact_surface_record(text) {
      records.push(text);
      return { ops: [() => {
        if (expectedView !== null) assert.ok(views.has(expectedView), 'recommit addressed a view before create');
        hud = text;
      }] };
    } }, send: batch => applyBatch(batch),
  };
  const gpu = { default() {}, gpu_unload() { order.push("old unload"); }, gpu_load() { if (options.loadFail) throw new Error("initial load failed"); }, gpu_seekable() {}, gpu_shader_names: () => '[]', gpu_shaders_clear() {},
    gpu_load_headless() {}, gpu_create_headless: () => ++next, gpu_attach: () => true, gpu_create: () => ++next, gpu_bind_at(id) { if (options.initialBindFail === id) return false; changed.set(id, JSON.stringify({ value: id })); return true; },
    gpu_published(id) { const r = changed.get(id); changed.delete(id); return r; },
    gpu_messages: () => undefined, gpu_wants_input: () => Boolean(options.input), gpu_destroy() { order.push("old destroy"); },
    gpu_carry: () => new Uint8Array([1]), gpu_restore(id, bytes, mode) { order.push(`restore mode ${mode}`); if (options.refuse?.(id)) return false; if (!options.deferredRestore) restored.add(id); return true; },
    gpu_error: () => options.error ?? "fixture refusal", gpu_render: () => 0, gpu_dirty: () => false,
    gpu_agent: (id, text) => JSON.parse(text).reload ? JSON.stringify({reload:{values:options.values ?? [],names:options.names ?? [],setupIndices:[]}}) : JSON.stringify({world:{tick:0,restored:restored.has(id),input:{forwarded:options.forwarded ?? [],controlContacts:options.controlContacts ?? []}}, lines:[], from:0, next:0}),
    gpu_input: (id, json) => { events.push(JSON.parse(json)); return true; }, gpu_shader_check: async () => true,
    gpu_shader: () => true,
    gpu_assets: () => '{"requests":[],"retired":[]}', gpu_asset: () => true, gpu_lifecycle: () => true, gpu_clock: () => true, gpu_period: () => {},
    ...options.gpu,
  };
  const glue = readFileSync(process.env.R8A_GLUE_SOURCE || new URL('../glue.js', import.meta.url), 'utf8');
  const applySource = glue.slice(glue.indexOf('function applyBatch(batch)'), glue.indexOf('\nfunction send(', glue.indexOf('function applyBatch(batch)')));
  const operationSource = glue.slice(glue.indexOf('function apply(batch)'), glue.indexOf('function applyBatch(batch)', glue.indexOf('function apply(batch)')));
  const applyOperations = new Function('exact', 'views', 'globalThis', `
    const retiredViews = new WeakSet(), followedScrolls = new Map(), pendingScrolls = new Map();
    const root = {}, log = () => {}, navigation = {project() {}}, inputReady = false;
    const prepareContexts = () => {}, refreshSymbols = () => {}, focusAutofocus = () => {}, positionContexts = () => {};
    const viewFor = (_, id) => views.get(id);
    ${operationSource}; return apply;
  `)(exact, views, {exact});
  const applyBatch = new Function('globalThis', 'apply', `let agentClock = null, hasTimers = false; ${applySource}; return applyBatch;`)({ exact }, batch => { for (const op of batch.ops) { if (typeof op === 'function') op(); else applyOperations({ops:[op]}); } });
  const nextGpu = {...gpu, gpu_load() {}, gpu_unload() { order.push("next unload"); },
    gpu_create: () => { order.push("next create"); return options.createFail ? 0 : ++next; },
    gpu_bind_at: () => { order.push("next bind"); return !options.bindFail; },
    gpu_destroy() { order.push("next destroy"); }, ...options.nextGpu};
  const lifecycleDouble = module => {
    const restore = module.gpu_restore, agent = module.gpu_agent;
    return {...module, gpu_restore(id, bytes, mode) {
      const ok = restore(id, bytes, mode);
      if (ok && !options.deferredRestore) restored.add(id);
      return ok;
    }, gpu_agent(id, json) {
      const text = agent(id, json); if (!text) return text;
      const reply = JSON.parse(text);
      if (reply.world && reply.world.restored === undefined) reply.world.restored = restored.has(id);
      if (reply.reload) { reply.reload.values ??= []; reply.reload.names ??= []; }
      return JSON.stringify(reply);
    }};
  };
  const source = readFileSync(process.env.E2B_GPU_SOURCE || new URL('../gpu-glue.js', import.meta.url), 'utf8')
    .replace('import { assetDelivery, assetName } from "./gpu-assets.js";', '')
    .replaceAll('import.meta.url', '"http://fixture/"')
    .replace('import { pacer } from "./pace.js";', 'const pacer = () => Object.assign(now => now, {period_ms: 1000 / 120});') // the frame clock is tested in pace.test.mjs
    .replace('await import(`./gpu.js?g=${version}`)', 'await candidate(version)')
    .replace('await import(`./gpu.js${query}`)', 'await candidate(0)')
    .replaceAll('await loadModule(version)', 'await candidate(version)');
  class Element {
    constructor(kind = 'canvas') { this.kind = kind; this.listeners = {}; this.handlers = new Map(); this.isConnected = true; this.style = {}; this.dataset = {}; this.tabIndex = 0; }
    matches() { return false; }
    querySelector() { return this.canvas; }
    querySelectorAll() { return this.buttons ?? []; }
    getBoundingClientRect() { return {width:10,height:10}; }
    cloneNode() { order.push('clone'); return new Element(this.kind); }
    remove() { order.push('remove clone'); this.isConnected = false; }
    replaceWith(el) { order.push("replace"); this.isConnected = false; el.isConnected = true; if (this.parent) { this.parent.canvas = el; el.parent = this.parent; } }
    getAttribute(name) { return this.attributes?.[name] ?? null; }
    removeAttribute(name) { delete this.attributes?.[name]; }
    setAttribute(name,value) { (this.attributes ??= {})[name] = String(value); }
    addEventListener(name, fn) { const set = this.handlers.get(name) ?? new Set(); set.add(fn); this.handlers.set(name,set); this.listeners[name] = event => [...set].forEach(f => f(event)); }
    removeEventListener(name, fn) { this.handlers.get(name)?.delete(fn); }
    contains(el) { return el === this || el?.parent === this; }
    closest(selector) { return this.kind === 'button' ? (selector.includes('button') ? this : null) : this.kind === 'input' ? (selector.includes('input') ? this : null) : null; }
    focus() {}
  }
  if (options.pendingCount) {
    exact.generation=0; exact.pendingSurfaces=[];
    for(let id=1;id<=options.pendingCount;id++) {
      const host=new Element('host');host.canvas=new Element();views.set(id,host);
      exact.pendingSurfaces.push({id,name:`surface-${id}`,values:[],generation:0});
    }
  }
  await new (Object.getPrototypeOf(async function() {}).constructor)(
    'assetDelivery', 'assetName', 'globalThis', 'candidate', 'document', 'Element', 'devicePixelRatio', 'MutationObserver', 'ResizeObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'location', 'console', 'window', 'localStorage', 'navigator', 'fetch',
    source + `;exact.entries = () => [...surfaces.values()]; exact.checkpoint = (view,kind) => checkpoint(surfaces.get(view),kind); exact.finishCheckpoint = (view,error) => finishRestore(surfaces.get(view),gpu,error); exact.finishRestore = (view) => { const e = surfaces.get(view); e.pendingRestore = {bytes:new Uint8Array([7])}; finishRestore(e, gpu); };`
  )(settings => assetDelivery({...settings, ...options.delivery}), assetName, { exact }, async version => version ? lifecycleDouble(options.candidate ? await options.candidate(version, {...nextGpu}) : {...nextGpu}) : gpu, document, Element, 3, Observer, Observer, cb=>{if(cb.name === "frame") frame=cb;frames.set(++frameId,cb);return frameId;}, id=>frames.delete(id), { search: options.search ?? '' }, { error: (...args) => diagnostics.push(args.join(' ')), warn: (...args) => diagnostics.push(args.join(' ')), info() {} }, window, {getItem:key=>storage.get(key) ?? null,setItem:(key,value)=>storage.set(key,value)}, {sendBeacon:url=>beacons.push(url)}, options.fetch ?? globalThis.fetch);
  await new Promise(resolve => setTimeout(resolve, 0));
  function create(id, name = 'world', values = []) {
    const el = new Element("host"); el.canvas = new Element(); el.canvas.parent = el;
    views.set(id, el); exact.gpu.surface(id, name, values); return el;
  }
  function destroy(id) { views.delete(id); exact.gpu.destroy(id); }
  return { window, document, exact, beacons, records, diagnostics, create, destroy, applyBatch, events, order, gpu, nextGpu, Element, checkpointMessages, storage, restored, stagedRecords, observers, mutation: () => [...observers].forEach(o => o.callback([])),
    paint(at = performance.now()) { const callbacks=[...frames.values()]; frames.clear(); for(const cb of callbacks) cb(at); },
    frame: () => frame?.(0), expectView: id => { expectedView = id; }, stale: () => { hud = 'stale'; }, hud: () => hud };
}

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
    gpu_agent:(_id, request)=>JSON.stringify(JSON.parse(request).op === "clock"
      ? {reload:{rebased:true, releasedInput:true, values:[], setupIndices:[]}}
      : {world:{restored}}),
    gpu_render(){assert.ok(restored, 'render preceded pending restore');return 0;},
  }});
  f.exact.worldCarry = new Uint8Array([7]); f.create(1);
  await f.exact.gpu.swap(1);
  assert.equal(f.exact.worldCarry,undefined);
});
test('a failed later staged surface does not consume a pending file carry', async () => {
  let count = 0;
  const f = await fixture({gpu:{gpu_load_headless(){throw new Error("ownership unavailable");}}, nextGpu:{gpu_create:()=>++count===2 ? 0 : count}});
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
  assert.deepEqual(f.events.slice(0,3).map(e=>[e.t,e.name,e.phase,e.id,e.x,e.y,e.at]), [
    ['control','jump','down',7,30,40,0],['control','jump','move',7,60,40,0],['control','jump','up',7,30,40,0]]);
  assert.deepEqual(f.events.slice(3).map(e=>[e.t,e.phase]), [['control','down'],['control','up'],['blur',undefined]]);
});

test('pointer release returns focus inside its canvas without clearing world input', async () => {
  const f=await fixture({input:true}),canvas=f.create(1),button=restoredButton(f,canvas);
  button.focus=()=>{f.document.activeElement=button;};
  button.blur=()=>{f.document.activeElement=null;canvas.listeners.focusout({target:button,relatedTarget:null});};
  canvas.focus=()=>{const target=f.document.activeElement;f.document.activeElement=canvas;canvas.listeners.focusout({target,relatedTarget:canvas});};
  const event={target:button,pointerId:7,clientX:10,clientY:20,timeStamp:0,preventDefault(){}};
  canvas.listeners.pointerdown(event);canvas.listeners.pointerup(event);
  assert.equal(f.document.activeElement,canvas);
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


test('checkpoint tokens scope bytes, retain them on refusal, and acknowledge deferred loads only at commit', async () => {
  const f = await fixture({deferredRestore:true});
  const host = f.create(1);
  host.setAttribute('surface-save',1); f.exact.checkpoint(1,'save');
  await Promise.resolve();
  assert.equal(f.storage.size,1);
  assert.equal(f.checkpointMessages.at(-1)[1],'surface-save:saved');
  const saved = [...f.storage.values()][0];
  f.exact.checkpoint(1,'save'); await Promise.resolve();
  assert.equal(f.checkpointMessages.length,1,'unchanged token performs no I/O or reply');
  const duplicate = f.create(2);
  duplicate.setAttribute('surface-save',1); f.exact.checkpoint(2,'save'); await Promise.resolve();
  assert.equal(f.checkpointMessages.at(-1)[1],'surface-save:error');
  assert.equal([...f.storage.values()][0],saved);
  host.setAttribute('surface-load',1); f.exact.checkpoint(1,'load'); await Promise.resolve();
  assert.equal(f.checkpointMessages.length,2,'pending assets cannot claim loaded');
  assert.ok(f.order.includes('restore mode 0'),'user load uses Open semantics');
  f.restored.add(1); f.exact.finishCheckpoint(1); await Promise.resolve();
  assert.equal(f.checkpointMessages.at(-1)[1],'surface-load:loaded');
  f.exact.finishCheckpoint(1); await Promise.resolve(); assert.equal(f.checkpointMessages.length,3);
  f.restored.clear(); host.setAttribute('surface-load',2); f.exact.checkpoint(1,'load');
  f.exact.finishCheckpoint(1,'late asset refusal'); await Promise.resolve();
  assert.equal(f.checkpointMessages.at(-1)[1],'surface-load:error');
  assert.equal([...f.storage.values()][0],saved);
  const other = await fixture({app:'other.app',storage:f.storage});
  const foreign = other.create(1); foreign.setAttribute('surface-load',1); other.exact.checkpoint(1,'load');
  await Promise.resolve(); assert.equal(other.checkpointMessages.at(-1)[1],'surface-load:error');
  const collision = await fixture({app:'fixture',storage:f.storage});
  const dotted = collision.create(1,'app.world'); dotted.setAttribute('surface-load',1); collision.exact.checkpoint(1,'load');
  await Promise.resolve(); assert.equal(collision.checkpointMessages.at(-1)[1],'surface-load:error','app/surface separators cannot collide');
});

test('live resize and reload redraw at the last paced frame time', async () => {
  const renders = [], rebases = [];
  const f = await fixture({gpu: {
    gpu_dirty: () => true,
    gpu_render: (id, w, h, scale, at) => { renders.push(at); return 0; },
    gpu_agent: (id, text) => {
      const q = JSON.parse(text);
      if (q.reload) { rebases.push(q.now); return '{"reload":{}}'; }
      return '{"world":{"restored":true}}';
    },
  }});
  delete f.exact.now;
  f.create(1); await f.exact.gpu.settled();
  f.paint(10);
  for (const observer of f.observers) observer.callback([]);
  await f.exact.gpu.swap(1);
  assert.ok(renders.length >= 3);
  assert.ok(renders.every(at => at === 10));
  assert.deepEqual(rebases, [10, 10]);
  f.paint(11);
  assert.equal(renders.at(-1), 11);
});

test('initial authored carry is explicit and ordinary initial saves retain Open',async()=>{
  for(const [worldMode,mode] of [[undefined,0],['open',0],['carry',1]]) {
    const f=await fixture();f.exact.worldCarry=new Uint8Array([7]);f.exact.worldMode=worldMode;f.create(1);
    assert.ok(f.order.includes(`restore mode ${mode}`));
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
