// A TypeScript data source on the exact3 runtime (LLP 1027): the app's own
// `app.ts`, bundled with the page, runs in the browser — the executor on
// the web anyway (D6). Values cross by the plan's types: the runtime's
// records are arrays, the module's are objects by field name.
import { answer } from '__APP_TS__';
import { sourceTypes } from './names.js';
const named = (v, t) => v == null ? null : typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? named(v, t[1]) : v.map(x => named(x, t[1]))) : Object.fromEntries(Object.keys(t).map((k, i) => [k, named(v[i], t[k])]));
const arrays = (v, t) => v == null ? null : typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? arrays(v, t[1]) : v.map(x => arrays(x, t[1]))) : Object.keys(t).map(k => arrays(v[k], t[k]));
export function install(data) {
  data.answer = (source, args, store) => {
    const [params, result] = sourceTypes[source] ?? [[], 'u'];
    // The store as the module sees it (LLP 1018): a read marks the answer.
    const seen = { get: k => { seen.read = true; return store.get(k); }, set: (k, v) => store.set(k, String(v)), forget: k => store.set(k, null) };
    const r = answer(source, args.map((a, i) => named(a, params[i])), seen, undefined);
    if (r && typeof r.then === 'function') return { promise: r.then(v => arrays(v, result)), store: seen.read };
    return { v: arrays(r, result), store: seen.read };
  };
  for (const f of data.q.splice(0)) f();
}
