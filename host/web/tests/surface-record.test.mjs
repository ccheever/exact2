import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Run the production lazy module and applyBatch with a deterministic GPU and
// presenter. The runner's returned batch addresses the *final* outer tree.
export async function fixture(options = {}) {
  const views = new Map(), records = [], diagnostics = [];
  let next = 0, hud = null, expectedView = null;
  const changed = new Map(), events = [], order = [], restored = new Set();
  let mutations = Promise.resolve(), frame;
  const exact = { mutate: fn => { const p = mutations.then(fn); mutations = p.catch(() => {}); return p; }, views, root: { dataset: {} }, now: () => 0, devAssets: [],
    writeIn: text => text, wasm: { exact_surface_record(text) {
      records.push(text);
      return { ops: [() => {
        if (expectedView !== null) assert.ok(views.has(expectedView), 'recommit addressed a view before create');
        hud = text;
      }] };
    } }, send: batch => applyBatch(batch),
  };
  const gpu = { default() {}, gpu_unload() { order.push("old unload"); }, gpu_load() { if (options.loadFail) throw new Error("initial load failed"); }, gpu_seekable() {}, gpu_shader_names: () => '[]', gpu_shaders_clear() {},
    gpu_create: () => ++next, gpu_bind_at(id) { if (options.initialBindFail === id) return false; changed.set(id, JSON.stringify({ value: id })); return true; },
    gpu_published(id) { const r = changed.get(id); changed.delete(id); return r; },
    gpu_messages: () => undefined, gpu_wants_input: () => Boolean(options.input), gpu_destroy() { order.push("old destroy"); },
    gpu_carry: () => new Uint8Array([1]), gpu_restore(id) { if (options.refuse?.(id)) return false; restored.add(id); return true; },
    gpu_error: () => "fixture refusal", gpu_render: () => 0, gpu_dirty: () => false,
    gpu_agent: id => JSON.stringify({world:{tick:0,restored:restored.has(id),input:{forwarded:options.forwarded ?? []}}, lines:[], from:0, next:0}),
    gpu_input: (id, json) => { events.push(JSON.parse(json)); return true; }, gpu_shader_check: async () => true,
    gpu_shader: () => true,
    gpu_assets: () => '[]', gpu_asset: () => true, gpu_lifecycle: () => true, gpu_clock: () => true, gpu_period: () => {},
  };
  const glue = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
  const applySource = glue.slice(glue.indexOf('function applyBatch(batch)'), glue.indexOf('\nfunction send(', glue.indexOf('function applyBatch(batch)')));
  const applyBatch = new Function('globalThis', 'apply', `const agentMode = false; ${applySource}; return applyBatch;`)({ exact }, batch => { for (const op of batch.ops) op(); });
  const nextGpu = {...gpu, gpu_load() {}, gpu_unload() { order.push("next unload"); },
    gpu_create: () => { order.push("next create"); return options.createFail ? 0 : ++next; },
    gpu_bind_at: () => { order.push("next bind"); return !options.bindFail; },
    gpu_destroy() { order.push("next destroy"); }, ...options.nextGpu};
  const source = readFileSync(process.env.E2B_GPU_SOURCE || new URL('../gpu-glue.js', import.meta.url), 'utf8')
    .replaceAll('import.meta.url', '"http://fixture/"')
    .replace('import { pacer } from "./pace.js";', 'const pacer = () => Object.assign(now => now, {period_ms: 1000 / 120});') // the frame clock is tested in pace.test.mjs
    .replace('await import(`./gpu.js?g=${version}`)', 'await candidate(version)')
    .replace('await import(`./gpu.js${query}`)', 'await candidate(0)')
    .replaceAll('await loadModule(version)', 'await candidate(version)');
  class Element {
    constructor(kind = 'canvas') { this.kind = kind; this.listeners = {}; this.isConnected = true; this.style = {}; this.dataset = {}; this.tabIndex = 0; }
    matches() { return false; }
    querySelector() { return this.canvas; }
    getBoundingClientRect() { return {width:10,height:10}; }
    cloneNode() { return new Element(this.kind); }
    replaceWith(el) { order.push("replace"); this.isConnected = false; el.isConnected = true; }
    getAttribute() { return null; }
    removeAttribute() {}
    setAttribute() {}
    addEventListener(name, fn) { this.listeners[name] = fn; }
    removeEventListener(name) { delete this.listeners[name]; }
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
    'globalThis', 'candidate', 'document', 'Element', 'devicePixelRatio', 'ResizeObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'location', 'console', 'window',
    source
  )({ exact }, async version => version ? nextGpu : gpu, { createElement: () => ({}), head: { append() {} }, activeElement:{}, hidden: false, addEventListener() {} }, Element, 1,
    class { observe() {} disconnect() {} }, fn => { if (fn.name === "frame") frame = fn; return 1; }, () => {}, { search: '' }, { error: (...args) => diagnostics.push(args.join(' ')), info() {} }, { addEventListener() {} });
  function create(id, name = 'world') {
    const el = new Element("host"); el.canvas = new Element();
    views.set(id, el); exact.gpu.surface(id, name, []); return el;
  }
  function destroy(id) { views.delete(id); exact.gpu.destroy(id); }
  return { exact, records, diagnostics, create, destroy, applyBatch, events, order, gpu, nextGpu, Element,
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
