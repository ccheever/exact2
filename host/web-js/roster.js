// The roster's pure entries (runner/src/stdlib.rs) on the JS target, re-exported by rt.js, which keeps the ones that
// read its state (`x_now`, the router's, `t`).
import { Refusal } from "./rt.js";
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
// Strings as JavaScript has them (LLP 1088 D2), each result made well formed once, as the runner's (runner/src/strings.rs).
/** `slice(s, start, end?)`: the compiler writes an omitted `end` as `Number.MAX_VALUE`, which clamps as `undefined` does. */
export const x_slice = (s, a, b) => s.slice(a, b).toWellFormed();
/** The runner's `MAX_STRING` (runner/src/vm.rs), UTF-8 bytes of the well-formed result. */
const MAX_STRING = 1 << 26;
const tooLong = () => { throw new Refusal("a string would pass MAX_STRING bytes"); };
/** `replaceAll(s, find, with)` with a string `find`, built piece by piece through one counter of the result's
 * well-formed UTF-8 that carries a high surrogate across pieces (a low one completing it is the pair's 4 bytes,
 * anything else settles it as U+FFFD's 3), so a quadratic `` $` `` stops at the bound before it is built. */
export function x_replaceAll(s, find, w) {
  let bytes = 0, high = false;
  const out = [], put = piece => {
    for (let i = 0; i < piece.length; i++) {
      const u = piece.charCodeAt(i);
      if (high) { high = false; if (u >= 0xdc00 && u <= 0xdfff) { bytes += 4; continue; } bytes += 3; }
      if (u >= 0xd800 && u <= 0xdbff) high = true;
      else bytes += u < 0x80 ? 1 : u < 0x800 ? 2 : 3; // a lone low half is U+FFFD's 3
      if (bytes > MAX_STRING) tooLong();
    }
    out.push(piece);
  };
  // GetSubstitution for a string pattern: `$$`, `$&`, `` $` ``, `$'`; anything else after `$` is itself.
  const parts = [];
  for (let i = 0, text = ""; i <= w.length; i++) {
    const c = w[i], n = w[i + 1];
    if (i === w.length || c === "$" && "&`'".includes(n ?? "!")) {
      if (text) parts.push(text);
      text = "";
      if (i < w.length) { parts.push(n === "&" ? 0 : n === "`" ? 1 : 2); i++; }
    } else if (c === "$" && n === "$") { text += "$"; i++; } else text += c;
  }
  const sub = at => { for (const p of parts) put(p === 0 ? find : p === 1 ? s.slice(0, at) : p === 2 ? s.slice(at + find.length) : p); };
  if (find === "") for (let at = 0; at <= s.length; at++) { if (at) put(s[at - 1]); sub(at); }
  else {
    let end = 0;
    for (let at = s.indexOf(find); at >= 0; at = s.indexOf(find, at + find.length)) { put(s.slice(end, at)); sub(at); end = at + find.length; }
    put(s.slice(end));
  }
  if (high && bytes + 3 > MAX_STRING) tooLong();
  return out.join("").toWellFormed();
}
/** `toLowerCase(s)`: the browser's whole-string mapping, context rules kept, then the bound checked. */
export const x_toLowerCase = s => { const o = s.toLowerCase().toWellFormed(); if (o.length > MAX_STRING || new TextEncoder().encode(o).length > MAX_STRING) tooLong(); return o; };

