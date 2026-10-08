// A TypeScript data source on the JS runtime (LLP 1027): the app's own
// `app.ts`, bundled with the page, runs in the browser — the executor on
// the web anyway (D6). Values cross by the plan's types: the runtime's
// records are arrays, the module's are objects by field name. The store
// keeps keys as the web host does (LLP 1069.005 D1b: the pair itself in
// IndexedDB under a handle the store holds; its code is fetched on first
// use); `openAuthSession` is auth.js.
import * as source from '__APP_TS__';
import { createSecretFacade, hasGrant, setAppGrantSet } from './admission.js';
import { tsGrantSet } from './admission-data.js';
import { answering } from './ts-fetch.js';
import { sourceTypes } from './names.js';
import { checkpoint, clock, commit, inflight, journal, painted, R, Resources } from './rt.js';
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
  if (e) throw Object.assign(new Error(`\`${name}\` answered outside its shape: ${e}`), { kind: 'Unavailable' });
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
const pageModule = () => page ??= load().then(m => m.pageModule({
  agent: clock.agent, now: () => clock.now,
  changed: topic => commit(() => { const s = watched.get(topic); for (const r of Resources) if (s?.has(r.source)) changed(r, topic); }, `native ${topic}`),
}));
const native = Object.freeze({
  get available() { return true; },
  call() { throw new Error('native.call: the web has no synchronous module call; use native.later'); },
  watch(topic) { if (!watching) throw new Error('native.watch outside an answer'); const t = String(topic); (watched.get(t) ?? watched.set(t, new Set()).get(t)).add(watching); },
  later: request => pageModule().then(p => p.later(request)),
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
    if (k == null) {
      // Said once in the journal, as on every host (trivia F7).
      if (!toldAgent) { toldAgent = true; journal.push(`t=${clock.now} storage refused (agent): ${agentStorageRefusal}`); }
      throw Object.assign(new Error(agentStorageRefusal), { kind: 'Unavailable', code: 'agent' });
    }
    return k;
  });
  const files = () => fs ??= key().then(k => import(new URL('./storage-fs.js', import.meta.url).href).then(m => m.createFileSystem(k, grants)));
  // A document the person chose (`doc:/`, LLP 1069.010 D1) is the page's
  // handle, not app storage: no store key, so a drive without a scratch
  // store reaches it too, as a Rust source's storage request does.
  let docs;
  const documents = () => docs ??= ((globalThis.exact ??= {}), import(new URL('./documents-glue.js', import.meta.url).href)).then(() => globalThis.exact.documents.files(grants));
  const isDocument = args => args.slice(0, 2).some(p => typeof p === 'string' && p.startsWith('doc:/'));
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
  return Object.freeze({
    fs: Object.freeze({ directories: Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' }),
      ...Object.fromEntries(methods.map(m => [m, (...args) => {
        const captured = structuredClone(args);
        const run = () => (isDocument(captured) ? documents() : files()).then(f => f[m](...captured));
        return (admitted ? queued(`${m}${typeof args[0] === 'string' ? ` ${args[0]}` : ''}`, run) : denied(`fs.${m}`, isDocument(captured))).catch(coded);
      }])) }),
    sqlite: Object.freeze({ open: path => { const call = answering.call; return (admitted ? queued(`open ${path}`, () => databases().then(d => d.open(path))).then(d => database(d, path, call)) : denied(`sqlite.open ${path}: no grant covers it; grant \`sqlite.open ${path}\`, or \`sqlite.open ${String(path).slice(0, String(path).lastIndexOf('/'))}\` for every file there (a grant covers its path and what is below it, by whole names)`)).catch(coded); } }),
    work: promise => Promise.resolve(promise),
  });
}
// A stream (LLP 1016.000; ts-fetch.js), opened by ts-stream.js, loaded on
// first use so a module that never streams carries none of it.
const opener = (stream, conv) => (deliver, controller) => import('./ts-stream.js').then(m => m.open(stream, conv, tsGrantSet, deliver, controller));
export function install(data, mixed = false, modules = null) {
  data.appId = source.appId;
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
  let keys = null, asking = '';
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
  const ts = (name, args, store, target) => {
    if (!seeded) seed();
    const types = sourceTypes[name], [params, result] = types ?? [[], 'u'];
    // The store as the module sees it (LLP 1018): a read marks the answer.
    const seen = createSecretFacade(store, tsGrantSet, kept);
    asking = target ?? name; watching = name;
    const call = answering.call = { stream: null };
    let r;
    try { r = source.answer(name, args.map((a, i) => named(a, params[i])), seen, storage, modules ? native : null); } catch (e) { call.settled = true; throw e; } finally { asking = ''; watching = null; answering.call = null; }
    const target_ = target ?? name;
    const shaped = types ? v => checked(name, v, result) : v => v;
    // A stream's answer has replied once it returns (LLP 1016.000): what runs on is background work.
    if (call.stream) { call.settled = true; return { stream: opener(call.stream, v => conv(shaped(v), result, target_)), store: seen.read }; }
    if (r && typeof r.then === 'function') {
      pendingAnswers++;
      const settle = () => { call.settled = true; pendingAnswers--; if (!pendingAnswers && owed) { owed = false; leftOpen(); } };
      return { promise: r.then(v => (settle(), conv(shaped(v), result, target_)), e => { settle(); leftOpen(); throw e; }), store: seen.read };
    }
    call.settled = true;
    return { v: conv(shaped(r), result, target_), store: seen.read };
  };
  // Beside a Rust source (LLP 1027.002): a source this module does not
  // answer is the Rust module's, not ready until it loads; rust-data.js
  // then asks it first and this module for what it calls unknown.
  data.answer = mixed ? (name, args, store, target) => { try { return ts(name, args, store, target); } catch { return null; } } : ts;
  data.ts = ts;
  for (const f of data.q.splice(0)) f();
}
