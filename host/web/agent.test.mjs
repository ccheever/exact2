// @ref LLP 1043.000 §3 D7/D8 — flow settlement must not change LLP 1012's API.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { render } from '../../scripts/agent.mjs';

// Like http-body/request-refusal.test.mjs, run the actual glue with host doubles.
// Include the real public object and nodeDetail so registration and flow facts
// are covered too. No browser geometry or scheduling is simulated as evidence.
const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
const declaration = name => source.match(new RegExp(`(?:async )?function ${name}\\([^]*?\\n\\}`))[0];
const publicObject = source.match(/globalThis.exact = \{[^]*?\n\};/)[0];
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function fixture(agentMode = true) {
  const events = [], state = { slots: { backPresses: 3, navigatePresses: 4 } };
  const outline = { roots: [1], nodes: [{ id: 1, type: 'Text' }] };
  const logs = { next: 1, from: 0, lines: ['boot'] };
  const box = { x: 0, y: 0, width: 100, height: 50, left: 0, top: 0, right: 100, bottom: 50 };
  const el = { isConnected: true, localName: 'p', dataset: {}, clientWidth: 100, clientHeight: 50,
    getBoundingClientRect: () => box, hasAttribute: () => false };
  const root = { dataset: {}, replaceChildren() { events.push('replace'); } };
  const context = vm.createContext({ events, agentMode, root, views: new Map([[1, el]]),
    state, outline, logs, textflow: null, flowLoading: null, flowContexts: [], flowDue: null, lists: new Map(),
    Date: { now: () => 123 }, performance: { now: () => 10 }, TextEncoder, Uint8Array,
    HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLIFrameElement: class {}, HTMLVideoElement: class {},
    document: { activeElement: null, body: {}, querySelector: () => null },
    innerWidth: 300, innerHeight: 200, devicePixelRatio: 1, scrollX: 0, scrollY: 0,
    INHERITED_CSS: {}, getComputedStyle: () => ({}), inertAncestor: () => false,
    navigation: { observation: () => ({ location: '/' }), reset() {} },
    ask: req => req.op === 'state' ? state : req.op === 'logs' ? logs : req.op === 'node' ? { id: req.id, type: 'Text' }
      : req.op === 'tags' ? { epoch: 2, incarnation: 1, clock: 0 } : { error: 'unknown op' },
    tree: () => outline, now: () => 0, environment: () => ({}),
    agentClock: 0, SETTLE_DEADLINE_MS: 20000, inflight: new Set(),
    memory: { buffer: new ArrayBuffer(1024) }, readOut: value => value,
    wasm: { exact_in: () => 0, exact_plan: () => 1, exact_out: () => 0,
      exact_plan_fonts: () => '{}', exact_boot: () => '{"ops":[],"timers":true}',
      exact_boot_plan: () => '{"ops":[],"timers":true}', exact_advance: () => '{"ops":[],"clock":16}' },
    devAssets: null, bootAttempt: 0, encoder: new TextEncoder(), location: { pathname: '/', search: '' },
    prepareFonts: async () => [], commitFonts() {}, releaseAssets() {}, incarnation: 0,
    // Ordinary boot tests model an already-loaded post-paint scheduler.
    timerFactory: () => ({ update() { events.push('ticker'); }, dispose() {} }),
    ticker: null, motion: { reset() {} }, arrange: { reset() {} }, retiredViews: new WeakSet(), followedScrolls: new Map(),
    pendingScrolls: new Set(), collections: { reset() {} }, messageFrames: new Map(),
    storageRequests: null, controllers: new Set(), grants: [],
    commitTurns: [],
    applyBatch: batch => { events.push('batch'); context.agentClock = batch.clock ?? 0;
      queueMicrotask(() => context.commitTurns.push([...events])); return { timers: batch.timers, batch }; },
    activateData: () => events.push('activate'), setInterval: () => { events.push('ticker'); return 1; },
    requestAnimationFrame: () => events.push('raf'), clearInterval() {},
    ready: Promise.resolve(), moduleReady: Promise.resolve(), inputReady: true, logicInfo: null, activeModule: null,
  });
  vm.runInContext(['nodeDetail', 'agent', 'agentSettled', 'tagged', 'clock', 'startClock', 'boot'].map(declaration).join('\n') + '\n' + publicObject, context);
  return context;
}
function plain(reply) {
  expect(reply).toBeDefined();
  expect(typeof reply).toBe('object');
  expect(reply.then).toBeUndefined();
}

