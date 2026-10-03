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
import { sourceTypes } from './names.js';
import { checkpoint, clock, commit, journal, painted, R, Resources } from './rt.js';
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
    // A list of leaves is copied and compared in one loop, with no call per
    // item (a 10,000-row grid's 20-point histories are 200,000 items).
    c[0] = leaf
      ? v => { if (v == null) return null; const n = v.length, out = new Array(n); for (let i = 0; i < n; i++) { const x = v[i]; out[i] = x == null ? null : x; } return out; }
      : v => v == null ? null : v.map(x => named(x));
    // An item is matched by its index even when the list grew or shrank.
    c[1] = leaf ? (v, o) => {
      if (v == null) return null;
      if (!Array.isArray(v)) return v.map(x => x == null ? null : x);
      const n = v.length;
      let i = 0;
      if (Array.isArray(o) && o.length === n) {
        for (; i < n; i++) { const x = v[i] == null ? null : v[i], y = o[i]; if (x === y ? x === 0 && 1 / x !== 1 / y : x === x || y === y) break; }
        if (i === n) return o;
      }
      const out = i ? o.slice(0, i) : [];
      for (; i < n; i++) out.push(v[i] == null ? null : v[i]);
      return out;
    } : (v, o) => {
      if (v == null) return null;
      if (!Array.isArray(v)) return v.map(x => arrays(x));
      const n = v.length, was = Array.isArray(o) ? o : null, f = sub[1];
      let out = was && was.length === n ? null : [];
      for (let i = 0; i < n; i++) {
        const x = f(v[i], was?.[i]);
        if (out) out.push(x); else if (!same(x, o[i])) { out = o.slice(0, i); out.push(x); }
      }
      return out ?? o;
    };
    return c;
  }
  const k = Object.keys(t), n = k.length, subs = k.map(f => converters(t[f]));
  const leaves = subs.map(s => s[2] === true);
  // Every object of a type is copied from one template, so all share a shape.
  const tmpl = {}; for (const f of k) tmpl[f] = null;
  c[0] = k.includes('__proto__')
    ? v => v == null ? null : Object.fromEntries(k.map((f, i) => [f, subs[i][0](v[i])]))
    : v => {
      if (v == null) return null;
      const out = { ...tmpl };
      for (let i = 0; i < n; i++) out[k[i]] = leaves[i] ? (v[i] == null ? null : v[i]) : subs[i][0](v[i]);
      return out;
    };
  c[1] = (v, o) => {
    if (v == null) return null;
    const was = Array.isArray(o) ? o : null;
    let out = was && was.length === n ? null : [];
    for (let i = 0; i < n; i++) {
      const y = v[k[i]], x = leaves[i] ? (y == null ? null : y) : subs[i][1](y, was?.[i]);
      if (out) out.push(x); else { const z = o[i]; if (x === z ? x === 0 && 1 / x !== 1 / z : x === x || z === z) { out = o.slice(0, i); out.push(x); } }
    }
    return out ?? o;
  };
  return c;
}
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
const pageModule = () => page ??= load().then(m => m.pageModule({
  agent: clock.agent, now: () => clock.now,
  changed: topic => commit(() => { const s = watched.get(topic); for (const r of Resources) if (s?.has(r.source)) R(r); }, `native ${topic}`),
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
// the drive names a scratch store (`storageKey`). Only where the grants
// name `fs.` or `sqlite.`.
function storageOf(grants) {
  if (!['fs-read', 'fs-write', 'sqlite-open'].some(kind => hasGrant(grants, kind))) return undefined;
  let fs, sqlite;
  const key = () => import('./storage-environment.js').then(({ storageKey, agentStorageRefusal }) => {
    const k = source.appId ? storageKey(source.appId, location.href) : null;
    if (k == null) throw Object.assign(new Error(agentStorageRefusal), { kind: 'Unavailable' });
    return k;
  });
  const files = () => fs ??= key().then(k => import(new URL('./storage-fs.js', import.meta.url).href).then(m => m.createFileSystem(k, grants)));
  const databases = () => sqlite ??= key().then(k => import(new URL('./storage-sqlite.js', import.meta.url).href).then(m => m.createSqlite(k, grants)));
  const methods = ['readFile', 'writeFile', 'atomicWriteFile', 'appendFile', 'readdir', 'mkdir', 'rm', 'stat', 'rename', 'copyFile', 'realpath'];
  return Object.freeze({
    fs: Object.freeze({ directories: Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' }),
      ...Object.fromEntries(methods.map(m => [m, (...args) => files().then(f => f[m](...structuredClone(args)))])) }),
    sqlite: Object.freeze({ open: path => databases().then(d => d.open(path)) }),
    work: promise => Promise.resolve(promise),
  });
}
export function install(data, mixed = false, modules = null) {
  data.appId = source.appId;
  data.grants = setAppGrantSet(tsGrantSet);
  const storage = storageOf(tsGrantSet);
  // `modules` loads native.js (an app with a module artifact), after first
  // paint (rt.js `painted`), whether or not anything asks `later`.
  load = modules && (() => painted().then(modules));
  if (modules && typeof requestAnimationFrame === 'function') pageModule().catch(e => journal.push(`t=${clock.now} native: ${e.message}`));
  let keys = null, asking = '';
  const kept = () => keys ??= import('./storage-environment.js').then(({ keyStore, storageKey }) =>
    keyStore(typeof location === 'object' && source.appId ? storageKey(source.appId, location.href) : null, globalThis.indexedDB));
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
    const [params, result] = sourceTypes[name] ?? [[], 'u'];
    // The store as the module sees it (LLP 1018): a read marks the answer.
    const seen = createSecretFacade(store, tsGrantSet, kept);
    asking = target ?? name; watching = name;
    let r;
    try { r = source.answer(name, args.map((a, i) => named(a, params[i])), seen, storage, modules ? native : null); } finally { asking = ''; watching = null; }
    const target_ = target ?? name;
    if (r && typeof r.then === 'function') return { promise: r.then(v => conv(v, result, target_)), store: seen.read };
    return { v: conv(r, result, target_), store: seen.read };
  };
  // Beside a Rust source (LLP 1027.002): a source this module does not
  // answer is the Rust module's, not ready until it loads; rust-data.js
  // then asks it first and this module for what it calls unknown.
  data.answer = mixed ? (name, args, store, target) => { try { return ts(name, args, store, target); } catch { return null; } } : ts;
  data.ts = ts;
  for (const f of data.q.splice(0)) f();
}
