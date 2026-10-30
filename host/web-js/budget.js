// The runner's evaluation bounds on the JS target (LLP 1090; runner/src/vm.rs): an app module imports what it calls from
// here, beside rt.js, which never does. A metered body counts its own list steps in a local `$s` (code.rs); what is here
// measures values as the runner's `Extent` does, and checks the strings the roster builds.
import { Refusal, text, segmentOf, say } from "./rt.js";

/** `MAX_STRING` and `MAX_VALUE_BYTES`, in UTF-8 bytes; `MAX_VALUE_NODES`. */
const MAX = 67108864, NODES = 16777216;
/** A trap: the runner's `Trap` at `pc`, journaled as its `Debug` text (D6). */
export class Trap extends Refusal { constructor(kind, pc) { super(`Trap(${kind} { pc: ${pc} })`); this.kind = kind; this.pc = pc; } }
/** Throw a trap, where an expression stands. */
export function $T(kind, pc) { throw new Trap(kind, pc); }
/** Prop or style binding `f` of plan node `site` on element `e`, whose trap unsets its row `row` and the commit goes on
 * (LLP 1017.003 D3; the runner's `instance/trapped.rs`). One journal line when it starts trapping, however many writes
 * read it: the element keeps which of its rows' bindings trapped last, until one evaluates to a value. */
export const tg = (e, site, row, f) => () => {
  try { const v = f(); e.$tg?.delete(row); return v; }
  catch (x) { if (!(x instanceof Trap)) throw x; const t = (e.$tg ??= new Set); if (!t.has(row)) { t.add(row); say(`site ${site}: ${row} trapped ${x.message}; unset`); } return null; }
};
/** A string's UTF-8 bytes, as Rust's `str::len`; a lone surrogate is U+FFFD's 3 (LLP 1088 D2). */
export function utf8(s, limit = Infinity) {
  let n = s.length;
  for (let i = 0; i < s.length && n <= limit; i++) {
    const c = s.charCodeAt(i);
    if (c < 0x80) continue;
    if (c < 0x800) { n += 1; continue; }
    if (c < 0xdc00 && c >= 0xd800 && i + 1 < s.length) { const d = s.charCodeAt(i + 1); if (d >= 0xdc00 && d < 0xe000) { n += 2; i++; continue; } }
    n += 2;
  }
  return n;
}
/** Whether `s` is past `MAX_STRING` bytes: exact, counted only once 3 bytes a unit could pass it. */
const long = s => s.length > 22369621 && (s.length > MAX || utf8(s) > MAX);
/** `s`, or `StringTooLong` at `pc`. */
const str = (s, pc) => { if (long(s)) throw new Trap("StringTooLong", pc); return s; };

