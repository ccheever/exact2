// The roster's pure entries (runner/src/stdlib.rs) on the JS target, re-exported by rt.js, which keeps the ones that
// read its state (`x_now`, the router's, `t`); budget.js holds the ones that build a string past MAX_STRING's reach.
export const x_length = v => v.length;
export const x_isEmpty = v => v.length === 0;
export const x_floor = Math.floor;
// `f64::max`/`min` (runner/src/stdlib.rs): a NaN operand gives the other one.
export const x_max = (a, b) => a !== a ? b : b !== b ? a : Math.max(a, b), x_min = (a, b) => a !== a ? b : b !== b ? a : Math.min(a, b);
// Numbers print as JavaScript prints them (`push_number`), `-0` as `0`.
export const x_toString = v => String(v);
export const x_startsWith = (a, b) => a.startsWith(b), x_endsWith = (a, b) => a.endsWith(b);
export const x_trim = s => s.trim();
export const x_first = l => l.length ? l[0] : null;
/** `Array.prototype.at`, `none` where JavaScript answers undefined (`Stdlib::At`). */
export const x_at = (l, i) => { const v = l.at(i); return v === undefined ? null : v; };
// LLP 1102 §3.1–§3.4 (runner/src/stdlib.rs `js_round`, `parse_number`, `calendar_diff`).
export const x_ceil = Math.ceil, x_round = Math.round;
/** `parseNumber`: `trim`'s whitespace, a sign, digits and an optional fraction (or a fraction alone), an optional exponent;
 * `Number()`'s correctly rounded value, or null for other text, a result past the largest finite, or a nonzero numeral
 * that rounds to zero. `\d` without the `u` flag is ASCII only, as the runner's grammar is. */
export const x_parseNumber = s => {
  const t = s.trim(), m = /^[+-]?(\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.exec(t);
  if (!m) return null;
  const n = Number(t);
  return Number.isFinite(n) && (n !== 0 || !/[1-9]/.test(m[1])) ? n : null;
};
/** A `YYYY-MM-DD` date (years 0–9999, a real day of the month), or null. */
const isoDate = s => {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return null;
  const y = +m[1], mo = +m[2], d = +m[3], leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
  return mo >= 1 && mo <= 12 && d >= 1 && d <= [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][mo - 1] ? [y, mo, d] : null;
};
const before = (a, b) => a[0] < b[0] || a[0] === b[0] && (a[1] < b[1] || a[1] === b[1] && a[2] < b[2]);
/** `calendarDiff`: whole years or months counted as an age is, negated when `to` is earlier; a zero count is +0, as the
 * runner's integer is. */
export const x_calendarDiff = (from, to, unit) => {
  let a = isoDate(from), b = isoDate(to), sign = 1;
  if (!a || !b) return null;
  if (before(b, a)) [a, b, sign] = [b, a, -1];
  const n = unit === "months" ? (b[0] - a[0]) * 12 + b[1] - a[1] - (b[2] < a[2] ? 1 : 0)
    : b[0] - a[0] - (b[1] < a[1] || b[1] === a[1] && b[2] < a[2] ? 1 : 0);
  return n && sign * n;
};
export { x_formatTime, x_formatDate, x_formatNumber } from "./format.js";