test('ordinary reads and inputs are synchronous; the awaited entry returns the same reply', async () => {
  const f = fixture();
  for (const op of ['state', 'tree', 'logs', 'tags', 'layout', 'focus', 'tap', 'type', 'unknown']) {
    const request = { op, id: 1 }, reply = f.exact.agent(request);
    plain(reply);
    expect(await f.exact.agentSettled(request)).toEqual(reply);
  }
  expect(f.exact.agent({ op: 'state' }).slots.backPresses).toBe(3);
  expect(f.exact.agent({ op: 'tree' }).roots[0]).toBe(1);
  expect(await f.exact.agentSettled({ op: 'logs' })).toBe(f.logs);
  expect(f.exact.agent({ op: 'layout', id: 1 }).node.flow).toBeUndefined();
  // No flow means not even one internal microtask before dispatch.
  delete f.state.navigation;
  const pending = f.exact.agentSettled({ op: 'state' });
  expect(f.state.navigation).toEqual({ location: '/' });
  await pending;
  f.wasm = null;
  expect(f.exact.agent({ op: 'state' })).toEqual({ error: 'not booted' });
});

test('pending module and settlement leave synchronous reads usable with last settled facts', async () => {
  const f = fixture(), loading = deferred(), settling = deferred();
  const old = { shapes: [{ kind: 'Circle', cx: 10 }], fragments: [{ start: 0, end: 3 }] };
  const next = { shapes: [{ kind: 'Circle', cx: 20 }], fragments: [{ start: 0, end: 6 }] };
  let facts = old, calls = 0;
  f.flowLoading = loading.promise;
  const pending = f.exact.agentSettled({ op: 'layout', id: 1 });
  let answered = false;
  pending.then(() => { answered = true; });
  // Worst realistic input: a module that has not loaded must not stall reads.
  for (let i = 0; i < 1000; i++) expect(f.exact.agent({ op: 'state' }).slots.backPresses).toBe(3);
  const entered = deferred();
  f.textflow = { facts: () => facts, async settle() { calls++; entered.resolve(); await settling.promise; facts = next; } };
  loading.resolve();
  await entered.promise; // VM promise assimilation may cross more than one microtask.
  expect(calls).toBe(1);
  expect(answered).toBe(false);
  const before = f.exact.agent({ op: 'layout', id: 1 });
  plain(before);
  expect(before.node.flow).toBe(old);
  expect(before.node.flow_shapes).toBe(old.shapes);
  settling.resolve();
  const after = await pending;
  expect(after.node.flow).toBe(next);
  expect(after).toEqual(f.exact.agent({ op: 'layout', id: 1 }));
  expect(calls).toBe(1); // Synchronous reads never trigger settlement work.
  expect(await f.exact.agentSettled({ op: 'logs' })).toBe(f.logs);
});

test('flow loader or executor failure rejects only the awaited entry', async () => {
  const f = fixture();
  f.flowLoading = Promise.reject(new Error('module failed'));
  expect(f.exact.agent({ op: 'tree' })).toBe(f.outline);
  await expect(f.exact.agentSettled({ op: 'tree' })).rejects.toThrow('module failed');
  f.flowLoading = null;
  f.textflow = { settle: async () => { throw Error('settle failed'); } };
  await expect(f.exact.agentSettled({ op: 'tree' })).rejects.toThrow('settle failed');
  expect(f.exact.agent({ op: 'tree' })).toBe(f.outline);
});

test('agent mode alone exposes both entry points; clock keeps its existing Promise', async () => {
  const normal = fixture(false);
  expect(normal.exact.agent).toBeUndefined();
  expect(normal.exact.agentSettled).toBeUndefined();
  const f = fixture();
  const result = f.exact.agent({ op: 'clock', to: 16 });
  expect(typeof result.then).toBe('function');
  let completed = false;
  result.then(() => { completed = true; });
  await Promise.resolve();
  expect(completed).toBe(true); // No added flow microtasks for ordinary apps.
  expect(await result).toEqual({ clock: 16 });
});

test('ordinary boot and restart do not yield between DOM commit and ticker startup', async () => {
  for (const bytes of [null, new Uint8Array([1])]) {
    const f = fixture(false), el = f.views.get(1);
    const boot = f.boot(bytes);
    expect(f.events).toEqual([]);
    expect(await boot).toBe(0);
    const expected = ['replace', 'batch', ...(bytes ? ['activate'] : []), 'ticker', ...(bytes ? ['raf'] : [])];
    expect(f.events).toEqual(expected);
    // A yield after DOM commit would let its queued microtask see no ticker.
    expect(f.commitTurns).toEqual([expected]);
    expect(f.retiredViews.has(el)).toBe(false);
  }
});

