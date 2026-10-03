// A value's shape check by the plan's type codes (LLP 1005 §3), for rt.js.
/** Whether `v` conforms to type code `t` (`n b s u ?T [T {T…}`), from `i`;
 * numbers are finite, as the runner's shape checks require. `o` is a value
 * that conformed: a part of `v` that is the same array as its part in `o`
 * conforms as it did, unchecked (the Rust runner's `Conformed` re-checks
 * only the list items that are not the same object, runner/src/conform.rs). */
export function conforms(v, t, i = [0], o, d = 0) {
  if (typeof v === "object" && v !== null) {
    if (v === o) { skip(t, i); return true; }
    // A value and its parts one level down remember the type they conformed
    // to: a derive that passes on (part of) a checked answer is not walked
    // again (runtime values are never edited in place).
    if (d < 2) {
      const k = code(t, i[0]);
      if (Ok.get(v) === k) { skip(t, i); return true; }
      if (!shape(v, t, i, o, d)) return false;
      Ok.set(v, k);
      return true;
    }
  }
  return shape(v, t, i, o, d);
}
const Ok = new WeakMap();
function shape(v, t, i, o, d) {
  const c = t[i[0]++];
  if (c === "n") return typeof v === "number" && isFinite(v);
  if (c === "b") return typeof v === "boolean";
  if (c === "s") return typeof v === "string";
  if (c === "u") return v == null;
  if (c === "?") { if (v == null) { skip(t, i); return true; } return conforms(v, t, i, o, d); }
  if (c === "[") {
    const at = i[0], was = Array.isArray(o) ? o : null;
    if (!Array.isArray(v)) return false;
    for (let k = 0; k < v.length; k++) { i[0] = at; if (!conforms(v[k], t, i, was?.[k], d + 1)) return false; }
    i[0] = at; skip(t, i); return true;
  }
  if (c === "{") { let k = 0; const was = Array.isArray(o) ? o : null; for (; t[i[0]] !== "}"; k++) if (!Array.isArray(v) || !conforms(v[k], t, i, was?.[k], d + 1)) return false; i[0]++; return v.length === k; }
  return true;
}
/** Past the type at `i`: each position's end, found once per type code (a
 * list of 10,000 unchanged rows skips each row in one step, not its code's length). */
const Ends = new Map(), Codes = new Map();
function ends(t) {
  let e = Ends.get(t);
  if (!e) {
    e = new Int32Array(t.length + 1);
    const walk = p => { const c = t[p]; let q = p + 1; if (c === "?" || c === "[") q = walk(q); else if (c === "{") { while (q < t.length && t[q] !== "}") q = walk(q); q++; } return e[p] = q; };
    for (let p = 0; p < t.length;) p = walk(p);
    Ends.set(t, e);
  }
  return e;
}
function skip(t, i) { i[0] = ends(t)[i[0]]; }
/** The type code at `p` of `t`, the same string each time it is asked. */
function code(t, p) {
  let c = Codes.get(t);
  if (!c) Codes.set(t, c = []);
  return c[p] ??= typeof t === "string" ? t.slice(p, ends(t)[p]) : String(t.slice(p, ends(t)[p]));
}
/** Plan value equality: signed zero, NaN, and recursively equal lists. */
export function eq(a, b) {
  if (a === b) return a !== 0 || 1 / a === 1 / b;
  if (typeof a === "number" && typeof b === "number") return a !== a && b !== b;
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!eq(a[i], b[i])) return false;
  return true;
}
