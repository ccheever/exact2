import {assetDelivery} from '../gpu-assets.js';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Run the production lazy module and applyBatch with a deterministic GPU and
// presenter. The runner's returned batch addresses the *final* outer tree.
export async function fixture(options = {}) {
  const views = new Map(), records = [], diagnostics = [], observers = [];
  let next = 0, hud = null, expectedView = null;
  const changed = new Map(), events = [], order = [], restored = new Set();
  let mutations = Promise.resolve(), frame;
  const window = new EventTarget();
  const document = { createElement: () => ({}), head: { append() {} }, activeElement:{}, hidden:false, baseURI:"http://fixture/", addEventListener() {} };
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
    gpu_error: () => options.error ?? "fixture refusal", gpu_render: () => 0, gpu_flush: () => true, gpu_dirty: () => false,
    gpu_agent: id => JSON.stringify({world:{tick:0,restored:restored.has(id),input:{forwarded:options.forwarded ?? [],controlContacts:options.controlContacts ?? []}}, lines:[], from:0, next:0}),
    gpu_input: (id, json) => { events.push(JSON.parse(json)); return true; }, gpu_shader_check: async () => true,
    gpu_shader: () => true,
    gpu_assets: () => '{"requests":[],"retired":[]}', gpu_asset: () => true, gpu_lifecycle: () => true, gpu_clock: () => true, gpu_period: () => {},
  };
  const glue = readFileSync(process.env.R8A_GLUE_SOURCE || new URL('../glue.js', import.meta.url), 'utf8');
  const applySource = glue.slice(glue.indexOf('function applyBatch(batch)'), glue.indexOf('\nfunction send(', glue.indexOf('function applyBatch(batch)')));
  const operationSource = glue.slice(glue.indexOf('function apply(batch)'), glue.indexOf('function applyBatch(batch)', glue.indexOf('function apply(batch)')));
  const applyOperations = new Function('exact', 'views', 'globalThis', `
    const retiredViews = new WeakSet(), followedScrolls = new Map(), pendingScrolls = new Map();
    const listSelection = null, syncLists = () => {}, collections = {commit() {}}, motion = {style(id, text) { const el = views.get(id); if (el) el.style.cssText = text; }, destroy() {}}, arrange={destroy() {}}, presence = {live: null};
    const root = {}, log = () => {}, navigation = {project() {}}, inputReady = false, markScrollDocument = () => {};
    const prepareContexts = () => {}, runFocusCommands = () => {}, inertAncestor = () => false, refreshSymbols = () => {}, focusAutofocus = () => {}, positionContexts = () => {};
    const viewFor = (_, id) => views.get(id);
    ${operationSource}; return apply;
  `)(exact, views, {exact});
  const applyBatch = new Function('globalThis', 'apply', `const agentMode = false, textflow = null, page = null, presence = {hold: () => false}, letGo = () => {}, motion = {commit() {}}, arrange = {commit() {}}, flowBatch = () => {}; let timelinesMoved = false; ${applySource}; return applyBatch;`)({ exact }, batch => { for (const op of batch.ops) { if (typeof op === 'function') op(); else applyOperations({ops:[op]}); } });
  const nextGpu = {...gpu, gpu_load() {}, gpu_unload() { order.push("next unload"); },
    gpu_create: () => { order.push("next create"); return options.createFail ? 0 : ++next; },
    gpu_bind_at: () => { order.push("next bind"); return !options.bindFail; },
    gpu_destroy() { order.push("next destroy"); }, ...options.nextGpu};
  const source = readFileSync(process.env.E2B_GPU_SOURCE || new URL('../gpu-glue.js', import.meta.url), 'utf8')
    .replace('import { assetDelivery } from "./gpu-assets.js";', '')
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
    replaceWith(el) { order.push("replace"); this.isConnected = false; el.isConnected = true; }
    getAttribute() { return null; }
    removeAttribute() {}
    setAttribute() {}
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
    'assetDelivery', 'globalThis', 'candidate', 'document', 'Element', 'devicePixelRatio', 'MutationObserver', 'ResizeObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'location', 'console', 'window',
    source + `;exact.finishRestore = (view) => { const e = surfaces.get(view); e.pendingRestore = {bytes:new Uint8Array([7])}; finishRestore(e, gpu); };`
  )(settings => assetDelivery({...settings, ...options.delivery}), { exact }, async version => version ? nextGpu : gpu, document, Element, 3, class { constructor(fn) { observers.push(fn); } observe() {} disconnect() {} },
    class { observe() {} disconnect() {} }, fn => { if (fn.name === "frame") frame = fn; return 1; }, () => {}, { search: '' }, { error: (...args) => diagnostics.push(args.join(' ')), info() {} }, window);
  function create(id, name = 'world') {
    const el = new Element("host"); el.canvas = new Element();
    views.set(id, el); exact.gpu.surface(id, name, []); return el;
  }
  function destroy(id) { views.delete(id); exact.gpu.destroy(id); }
  return { window, document, exact, records, diagnostics, create, destroy, applyBatch, events, order, gpu, nextGpu, Element, publish:(id,value)=>changed.set(id,JSON.stringify(value)), mutation: () => observers.forEach(fn => fn()),
    frame: () => frame?.(0), expectView: id => { expectedView = id; }, stale: () => { hud = 'stale'; }, hud: () => hud };
}
