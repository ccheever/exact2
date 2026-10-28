// App-module bindings, injected by the bundler, never installed as page-wide
// globals. Host modules keep the browser's functions at every load time.
import { FetchError, fetchWith, inOverlay, overlaying, ownWithout, redirectOf } from './admission.js';
import { tsGrantSet } from './admission-data.js';
// A stream or an upload takes the redirect mode as `fetch` is called, as Hermes
// does: a bad one rejects before a file is read or a stream registered.
// A mutation does one kind of effect (LLP 1016.000 D7): socket operations
// or a fetch. Whichever comes second refuses, and so does the mutation
// (ts-data.js). In a module that holds a socket, a mutation's fetch is
// captured when called and starts after its source returns, so a refused
// one never starts; a module with none fetches at once, as before.
export const SOCKET_AND_FETCH = 'a mutation cannot both send on a socket and fetch';
// `exactHeld` (LLP 1016.000 D9), with Hermes's words (js/src/prelude.js): the server may hold the reply. Each
// fetch is counted on its answer, held (`call.held`) or not (`call.plain`), or as tied to no answer
// (`answering.loose`); an answer with only held fetches out counts for no clock (held.js `counted`, whose
// hooks the page's entry sets on `answering`, build.mjs).
export function fetch(input, options) {
  // Checked before any path, a mutation's captured fetch included (LLP 1016.000 D8).
  if (options?.exactLongPoll !== undefined && options.exactStream === undefined)
    return Promise.reject(new TypeError('exactLongPoll asks again for an answer that keeps coming: give exactStream too'));
  const upload = options?.exactUploadProgress; // LLP 1016.000 D10
  if (upload !== undefined && upload !== false && options.exactStream === undefined)
    return Promise.reject(new TypeError(upload === true ? "exactUploadProgress reports an upload's progress to its stream: give exactStream too" : 'exactUploadProgress is true or false'));
  const held = options?.exactHeld;
  if (held !== undefined && held !== true && held !== false) return Promise.reject(new TypeError('exactHeld is true or false'));
  if (held && options.exactStream !== undefined) return Promise.reject(new TypeError('exactHeld: a stream is never held; it stops counting at its first message'));
  // A stream counts by its own rule (D5), never here.
  if (options?.exactStream !== undefined) return fetchNow(input, options);
  // An upload or a download to a file is never held (Request::is_held).
  return counting(held && options.exactBodyFrom === undefined && options.exactBodyTo === undefined ? 'held' : 'plain', () => fetchNow(input, options));
}
/** Work an answer awaits that is no held fetch (a fetch, `crypto.subtle`, `native.later`, a kept key): counted on
 * its answer (`call.plain`), so the answer is never held while it is out (LLP 1016.000 D9). */
export const ordinary = start => counting('plain', start);
globalThis.__exact_ordinary = ordinary; // for host modules ts-fetch.js does not reach (auth.js)
function counting(kind, start) {
  const call = answering.call;
  const end = () => { if (call) { call[kind]--; if (kind === 'held') answering.heldReply(call); call.recount?.(); } else answering.loose(-1); };
  if (call) { call[kind] = (call[kind] ?? 0) + 1; call.recount?.(); } else answering.loose(1);
  let out;
  try { out = start(); } catch (e) { end(); throw e; }
  return Promise.resolve(out).finally(end);
}
function fetchNow(input, options) {
  if (overlaying.on) return Promise.reject(inOverlay('fetch()'));
  // A socket's mapper answering a frame makes no fetch (D7).
  if (answering.mapper) return Promise.reject(new Error("fetch() is unavailable in a socket's mapper: it answers each frame now"));
  const call = answering.call;
  if (call?.mutation) {
    if (call.socketConflict || call.socketOps?.length) { call.socketConflict = true; return Promise.reject(new Error(SOCKET_AND_FETCH)); }
    call.fetched = true;
    // A stream only registers with its answer, which ts-data.js then refuses;
    // a body from a file is read after its source returns, and checks then.
    if (answering.sockets && options?.exactStream === undefined && options?.exactBodyFrom === undefined && options?.exactBodyTo === undefined) {
      let target, init;
      try { target = input instanceof Request ? input : String(input); init = captured(options); } catch (e) { return Promise.reject(e); }
      return Promise.resolve().then(() => {
        if (call.socketConflict) throw new Error(SOCKET_AND_FETCH);
        return fetchWith(tsGrantSet, target, init, null, undefined, undefined, call.origin);
      });
    }
    return fetchOf(input, options, call);
  }
  return fetchOf(input, options);
}
// What `fetch` reads of its options, read now (inherited ones too), with
// the headers and a body that can change copied.
const OPTIONS = ['method', 'headers', 'body', 'redirect', 'signal', 'mode', 'credentials', 'cache', 'referrer',
  'referrerPolicy', 'integrity', 'keepalive', 'priority', 'exactTimeout', 'exactIndependentHttp'];
