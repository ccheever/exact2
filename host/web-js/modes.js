// The app's modes on the JS target (LLP 1121 D7), imported by an app's
// module only when its plan has app axes, or a theme and a `color-scheme`
// boundary (exact-web-js `modes.rs`). The page's rules declare each varying
// token per full tuple, `[data-x-modes="axis=value,…"]`; this gives the root
// the window's tuple and each boundary (`data-x-b`) its own, its pins
// (`data-x-pin`) over the tuple around it, and writes on the boundary every
// inherited colour row it does not set, from the text of the nearest
// ancestor's row when that row read a token, as the runner's implied rows
// are (`instance/theme.rs`). It runs after each commit, a pin's change and
// `setMode`. Like the runner's, the window's modes do not persist (§9 Q5).
import { After, PropHooks, inflight, say } from "./rt.js";

// The rows by bit (`modes.rs` `CHANNELS`).
const CH = ["color", "caret-color", "fill", "stroke", "accent-color", "text-shadow", "-webkit-text-stroke-color"];
// A value that sets no row: the row is the one around (`StyleValue::text_unsets`).
const UNSETS = /^\s*(unset|inherit)?\s*$/i;
let Axes = [], Classes = {}, Texts = [], Vars = {}, Accent = [], Names = {}, Win = [], Pending = false;
// A development build's design digest (LLP 1121 stage 4): the base a live
// batch applies to. Null in a release build, which takes none.
let Design = null, Structure = null, Rules = null, AccentRules = null;
const key = d => Axes.map(([a, values], i) => `${a}=${values[d[i]]}`).join(",");
const rendering = () => typeof document === "undefined" || !!globalThis.__exactRender;

/** The digits `text`'s pins make over `base` (`bridge::pinned`): `null` for
 * none, or one naming no axis or value, or an axis twice. */
function pinned(text, base) {
  const d = [...base], seen = new Set(), pins = String(text).split(/[ ,]/).filter(Boolean);
  for (const pin of pins) {
    const at = pin.indexOf("="), i = at < 0 ? -1 : Axes.findIndex(([a]) => a === pin.slice(0, at)), v = i < 0 ? -1 : Axes[i][1].indexOf(pin.slice(at + 1));
    if (v < 0 || seen.has(i)) return null;
    seen.add(i); d[i] = v;
  }
  return pins.length ? d : null;
}

/** Row `j` at `e`, as the nearest element that sets it wrote it: its text
 * (`property:value`) when it read a token, null when it read none or none
 * sets it. A binding's is on its element (`Sx`), a class's in `Texts`. */
function channel(e, j) {
  for (; e?.nodeType === 1; e = e.parentElement) {
    const own = e.$xi?.[j];
    if (own !== undefined) return own;
    for (const c of e.classList) { const at = Classes[c]?.[j]; if (at !== undefined) return at >= 0 ? Texts[at] : null; }
  }
  return null;
}

/** The boundaries under `root`, in document order (the render DOM has no
 * selectors). */
function boundaries(root, out = []) {
  if (!rendering()) return root.querySelectorAll("[data-x-b]");
  for (const c of root.childNodes ?? []) if (c.nodeType === 1) { if (c.getAttribute("data-x-b") != null) out.push(c); boundaries(c, out); }
  return out;
}
const around = b => { for (let e = b.parentElement; e; e = e.parentElement) if (e.$xd) return e.$xd; return Win; };

