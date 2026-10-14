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

// The rows by bit (`modes.rs` `CHANNELS`); an accent writes the property
// buttons read too, as css.rs does.
const CH = ["color", "caret-color", "fill", "stroke", "accent-color", "text-shadow", "-webkit-text-stroke-color"];
let Axes = [], Classes = {}, Texts = [], Vars = {}, Win = [], Pending = false, Adopted = false;
const key = d => Axes.map(([a, values], i) => `${a}=${values[d[i]]}`).join(",");

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

/** Row `j`'s channel at `e`: the text's index from the nearest element that
 * sets the row, -1 when that one read no token or none sets it. */
function channel(e, j) {
  for (; e?.nodeType === 1; e = e.parentElement) {
    const own = e.$xi?.[j];
    if (own !== undefined) return own;
    for (const c of e.classList) { const at = Classes[c]?.[j]; if (at !== undefined) return at; }
  }
  return -1;
}

function apply() {
  Pending = false;
  // A page rendered ahead wrote its classes inline (rt.js `adopt`), a
  // token's default tuple's value: the class's `var()` is the one.
  if (!Adopted) {
    Adopted = true;
    for (const c in Vars) for (const e of document.getElementsByClassName(c)) for (const p of Vars[c]) e.style.removeProperty(p);
  }
  if (Axes.length) set(document.documentElement, key(Win));
  // Document order: a boundary's tuple is known before those inside it.
  for (const b of document.querySelectorAll("[data-x-b]")) {
    const base = b.parentElement?.closest("[data-x-b]")?.$xd ?? Win, pin = b.getAttribute("data-x-pin");
    let d = pin == null ? base : pinned(pin, base);
    if (!d) { if (b.$xbad !== pin) say(`invalid -exact-mode value ${JSON.stringify(pin)}: an axis of the app's design and one of its values, each axis once; unset`); b.$xbad = pin; d = base; }
    else b.$xbad = undefined;
    b.$xd = d;
    if (Axes.length) set(b, key(d));
    const own = Number(b.getAttribute("data-x-b")), was = b.$xs ??= [];
    for (let j = 0; j < CH.length; j++) {
      if (own & 1 << j) continue;
      const at = channel(b.parentElement, j), text = at >= 0 ? Texts[at] : null;
      if (was[j] === text) continue;
      was[j] = text;
      for (const p of j === 4 ? [CH[j], "--exact-accent"] : [CH[j]]) {
        if (text == null) b.style.removeProperty(p); else b.style.setProperty(p, text);
      }
    }
  }
}
const set = (e, k) => { if (e.getAttribute("data-x-modes") !== k) e.setAttribute("data-x-modes", k); };

// A page rendered ahead (render.mjs) has no styles to resolve: the page
// that adopts it gives each boundary its tuple after its first commit.
function schedule() {
  if (typeof document === "undefined" || globalThis.__exactRender || Pending) return;
  Pending = true; inflight.n++;
  queueMicrotask(() => { try { apply(); } finally { inflight.n--; } });
}

/** A bound inherited colour row (`S`, `Sm` or `Sr` as `s`): its value as
 * the binding writes it, and whether it read a token (the theme map says,
 * style.rs `theme_map`), which the boundaries below read. */
export function Sx(s, e, prop, unit, f, j) {
  s(e, prop, unit, () => {
    const x = globalThis.exact ??= {};
    x.$th = 0;
    const v = f(), text = v == null ? null : typeof v === "number" ? v + unit : String(v);
    let at = text == null ? undefined : -1;
    if (text != null && x.$th) { at = Texts.indexOf(text); if (at < 0) at = Texts.push(text) - 1; }
    const xi = e.$xi ??= {};
    if (xi[j] !== at) { xi[j] = at; schedule(); }
    return v;
  });
}

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

/** `axes`: [name, values] as declared, the first value the default;
 * `classes`: each class's rows by bit, the text's index or -1; `texts`;
 * `vars`: each class's properties that name a varying token. */
export function modes(axes, classes, texts, vars) {
  Axes = axes; Classes = classes; Texts = texts; Vars = vars; Win = axes.map(() => 0);
  After.push(schedule);
  PropHooks["data-x-pin"] = (e, v) => { if (v == null) e.removeAttribute("data-x-pin"); else e.setAttribute("data-x-pin", v); schedule(); return true; };
  (globalThis.exact ??= {}).modes = {
    // The window's modes, `{axis: value}`.
    read: () => Object.fromEntries(Axes.map(([a, values], i) => [a, values[Win[i]]])),
    check: pins => refused(pins),
    // The agent's `prefer mode`: every pin checked first, then all set, now.
    set: pins => {
      const why = refused(pins);
      if (why) return why;
      for (const [a, v] of pins) { const i = Axes.findIndex(([n]) => n === a); Win[i] = Axes[i][1].indexOf(v); }
      if (typeof document !== "undefined" && !globalThis.__exactRender) apply();
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