function captured(options) {
  if (options == null) return options;
  const init = {};
  for (const key of OPTIONS) { const value = options[key]; if (value !== undefined) init[key] = value; }
  if (init.headers !== undefined) init.headers = new Headers(init.headers);
  const body = init.body;
  if (body instanceof ArrayBuffer) init.body = body.slice(0);
  else if (ArrayBuffer.isView(body)) init.body = new Uint8Array(body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength));
  else if (body instanceof URLSearchParams) init.body = new URLSearchParams(body);
  else if (typeof FormData === 'function' && body instanceof FormData) { init.body = new FormData(); for (const [k, v] of body) init.body.append(k, v); }
  return init;
}
// The origin a fetch carries (LLP 1133.036 D6): its answer's, or, after the answer's first await, where no call is
// traced, the least privileged of the answers in flight (LLP 1133.002 D12).
export const fetchOrigin = () => answering.call?.origin ?? ([...answering.untraced].find(c => !c.settled)?.origin ?? 'app');
function fetchOf(input, options, call) {
  if (options?.exactStream === undefined && options?.exactBodyFrom === undefined && options?.exactBodyTo === undefined) return fetchWith(tsGrantSet, input, options, null, undefined, undefined, fetchOrigin());
  if (options.exactStream !== undefined && options.exactBodyTo !== undefined) return Promise.reject(new TypeError("exactBodyTo: a stream's body is its messages, not a file"));
  let redirect;
  try { redirect = redirectOf(input, options); } catch (e) { return Promise.reject(e); }
  if (options.exactStream === undefined) {
    let init, ms, signal;
    try {
      // Keep inherited validation fields inherited, but take their values now.
      const own = ownWithout(options, ['redirect']);
      const inherited = {};
      for (const key of ['exactBodyFrom', 'exactBodyTo', 'exactIndependentHttp', 'method', 'body', 'exactStream', 'signal', 'exactTimeout'])
        if (!Object.hasOwn(own, key)) inherited[key] = options[key];
      init = Object.assign(Object.create(inherited), own);
      ms = init.exactTimeout;
      if (ms !== undefined && (!Number.isInteger(ms) || ms < 1 || ms > 3600000)) throw new TypeError('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
      signal = init.signal !== undefined ? init.signal : typeof Request === 'function' && input instanceof Request ? input.signal : null;
    } catch (e) { return Promise.reject(e); }
    const started = globalThis.performance.now();
    if (signal?.aborted) return Promise.reject(signal.reason ?? new FetchError('Aborted', 'the fetch was aborted'));
    const deadline = ms === undefined ? null : AbortSignal.timeout(ms);
    let stop = () => {};
    const ended = new Promise((_, reject) => {
      const timedOut = () => reject(new FetchError('Timeout', `the request timed out after ${ms} ms`));
      const aborted = () => reject(signal.reason ?? new FetchError('Aborted', 'the fetch was aborted'));
      signal?.addEventListener?.('abort', aborted, { once: true });
      deadline?.addEventListener('abort', timedOut, { once: true });
      stop = () => { signal?.removeEventListener?.('abort', aborted); deadline?.removeEventListener('abort', timedOut); };
    });
    const origin = fetchOrigin();
    return Promise.race([import('./ts-body-from.js'), ended]).finally(stop).then(m => {
      if (call?.socketConflict) throw new Error(SOCKET_AND_FETCH);
      return m.fromFile(input, init, redirect, started, origin);
    });
  }
  return options.exactTimeout !== undefined && options.exactUploadProgress !== true ? Promise.reject(new TypeError('exactTimeout: a stream has no timeout')) : stream(input, options, redirect);
}

export const files = { appId: null };

// An answer that keeps coming (LLP 1016.000), with Hermes's words
// (js/src/prelude.js): the stream is the answer's, so its fetch is made while
// the answer is asked (`answering`, set by ts-data.js), which opens it. Its
// promise never settles: the answer is what `exactStream` maps each event to.
export const answering = { call: null, untraced: new Set(), sockets: false, mapper: null, loose: () => {}, heldReply: () => {}, ordinary: start => ordinary(start), page: null }; // the hooks: held.js's, set by the page's entry; `ordinary` for ts-data.js; `page`, the runtime's clock, journal and `drive`, set by ts-data.js
function stream(input, init, redirect) {
  const call = answering.call;
  // Hermes also claims a stream started after an await; this page has no
  // turn to tie one to, so it says where to start it.
  if (!call?.turn) return Promise.reject(new Error('fetch() with exactStream outside an answer: on the web a stream starts as its answer is asked, before the answer\'s first await'));
  if (call.stream) return Promise.reject(new Error('an answer streams one request'));
  // Checked and opened by ts-stream.js, which only a streaming module loads.
  call.stream = { input, init, redirect };
  return new Promise(() => {});
}

// Time and seeds are source arguments (LLP 1027.000 D3): the app's modules see
// these in place of the page's Date, Math, Intl, timers and performance, each
// refusing what Hermes refuses with Hermes's words (js/src/prelude.js), so an
// app that reads the clock fails on the web as it would on a device. An
// explicit date, UTC arithmetic and an explicit timestamp's formatting work.
const refuse = api => { throw new Error(`${api} is unavailable in data sources; pass time or a random seed as an argument`); };
const noTimers = api => () => { throw new Error(`${api} is unavailable in data sources: an answer waits with setTimeout; pass time as an argument`); };
// An answer's own timers (LLP 1027.000 D1 as amended 2026-10-28), with Hermes's rules (js/src/binary.js `timers`).
// Each is an entry of the page's clock (the driver's under the agent, whose `clock +N` and `clock settle` fire it)
// that goes after the tasks due with it and is no commit (rt.js `advance`). It belongs to the answer that armed it:
// one that has answered, or whose ticket was let go, never runs it. An answer with only timers out counts in no
// clock's wait (held.js `counted`), as a native host keeps its continuation back until it is due.
const Armed = new Map();
let NextTimer = 1;
function arm(api, run, ms, args) {
  if (overlaying.on) throw inOverlay(api);
  const call = answering.call;
  if (answering.mapper || !call || call.settled) throw new Error(`${api} is unavailable here: a timer belongs to the answer that arms it, and none is running (module initialization, a stream's mapper, background work, or an answer that has given its value)`);
  const { clock, journal, drive } = answering.page, id = NextTimer++;
  // The action answers true, as every clock entry's does (rt.js `advance`: anything else stops the advance).
  // `live`: its answer has not answered nor been let go (the agent's `clock settle` goes only to those).
  const live = () => { const t = call.ticket; return !call.settled && !(t && (t.owner.ticket !== t || t.owner.gone)); };
  const entry = { due: clock.now + ms, once: true, data: true, call, live, action() {
    disarm(id);
    if (!live()) { dropTimers(call, true); return true; }
    // In flight until the page's next task, so the agent's jump lands what it started (its microtasks, the answer's
    // continuation) before the next timer fires, as a browser runs them between tasks.
    call.firing = (call.firing ?? 0) + 1; call.recount?.();
    globalThis.setTimeout(() => { call.firing--; call.recount?.(); });
    const before = answering.call;
    answering.call = call;
    try { run(...args); } catch (e) { journal.push(`t=${clock.now} data: a timer's callback threw: ${e?.message ?? e}`); } finally { answering.call = before; }
    return true;
  } };
  Armed.set(id, { call, entry }); clock.timers.push(entry);
  call.timers = (call.timers ?? 0) + 1; call.recount?.();
  if (!clock.agent) drive();
  return id;
}
function disarm(id) {
  const armed = Armed.get(id);
  if (!armed) return;
  Armed.delete(id);
  const timers = answering.page.clock.timers, i = timers.indexOf(armed.entry);
  if (i >= 0) timers.splice(i, 1);
  armed.call.timers--; armed.call.recount?.();
}
/** Clear `call`'s timers, unrun: it has answered, or (`gone`) its ticket was let go, which ends its bookkeeping. */
export function dropTimers(call, gone) { for (const [id, armed] of Armed) if (armed.call === call) disarm(id); if (gone) call.abandon?.(); }
// `AbortSignal.timeout` is an answer's timer too; the page's AbortSignal otherwise.
const PageAbortSignal = globalThis.AbortSignal;
const GuardedAbortSignal = PageAbortSignal && new Proxy(PageAbortSignal, { get(target, name, receiver) {
  if (name !== 'timeout') return Reflect.get(target, name, receiver);
  return function timeout(ms) {
    const n = Number(ms);
    if (!(n >= 0 && n <= Number.MAX_SAFE_INTEGER)) throw new TypeError('AbortSignal.timeout(): the delay is a number of milliseconds from 0');
    const controller = new AbortController();
    arm('AbortSignal.timeout()', () => controller.abort(new DOMException('signal timed out', 'TimeoutError')), Math.floor(n), []);
    return controller.signal;
  };
} });
// The engine's own, read through the prototypes a page's fake clock keeps
// (Playwright's `clock.install`, sinon: a Date and DateTimeFormat whose
// prototype is or inherits the real one, with no `format` accessor of its own):
// a guard over the fake would find no getter and the page would not start.
const NativeDate = globalThis.Date.prototype.constructor, NativeMath = globalThis.Math, NativeIntl = globalThis.Intl, construct = Reflect.construct;
// A function, not a Proxy of the page's Date: its instances' prototype says
// `constructor` is this one, so `new (new Date(0).constructor)()` refuses too.
function GuardedDate(...args) {
  if (!new.target) return refuse('Date()');
  if (!args.length) return refuse('new Date()');
  return construct(NativeDate, args, new.target);
}
GuardedDate.prototype = Object.create(NativeDate.prototype, { constructor: { value: GuardedDate, writable: true, configurable: true } });
Object.defineProperties(GuardedDate, {
  now: { value: () => refuse('Date.now()') }, UTC: { value: NativeDate.UTC }, parse: { value: NativeDate.parse },
  name: { value: 'Date' }, [Symbol.hasInstance]: { value: v => v instanceof NativeDate },
});
const GuardedMath = Object.freeze(Object.create(Object.getPrototypeOf(NativeMath), Object.fromEntries(Reflect.ownKeys(NativeMath).map(k => [k,
  k === 'random' ? { value: () => { throw new Error('Math.random() is unavailable in data sources; seed from crypto.getRandomValues in a source (docs/recipes/random-and-shuffle.md), or pass a seed as an argument; a roll in the view needs no source (docs/recipes/random-rolls.md)'); } }
    : Object.getOwnPropertyDescriptor(NativeMath, k)]))));
// Intl.DateTimeFormat defaults an omitted date to the clock: its `format` and
// `formatToParts` refuse one, through any alias the app can reach.
let formatProto = NativeIntl.DateTimeFormat.prototype;
while (!Object.getOwnPropertyDescriptor(formatProto, 'format')?.get) formatProto = Object.getPrototypeOf(formatProto);
const NativeFormat = formatProto.constructor, nativeFormat = Object.getOwnPropertyDescriptor(formatProto, 'format').get;
const nativeParts = NativeFormat.prototype.formatToParts, formats = new WeakMap();
function DateTimeFormat(locales, options) { return construct(NativeFormat, [locales, options], new.target ?? DateTimeFormat); }
DateTimeFormat.prototype = Object.create(NativeFormat.prototype, {
  constructor: { value: DateTimeFormat, writable: true, configurable: true },
  format: { configurable: true, get() {
    const f = nativeFormat.call(this);
    if (!formats.has(f)) formats.set(f, (...args) => args[0] === undefined ? refuse('Intl.DateTimeFormat.format()') : f(...args));
    return formats.get(f);
  } },
  formatToParts: { configurable: true, writable: true, value(...args) { return args[0] === undefined ? refuse('Intl.DateTimeFormat.formatToParts()') : nativeParts.apply(this, args); } },
});
Object.setPrototypeOf(DateTimeFormat, NativeFormat);
// A data module does no I/O of its own (LLP 1016.000 D3, D7: its socket
// belongs to a resource opened by `fetch` with `exactStream`): the page's
// XMLHttpRequest, WebSocket and EventSource refuse, as the wasm target's realm
// refuses them (host/web/module-glue.js), so no independent socket or origin
// is reached past the grants.
const noIo = api => function () { throw new Error(`${api} is unavailable in data sources`); };
// `scheduler.yield()` (LLP 1141.001 D10): the browser's where it has one, else
// a task posted on a MessageChannel, which no timer clamp delays. The page has
// no turn budget; a yield here lets it paint and answer input mid-walk.
const pageYield = globalThis.scheduler?.yield?.bind(globalThis.scheduler) ?? (typeof MessageChannel === 'function'
  ? () => new Promise(resolve => { const { port1, port2 } = new MessageChannel(); port1.onmessage = () => { port1.close(); resolve(); }; port2.postMessage(null); })
  : () => Promise.resolve());
const guarded = {
  Date: GuardedDate, Math: GuardedMath,
  XMLHttpRequest: noIo('XMLHttpRequest'), WebSocket: noIo('WebSocket'), EventSource: noIo('EventSource'),
  Intl: Object.freeze(Object.create(NativeIntl, { DateTimeFormat: { value: DateTimeFormat } })),
  setTimeout(callback, delay, ...args) {
    if (typeof callback !== 'function') throw new TypeError('setTimeout(): the callback must be a function; a string of code is never run');
    return arm('setTimeout()', callback, Math.max(Number(delay) | 0, 0), args); // `long`, as WebIDL converts it
  },
  clearTimeout: disarm, setInterval: noTimers('setInterval()'), AbortSignal: GuardedAbortSignal,
  requestAnimationFrame: noTimers('requestAnimationFrame()'), requestIdleCallback: noTimers('requestIdleCallback()'),
  clearInterval() {}, cancelAnimationFrame() {}, cancelIdleCallback() {},
  performance: Object.freeze({ now: () => refuse('performance.now()') }),
  scheduler: Object.freeze({
    yield: () => overlaying.on ? Promise.reject(inOverlay('scheduler.yield()')) : pageYield(),
    postTask() { throw new Error('scheduler.postTask() is unavailable in data sources; split long work with await scheduler.yield()'); },
  }),
  // The page's own, but no draw while an overlay runs.
  crypto: Object.freeze({ subtle: globalThis.crypto?.subtle && new Proxy(globalThis.crypto.subtle, { get(target, name) {
      const v = target[name];
      return typeof v !== 'function' ? v : (...a) => overlaying.on ? Promise.reject(inOverlay(`crypto.subtle.${String(name)}()`)) : ordinary(() => v.apply(target, a));
    } }),
    getRandomValues: a => { if (overlaying.on) throw inOverlay('crypto.getRandomValues()'); return globalThis.crypto.getRandomValues(a); },
    randomUUID: () => { if (overlaying.on) throw inOverlay('crypto.randomUUID()'); return globalThis.crypto.randomUUID(); } }),
};
export const { Date, Math, Intl, setTimeout, setInterval, requestAnimationFrame, requestIdleCallback, clearTimeout, clearInterval,
  cancelAnimationFrame, cancelIdleCallback, performance, scheduler, XMLHttpRequest, WebSocket, EventSource, crypto } = guarded;
// Not a binding of this module's own: its `fetch` keeps the page's `AbortSignal.timeout` for `exactTimeout`.
export { GuardedAbortSignal as AbortSignal };

// The usual browser global spellings share this app-local view. Computed
// access, aliases and destructuring therefore get the same scoped fetch.
const methods = new WeakMap();
// What Hermes's realm lacks and the page has: the global object's other
// names, the DOM and Node's globals, absent from the module here too, so a
// test of one (`typeof document`) answers as it does natively
// (js/bake/src/typescript.mjs `packageRefusals`). This keeps a module
// portable; it is no security boundary (LLP 1027 D5): the module runs in the
// page's realm, and its code is the app author's.
const ABSENT = new Set(['window', 'self', 'frames', 'parent', 'top', 'opener', 'document', 'navigator', 'location', 'history', 'localStorage', 'sessionStorage',
  'indexedDB', 'HTMLElement', 'Element', 'getComputedStyle', 'customElements', 'alert', 'process', 'Buffer', 'global']);
export const absent = undefined;
// Over a target of its own, not the page's global object, whose
// non-configurable properties (`window`, `document`, `top`) a proxy over it
// could not report absent.
const page = globalThis;
export const appGlobal = new Proxy(Object.create(null), {
  has(_, name) { return !ABSENT.has(name) && Reflect.has(page, name); },
  getOwnPropertyDescriptor(_, name) {
    const found = ABSENT.has(name) ? undefined : Reflect.getOwnPropertyDescriptor(page, name);
    return found && { ...found, configurable: true };
  },
  ownKeys() { return Reflect.ownKeys(page).filter(name => !ABSENT.has(name)); },
  getPrototypeOf() { return Object.prototype; },
  set(_, name, value) { return Reflect.set(page, name, value); },
  defineProperty(_, name, descriptor) { return Reflect.defineProperty(page, name, descriptor); },
  deleteProperty(_, name) { return Reflect.deleteProperty(page, name); },
  get(_, name) {
    if (name === 'fetch') return fetch;
    if (Object.hasOwn(guarded, name)) return guarded[name];
    if (ABSENT.has(name)) return undefined;
    if (name === 'globalThis') return appGlobal;
    const value = Reflect.get(page, name, page);
    if (typeof value !== 'function') return value;
    if (!methods.has(value)) methods.set(value, new Proxy(value, { apply(fn, receiver, args) { return Reflect.apply(fn, receiver === appGlobal ? page : receiver, args); } }));
    return methods.get(value);
  },
});