test('restart waits for an old flow load, disposes synchronously, and ordinary clocks resume', async () => {
  const f = fixture(false), loading = deferred(), el = f.views.get(1);
  f.flowLoading = loading.promise;
  const restart = f.boot(new Uint8Array([1]));
  await Promise.resolve();
  expect(f.events).toEqual([]);
  f.textflow = { dispose() {
    expect(f.retiredViews.has(el)).toBe(true);
    f.events.push('dispose');
  } };
  loading.resolve();
  await restart;
  expect(f.events).toEqual(['dispose', 'replace', 'batch', 'activate', 'ticker', 'raf']);
  expect(f.textflow).toBeNull();
  expect(f.flowLoading).toBeNull();
});

test('non-flow operation transcripts remain byte-for-byte equal to the existing fixture', () => {
  const samples = JSON.parse(readFileSync(new URL('../../scripts/fixtures/transcript.json', import.meta.url), 'utf8'));
  const expected = readFileSync(new URL('../../scripts/fixtures/transcript.txt', import.meta.url), 'utf8');
  const actual = Object.entries(samples).map(([name, value]) => `--- ${name}\n${render(['empty', 'dropped'].includes(name) ? 'logs' : name, value)}`).join('\n\n') + '\n';
  expect(actual).toBe(expected);
});

// Exercise the moved module through the real attach() adapter, with browser
// scheduling under the test's control. Geometry remains a browser concern.
function inputFixture() {
  const listeners = new Map(), frames = new Map(), sent = [], captured = [];
  let serial = 0;
  const el = { dataset: {}, inert: false, disabled: false, isConnected: true,
    addEventListener(kind, fn) { const list = listeners.get(kind) ?? []; list.push(fn); listeners.set(kind, list); },
    closest() { return this.disabled ? this : null; }, setPointerCapture(id) { captured.push(id); } };
  const buttons = [];
  const f = vm.createContext({ inputReady: false, inputHandlers: null,
    views: new Map([[7, el]]), retiredViews: new WeakSet(), frames, sent, captured, el,
    root: { querySelectorAll: () => buttons },
    document: { addEventListener(kind, fn) { f.keydown = fn; }, activeElement: { closest: () => null } },
    HTMLIFrameElement: class {}, HTMLInputElement: class {}, HTMLButtonElement: class {},
    inertAncestor: node => node.inert, getComputedStyle: () => ({ visibility: 'visible' }),
    requestAnimationFrame(fn) { frames.set(++serial, fn); return serial; },
    cancelAnimationFrame(id) { frames.delete(id); },
    wasm: { exact_dispatch: (...args) => args }, writeIn: value => value, now: () => 0,
    send: value => sent.push(value),
  });
  const module = readFileSync(new URL('./input-glue.js', import.meta.url), 'utf8');
  vm.runInContext(module.replace('export function', 'function') + '\n' + declaration('attach'), f);
  f.attach(el, 7, ['pan']);
  function event(extra = {}) {
    return { isPrimary: true, button: 0, pointerId: 1, clientX: 0, clientY: 0,
      target: { closest: () => null }, prevented: false, stopped: false,
      preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; },
      stopImmediatePropagation() { this.stopped = true; }, ...extra };
  }
  return { f, el, buttons, frames, sent, captured,
    load() {
      vm.runInContext(`inputHandlers = createInputHandlers({ root, views, retiredViews,
        ready: () => inputReady, inertAncestor,
        dispatch: (id, payload) => send(wasm.exact_dispatch(id, 20, writeIn(payload), now())),
      }); inputReady = true;`, f);
    },
    pointer(kind, extra) { const e = event(extra); for (const fn of listeners.get(kind) ?? []) fn(e); return e; },
    key(extra) { const e = event({ key: 'k', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, ...extra }); f.keydown(e); return e; },
    tick() { const pending = [...frames.values()]; frames.clear(); pending.forEach(fn => fn()); },
  };
}

test('post-paint pan keeps one pending frame through 10000 moves and flushes the release', () => {
  const h = inputFixture();
  expect(h.pointer('pointerdown').prevented).toBe(false); // Input is gated while the module is absent.
  h.load();
  expect(h.pointer('pointerdown').prevented).toBe(true);
  expect(h.captured).toEqual([1]);
  h.pointer('pointermove', { clientX: 4 }); h.tick();
  expect(h.sent).toEqual([]); // Preserve the platform slop before activating.
  for (let x = 5; x <= 10004; x++) h.pointer('pointermove', { clientX: x });
  expect(h.frames.size).toBe(1);
  expect(h.sent).toEqual([]);
  h.tick();
  expect(h.sent).toEqual([[7, 20, '10004,0', 0]]); // Negative control: an empty handler fails.
  h.pointer('pointermove', { clientX: 10005, clientY: 2 });
  h.pointer('pointerup', { clientX: 10006, clientY: 3 });
  expect(h.sent.at(-1)).toEqual([7, 20, '2,3', 0]);
  expect(h.frames.size).toBe(0);
  h.pointer('pointerdown'); h.pointer('pointerup', { clientX: 8 });
  expect(h.sent.at(-1)).toEqual([7, 20, '8,0', 0]); // Reuse does not install duplicate listeners.
  expect(h.sent.length).toBe(3);
});

