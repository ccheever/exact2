// The runner's evaluation bounds on the JS target (LLP 1090; runner/src/vm.rs): an app module imports what it calls from
// here, beside rt.js, which never does. A metered body counts its own list steps in a local `$s` (code.rs); what is here
// measures values as the runner's `Extent` does, and checks the strings the roster builds.
import { Refusal, text, segmentOf } from "./rt.js";

/** `MAX_STRING` and `MAX_VALUE_BYTES`, in UTF-8 bytes; `MAX_VALUE_NODES`. */
const MAX = 67108864, NODES = 16777216;
/** A trap: the runner's `Trap` at `pc`, journaled as its `Debug` text (D6). */
export class Trap extends Refusal { constructor(kind, pc) { super(`Trap(${kind} { pc: ${pc} })`); this.kind = kind; this.pc = pc; } }
/** Throw a trap, where an expression stands. */
export function $T(kind, pc) { throw new Trap(kind, pc); }
/** A string's UTF-8 bytes, as Rust's `str::len`; a lone surrogate is U+FFFD's 3 (LLP 1088 D2). */
export function utf8(s) {
  let n = s.length;
  for (let i = 0; i < s.length; i++) {
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
// `concat`, `split`, and `slice`, `includes` and `indexOf` over a list, on the caller's budget as `join` is: each takes
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
  const r = v.slice(a, b);
  step(s, r.length, pc);
  return K(r, pc);
}
/** `includes(v, x)`: a substring of text, or a list's item by SameValueZero (NaN is NaN), a step an item scanned. */
export function x_includes(v, x, s, pc) {
  if (typeof v === "string") { ST = 0; return v.includes(x); }
  let i = 0;
  while (i < v.length && v[i] !== x && (x === x || v[i] === v[i])) i++;
  step(s, i < v.length ? i + 1 : v.length, pc);
  return i < v.length;
}
/** `indexOf(v, x)`: text's first match in UTF-16 code units, or a list's item by IsStrictlyEqual (NaN is never found),
 * a step an item scanned; -1 for none. */
export function x_indexOf(v, x, s, pc) {
  if (typeof v === "string") { ST = 0; return v.indexOf(x); }
  const i = v.indexOf(x);
  step(s, i < 0 ? v.length : i + 1, pc);
  return i;
}
/** `split(t, sep)`: a step a piece; an empty `sep` splits into code units, each made well formed (LLP 1088 D2), and
 * steps before it splits, as the runner counts first. */
export function x_split(t, sep, s, pc) {
  if (sep === "") { step(s, t.length, pc); return K(t.split("").map(u => u.toWellFormed()), pc); }
  const r = t.split(sep);
  step(s, r.length, pc);
  return K(r, pc);
}

// ---------------------------------------------------------------- strings (`Opcode::Concat`, the roster's builders)
/** `a + b` (`Opcode::Concat`). */
export function cc(a, b, pc) { return str(a + b, pc); }
/** `join(list, separator)`: its steps are the caller's (`$s`); one string item is itself, unchecked (`stdlib::join`). */
export function x_join(l, s, pc) { return l.length === 1 && typeof l[0] === "string" ? l[0] : str(l.map(String).join(s), pc); }
/** A native module's props (LLP 1024 D1): key/value pairs to one JSON object of strings, a none left out (`stdlib::native_props`). */
export function NP(p, pc) { const o = {}; for (let i = 0; i < p.length; i += 2) if (p[i + 1] != null) o[p[i]] = String(p[i + 1]); return str(JSON.stringify(o), pc); }
/** `t(key, name=value…)` (rt.js `text`). */
export function x_t(name, key, pairs, pc) { return str(text(name, key, pairs), pc); }
export function x_encodeURIComponent(v, pc) { return str(encodeURIComponent(v), pc); }
/** After the route segment's own refusal of `""`, `.` and `..` (D6). */
export function x_encodeRouteSegment(v, pc) { return str(segmentOf(v), pc); }
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
/** `toLowerCase(s)` (LLP 1088 D2): the browser's whole-string mapping, context rules kept, then the bound checked. */
export function x_toLowerCase(s, pc) { return str(s.toLowerCase().toWellFormed(), pc); }