function apply() {
  Pending = false;
  // A page rendered ahead wrote its classes inline (rt.js `adopt`): a baked
  // value gives way to the class's `var()`; one that is a `var()` (a
  // symbol's, emit.rs) stays.
  if (!rendering()) {
    for (const c in Vars) for (const e of document.getElementsByClassName(c)) {
      if (e.$xc) continue;
      e.$xc = true;
      // A boundary whose accent names the accent declares the companion by
      // its tuple (`data-x-accent`), as its class does not.
      for (const p of e.getAttribute("data-x-accent") == null ? Vars[c] : [...Vars[c], "--exact-accent"]) { const v = e.style.getPropertyValue(p); if (v && !v.includes("var(--x-")) e.style.removeProperty(p); }
    }
  }
  // Every token is a property in a development build: its boundaries take
  // the empty tuple too, so one naming the accent resolves there.
  const tuples = Axes.length > 0 || Design != null;
  if (tuples) set(document.documentElement, key(Win));
  // Document order: a boundary's tuple is known before those inside it.
  for (const b of boundaries(document.getElementById("exact-root") ?? document)) {
    const base = around(b), pin = b.getAttribute("data-x-pin");
    let d = pin == null || UNSETS.test(pin) ? base : pinned(pin, base);
    if (!d) { if (b.$xbad !== pin) say(`invalid -exact-mode value ${JSON.stringify(pin)}: an axis of the app's design and one of its values, each axis once; unset`); b.$xbad = pin; d = base; }
    else b.$xbad = undefined;
    b.$xd = d;
    if (tuples) set(b, key(d));
    const own = Number(b.getAttribute("data-x-b")), was = b.$xs ??= [];
    for (let j = 0; j < CH.length; j++) {
      // Its own row stands, from its CSS or a binding that sets it now.
      if (own & 1 << j || b.$xi?.[j] !== undefined) { was[j] = undefined; continue; }
      const text = channel(b.parentElement, j);
      if (was[j] === text) continue;
      write(b, j, was[j], text);
      was[j] = text;
    }
  }
}
const set = (e, k) => { if (e.getAttribute("data-x-modes") !== k) e.setAttribute("data-x-modes", k); };

/** Row `j` written again on `b` as `text` (`property:value`), over `was`. An
 * accent names the property buttons read too, unless it names the accent
 * (it would name itself here, as css.rs leaves it out). */
function write(b, j, was, text) {
  const decl = t => { const at = t.indexOf(":"); return [t.slice(0, at), t.slice(at + 1)]; };
  if (was != null) { const [p] = decl(was); b.style.removeProperty(p); if (j === 4) { b.style.removeProperty("--exact-accent"); b.removeAttribute("data-x-accent"); } }
  if (text == null) return;
  const [p, v] = decl(text);
  b.style.setProperty(p, v);
  if (j === 4) accent(b, v);
}

/** An accent `v` on boundary `b`: its companion, or, when it names a token
 * that names the accent, that token's colour by tuple (`data-x-accent`, the
 * page's rules) and the accent around where the token is the accent. */
function accent(b, v) {
  const named = Accent.find(a => v.includes(`var(${a})`));
  if (named) b.setAttribute("data-x-accent", named);
  else if (!v.includes("--exact-accent")) b.style.setProperty("--exact-accent", v);
}

// A page rendered ahead resolves its boundaries after each commit's writes
// (`After`), as it is built.
function schedule() {
  if (Pending || rendering()) return;
  Pending = true; inflight.n++;
  queueMicrotask(() => { try { apply(); } finally { inflight.n--; } });
}

/** A bound inherited colour row (`S`, `Sm` or `Sr` as `s`): its value as
 * the binding writes it, and on the element what it says of the row, which
 * the boundaries below read: unset (a clear, or a value the property does
 * not take, which the runner refuses), its text when it read a token (the
 * theme map says, style.rs `theme_map`), else null. A clear writes nothing,
 * so on a boundary the row written again stands. A reference (`url(#…)`) is
 * resolved per instance (`Sr`): no boundary writes it again; a stroke's
 * shorthand is written again by its colour alone. */