test('pan rejects foreign and editable contacts and drops cancelled or stale queued work', () => {
  for (const extra of [{ isPrimary: false }, { button: 1 }, { target: { closest: () => ({}) } }]) {
    const h = inputFixture(); h.load();
    expect(h.pointer('pointerdown', extra).prevented).toBe(false);
    h.pointer('pointermove', { clientX: 20 }); h.tick();
    expect(h.sent).toEqual([]);
  }
  for (const cancel of [h => h.pointer('pointercancel'), h => h.pointer('lostpointercapture'),
    h => h.f.views.set(7, {}), h => h.f.retiredViews.add(h.el),
    h => { h.el.inert = true; }, h => { h.el.disabled = true; }, h => { h.f.inputReady = false; }]) {
    const h = inputFixture(); h.load(); h.pointer('pointerdown');
    h.pointer('pointermove', { pointerId: 2, clientX: 100 });
    expect(h.frames.size).toBe(0);
    h.pointer('pointermove', { clientX: 20 }); cancel(h); h.tick();
    expect(h.frames.size).toBe(0);
    expect(h.sent).toEqual([]);
  }
});

test('moved keyboard shortcuts preserve modifiers, readiness, repeat and modal gating', () => {
  const h = inputFixture(); h.load(); let clicks = 0;
  const button = { isConnected: true, disabled: false, inert: false,
    getClientRects: () => [{}], getAttribute: () => 'Meta+k Escape', click() { clicks++; } };
  h.buttons.push(button);
  expect(h.key().prevented).toBe(true);
  expect(clicks).toBe(1); // Negative control for the moved document listener.
  expect(h.key({ repeat: true }).prevented).toBe(true);
  expect(clicks).toBe(1);
  for (const extra of [{ isComposing: true }, { defaultPrevented: true }, { shiftKey: true }, { metaKey: false }]) {
    expect(h.key(extra).prevented).toBe(false);
  }
  h.f.inputReady = false; expect(h.key().prevented).toBe(false); h.f.inputReady = true;
  h.f.document.activeElement.closest = () => ({ contains: () => false });
  expect(h.key().prevented).toBe(false);
  h.f.document.activeElement.closest = () => null;
  button.inert = true; expect(h.key().prevented).toBe(false); button.inert = false;
  button.disabled = true; expect(h.key().prevented).toBe(true); button.disabled = false;
  expect(clicks).toBe(1);
  expect(h.key({ key: 'Escape', metaKey: false }).prevented).toBe(true);
  expect(clicks).toBe(2);
});

// @ref LLP 1043.000 §3 D7/D8 — optional host code cannot gate data readiness.
const checkpoint = () => new Promise(resolve => setImmediate(resolve));
async function startupFixture(rustOnly = false) {
  const data = deferred(), input = deferred(), timer = deferred();
  const frames = [], loads = [], errors = [], events = [];
  const exports = { memory: {}, exact_compat: () => '{"inputs":{}}',
    exact_logic: () => '{"appId":"test.startup"}',
    exact_data_ready: () => { events.push('activate'); return '{"ops":[]}'; },
    ...(rustOnly ? {} : { exact_module_artifact() {} }) };
  const f = vm.createContext({ frames, loads, errors, events,
    root: { dataset: {}, setAttribute(key, value) { this[key] = value; } },
    views: new Map(), retiredViews: new WeakSet(), authoredDisabled: new WeakMap(),
    inputReady: false, inputHandlers: null, wasm: null, memory: null, logicInfo: null,
    moduleLoader: null, activeModule: null, timerFactory: null, agentMode: false,
    performance: { now: () => 1 }, t0: 0, URL, localStorage: { length: 0 },
    fetch: async () => ({}), WebAssembly: { instantiateStreaming: async () => ({ instance: { exports } }) },
    moduleCall() {}, rustImports: {}, readOut: value => value,
    boot: async () => events.push('boot'), loadGpuIfNeeded() {}, startClock() {},
    requestAnimationFrame: fn => frames.push(fn), console: { error: error => errors.push(String(error)) },
    motion: { commit() {} }, collections: { dataReady: () => events.push('collections') },
    applyBatch: () => events.push('batch'), inertAncestor: () => false,
    resolveModuleReady: () => events.push('ready'),
    loadAfterPaint(file) {
      loads.push(file);
      if (file === './input-glue.js') return input.promise;
      if (file === './timer-glue.js') return timer.promise;
      if (file === './module-glue.js') return Promise.resolve({ baked: () => data.promise, prepare: async () => ({ id: 0 }) });
      throw new Error('unexpected startup module: ' + file);
    },
  });
  f.exact = {};
  vm.runInContext(['setInputReady', 'activateData', 'main'].map(declaration).join('\n')
    .replaceAll('import.meta.url', '"https://fixture.invalid/glue.js"'), f);
  await f.main();
  expect(loads).toEqual([]); // Neither optional nor app modules run before paint.
  frames.shift()();
  expect(loads).toEqual([]);
  const activation = frames.shift()();
  return { f, data, input, timer, activation };
}

