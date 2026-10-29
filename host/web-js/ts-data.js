// A TypeScript data source on the JS runtime (LLP 1027): the app's own
// `app.ts`, bundled with the page, runs in the browser — the executor on
// the web anyway (D6). Values cross by the plan's types: the runtime's
// records are arrays, the module's are objects by field name. The store
// keeps keys as the web host does (LLP 1069.005 D1b: the pair itself in
// IndexedDB under a handle the store holds; its code is fetched on first
// use); `openAuthSession` is auth.js.
import * as source from '__APP_TS__';
import { createSecretFacade, hasGrant, inOverlay, overlaying, setAppGrantSet } from './admission.js';
import { tsGrantSet } from './admission-data.js';
import { answering, dropTimers, files as bodyFiles, SOCKET_AND_FETCH } from './ts-fetch.js';
import { sourceTypes } from './names.js';
import { checkpoint, clock, commit, drive, inflight, journal, Origin, painted, R, Resources } from './rt.js';
import { Shaped } from './shape.js';
import { requestKey } from './uploads.js';
// Work an answer awaits that is no held fetch, counted on its answer (ts-fetch.js `answering.ordinary`, LLP 1016.000 D9).
const ordinary = start => (answering.ordinary ?? (f => f()))(start);
// What an answer's timers run on (ts-fetch.js `arm`).
answering.page = { clock, journal, drive };
__AUTH_IMPORT__
// Values cross by the plan's types (`named` into the module's objects,
// `arrays` back into the runtime's arrays), with each type's converters made
// once. `arrays` converts against the last answer its target was given: a
// record or list that converts to what it did is that array itself, so the
// runtime compares and shape-checks an unchanged row as an identity (the Rust
// runner keeps an equal answer's old object and re-checks only the list items
// that are new, runner/src/conform.rs).
const same = (a, b) => a === b ? a !== 0 || 1 / a === 1 / b : a !== a && b !== b;
const LEAF = [v => v == null ? null : v, v => v == null ? null : v, true];
const made = new WeakMap();
function converters(t) {
  if (!t || typeof t === 'string') return LEAF;
  let c = made.get(t);
  if (c) return c;
  c = [];
  made.set(t, c);
  if (Array.isArray(t)) {
    // A type may name itself: its converters are read when called.
    const sub = converters(t[1]), leaf = sub[2] === true, named = v => sub[0](v), arrays = (v, o) => sub[1](v, o);
    if (t[0] === '?') { c[0] = named; c[1] = arrays; return c; }
    c[0] = v => v == null ? null : v.map(x => named(x));
    // An item is matched by its index even when the list grew or shrank.
    c[1] = (v, o) => {
      if (v == null) return null;
      if (!Array.isArray(v)) return v.map(x => arrays(x));
      const n = v.length, was = Array.isArray(o) ? o : null;
      let out = was && was.length === n ? null : [];
      for (let i = 0; i < n; i++) {
        const x = leaf ? (v[i] == null ? null : v[i]) : arrays(v[i], was?.[i]);
        if (out) out.push(x); else if (!same(x, o[i])) { out = o.slice(0, i); out.push(x); }
      }
      return out ?? o;
    };
    return c;
  }
  const k = Object.keys(t), n = k.length, subs = k.map(f => converters(t[f]));
  const leaves = subs.map(s => s[2] === true);
  c[0] = k.includes('__proto__')
    ? v => v == null ? null : Object.fromEntries(k.map((f, i) => [f, subs[i][0](v[i])]))
    : v => {
      if (v == null) return null;
      const out = {};
      for (let i = 0; i < n; i++) out[k[i]] = leaves[i] ? (v[i] == null ? null : v[i]) : subs[i][0](v[i]);
      return out;
    };
  c[1] = (v, o) => {
    if (v == null) return null;
    const was = Array.isArray(o) ? o : null;
    let out = was && was.length === n ? null : [];
    for (let i = 0; i < n; i++) {
      const y = v[k[i]], x = leaves[i] ? (y == null ? null : y) : subs[i][1](y, was?.[i]);
      if (out) out.push(x); else if (!same(x, o[i])) { out = o.slice(0, i); out.push(x); }
    }
    return out ?? o;
  };
  return c;
}
// An answer is checked as Hermes checks it (js/value's `decode_tree`, over
// the prelude's `JSON.stringify`): each declared field present, none
// undeclared, each value of its declared kind, with Hermes's message, so a
// reply a device refuses fails the web loop too (ledger F11: a spread left
// an internal field in a nested record, accepted here, refused on macOS).
const kind = v => v === null ? 'null' : typeof v === 'boolean' ? 'a bool' : typeof v === 'number' ? 'a number'
  : typeof v === 'string' ? 'a string' : Array.isArray(v) ? 'an array' : 'an object';
