// The roster's pure entries (runner/src/stdlib.rs) on the JS target, re-exported by rt.js, which keeps the ones that
// read its state (`x_now`, the router's, `t`).
/** A native module's props (LLP 1024 D1): key/value pairs to one JSON
 * object of strings, a none left out (`stdlib::native_props`). */
export const NP = p => { const o = {}; for (let i = 0; i < p.length; i += 2) if (p[i + 1] != null) o[p[i]] = String(p[i + 1]); return JSON.stringify(o); };
export const x_length = v => v.length;
export const x_isEmpty = v => v.length === 0;
export const x_floor = Math.floor;
// `f64::max`/`min` (runner/src/stdlib.rs): a NaN operand gives the other one.
export const x_max = (a, b) => a !== a ? b : b !== b ? a : Math.max(a, b), x_min = (a, b) => a !== a ? b : b !== b ? a : Math.min(a, b);
// Numbers print as JavaScript prints them (`push_number`), `-0` as `0`.
export const x_toString = v => String(v);
export const x_includes = (a, b) => a.includes(b), x_startsWith = (a, b) => a.startsWith(b), x_endsWith = (a, b) => a.endsWith(b);
export const x_trim = s => s.trim();
export const x_first = l => l.length ? l[0] : null;
/** `Array.prototype.at`, `none` where JavaScript answers undefined (`Stdlib::At`). */
export const x_at = (l, i) => { const v = l.at(i); return v === undefined ? null : v; };
export const x_join = (l, s) => l.map(String).join(s);
export const x_encodeURIComponent = encodeURIComponent;
export { x_formatTime, x_formatDate, x_formatNumber } from "./format.js";
