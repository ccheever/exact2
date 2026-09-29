// A TypeScript data source on the JS runtime (LLP 1027): the app's own
// `app.ts`, bundled with the page, runs in the browser — the executor on
// the web anyway (D6). Values cross by the plan's types: the runtime's
// records are arrays, the module's are objects by field name. The store
// keeps keys as the web host does (LLP 1069.005 D1b: the pair itself in
// IndexedDB under a handle the store holds; its code is fetched on first
// use); `openAuthSession` is auth.js.
import * as source from '__APP_TS__';
import { sourceTypes } from './names.js';
__AUTH_IMPORT__
const named = (v, t) => v == null ? null : typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? named(v, t[1]) : v.map(x => named(x, t[1]))) : Object.fromEntries(Object.keys(t).map((k, i) => [k, named(v[i], t[k])]));
const arrays = (v, t) => v == null ? null : typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? arrays(v, t[1]) : v.map(x => arrays(x, t[1]))) : Object.keys(t).map(k => arrays(v[k], t[k]));
export function install(data) {
  let keys = null, asking = '';
  const kept = () => keys ??= import('./storage-environment.js').then(({ keyStore, storageKey }) =>
    keyStore(typeof location === 'object' && source.appId ? storageKey(source.appId, location.href) : null, globalThis.indexedDB));
  __AUTH_INSTALL__
  data.answer = (name, args, store, target) => {
    const [params, result] = sourceTypes[name] ?? [[], 'u'];
    // The store as the module sees it (LLP 1018): a read marks the answer.
    const seen = { get: k => { seen.read = true; return store.get(k); }, set: (k, v) => store.set(k, String(v)), forget: k => store.set(k, null),
      keepKey: (k, pair) => { const handle = 'exact.key:' + crypto.randomUUID(); store.set(k, handle); return kept().then(s => s.put(handle, pair)); },
      key: k => { const handle = store.get(k); return handle == null ? Promise.resolve(null) : kept().then(s => s.get(handle)); } };
    asking = target ?? name;
    let r;
    try { r = source.answer(name, args.map((a, i) => named(a, params[i])), seen, undefined); } finally { asking = ''; }
    if (r && typeof r.then === 'function') return { promise: r.then(v => arrays(v, result)), store: seen.read };
    return { v: arrays(r, result), store: seen.read };
  };
  for (const f of data.q.splice(0)) f();
}
