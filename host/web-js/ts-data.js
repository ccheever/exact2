// A TypeScript data source on the JS runtime (LLP 1027): the app's own
// `app.ts`, bundled with the page, runs in the browser — the executor on
// the web anyway (D6). Values cross by the plan's types: the runtime's
// records are arrays, the module's are objects by field name. The store
// keeps keys as the web host does (LLP 1069.005 D1b: the pair itself in
// IndexedDB under a handle the store holds; its code is fetched on first
// use); `openAuthSession` is auth.js.
import * as source from '__APP_TS__';
import { sourceTypes } from './names.js';
import { clock, commit, journal, R, Resources } from './rt.js';
__AUTH_IMPORT__
export const named = (v, t) => v == null ? null : !t || typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? named(v, t[1]) : v.map(x => named(x, t[1]))) : Object.fromEntries(Object.keys(t).map((k, i) => [k, named(v[i], t[k])]));
const arrays = (v, t) => v == null ? null : typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? arrays(v, t[1]) : v.map(x => arrays(x, t[1]))) : Object.keys(t).map(k => arrays(v[k], t[k]));
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
export function install(data, mixed = false, modules = null) {
  // `modules` loads native.js (an app with a module artifact). Connected
  // after first paint, whether or not anything asks `later`.
  load = modules;
  if (modules && typeof requestAnimationFrame === 'function') requestAnimationFrame(() => requestAnimationFrame(() => pageModule().catch(e => journal.push(`t=${clock.now} native: ${e.message}`))));
  let keys = null, asking = '';
  const kept = () => keys ??= import('./storage-environment.js').then(({ keyStore, storageKey }) =>
    keyStore(typeof location === 'object' && source.appId ? storageKey(source.appId, location.href) : null, globalThis.indexedDB));
  __AUTH_INSTALL__
  const ts = (name, args, store, target) => {
    const [params, result] = sourceTypes[name] ?? [[], 'u'];
    // The store as the module sees it (LLP 1018): a read marks the answer.
    const seen = { get: k => { seen.read = true; return store.get(k); }, set: (k, v) => store.set(k, String(v)), forget: k => store.set(k, null),
      keepKey: (k, pair) => { const handle = 'exact.key:' + crypto.randomUUID(); store.set(k, handle); return kept().then(s => s.put(handle, pair)); },
      key: k => { const handle = store.get(k); return handle == null ? Promise.resolve(null) : kept().then(s => s.get(handle)); } };
    asking = target ?? name; watching = name;
    let r;
    try { r = source.answer(name, args.map((a, i) => named(a, params[i])), seen, undefined, modules ? native : null); } finally { asking = ''; watching = null; }
    if (r && typeof r.then === 'function') return { promise: r.then(v => arrays(v, result)), store: seen.read };
    return { v: arrays(r, result), store: seen.read };
  };
  // Beside a Rust source (LLP 1027.002): a source this module does not
  // answer is the Rust module's, not ready until it loads; rust-data.js
  // then asks it first and this module for what it calls unknown.
  data.answer = mixed ? (name, args, store, target) => { try { return ts(name, args, store, target); } catch { return null; } } : ts;
  data.ts = ts;
  for (const f of data.q.splice(0)) f();
}
