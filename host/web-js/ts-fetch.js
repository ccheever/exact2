// App-module bindings, injected by the bundler, never installed as page-wide
// globals. Host modules keep the browser's functions at every load time.
import { fetchWith } from './admission.js';
import { tsGrantSet } from './admission-data.js';
export const fetch = (input, options) => options?.exactStream === undefined ? fetchWith(tsGrantSet, input, options)
  : options.exactTimeout !== undefined ? Promise.reject(new TypeError('exactTimeout: a stream has no timeout')) : stream(input, options);

// An answer that keeps coming (LLP 1016.000), with Hermes's words
// (js/src/prelude.js): the stream is the answer's, so its fetch is made while
// the answer is asked (`answering`, set by ts-data.js), which opens it. Its
// promise never settles: the answer is what `exactStream` maps each event to.
export const answering = { call: null };
function stream(input, init) {
  const call = answering.call;
  // Hermes also claims a stream started after an await; this page has no
  // turn to tie one to, so it says where to start it.
  if (!call) return Promise.reject(new Error('fetch() with exactStream outside an answer: on the web a stream starts as its answer is asked, before the answer\'s first await'));
  if (call.stream) return Promise.reject(new Error('an answer streams one request'));
  // Checked and opened by ts-stream.js, which only a streaming module loads.
  call.stream = { input, init };
  return new Promise(() => {});
}

// Time and seeds are source arguments (LLP 1027.000 D3): the app's modules see
// these in place of the page's Date, Math, Intl, timers and performance, each
// refusing what Hermes refuses with Hermes's words (js/src/prelude.js), so an
// app that reads the clock fails on the web as it would on a device. An
// explicit date, UTC arithmetic and an explicit timestamp's formatting work.
const refuse = api => { throw new Error(`${api} is unavailable in data sources; pass time or a random seed as an argument`); };
const noTimers = api => () => { throw new Error(`${api} is unavailable in data sources: there are no timers; pass time as an argument`); };
const NativeDate = globalThis.Date, NativeMath = globalThis.Math, NativeIntl = globalThis.Intl, construct = Reflect.construct;
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
  k === 'random' ? { value: () => { throw new Error('Math.random() is unavailable in data sources; pass time or a random seed as an argument, or use crypto.getRandomValues'); } }
    : Object.getOwnPropertyDescriptor(NativeMath, k)]))));
// Intl.DateTimeFormat defaults an omitted date to the clock: its `format` and
// `formatToParts` refuse one, through any alias the app can reach.
const NativeFormat = NativeIntl.DateTimeFormat, nativeFormat = Object.getOwnPropertyDescriptor(NativeFormat.prototype, 'format').get;
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
// A data module does no I/O of its own (LLP 1016.000 D3: a socket only
// listens, opened by the runtime as a `fetch` with `exactStream`): the page's
// XMLHttpRequest, WebSocket and EventSource refuse, with the words Hermes and
// the wasm target's realms use (js/src/prelude.js), so no frame is sent and
// no origin is reached past the grants.
const noIo = api => function () { throw new Error(`${api} is unavailable in data sources`); };
const guarded = {
  Date: GuardedDate, Math: GuardedMath,
  XMLHttpRequest: noIo('XMLHttpRequest'), WebSocket: noIo('WebSocket'), EventSource: noIo('EventSource'),
  Intl: Object.freeze(Object.create(NativeIntl, { DateTimeFormat: { value: DateTimeFormat } })),
  setTimeout: noTimers('setTimeout()'), setInterval: noTimers('setInterval()'),
  requestAnimationFrame: noTimers('requestAnimationFrame()'), requestIdleCallback: noTimers('requestIdleCallback()'),
  clearTimeout() {}, clearInterval() {}, cancelAnimationFrame() {}, cancelIdleCallback() {},
  performance: Object.freeze({ now: () => refuse('performance.now()') }),
};
export const { Date, Math, Intl, setTimeout, setInterval, requestAnimationFrame, requestIdleCallback, clearTimeout, clearInterval,
  cancelAnimationFrame, cancelIdleCallback, performance, XMLHttpRequest, WebSocket, EventSource } = guarded;

// The usual browser global spellings share this app-local view. Computed
// access, aliases and destructuring therefore get the same scoped fetch.
const methods = new WeakMap();
export const appGlobal = new Proxy(globalThis, { get(target, name) {
  if (name === 'fetch') return fetch;
  if (Object.hasOwn(guarded, name)) return guarded[name];
  if (name === 'globalThis' || name === 'self' || name === 'window') return appGlobal;
  const value = Reflect.get(target, name, target);
  if (typeof value !== 'function') return value;
  if (!methods.has(value)) methods.set(value, new Proxy(value, { apply(fn, receiver, args) { return Reflect.apply(fn, receiver === appGlobal ? target : receiver, args); } }));
  return methods.get(value);
} });
