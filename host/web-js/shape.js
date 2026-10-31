// A value's shape check by the plan's type codes (LLP 1005 §3), for rt.js.
/** Whether `v` conforms to type code `t` (`n N b s u ?T [T {T…}`), from `i`;
 * numbers are finite (`N`: any number), as the runner's checks require. `o` is a value
 * that conformed: a part of `v` that is the same array as its part in `o`
 * conforms as it did, unchecked (the Rust runner's `Conformed` re-checks
 * only the list items that are not the same object, runner/src/conform.rs). */
export function conforms(v, t, i = [0], o) {
  if (o !== undefined && v === o && typeof v === "object" && v !== null) { skip(t, i); return true; }
  const c = t[i[0]++];
  if (c === "n") return typeof v === "number" && isFinite(v);
  if (c === "N") return typeof v === "number"; // any number: a hidden parameter's (emit.rs)
  if (c === "b") return typeof v === "boolean";
  if (c === "e") { const end = t.indexOf(":", i[0]), n = Number(t.slice(i[0], end)); i[0] = end + 1; const values = JSON.parse(t.slice(i[0], i[0] + n)); i[0] += n; return typeof v === "string" && values.includes(v); }
  // A lone surrogate is no string a native host holds (Rust's `String`; js/src/engine.rs refuses one), so none here either (LLP 1146 D6).
  if (c === "s") return typeof v === "string" && v.isWellFormed();
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
/** Whether a string anywhere in `v` holds a lone UTF-16 surrogate. */
export const loneSurrogate = v => typeof v === "string" ? !v.isWellFormed() : Array.isArray(v) && v.some(loneSurrogate);
function skip(t, i) { const c = t[i[0]++]; if (c === "e") { const end = t.indexOf(":", i[0]); i[0] = end + 1 + Number(t.slice(i[0], end)); } else if (c === "?" || c === "[") skip(t, i); else if (c === "{") { while (t[i[0]] !== "}") skip(t, i); i[0]++; } }
/** Contract's `==`, and what the runner compares a resource's arguments
 * with (runner/src/compare.rs `equal`, `Value`'s `PartialEq`): numbers as
 * IEEE compares them (`-0` equals `0`, NaN equals nothing, itself
 * included), lists and records item by item. */
export function equal(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b)) return a === b;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!equal(a[i], b[i])) return false;
  return true;
}
/** Plan value identity, for change detection (an equal value keeps its old
 * object): signed zero told apart, NaN the same as itself, and recursively
 * equal lists. Not Contract's `==`, which is `equal`. */
export function eq(a, b) {
  if (a === b) return a !== 0 || 1 / a === 1 / b;
  if (typeof a === "number" && typeof b === "number") return a !== a && b !== b;
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!eq(a[i], b[i])) return false;
  return true;
}
/** `failure(x)`'s code for what a source let through (LLP 1109 D3), as js/src/prelude.js `failureCode` and
 * runner/src/failure.rs read it: a fetch's rejection by its kind, a coded storage refusal, an answer ts-data.js
 * found outside its shape (`Shaped`), else the source's own error. */
export const Shaped = Symbol("shape");
const FETCH_FAILURE = { Network: "offline", Timeout: "timeout", Refused: "refused" };
export function failureCode(e) {
  if (!e || typeof e !== "object") return "error";
  if (e[Shaped]) return "shape";
  if (e.name === "FetchError") return FETCH_FAILURE[e.kind] ?? "error";
  return e.kind === "Unavailable" && typeof e.code === "string" ? "storage" : "error";
}
/** `failure(x).reason`: a string the thrown value carries as `reason` (the snapback4 driver's refusal code, `SLOT_FULL`),
 * else empty; as js/src/prelude.js `fail` reads it (the author's ruling of 2026-10-12). */
export const failureReason = e => e && typeof e === "object" && typeof e.reason === "string" ? e.reason : "";

/** `atomFaces`' `AtomFace` records as the named JSON the runner's bridge writes (LLP 1045.000 D2), each face's colours
 * read as a bound colour row's are on this target: through the build's colour map `cm`, which takes a theme entry's
 * name and a role and refuses a `-exact-platform-color()` none of the plan's literals is (LLP 1095 D3, D4), then as the
 * browser takes them. A role in the web form the kernel reads as one (`var(--exact-separator, …)`, `roles`) is that
 * role, its fallback unread, unless it holds a platform colour the map refuses, as the runner's bridge refuses it.
 * Any other value is no colour when it holds, anywhere, `currentcolor` or a keyword that defers to the cascade, where
 * nothing cascades into a pill, or one of CSS Values 5's substitution functions (`var()`, `env()`, `attr()`, `if()`,
 * `inherit()`), which can find nothing when the style is computed and then unset the kind's paint (D4); the native
 * hosts read none of these. One that is none is empty, so the kind's paint stands, and the journal says so as the
 * runner does, naming the node's view (`view`, called only then). */