// ---------------------------------------------------------------- extents
// What the runner counts of a value (D2): a string, number, bool or `none` is one value, a record or list one plus its
// parts', a `some` nothing (it is erased here); a shared part counts once per place. Bytes are an upper bound, 3 a UTF-16
// unit, until a sum passes MAX; then the sum is recounted exactly and stays exact (D3), so a trap falls on the
// runner's item. Values are never mutated once compiled code holds them (the router's verbs copy, a data answer is
// the runtime's), so an array's extent is remembered: one of 64 values or more (the runner's REMEMBERED) here, with
// whether its bytes are exact, and the last one `K` built.
const Ext = new WeakMap();
let LK = null, LN = 0, LB = 0, LX = 0;
/** The bytes of the part `$p` last measured. */
export let EB = 0;
function walk(a, x) {
  if (a === LK && (LX || !x)) { EB = LB; return LN; }
  const c = Ext.get(a);
  if (c && (c[2] || !x)) { EB = c[1]; return c[0]; }
  let n = 1, b = 0;
  for (let i = 0; i < a.length && n <= NODES; i++) {
    const v = a[i];
    if (typeof v === "string") { n++; b += x ? utf8(v) : 3 * v.length; }
    else if (Array.isArray(v)) { n += walk(v, x); b += EB; }
    else n++;
  }
  if (n >= 64 && n <= NODES) Ext.set(a, [n, b, x]);
  EB = b;
  return n;
}
/** A part's values; its bytes in `EB`, exact when `x`. */
export function $p(v, x) {
  if (typeof v === "string") { EB = x ? utf8(v) : 3 * v.length; return 1; }
  if (Array.isArray(v)) return walk(v, x);
  EB = 0;
  return 1;
}
/** The exact bytes of `o`'s first `k` parts: a sum's recount once its upper bound passes MAX. */
export function $x(o, k) {
  let b = 0;
  for (let i = 0; i < k; i++) { $p(o[i], 1); b += EB; }
  return b;
}
/** A `map`'s or `filter`'s result `o`, its extent summed as it was built (code.rs `looped`). */
export function $m(o, n, b, x) { if (n >= 64) Ext.set(o, [n, b, x]); return o; }
/** A record or list built at `pc` (`Opcode::Record`, `List`): its parts', or `ValueTooLarge`. */
export function K(a, pc) {
  let n = 1, b = 0, x = 0;
  for (let i = 0; i < a.length; i++) {
    const v = a[i];
    if (typeof v === "string") { n++; b += 3 * v.length; }
    else if (typeof v === "object" && v !== null) { n += walk(v, 0); b += EB; }
    else n++;
  }
  if (b > MAX) { b = $x(a, a.length); x = 1; }
  if (n > NODES || b > MAX) throw new Trap("ValueTooLarge", pc);
  if (n >= 64) Ext.set(a, [n, b, x]);
  LK = a; LN = n; LB = b; LX = x;
  return a;
}

// ---------------------------------------------------------------- lists (LLP 1088 §9.1)
// `concat`, `split`, `sum`, `range`, `flat`, and `slice`, `includes` and `indexOf` over a list, on the caller's budget as `join` is: each takes
// the caller's `$s` and traps where the sum passes the bound, before it builds (`vm.rs`, `list_call`), leaving its own
// steps in `ST`, which the caller adds to its `$s` (code.rs); a list it builds is `K`'s. Over text, `slice`, `includes`
// and `indexOf` take no step.
/** The list steps the last of them took. */
export let ST = 0;
const step = (s, n, pc) => { ST = n; if (s + n > 65536) throw new Trap("IterationLimit", pc); };
/** `concat(xs, ys)`: one step an item. */
export function x_concat(a, b, s, pc) { step(s, a.length + b.length, pc); return K(a.concat(b), pc); }
/** `slice(v, start, end?)`: the compiler writes an omitted `end` as `Number.MAX_VALUE`, which clamps as `undefined`
 * does; text's result is made well formed once (LLP 1088 D2); a list's takes a step an item it keeps. */
export function x_slice(v, a, b, s, pc) {
  if (typeof v === "string") { ST = 0; return v.slice(a, b).toWellFormed(); }
  const at = n => { const i = Math.trunc(n) || 0; return Math.min(v.length, Math.max(0, i < 0 ? v.length + i : i)); };
  step(s, Math.max(0, at(b) - at(a)), pc);
  return K(v.slice(a, b), pc);
}
/** `includes(v, x)`: a substring of text, or a list's item by SameValueZero (NaN is NaN), a step an item scanned. */
export function x_includes(v, x, s, pc) {
  if (typeof v === "string") { ST = 0; return v.includes(x); }
  let i = 0;
  while (i < v.length) {
    step(s, i + 1, pc);
    const item = v[i];
    if (item === x || item !== item && x !== x) break;
    i++;
  }
  if (i === 0 && v.length === 0) step(s, 0, pc);
  return i < v.length;
}
/** `indexOf(v, x)`: text's first match in UTF-16 code units, or a list's item by IsStrictlyEqual (NaN is never found),
 * a step an item scanned; -1 for none. */