export function Sx(s, e, prop, unit, f, j) {
  const after = { "-webkit-text-stroke": "-webkit-text-stroke-color" }[prop] ?? prop;
  s(e, prop, unit, () => {
    const x = globalThis.exact ??= {};
    x.$th = 0;
    const v = f(), text = v == null ? null : typeof v === "number" ? v + unit : String(v);
    const clear = text == null || UNSETS.test(text) || j === 0 && /^\s*currentcolor\s*$/i.test(text);
    const refused = !clear && typeof CSS !== "undefined" && CSS.supports && !CSS.supports(prop, text);
    let said = clear || refused ? undefined : null;
    if (said === null && x.$th && !text.includes("url(")) said = `${after}:${after === prop ? text : colourOf(text)}`;
    const xi = e.$xi ??= {};
    // A refused value is written (and journaled, rt.js `css`) as `unset`,
    // and a cleared accent drops its marker (rows.rs), over a row written
    // again: it is written again after.
    const over = said === undefined && e.$xs?.[j] != null;
    if (xi[j] !== said || over) { xi[j] = said; if (over) e.$xs[j] = undefined; schedule(); }
    // Off a boundary a clear is written as itself (a control's own colour
    // gives way to it); on one the row written again stands.
    return clear && e.getAttribute("data-x-b") != null ? null : v;
  });
}
/** A stroke shorthand's colour: what is not its width. */
const colourOf = t => (t.match(/(?:[^\s(]+|\((?:[^()]|\([^()]*\))*\))+/g) ?? []).filter(w => !/^([+-]?[\d.]+[a-z%]*|thin|medium|thick)$/i.test(w)).join(" ") || "currentcolor";

/** Each pin `[axis, value]` checked: an error naming them, or null. */
const refused = pins => {
  const seen = new Set();
  for (const [a, v] of pins) {
    const i = Axes.findIndex(([n]) => n === a), k = i < 0 ? -1 : Axes[i][1].indexOf(v);
    if (k < 0 || seen.has(i)) return `${pins.map(([p, q]) => `\`${p}=${q}\``).join(" ")} names an axis or value the app's design lacks, or an axis twice`;
    seen.add(i);
  }
  return null;
};

// dev-only {: the live table's runtime, which a release build's copy leaves
// out (build.mjs `withoutDev`).
/** The page's rule for each full tuple, by its key, and its accent
 * companions (`[data-x-accent]`, modes.rs `rules`), by key and property. */
function tupleRules() {
  if (Rules) return Rules;
  Rules = new Map(); AccentRules = new Map();
  for (const sheet of document.styleSheets ?? []) {
    let rules;
    try { rules = sheet.cssRules; } catch { continue; }
    for (const r of rules) {
      const m = /\[data-x-modes="([^"]*)"\]/.exec(r.selectorText ?? "");
      if (!m) continue;
      const a = /\[data-x-accent="([^"]*)"\]/.exec(r.selectorText);
      if (a) AccentRules.set(`${m[1]}|${a[1]}`, r); else Rules.set(m[1], r);
    }
  }
  return Rules;
}
/** The theme's own `:root` rule that declares `prop` (roles.rs `theme_css`). */
function themeRule(prop) {
  for (const sheet of document.styleSheets ?? []) {
    let rules;
    try { rules = sheet.cssRules; } catch { continue; }
    for (const r of rules) if (r.selectorText === ":root" && r.style?.getPropertyValue(prop)) return r;
  }
  return null;
}
/** Full tuple `t`'s key (mixed radix over the axes, the first most
 * significant), as modes.rs `key` writes it. */
const tupleKey = t => {
  const pins = [];
  for (let i = Axes.length - 1; i >= 0; i--) { const [a, values] = Axes[i]; pins.unshift(`${a}=${values[t % values.length]}`); t = Math.floor(t / values.length); }
  return pins.join(",");
};
/** A live batch or snapshot (LLP 1121 stage 4): each changed token's
 * property set again in every tuple's rule, in one style update. Stale when
 * its base is not this page's design. */