const DEFERS = /(?<![-\w])(initial|inherit|unset|revert|revert-layer|currentcolor)(?![-\w])/i;
const SUBSTITUTES = /(?<![-\w])(var|env|attr|if|inherit)\s*\(/i;
/** `c` as CSS reads its identifiers: newlines folded as the input is (CSS Syntax 3 §3.3), escapes undone (§4.3.7). */
const unescaped = c => c.replace(/\r\n?|\f/g, "\n").replace(/\\(?:([0-9a-f]{1,6})[ \t\n]?|([^\n]))/gi, (_, hex, ch) => {
  if (!hex) return ch;
  const n = parseInt(hex, 16);
  return String.fromCodePoint(n > 0 && n <= 0x10ffff && (n < 0xd800 || n > 0xdfff) ? n : 0xfffd);
});
// The kernel's `str::trim` and `split_whitespace` (Unicode's White_Space) and `to_ascii_lowercase`.
const WS = "\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000";
const EDGES = new RegExp(`^[${WS}]+|[${WS}]+$`, "g"), SPACES = new RegExp(`[${WS}]+`, "g");
const ascii = s => s.replace(/[A-Z]/g, c => String.fromCharCode(c.charCodeAt(0) + 32));
/** The index in `roles` (`[name, alias, a CSS system colour, drawn in the accent, the name a bound colour gives it]`
 * in the kernel's order) of the role `text` names, read as `roles::role` reads one; -1 for none. */
export function roleIndex(text, roles) {
  const t = text.replace(EDGES, ""), lower = ascii(t);
  // The tint as the web writes it, `var(--exact-accent, <role>)`, is that role if it is drawn in the accent.
  if (lower.startsWith("var(--exact-accent,") && lower.endsWith(")")) {
    const i = roleIndex(lower.slice("var(--exact-accent,".length, -1), roles);
    return i >= 0 && roles[i][3] ? i : -1;
  }
  if (lower.replace(SPACES, "") === "var(--exact-accent-text,accentcolortext)") return roleIndex("AccentColorText", roles);
  const v = lower.startsWith("var(--exact-") ? lower.slice("var(--exact-".length) : null;
  const end = v?.search(/[,)]/);
  if (end < 0) return -1;
  const exact = v != null ? v.slice(0, end) : lower.startsWith("-exact-") ? lower.slice("-exact-".length) : null;
  return roles.findIndex(([name, alias, css, accent]) => exact != null
    ? (css ? v != null && accent : true) && ascii(name) === exact
    : (css && ascii(name) === lower) || (alias !== "" && ascii(alias) === lower));
}
export function atomFacesRecord(v, cm = c => c, view = () => 0, roles = []) {
  const none = c => {
    const x = globalThis.exact;
    x?.journal?.push(`t=${x.now?.() ?? 0} view ${view()}: invalid atomFaces colour ${JSON.stringify(c)}; its kind's paint stands`);
    return "";
  };
  const paint = c => {
    if (typeof c !== "string" || !c.trim()) return "";
    let refused = false;
    const refuse = () => { refused = true; return ""; };
    const u = unescaped(c), substitutes = SUBSTITUTES.test(u), role = substitutes ? roleIndex(c, roles) : -1;
    if (role >= 0 && c.includes("-exact-platform-color(")) cm(c, refuse);
    const named = role >= 0 ? roles[role][4] : substitutes || DEFERS.test(u) ? null : c;
    if (named == null || refused) return none(c);
    const m = cm(named, refuse) ?? "";
    return refused || typeof CSS !== "undefined" && !CSS.supports("color", m) ? none(c) : m;
  };
  return v == null ? null : JSON.stringify(v.map(([source, label, icon, title, color, backgroundColor, direction, ariaLabel]) =>
    ({ source, label, icon, title, color: paint(color), backgroundColor: paint(backgroundColor), direction, ariaLabel })));
}
/** The compiler's positional `TextDecorations(value, spans)` as the named JSON the web host reads (LLP 1114.007). */
export function decorationRecord(v) {
  return v == null ? null : JSON.stringify({ value: v[0], spans: v[1].map(([name, start, end]) => ({ name, start, end })) });
}