export function x_indexOf(v, x, s, pc) {
  if (typeof v === "string") { ST = 0; return v.indexOf(x); }
  for (let i = 0; i < v.length; i++) {
    step(s, i + 1, pc);
    if (v[i] === x) return i;
  }
  step(s, v.length, pc);
  return -1;
}
/** `sortBy(xs, x => key, direction)` once a `Map` computed `keys` (LLP 1116.016 §5): the items by key, equal keys in
 * input order both ways; numbers by `<` with NaN as +0, strings by `<` (UTF-16 code units). No step: the `Map` took one an
 * item. The runner's bottom-up merge sort (`lists::sort_by`), step for step, never `Array.prototype.sort`. */
export function x_sortBy(xs, keys, desc, pc) {
  const n = xs.length, k = new Array(n);
  // The checker gives every key one type, a number or a string.
  for (let i = 0; i < n; i++) k[i] = keys[i] !== keys[i] ? 0 : keys[i];
  // Whether the item at position `b` goes before the one at `a`.
  const before = desc ? (b, a) => k[a] < k[b] : (b, a) => k[b] < k[a];
  let src = new Array(n), dst = new Array(n);
  for (let i = 0; i < n; i++) src[i] = i;
  for (let width = 1; width < n; width *= 2) {
    for (let lo = 0; lo < n; lo += 2 * width) {
      const mid = Math.min(lo + width, n), hi = Math.min(lo + 2 * width, n);
      let i = lo, j = mid, o = lo;
      while (i < mid && j < hi) dst[o++] = before(src[j], src[i]) ? src[j++] : src[i++];
      while (i < mid) dst[o++] = src[i++];
      while (j < hi) dst[o++] = src[j++];
    }
    const t = src; src = dst; dst = t;
  }
  const out = new Array(n);
  for (let i = 0; i < n; i++) out[i] = xs[src[i]];
  return K(out, pc);
}
/** `sum(xs)`: `xs.reduce((a, x) => a + x, 0)`, a step an item, taken before it adds. */
export function x_sum(l, s, pc) {
  step(s, l.length, pc);
  let total = 0;
  for (const x of l) total += x;
  return total;
}
/** `range(n)`: 0 to n - 1, a step an item, taken before it builds; an `n` that is not a whole number at least 0 does not
 * fit, the runner's `TypeMismatch` (format.json `range`). */
export function x_range(n, s, pc) {
  if (!Number.isInteger(n) || n < 0) throw new Trap("TypeMismatch", pc);
  step(s, n, pc);
  return K(Array.from({ length: n }, (_, i) => i), pc);
}
/** `xs.flat()`: `Array.prototype.flat()` at depth 1, a step an outer item and one an item it keeps, taken before it builds. */
export function x_flat(l, s, pc) {
  let n = l.length;
  for (const x of l) n += x.length;
  step(s, n, pc);
  return K(l.flat(), pc);
}
/** `split(t, sep)`: a step a piece; an empty `sep` splits into code units, each made well formed (LLP 1088 D2), and
 * steps before it splits, as the runner counts first. */
export function x_split(t, sep, s, pc) {
  if (sep === "") { step(s, t.length, pc); return K(t.split("").map(u => u.toWellFormed()), pc); }
  let pieces = 1, at = 0;
  step(s, pieces, pc);
  while ((at = t.indexOf(sep, at)) >= 0) { step(s, ++pieces, pc); at += sep.length; }
  return K(t.split(sep), pc);
}