const describe = t => typeof t === 'string' ? { n: 'a number', b: 'a bool', s: 'a string' }[t] ?? 'null'
  : Array.isArray(t) ? t[0] === '?' ? `null or ${describe(t[1])}` : 'an array' : 'an object';
const omitted = x => x === undefined || typeof x === 'function' || typeof x === 'symbol';
const enumerable = Object.prototype.propertyIsEnumerable;
function outside(v, t) {
  // What `JSON.stringify` makes of it: `toJSON`, null for a non-finite number.
  if (v !== null && typeof v === 'object' && typeof v.toJSON === 'function') v = v.toJSON();
  if (omitted(v) || (typeof v === 'number' && !isFinite(v))) v = null;
  if (typeof t === 'string') {
    const ok = t === 'n' ? typeof v === 'number' : t === 'b' ? typeof v === 'boolean' : t === 's' ? typeof v === 'string' : v === null;
    return ok ? null : `expected ${describe(t)}, got ${kind(v)}`;
  }
  if (Array.isArray(t) && t[0] === '?') return v === null ? null : outside(v, t[1]);
  if (Array.isArray(t)) {
    if (!Array.isArray(v)) return `expected an array, got ${kind(v)}`;
    for (const x of v) { const e = outside(x, t[1]); if (e) return e; }
    return null;
  }
  if (v === null || typeof v !== 'object' || Array.isArray(v)) return `expected an object, got ${kind(v)}`;
  for (const f in t) {
    if (!enumerable.call(v, f) || omitted(v[f])) return `field \`${f}\` is missing`;
    const e = outside(v[f], t[f]);
    if (e) return `field \`${f}\`: ${e}`;
  }
  let extra = null;
  for (const key of Object.keys(v)) if (!Object.hasOwn(t, key) && !omitted(v[key]) && (extra === null || key < extra)) extra = key;
  return extra === null ? null : `field \`${extra}\` is not in the shape`;
}
const checked = (name, v, t) => {
  const e = outside(v, t);
  if (e) throw Object.assign(new Error(`\`${name}\` answered outside its shape: ${e}`), { kind: 'Unavailable', [Shaped]: true });
  return v;
};
export const named = (v, t) => converters(t)[0](v);
const arrays = (v, t, o) => converters(t)[1](v, o);
const answered = new Map(); // target -> the arrays last made for it
const conv = (v, t, target) => { const a = arrays(v, t, answered.get(target)); answered.set(target, a); return a; };
// The app's page module as a source sees it (`native`, LLP 1067 D5), where
// the app has one: `later` goes to the module artifact's `later` (native.js,
// loaded after first paint); the web has no synchronous `call`; `watch`
// re-asks the resources of the answer's source when the module announces
// the topic (LLP 1016.002).
const watched = new Map(); // topic -> sources
let watching = null, page = null, load = null;
// A resource with a request in flight is not asked now: that request's
// reply lands, then it is asked again (`t.again`, rt.js), once however many
// changes came; a stream, or nothing in flight, is asked now (LLP 1016.002
// D4, the runner's `changed`).
const changed = (r, topic) => {
  const t = r.ticket;
  if (!t || t.stream || t.req?.stream) return R(r);
  if (!t.again) journal.push(`t=${clock.now} changed ${topic}: ticket ${t.id} (${r.name}) lands first, then it is asked again`);
  t.again = true;
};
const announce = (topic, why) => commit(() => { const s = watched.get(topic), m = moduleWatched.get(topic); for (const r of Resources) if (s?.has(r.source) || m?.has(r.source)) changed(r, topic); }, `${why} ${topic}`);
const watch = topic => (watched.get(topic) ?? watched.set(topic, new Set()).get(topic)).add(watching);
// A native module's name called from data code (LLP 1133.002 D14): admitted by
// the page runtime's modules.js as a Contract read is; a source's reader is
// asked again when its answer changes, as a topic's is.
// Its source's readers are the answers' that read it last: each answer's own
// reads replace the last's, as the runner's `watching` does.
const moduleWatched = new Map(); // module key -> sources
// Whether the answer that made the call still wants its reply is its call's `wanted`, its ticket's `held` (rt.js
// `reply`), which a registration's reply reads (LLP 1133.011 D11).
globalThis.__exact_module = body => {
  const key = `${body.extension}.${body.source}`, call = answering.call;
  if (body.source && watching) (moduleWatched.get(key) ?? moduleWatched.set(key, new Set()).get(key)).add(watching);
  // A tool handler's call carries its origin (`data.tool`); every other is the app's.
  const origin = call?.origin ?? 'app';
  return ordinary(() => globalThis.exact?.extensions?.call?.({ ...body, origin }, () => call?.wanted?.() ?? true) ?? Promise.resolve({ error: 'NotSupportedError' }));
};
(globalThis.exact ??= {}).announceModule = topic => announce(topic, 'module');
globalThis.exact.moduleReaders = key => Resources.filter(r => moduleWatched.get(key)?.has(r.source)).map(r => r.name);
// The documents the answers watch, for the agent's `state.watches`, as the runner lists them.
(globalThis.exact ??= {}).watchState = () => [...watched.keys()].sort().flatMap(t => {
  const m = /^doc-watch(-recursive)?:(.*)$/.exec(t);
  return m ? [{ path: m[2], recursive: !!m[1] }] : [];
});
const pageModule = () => page ??= load().then(m => m.pageModule({
  agent: clock.agent, now: () => clock.now,
  changed: topic => announce(topic, 'native'),
}));
const native = Object.freeze({
  get available() { return true; },
  call() { if (overlaying.on) throw inOverlay('native.call()'); throw new Error('native.call: the web has no synchronous module call; use native.later'); },
  watch(topic) { if (overlaying.on) throw inOverlay('native.watch()'); if (!watching) throw new Error('native.watch outside an answer'); watch(String(topic)); },
  // Counted on its answer as an ordinary fetch is (ts-fetch.js `ordinary`, LLP 1016.000 D9).
  later: request => overlaying.on ? Promise.reject(inOverlay('native.later()')) : ordinary(() => pageModule().then(p => p.later(request))),
});
// App storage as a source sees it (`storage`, LLP 1027.001): `fs` and
// `sqlite` over the web host's own adapters (`storage-fs.js`,
// `storage-sqlite.js`, beside the page and fetched on first use), under
// the app's grants and its page's store key — none under the agent unless
// the drive names a scratch store (`storageKey`) — and the documents the
// person chose (`documents-glue.js`, the handles its pickers keep). Without
// storage grants the same interface refuses, without fetching any adapters.
// A refusal is `{kind: 'Unavailable', code, message}`, with Hermes's codes
// (js/src/prelude.js `storageCode`; kanban F28): 'agent' for a drive with no
// scratch store, 'denied' past the grants, the filesystem's POSIX name, else
// 'failed'.
const codedError = e => {
  const error = e instanceof Error ? e : new Error(String(e?.message ?? e));
  error.kind ??= 'Unavailable';
  error.code ??= /^denied: /.test(error.message) ? 'denied' : /\bbusy\b|database is locked/.test(error.message) ? 'EBUSY' : 'failed';
  return error;
};
const coded = e => { throw codedError(e); };
// One queue for the module's storage, as on every host (LLP 1097 D3): each
// operation runs when the one before it has settled, in the order issued,
// so a read issued after a write sees it (storage-fs.js reads the store
// directly); at most 256 wait behind the one in flight. A write an answer
// does not await finishes after it, as a page's does, and counts in flight,
// so the agent's settle waits for it (D9). Every failure is journaled (D8).
const MAX_QUEUED = 256, queue = [], counts = { done: 0, failed: 0, last: null };
let head = null;
const issue = op => {
  head = op;
  Promise.resolve().then(op.run).then(v => landed(op, true, v), e => landed(op, false, e));
};
function landed(op, ok, value) {
  head = null;
  if (queue.length) issue(queue.shift());
  // What the settle waits for ends after the reactions it lands (hatches.js).
  setTimeout(() => inflight.n--);
  if (ok) { counts.done++; op.resolve(value); return; }
  const error = codedError(value), line = `storage failed: ${op.what}: ${error.code} ${error.message}`;
  counts.failed++; counts.last = line;
  journal.push(`t=${clock.now} ${line}`);
  op.reject(error);
}
function queued(what, run) {
  if (overlaying.on) return Promise.reject(inOverlay(`storage.${what.split(' ')[0]}()`)); // `what` is the method, then its path
  if (head && queue.length >= MAX_QUEUED) {
    journal.push(`t=${clock.now} storage refused: full (${what})`);
    return Promise.reject(Object.assign(new Error(`storage queue full: ${MAX_QUEUED} operations wait`), { kind: 'Unavailable', code: 'full' }));
  }
  inflight.n++;
  return new Promise((resolve, reject) => {
    const op = { what, run, resolve, reject };
    if (head) queue.push(op); else issue(op);
  });
}
let toldAgent = false;
// The databases open now, each with the answer that opened it (LLP 1097 D7). The host does not
// close a handle an app may keep or share (Charlie, 2026-10-07); one left open by background work
// that failed is journaled instead, since the next open would find it locked.
const openDatabases = new Set();
let pendingAnswers = 0, owed = false; // `owed`: a failure skipped an owner-less handle for a pending answer
// On a failure (an answer's own rejection, or a rejection nothing handled): each database opened by
// an answer that has since settled or replied (its chain runs on in the background). One opened by
// a continuation, after its answer's synchronous part, has no known owner here: it counts only
// when no answer is still pending, so a live answer's handle is never taken for background work's.
function leftOpen() {
  for (const h of openDatabases) {
    if (h.told) continue;
    if (!h.call && pendingAnswers > 0) { owed = true; continue; } // checked again once none is pending
    if (h.call && !h.call.settled) continue;
    h.told = true;
    journal.push(`t=${clock.now} storage: ${h.path} is still open after a failure in background work that opened it: if that work owns it, close it in a finally (finally { db.close() }), or the next open finds it locked (LLP 1097 D7)`);
  }
}
function storageOf(grants) {
  const admitted = ['fs-read', 'fs-write', 'sqlite-open'].some(kind => hasGrant(grants, kind));
  let fs, sqlite;
  const key = () => import('./storage-environment.js').then(({ storageKey, agentStorageRefusal }) => {
    const k = source.appId ? storageKey(source.appId) : null;
    if (k == null) throw Object.assign(new Error(agentStorageRefusal), { kind: 'Unavailable', code: 'agent' });
    return k;
  });
  // The first refused read or write is said once in the journal, as on every host (trivia F7).
  const tell = () => import('./storage-environment.js').then(env => {
    if (!toldAgent) { toldAgent = true; journal.push(`t=${clock.now} storage refused (agent): ${env.agentStorageRefusal}`); }
    return env;
  });
  const refused = e => (e?.code === 'agent' ? tell() : Promise.resolve()).then(() => coded(e));
  // A refused read answers as an empty store would (storage-environment.js `emptyRead`).
  const orEmpty = (m, path) => e => e?.code === 'agent' ? tell().then(env => { if (!env.agentReads.includes(m)) throw e; return env.emptyRead(m, path); }) : Promise.reject(e);
  const files = () => fs ??= key().then(k => import(new URL('./storage-fs.js', import.meta.url).href).then(m => m.createFileSystem(k, grants)));
  // A document the person chose (`doc:/`, LLP 1069.010 D1) is the page's
  // handle, not app storage: no store key, so a drive without a scratch
  // store reaches it too, as a Rust source's storage request does.
  let docs;
  // What the app keeps across launches is read back first (LLP 1069.010.000 D4).
  const documents = () => docs ??= ((globalThis.exact ??= {}), import(new URL('./documents-glue.js', import.meta.url).href)).then(() => globalThis.exact.documents.keeping(source.appId, grants)).then(() => globalThis.exact.documents.files(grants));
  // In the storage queue, as every storage call is (an overlay refuses it).
  const keptDocuments = (what, f) => ordinary(() => queued(what, () => documents().then(() => f(globalThis.exact.documents))).catch(refused));
  let listWatched = false;
  const isDocument = args => args.slice(0, 2).some(p => typeof p === 'string' && p.startsWith('doc:/'));
  // A document copied into an app file needs both (documents-glue.js `copyFile`).
  const copiesIn = (m, args) => m === 'copyFile' && typeof args[0] === 'string' && args[0].startsWith('doc:/') && typeof args[1] === 'string' && args[1].startsWith('app:/');
  const denied = (op, document = false) => (document ? Promise.resolve() : key()).then(() => {
    throw Object.assign(new Error(`denied: ${op}`), { kind: 'Unavailable', code: 'denied' });
  });
  const databases = () => sqlite ??= key().then(k => import(new URL('./storage-sqlite.js', import.meta.url).href).then(m => m.createSqlite(k, grants)));
  // A database's and a statement's methods refuse as storage's do, in the queue.
  const wrap = (o, convert, path) => Object.freeze(Object.fromEntries(Object.entries(convert).map(([m, then]) =>
    [m, (...args) => queued(`${m} ${path}`, () => o[m](...args)).then(then ? v => then(v, path) : undefined)])));
  const statement = (s, path) => wrap(s, { execute: null, query: null, close: null }, path);
  const database = (d, path, call) => {
    const handle = { path, call, told: false };
    openDatabases.add(handle);
    const db = wrap(d, { execute: null, query: null, prepare: statement, transaction: null, close: null }, path);
    // A close counts once issued; one refused or failed leaves the database open, and tracked.
    return Object.freeze({ ...db, close: (...args) => {
      openDatabases.delete(handle);
      // A failed close still rejects for the caller, so one left unhandled is still reported.
      return db.close(...args).catch((e) => { openDatabases.add(handle); throw e; });
    } });
  };
  const methods = ['readFile', 'writeFile', 'atomicWriteFile', 'appendFile', 'readdir', 'mkdir', 'rm', 'stat', 'rename', 'copyFile', 'realpath'];
  // `compressImage` (LLP 1069.002 A1) and `transformImage` (LLP 1133.019
  // D14): their options are checked before either is queued, as the native
  // prelude checks them (binary.js `imageOptions`); app files only.
  const image = (method, from, to, options, limits) => {
    const api = `storage.fs.${method}()`, out = { ...limits }, o = options ?? {};
    const finite = n => typeof n === 'number' && Number.isFinite(n), side = n => Number.isInteger(n) && n >= 1 && n <= 16384;
    const checks = { rotate: [finite, 'a finite number of degrees'], flip: [w => w === 'horizontal' || w === 'vertical', '"horizontal" or "vertical"'],
      crop: [c => c && typeof c === 'object' && [c.x, c.y, c.width, c.height].every(finite) && c.width > 0 && c.height > 0, '{x, y, width, height}, finite, with a positive width and height'] };
    if (method === 'transformImage') Object.assign(checks, { resizeWidth: [side, 'an integer from 1 to 16384'], resizeHeight: [side, 'an integer from 1 to 16384'],
      resizeQuality: [w => ['pixelated', 'low', 'medium', 'high'].includes(w), '"pixelated", "low", "medium" or "high"'], type: [t => typeof t === 'string' && /^image\/[a-z0-9.+-]+$/.test(t), 'a media type such as "image/jpeg"'],
      quality: [q => typeof q === 'number' && q >= 0 && q <= 1, 'a number from 0 to 1'], metadata: [w => ['keep', 'no-location', 'none'].includes(w), '"keep", "no-location" or "none"'] });
    if (typeof from !== 'string' || typeof to !== 'string') return Promise.reject(new TypeError(`${api}: from and to must be app:/ paths`));
    if (typeof o !== 'object') return Promise.reject(new TypeError(`${api}: options must be an object`));
    for (const [name, [ok, says]] of Object.entries(checks)) {
      if (o[name] === undefined) continue;
      if (!ok(o[name])) return Promise.reject(new TypeError(`${api}: ${name} must be ${says}`));
      out[name] = o[name];
    }
    return (admitted ? queued(`${method} ${from}`, () => files().then(f => f[method](from, to, out))) : denied(`fs.${method}`)).catch(refused);
  };
  const compressImage = (from, to, options) => {
    const api = 'storage.fs.compressImage()', d = options?.maxDimension, b = options?.maxBytes;
    if (typeof from !== 'string' || typeof to !== 'string') return Promise.reject(new TypeError(`${api}: from and to must be app:/ paths`));
    if (!options || typeof options !== 'object') return Promise.reject(new TypeError(`${api}: options must be {maxDimension, maxBytes}`));
    if (!Number.isInteger(d) || d < 1 || d > 8192) return Promise.reject(new TypeError(`${api}: maxDimension must be an integer from 1 to 8192`));
    if (!Number.isInteger(b) || b < 1 || b > 67108864) return Promise.reject(new TypeError(`${api}: maxBytes must be an integer from 1 to 67108864`));
    const type = options.type === undefined ? 'image/jpeg' : options.type;
    if (type !== 'image/jpeg' && type !== 'image/png') return Promise.reject(new TypeError(`${api}: type must be "image/jpeg" or "image/png"`));
    return image('compressImage', from, to, options, { maxDimension: d, maxBytes: b, type });
  };
  // `fs.watch(path, {recursive})` (LLP 1069.010.000 D1): the answer depends
  // on a document the person chose; the page's FileSystemObserver announces
  // its topic, where the browser has one (documents-glue.js `observe`).
  const watchDocument = (path, options) => {
    const api = 'storage.fs.watch()';
    if (overlaying.on) throw inOverlay(api);
    if (!watching) throw new Error(`${api} outside an answer`);
    if (typeof path !== 'string' || !path.startsWith('doc:/')) throw new TypeError(`${api}: path must be a doc:/ path`);
    const recursive = !!options?.recursive, topic = `${recursive ? 'doc-watch-recursive:' : 'doc-watch:'}${path}`;
    watch(topic);
    // `doc:/` itself is the kept list (LLP 1069.010.000 D5).
    if (path === 'doc:/') return void (listWatched || documents().then(() => { if (!listWatched) globalThis.exact.documents.onList(() => announce(topic, 'changed')); listWatched = true; }).catch(() => {}));
    documents().then(() => globalThis.exact.documents.observe(path, recursive, grants, () => announce(topic, 'changed'))).catch(() => {});
  };
  const expecting = (m, args) => ['writeFile', 'atomicWriteFile', 'appendFile'].includes(m) && args[2]?.expect;
  return Object.freeze({
    fs: Object.freeze({ directories: Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' }),
      compressImage: (...a) => ordinary(() => compressImage(...a)), watch: watchDocument,
      recentDocuments: () => keptDocuments('fs.recentDocuments', d => d.recent(grants)), forgetDocument: path => keptDocuments(`fs.forgetDocument ${path}`, d => d.forget(path, grants)),
      transformImage: (from, to, options) => ordinary(() => image('transformImage', from, to, options, {})),
      ...Object.fromEntries(methods.map(m => [m, (...args) => ordinary(() => {
        // A write's `expect` (LLP 1069.010.000 D2a) is the glue's, documents only.
        if (expecting(m, args) && !isDocument(args)) return Promise.reject(new TypeError(`storage.fs.${m}(): expect is for doc:/ paths`));
        const captured = structuredClone(args);
        const run = () => copiesIn(m, captured) ? documents().then(files).then(f => globalThis.exact.documents.copyFile(captured[0], captured[1], grants, f))
          : isDocument(captured) ? documents().then(f => f[m](...captured)) : files().then(f => f[m](...captured), orEmpty(m, captured[0]));
        return (admitted ? queued(`${m}${typeof args[0] === 'string' ? ` ${args[0]}` : ''}`, run) : denied(`fs.${m}`, isDocument(captured) && !copiesIn(m, captured))).catch(refused);
      })])) }),
    sqlite: Object.freeze({ open: path => ordinary(() => { const call = answering.call; return (admitted ? queued(`open ${path}`, () => databases().then(d => d.open(path))).then(d => database(d, path, call)) : denied(`sqlite.open ${path}: no grant covers it; grant \`sqlite.open ${path}\`, or \`sqlite.open ${String(path).slice(0, String(path).lastIndexOf('/'))}\` for every file there (a grant covers its path and what is below it, by whole names)`)).catch(refused); }) }),
    work: promise => Promise.resolve(promise),
  });
}
// A stream (LLP 1016.000; ts-fetch.js), opened by ts-stream.js, loaded on
// first use so a module that never streams carries none of it.
const opener = (stream, conv, name) => (deliver, controller) => { controller.streamName = name; return import('./ts-stream.js').then(m => m.open(stream, conv, tsGrantSet, deliver, controller)); };
export function install(data, mixed = false, modules = null) {
  data.appId = source.appId;
  bodyFiles.appId = source.appId ?? null; // `exactBodyFrom`'s store (ts-fetch.js)
  data.grants = setAppGrantSet(tsGrantSet);
  const storage = storageOf(tsGrantSet);
  // The module's storage, as the runner's `state.background` (LLP 1097 D8).
  if (storage) data.background = () => ({ queued: queue.length, inFlight: head ? 1 : 0, ...counts });
  // A rejection nothing handled reaches the journal, as on every host (D8):
  // the page's own code handles its, so one unhandled is the module's.
  if (typeof addEventListener === 'function') addEventListener('unhandledrejection', e => {
    const r = e.reason;
    journal.push(`t=${clock.now} data: unhandled rejection: ${r && typeof r === 'object' && r.message !== undefined ? r.message : String(r)}`);
    leftOpen();
  });
  // `modules` loads native.js (an app with a module artifact), after first
  // paint (rt.js `painted`), whether or not anything asks `later`.
  load = modules && (() => painted().then(modules));
  if (modules && typeof requestAnimationFrame === 'function') pageModule().catch(e => journal.push(`t=${clock.now} native: ${e.message}`));
  let keys = null, asking = '', thrown = null, threwRead = false;
  // The answer an async function was called under, put back around each of
  // its steps by the bundle's async helper (js/bake/src/typescript.mjs
  // `asyncContextHelper`, LLP 1133.002 D14), as Hermes's prelude does with
  // its `currentCall`: a watch, a fetch or a storage step after an `await` is
  // that answer's. The synchronous turn alone stays `call.turn`.
  globalThis.__exact_async_context = Object.freeze({
    current: () => ({ call: answering.call, watching, asking }),
    swap(to) {
      const before = { call: answering.call, watching, asking };
      answering.call = to?.call ?? null; watching = to?.watching ?? null; asking = to?.asking ?? '';
      return before;
    },
  });
  const kept = () => keys ??= import('./storage-environment.js').then(({ keyStore, storageKey }) =>
    keyStore(typeof location === 'object' && source.appId ? storageKey(source.appId) : null, globalThis.indexedDB));
  __AUTH_INSTALL__
  // A module's `kept(source, args, value)` is given the answers its page was
  // rendered with, once, before it is first asked: they are its own answers,
  // made by the render host, which it may keep as it keeps any other.
  let seeded = !source.kept;
  const seed = () => {
    seeded = true;
    const page = checkpoint().kept;
    for (const r of page ? Resources : []) {
      const k = page.get(r.name), types = sourceTypes[r.source];
      if (k && types) try { source.kept(r.source, k[0].map((a, i) => named(a, types[0][i])), named(k[1], types[1])); } catch {}
    }
  };
  const ts = (name, args, store, target, mutation = false) => {
    if (!seeded) seed();
    const types = sourceTypes[name], [params, result] = types ?? [[], 'u'];
    // The store as the module sees it (LLP 1018): a read marks the answer.
    const seen = createSecretFacade(store, tsGrantSet, kept);
    // A call that read the store and then threw read it too: its resource is a store reader (LLP 1146 §2.3).
    threwRead = false;
    try { return answerWith(name, args, target, mutation, types, params, result, seen); } catch (e) { threwRead = !!seen.read; throw e; }
  };
  const answerWith = (name, args, target, mutation, types, params, result, seen) => {
    // A kept key's work is counted on its answer as an ordinary fetch is (LLP 1016.000 D9).
    for (const name of ['key', 'keepKey']) { const op = seen[name]; seen[name] = (...a) => ordinary(() => op(...a)); }
    asking = target ?? name; watching = name;
    for (const sources of moduleWatched.values()) sources.delete(name);
    // The origin of the commit that asked it (LLP 1133.002 D12), which a fetch it makes carries (LLP 1133.036 D6).
    const call = answering.call = { stream: null, mutation, name: target ?? name, turn: true, origin: Origin.now() }, target_ = target ?? name;
    if (call.origin !== 'app') answering.untraced.add(call);
    // What the source sent goes once the commit that asked it stands: an answer in flight's (a resource's, or a mutation's that
    // answers with a promise) whatever it then answers, a refused commit saying so (LLP 1016.000 D7, amended 2026-10-18).
    const effectsOf = answer => {
      const said = why => journal.push(`t=${clock.now} ${answer ? 'answer' : 'send'} ${target_} failed: ${why}; socket frames were refused`);
      return call.effects?.map(effect => Object.assign(() => { try { effect(); } catch (e) { said(e?.message ?? e); } }, answer ? { refused: said } : {}));
    };
    let r;
    thrown = null;
    try { r = source.answer(name, args.map((a, i) => named(a, params[i])), seen, storage, modules ? native : null); } catch (e) { call.settled = true; if (!mutation) thrown = effectsOf(true); throw e; } finally { call.turn = false; asking = ''; watching = null; answering.call = null; }
    const shaped = types ? v => checked(name, v, result) : v => v;
    // A stream's answer has replied once it returns (LLP 1016.000): what runs on is background work.
    // Socket operations go out only with a value given now: not with a
    // promise, a stream answer or any other fetch (LLP 1016.000 D7).
    if (call.socketConflict || call.effects?.length && call.fetched) {
      call.socketConflict = true; Promise.resolve(r).catch(() => {}); call.settled = true;
      throw new Error(SOCKET_AND_FETCH);
    }
    const effects = effectsOf(!mutation || !!(r && typeof r.then === 'function'));
    // An upload's request identity: the same request for newer arguments keeps it (rt.js, LLP 1016.000 D10).
    if (call.stream) { call.settled = true; return { stream: opener(call.stream, v => conv(shaped(v), result, target_), target_), upload: requestKey(call.stream), store: seen.read, effects }; }
    if (r && typeof r.then === 'function') {
      pendingAnswers++;
      call.seen = seen; // a read after an `await` makes a store reader too: rt.js looks when the answer lands (LLP 1146 §2.3)
      // Once: an answer let go while its timers were armed never settles, and ends here (ts-fetch.js `dropTimers`).
      const settle = () => { if (call.settled) return; call.settled = true; dropTimers(call); answering.untraced.delete(call); pendingAnswers--; if (!pendingAnswers && owed) { owed = false; leftOpen(); } };
      call.abandon = settle;
      return { promise: r.then(v => (settle(), conv(shaped(v), result, target_)), e => { settle(); leftOpen(); throw e; }), store: seen.read, effects, call };
    }
    call.settled = true; dropTimers(call); answering.untraced.delete(call);
    return { v: conv(shaped(r), result, target_), store: seen.read, effects };
  };
  // Beside a Rust source (LLP 1027.002): a source this module does not
  // answer is the Rust module's, not ready until it loads; rust-data.js
  // then asks it first and this module for what it calls unknown.
  data.answer = mixed ? (name, args, store, target, mutation) => { try { return ts(name, args, store, target, mutation); } catch { return null; } } : ts;
  // An overlay (`overlay.js`): what a resource shows while writes affect it, in overlay mode; `undefined` without one.
  overlaying.say = line => journal.push(`t=${clock.now} ${line}`);
  data.overlay = source.overlay && ((name, args, answer, writes) => {
    const types = sourceTypes[name]; if (!types) return undefined;
    const keep = [], ws = writes.map(w => { const t = sourceTypes[w.source]; return Object.freeze({ id: w.id, mutation: w.mutation, source: w.source, answered: w.answered,
      args: w.args.map((a, i) => named(a, t?.[0][i])), ...(w.reply === undefined ? {} : { reply: named(w.reply, t?.[1]) }), keep: () => { if (!keep.includes(w.id)) keep.push(w.id); } }); });
    overlaying.on = true;
    try {
      const v = source.overlay(name, args.map((a, i) => named(a, types[0][i])), named(answer, types[1]), ws);
      if (v && typeof v.then === 'function') throw new Error('an overlay returned a promise: it runs synchronously inside a commit and cannot await; compute its value from `answer` and `writes` and return it');
      return v === undefined ? undefined : { value: conv(checked(name, v, types[1]), types[1], `overlay ${name}`), keep };
    } finally { overlaying.on = false; }
  });
  data.ts = ts;
  // A module calling the app back (LLP 1133.016 D9, D15 item 1): the handler `tools` exports under `name`, run as a
  // fresh answer that fills no slot, with the call's origin on it, so a module call it makes is admitted with that
  // origin (`__exact_module`). Its answer, or null for a name `tools` does not export.
  data.tool = (name, args, store, origin) => {
    const handler = source.tools?.[name];
    if (typeof handler !== 'function') return null;
    const call = answering.call = { stream: null, mutation: true, name, turn: true, origin };
    asking = name;
    let r;
    try { r = handler(args, createSecretFacade(store, tsGrantSet, kept), storage); }
    finally { call.turn = false; asking = ''; answering.call = null; }
    return Promise.resolve(r).finally(() => { call.settled = true; dropTimers(call); });
  };
  // A resource's source that sent and then threw: what it sent, which rt.js `ask` queues with the commit.
  data.thrown = () => { const t = thrown ?? []; thrown = null; return t; };
  data.threwRead = () => { const read = threwRead; threwRead = false; return read; };
  for (const f of data.q.splice(0)) f();
}
