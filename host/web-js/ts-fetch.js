// App-module bindings, injected by the bundler, never installed as page-wide
// globals. Host modules keep the browser's functions at every load time.
import { coversPath, FetchError, fetchWith, inOverlay, overlaying } from './admission.js';
import { tsGrantSet } from './admission-data.js';
export const fetch = (input, options) => overlaying.on ? Promise.reject(inOverlay('fetch()')) : options?.exactStream === undefined ? (options?.exactBodyFrom === undefined ? fetchWith(tsGrantSet, input, options) : fromFile(input, options))
  : options.exactTimeout !== undefined ? Promise.reject(new TypeError('exactTimeout: a stream has no timeout')) : stream(input, options);

// `exactBodyFrom` (LLP 1108 D6 R2), with Hermes's checks and words
// (js/src/prelude.js): the app file at that path is the body, read from the
// page's store as a Blob (a picked file is its own File; nothing becomes a
// string), under `fs.read`, at most 64 MiB. A refusal is a FetchError before
// anything is sent. `files.appId` is the module's, set by ts-data.js.
export const files = { appId: null };
export const methodOf = (input, init) => String(init.method ?? (typeof Request === 'function' && input instanceof Request ? input.method : 'GET')).toUpperCase();
export function bodyFromRefusal(input, init) {
  const path = init.exactBodyFrom, method = methodOf(input, init);
  if (typeof path !== 'string' || !path.startsWith('app:/')) return 'exactBodyFrom must be an app:/ path';
  if (new TextEncoder().encode(path).length > 4096) return 'exactBodyFrom: a path is at most 4096 bytes';
  if (/(^|\/)\.\.?(\/|$)|\0/.test(path.slice(5))) return 'exactBodyFrom: an app:/ path has no . or .. segment';
  if (method === 'GET' || method === 'HEAD') return `fetch: a ${method} request cannot have a body`;
  if (init.exactStream !== undefined && /^wss?:/i.test(typeof Request === 'function' && input instanceof Request ? input.url : String(input))) return 'exactBodyFrom: a WebSocket sends no body';
  if (init.body != null || typeof Request === 'function' && input instanceof Request && input.body != null) return 'fetch: a request has one body: body or exactBodyFrom';
  return null;
}
export function readBodyFile(path, grantSet) {
  // Refused before the storage adapters load, which an app without file grants does not ship.
  if (!coversPath(grantSet, 'fs.read', path)) return Promise.reject(Object.assign(new FetchError('Refused', `exactBodyFrom ${path}: denied: fs.read ${path}: no grant covers it; grant \`fs.read ${path}\``), { code: 'denied' }));
  return import(new URL('./storage-fs.js', import.meta.url).href)
    .then(m => m.requestBody(files.appId, grantSet, path))
    .catch(error => { throw Object.assign(new FetchError(error?.code === 'agent' ? 'Unsupported' : 'Refused', error?.message ?? error), { code: error?.code }); });
}
async function fromFile(input, init) {
  const refusal = bodyFromRefusal(input, init);
  if (refusal) throw new TypeError(refusal);
  const { exactBodyFrom: path, ...rest } = init, ms = init.exactTimeout;
  if (ms !== undefined && (!Number.isInteger(ms) || ms < 1 || ms > 3600000)) throw new TypeError('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
  // The read counts against the deadline and yields to the caller's abort,
  // as the exchange does: nothing is sent once either has ended it. (This
  // module's own `setTimeout` and `performance` are the app's refusals.)
  const signal = rest.signal !== undefined ? rest.signal : typeof Request === 'function' && input instanceof Request ? input.signal : null;
  const clock = globalThis.performance, started = clock.now(), deadline = ms === undefined ? null : AbortSignal.timeout(ms);
  // Already aborted: refused before anything is read or a rejection made that nobody would handle.
  if (signal?.aborted) throw signal.reason ?? new FetchError('Aborted', 'the fetch was aborted');
  let stop = () => {};
  const ended = new Promise((_, reject) => {
    const timedOut = () => reject(new FetchError('Timeout', `the request timed out after ${ms} ms`));
    const aborted = () => reject(signal.reason ?? new FetchError('Aborted', 'the fetch was aborted'));
    if (signal?.aborted) aborted(); else signal?.addEventListener?.('abort', aborted, { once: true });
    deadline?.addEventListener('abort', timedOut, { once: true });
    stop = () => { signal?.removeEventListener?.('abort', aborted); deadline?.removeEventListener('abort', timedOut); };
  });
  let body;
  const reading = readBodyFile(path, tsGrantSet);
  reading.catch(() => {}); // a read that loses the race is nobody's
  try { body = await Promise.race([reading, ended]); } finally { stop(); }
  // One deadline for the whole request: its instant goes to fetchWith, which
  // checks it by the clock just before sending, after its own grant work.
  const spent = clock.now() - started;
  if (ms !== undefined && spent >= ms) throw new FetchError('Timeout', `the request timed out after ${ms} ms`);
  const left = ms === undefined ? undefined : Math.max(1, Math.ceil(ms - spent));
  return fetchWith(tsGrantSet, input, { ...rest, body, ...(left === undefined ? {} : { exactTimeout: left }) }, ms === undefined ? null : { at: started + ms, ms });
}

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
// XMLHttpRequest, WebSocket and EventSource refuse, as the wasm target's realm
// refuses them (host/web/module-glue.js), so no frame is sent and no origin
// is reached past the grants.
const noIo = api => function () { throw new Error(`${api} is unavailable in data sources`); };
const guarded = {
  Date: GuardedDate, Math: GuardedMath,
  XMLHttpRequest: noIo('XMLHttpRequest'), WebSocket: noIo('WebSocket'), EventSource: noIo('EventSource'),
  Intl: Object.freeze(Object.create(NativeIntl, { DateTimeFormat: { value: DateTimeFormat } })),
  setTimeout: noTimers('setTimeout()'), setInterval: noTimers('setInterval()'),
  requestAnimationFrame: noTimers('requestAnimationFrame()'), requestIdleCallback: noTimers('requestIdleCallback()'),
  clearTimeout() {}, clearInterval() {}, cancelAnimationFrame() {}, cancelIdleCallback() {},
  performance: Object.freeze({ now: () => refuse('performance.now()') }),
  // The page's own, but no draw while an overlay runs.
  crypto: Object.freeze({ subtle: globalThis.crypto?.subtle && new Proxy(globalThis.crypto.subtle, { get(target, name) {
      const v = target[name];
      return typeof v !== 'function' ? v : (...a) => overlaying.on ? Promise.reject(inOverlay(`crypto.subtle.${String(name)}()`)) : v.apply(target, a);
    } }),
    getRandomValues: a => { if (overlaying.on) throw inOverlay('crypto.getRandomValues()'); return globalThis.crypto.getRandomValues(a); },
    randomUUID: () => { if (overlaying.on) throw inOverlay('crypto.randomUUID()'); return globalThis.crypto.randomUUID(); } }),
};
export const { Date, Math, Intl, setTimeout, setInterval, requestAnimationFrame, requestIdleCallback, clearTimeout, clearInterval,
  cancelAnimationFrame, cancelIdleCallback, performance, XMLHttpRequest, WebSocket, EventSource, crypto } = guarded;

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