// ---------------------------------------------------------------- strings (`Opcode::Concat`, the roster's builders)
/** `a + b` (`Opcode::Concat`). */
export function cc(a, b, pc) { const n = a.length + b.length; if (n > MAX) throw new Trap("StringTooLong", pc); return 3 * n <= MAX ? a + b : bounded([a, b], "", pc); }
/** Count the UTF-8 result before a builder can allocate past the bound, including pairs across pieces. */
function bounded(parts, sep, pc) {
  const out = parts.map(String);
  let units = sep.length * Math.max(0, out.length - 1);
  for (const part of out) units += part.length;
  // A UTF-16 unit is at least one UTF-8 byte: past the bound by units alone, no piece is scanned.
  if (units > MAX) throw new Trap("StringTooLong", pc);
  if (3 * units <= MAX) return out.join(sep);
  let bytes = 0, high = false;
  const add = part => {
    // Two bytes of slack: a piece that pairs across the boundary takes 2 back below.
    bytes += utf8(part, MAX - bytes + 2);
    if (high && part.length && part.charCodeAt(0) >= 0xdc00 && part.charCodeAt(0) <= 0xdfff) bytes -= 2;
    if (part.length) { const last = part.charCodeAt(part.length - 1); high = last >= 0xd800 && last <= 0xdbff; }
    if (bytes > MAX) throw new Trap("StringTooLong", pc);
  };
  for (let i = 0; i < out.length; i++) {
    if (i) add(sep);
    add(out[i]);
  }
  return out.join(sep);
}
/** `join(list, separator)`: its steps are the caller's (`$s`); one string item is itself, unchecked (`stdlib::join`). */
export function x_join(l, s, pc) { return l.length === 1 && typeof l[0] === "string" ? l[0] : bounded(l, s, pc); }
/** The bytes JSON.stringify emits for one quoted string, including escapes for lone surrogates. */
function jsonBytes(s) {
  let bytes = 2;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c === 34 || c === 92 || c === 8 || c === 9 || c === 10 || c === 12 || c === 13) bytes += 2;
    else if (c < 32) bytes += 6;
    else if (c < 128) bytes++;
    else if (c < 2048) bytes += 2;
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length && s.charCodeAt(i + 1) >= 0xdc00 && s.charCodeAt(i + 1) <= 0xdfff) { bytes += 4; i++; }
    else bytes += c >= 0xd800 && c <= 0xdfff ? 6 : 3;
  }
  return bytes;
}
/** A native module's props (LLP 1024 D1): key/value pairs to one JSON object of strings, a none left out (`stdlib::native_props`).
 * A key that is `[name, schema]` carries its value as JSON by the schema (a package view's record or list prop, LLP 1133.021 §9 item 1). */
