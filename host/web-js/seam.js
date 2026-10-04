// A big answer crossing the web seam, in a development build only (perf.js
// installs it; a production build references none of it). On the JS target a
// data module's answer is copied into the runtime whole at every ask, decoded
// from a Rust module's bytes (rust-data.js) or converted from a TypeScript
// module's objects (ts-data.js), then shape-checked: a mutation answering the
// heavy-list bench's 10,000-row feed costs a tap 80–96 ms, where a bounded
// answer (LLP 1027.004) costs 16–24. An answer whose longest list has more
// than ROWS rows is said once per resource or mutation, in the journal
// (`agent.mjs web … logs`; `exact.journal` in a browser's console).
//
// ROWS, measured (2026-10-04, Chrome, M5 Ultra): a rich row (the bench's
// message: paragraphs, runs, reactions) costs ~5 µs at the seam, 4 of them
// decoding; a flat one (messages-stress) ~0.9 µs. 2,000 rich rows are about a
// 60 Hz frame. The repo's apps answer at most 204 rows in normal use
// (Messages after four "add fifty"; Caltrain 29, Fieldnotes one per note,
// messages-stress 101, its bounded window 200, its eager control 1,000); only
// messages-stress's whole-history controls at 10,000 and up cross it.
export const ROWS = 2000;

/** The rows of the longest list in `v`, at any depth, by its plan type
 * (`sourceTypes`' result: a leaf letter, `['[', T]`, `['?', T]`, or a
 * record's fields by name): what an author counts as the list's length. */
export function rows(v, t) {
  if (v == null || !t || typeof t === 'string') return 0;
  if (Array.isArray(t)) {
    if (t[0] === '?') return rows(v, t[1]);
    let n = v.length;
    if (t[1] && typeof t[1] !== 'string') for (const x of v) n = Math.max(n, rows(x, t[1]));
    return n;
  }
  let n = 0, k = 0;
  for (const f in t) n = Math.max(n, rows(v[k++], t[f]));
  return n;
}

/** Watch `data`'s answers (`answer`, and `parse` after a request) as rust-data.js
 * and ts-data.js install them, before or after this runs. `owner(args)` names
 * the resource or mutation whose ticket a parsed reply answers. */
export function watch(data, { types, say, owner = () => null, limit = ROWS }) {
  const told = new Set();
  const check = (source, target, a) => {
    const t = types[source]?.[1];
    if (!a || !('v' in a) || !t || told.has(target)) return;
    const n = rows(a.v, t);
    if (n <= limit) return;
    told.add(target);
    say(`big answer: ${target} (source ${source}) carries ${n} list rows across the web seam (over ${limit}); each ask copies them all. Answer a window: LLP 1027.004, docs/agent-pitfalls.md "A tap that changes one row of a long list"`);
  };
  const wrap = {
    answer: f => (source, args, store, target) => {
      const a = f(source, args, store, target), name = target ?? source;
      if (a?.promise) return { ...a, promise: a.promise.then(v => (check(source, name, { v }), v)) };
      if (a?.then) return a.then(v => (check(source, name, { v }), v));
      check(source, name, a);
      return a;
    },
    parse: f => (source, args, outcome, store) => {
      const a = f(source, args, outcome, store);
      check(source, owner(args) ?? source, a);
      return a;
    },
  };
  for (const key of Object.keys(wrap)) {
    let f = data[key] && wrap[key](data[key]);
    Object.defineProperty(data, key, { configurable: true, enumerable: true, get: () => f, set: g => { f = g && wrap[key](g); } });
  }
}
