// The roster's pure entries (runner/src/stdlib.rs) on the JS target, re-exported by rt.js, which keeps the ones that
// read its state (`x_now`, the router's, `t`); budget.js holds the ones that build a string past MAX_STRING's reach.
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
export { x_formatTime, x_formatDate, x_formatNumber } from "./format.js";