export function NP(p, pc) {
  const o = {};
  for (let i = 0; i < p.length; i += 2) if (p[i + 1] != null) {
    if (Array.isArray(p[i])) o[p[i][0]] = JSON.stringify(typed(p[i + 1], JSON.parse(p[i][1])));
    else o[p[i]] = String(p[i + 1]);
  }
  let bytes = 2, first = true;
  for (const [key, value] of Object.entries(o)) {
    bytes += (first ? 0 : 1) + jsonBytes(key) + 1 + jsonBytes(value);
    if (bytes > MAX) throw new Trap("StringTooLong", pc);
    first = false;
  }
  return JSON.stringify(o);
}
/** A value by its schema (`native_props`'s `typed_json`): `0` itself, `o` an option, `l` a list, `r` a record by position, a none field left out. */
function typed(v, s) {
  if (v == null) return null;
  if (s?.o !== undefined) return typed(v, s.o);
  if (s?.l !== undefined) return v.map(x => typed(x, s.l));
  if (s?.r) { const out = {}; s.r.forEach(([f, t], i) => { if (v[i] != null) out[f] = typed(v[i], t); }); return out; }
  return typeof v === "number" && !isFinite(v) ? null : v;
}
/** `t(key, name=value…)` (rt.js `text`). */
export function x_t(name, key, pairs, pc) {
  const out = []; let bytes = 0, high = false;
  text(name, key, pairs, piece => {
    piece = String(piece);
    bytes += utf8(piece);
    if (high && piece.length && piece.charCodeAt(0) >= 0xdc00 && piece.charCodeAt(0) <= 0xdfff) bytes -= 2;
    if (piece.length) { const last = piece.charCodeAt(piece.length - 1); high = last >= 0xd800 && last <= 0xdbff; }
    if (bytes > MAX) throw new Trap("StringTooLong", pc);
    out.push(piece);
  });
  return out.join("");
}
/** The ASCII bytes an encoded URI result will hold. */
function encodedBytes(v, pc, route) {
  let bytes = 0;
  for (let i = 0; i < v.length; i++) {
    const c = v.charCodeAt(i);
    if (c < 128) bytes += c >= 65 && c <= 90 || c >= 97 && c <= 122 || c >= 48 && c <= 57 || "-_.!~*'()".includes(v[i]) ? 1 : 3;
    else if (c < 2048) bytes += 6;
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < v.length && v.charCodeAt(i + 1) >= 0xdc00 && v.charCodeAt(i + 1) <= 0xdfff) { bytes += 12; i++; }
    else if (c >= 0xd800 && c <= 0xdfff && !route) encodeURIComponent(v[i]);
    else bytes += 9;
    if (bytes > MAX) throw new Trap("StringTooLong", pc);
  }
}
export function x_encodeURIComponent(v, pc) { if (9 * v.length > MAX) encodedBytes(v, pc, false); return encodeURIComponent(v); }
/** After the route segment's own refusal of `""`, `.` and `..` (D6). */
export function x_encodeRouteSegment(v, pc) {
  if (v === "" || v === "." || v === "..") return segmentOf(v);
  if (9 * v.length > MAX) encodedBytes(v, pc, true);
  return segmentOf(v);
}
/** `replaceAll(s, find, with)` with a string `find` (LLP 1088 D2), built piece by piece through one counter of the
 * result's well-formed UTF-8 that carries a high surrogate across pieces (a low one completing it is the pair's 4
 * bytes, anything else settles it as U+FFFD's 3), so a quadratic `` $` `` stops at the bound before it is built. */
export function x_replaceAll(s, find, w, pc, max = MAX) {
  let bytes = 0, high = false;
  const out = [], put = piece => {
    for (let i = 0; i < piece.length; i++) {
      const u = piece.charCodeAt(i);
      // A completed pair is checked as it lands, as the runner's `Built::keep` does (runner/src/strings.rs).
      if (high) { high = false; if (u >= 0xdc00 && u <= 0xdfff) { bytes += 4; if (bytes > max) throw new Trap("StringTooLong", pc); continue; } bytes += 3; }
      if (u >= 0xd800 && u <= 0xdbff) high = true;
      else bytes += u < 0x80 ? 1 : u < 0x800 ? 2 : 3; // a lone low half is U+FFFD's 3
      if (bytes > max) throw new Trap("StringTooLong", pc);
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
  if (high && bytes + 3 > max) throw new Trap("StringTooLong", pc);
  return out.join("").toWellFormed();
}
/** `toLowerCase(s)` and `toUpperCase(s)` (LLP 1088 D2): the browser's whole-string mapping, context rules kept, with bytes
 * checked in chunks before building a large result (a final sigma's two forms are both 2 bytes, and uppercasing has no
 * context rule, so the chunks' bytes are the whole's). */
function cased(s, map, pc) {
  if (9 * s.length > MAX) {
    let bytes = 0;
    for (let at = 0; at < s.length;) {
      let end = Math.min(at + 65536, s.length);
      if (end < s.length && s.charCodeAt(end - 1) >= 0xd800 && s.charCodeAt(end - 1) <= 0xdbff && s.charCodeAt(end) >= 0xdc00 && s.charCodeAt(end) <= 0xdfff) end++;
      bytes += utf8(map(s.slice(at, end)).toWellFormed());
      if (bytes > MAX) throw new Trap("StringTooLong", pc);
      at = end;
    }
  }
  return str(map(s).toWellFormed(), pc);
}
export const x_toLowerCase = (s, pc) => cased(s, t => t.toLowerCase(), pc);
export const x_toUpperCase = (s, pc) => cased(s, t => t.toUpperCase(), pc);