function design(frame) {
  if (frame.structure !== Structure) return { live: "structure" };
  if (frame.base != null && frame.base !== Design) return { live: "stale" };
  if (frame.design === Design || !frame.tokens?.length) return { live: "applied" };
  const rules = tupleRules();
  // A type token is a property a row it writes (`parts`, modes.rs `design`).
  for (const t of frame.tokens.flatMap(t => t.parts ?? [t])) {
    if (!t.prop || !Array.isArray(t.css)) continue;
    // The tint's override: the theme's `:root` rule, under an authored
    // `accent-color`'s redeclaration (LLP 1121 stage 5).
    if (t.root) { themeRule(t.prop)?.style.setProperty(t.prop, t.css[0]); continue; }
    t.css.forEach((css, i) => {
      rules.get(tupleKey(i))?.style.setProperty(t.prop, css);
      // Its accent companion: the token's colour where it is one, else the
      // accent around (no companion, as the build writes none).
      const companion = AccentRules.get(`${tupleKey(i)}|${t.prop}`)?.style;
      if (companion) { if (css.includes("--exact-accent")) companion.removeProperty("--exact-accent"); else companion.setProperty("--exact-accent", css); }
    });
  }
  Design = (globalThis.exact ??= {}).designDigest = frame.design;
  return { live: "applied" };
}
// dev-only }

/** `axes`: [name, values] as declared, the first value the default;
 * `classes`: each class's rows by bit, the text's index or -1; `texts`,
 * each `property:value`; `vars`: each class's properties that name a
 * varying token; `accent`: the properties of those that name the accent;
 * `names`: each varying token's name by its property; `devDesign` and
 * `devStructure`: a development build's design and structure digests, which
 * install `exact.design`. */
export function modes(axes, classes, texts, vars, accent, names, devDesign, devStructure) {
  Axes = axes; Classes = classes; Texts = texts; Vars = vars; Accent = accent; Names = names ?? {}; Win = axes.map(() => 0);
  // dev-only {
  if (devDesign != null && typeof document !== "undefined") { Design = devDesign; Structure = devStructure; Rules = null; Object.assign(globalThis.exact ??= {}, { design, designDigest: devDesign }); }
  // A dev reload's checkpoint (dev.mjs `capture`) carries the window's modes the design still has.
  const kept = JSON.parse(globalThis.document?.querySelector?.('script[type="application/vnd.exact.dev-checkpoint"]')?.textContent ?? "null")?.modes;
  if (kept) Axes.forEach(([a, values], i) => { const j = values.indexOf(kept[a]); if (j >= 0) Win[i] = j; });
  // dev-only }
  After.push(() => rendering() ? typeof document !== "undefined" && apply() : schedule());
  PropHooks["data-x-pin"] = (e, v) => { if (v == null) e.removeAttribute("data-x-pin"); else e.setAttribute("data-x-pin", v); schedule(); return true; };
  (globalThis.exact ??= {}).modes = {
    // The window's modes, `{axis: value}`.
    read: () => Object.fromEntries(Axes.map(([a, values], i) => [a, values[Win[i]]])),
    check: pins => refused(pins),
    // `layout <id>`'s token beside a row's value (agent.js): the varying
    // tokens `text` names, the tuple the element is in, and whether the
    // boundary wrote the row again. A token folded inline is not one.
    token: (e, prop, text) => {
      const names = [...new Set([...String(text).matchAll(/var\((--x-[\w-]+)\)/g)].map(m => Names[m[1]]).filter(Boolean))];
      if (!names.length) return undefined;
      const j = CH.indexOf(prop === "--exact-accent" ? "accent-color" : prop);
      let at = e;
      while (at && at.getAttribute?.("data-x-modes") == null) at = at.parentElement;
      return { names, text: String(text), modes: at?.getAttribute("data-x-modes") ?? "", implied: j >= 0 && e.$xs?.[j] != null && e.$css?.[prop] == null };
    },
    // The agent's `prefer mode`: every pin checked first, then all set, now.
    set: pins => {
      const why = refused(pins);
      if (why) return why;
      for (const [a, v] of pins) { const i = Axes.findIndex(([n]) => n === a); Win[i] = Axes[i][1].indexOf(v); }
      if (typeof document !== "undefined") apply();
      return null;
    },
    // The app's `setMode(axis, value)` (commands.js), as the runner takes it.
    command: args => {
      const [a, v] = args;
      if (args.length === 2 && typeof a === "string" && typeof v === "string" && !refused([[a, v]])) return void globalThis.exact.modes.set([[a, v]]);
      say(`setMode(${args.map(x => JSON.stringify(x)).join(", ")}) refused: an axis of the app's design and one of its values`);
    },
  };
}