test('pending, failed and invalid optional modules leave Rust and JS apps ready after paint', async () => {
  for (const rustOnly of [false, true]) for (const failure of ['pending', 'reject', 'invalid']) {
    const h = await startupFixture(rustOnly), { f } = h;
    if (!rustOnly) expect(f.inputReady).toBe(false);
    h.data.resolve({});
    await checkpoint();
    expect(f.root.dataset.moduleReady).toBe('true');
    expect(f.inputReady).toBe(true);
    expect(f.root['aria-busy']).toBe('false');
    expect(f.events).toEqual(['boot', 'activate', 'batch', 'collections', 'ready']);
    expect(f.inputHandlers).toBeNull(); // A stalled import has no readiness deadline.
    expect(f.loads.filter(file => file === './input-glue.js')).toHaveLength(1);
    h.timer.reject(new Error('timer unavailable'));
    if (failure === 'reject') h.input.reject(new Error('input unavailable'));
    else if (failure === 'invalid') h.input.resolve(undefined);
    else h.input.resolve(() => ({ pan: 'installed after readiness' }));
    await h.activation;
    await checkpoint();
    expect(f.root.dataset.error).toBeUndefined();
    expect(f.root.dataset.moduleReady).toBe('true');
    expect(f.events.filter(event => event === 'activate')).toHaveLength(1);
    expect(f.errors).toHaveLength(failure === 'pending' ? 1 : 2);
    if (failure === 'pending') expect(f.inputHandlers.pan).toBe('installed after readiness');
  }
});

test('required app module failure still keeps dispatch gated and settles the error', async () => {
  const h = await startupFixture();
  h.data.reject(new Error('required app unavailable'));
  await h.activation;
  expect(h.f.inputReady).toBe(false);
  expect(h.f.root.dataset.moduleReady).toBeUndefined();
  expect(h.f.root.dataset.error).toContain('required app unavailable');
  expect(h.f.events).toEqual(['boot', 'ready']);
  h.input.resolve(() => ({})); h.timer.resolve(() => ({}));
  await checkpoint();
});

test('ready pan nodes tolerate a missing module and install exactly once when it arrives', () => {
  const h = inputFixture();
  h.f.inputReady = true;
  for (let i = 0; i < 1000; i++) expect(h.pointer('pointerdown').prevented).toBe(false);
  expect(h.frames.size).toBe(0);
  expect(h.sent).toEqual([]);
  h.load();
  h.pointer('pointerdown'); h.pointer('pointerup', { clientX: 10 });
  expect(h.sent).toEqual([[7, 20, '10,0', 0]]);
});

test('initial flow loading cannot hold the first frame or module activation', async () => {
  const f = fixture(false), loading = deferred();
  const apply = f.applyBatch;
  f.loadAfterPaint = () => loading.promise;
  f.log = line => f.logs.lines.push(line);
  vm.runInContext(declaration('flowBatch'), f);
  f.applyBatch = batch => {
    const result = apply(batch);
    f.flowBatch({ ops: [{ op: 'textflow', contexts: [{ root: 1 }] }] });
    return result;
  };
  let painted = false;
  const boot = f.boot(null).then(() => { painted = true; });
  await checkpoint();
  expect(painted).toBe(true);
  expect(f.events).toEqual(['replace', 'batch']);
  loading.reject(new Error('flow unavailable'));
  await checkpoint();
  expect(f.logs.lines.at(-1)).toContain('textflow module: Error: flow unavailable');
  await boot;
});
