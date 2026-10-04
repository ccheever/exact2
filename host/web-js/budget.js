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
