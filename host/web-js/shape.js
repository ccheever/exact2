// A value's shape check by the plan's type codes (LLP 1005 §3), for rt.js.
/** Whether `v` conforms to type code `t` (`n b s u ?T [T {T…}`), from `i`;
 * numbers are finite, as the runner's shape checks require. `o` is a value
 * that conformed: a part of `v` that is the same array as its part in `o`
 * conforms as it did, unchecked (the Rust runner's `Conformed` re-checks
 * only the list items that are not the same object, runner/src/conform.rs). */
export function conforms(v, t, i = [0], o) {
  if (o !== undefined && v === o && typeof v === "object" && v !== null) { skip(t, i); return true; }
  const c = t[i[0]++];
  if (c === "n") return typeof v === "number" && isFinite(v);
  if (c === "b") return typeof v === "boolean";
  if (c === "s") return typeof v === "string";
  if (c === "u") return v == null;
  if (c === "?") { if (v == null) { skip(t, i); return true; } return conforms(v, t, i, o); }
  if (c === "[") {
    const at = i[0], was = Array.isArray(o) ? o : null;
    if (!Array.isArray(v)) return false;
    for (let k = 0; k < v.length; k++) { i[0] = at; if (!conforms(v[k], t, i, was?.[k])) return false; }
    i[0] = at; skip(t, i); return true;
  }
  if (c === "{") { let k = 0; const was = Array.isArray(o) ? o : null; for (; t[i[0]] !== "}"; k++) if (!Array.isArray(v) || !conforms(v[k], t, i, was?.[k])) return false; i[0]++; return v.length === k; }
  return true;
}
function skip(t, i) { const c = t[i[0]++]; if (c === "?" || c === "[") skip(t, i); else if (c === "{") { while (t[i[0]] !== "}") skip(t, i); i[0]++; } }
/** Plan value equality: signed zero, NaN, and recursively equal lists. */
export function eq(a, b) {
  if (a === b) return a !== 0 || 1 / a === 1 / b;
  if (typeof a === "number" && typeof b === "number") return a !== a && b !== b;
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!eq(a[i], b[i])) return false;
  return true;
}
