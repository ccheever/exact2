// @ref LLP 1133.029 D3, D7 — `orientationlock` on the web: the Screen
// Orientation API's `screen.orientation.lock()`
// (https://w3c.github.io/screen-orientation/), for the session's lock. Both
// web targets load it once an element carries `data-exact-orientationlock`:
// glue.js after each batch and navigation projection, the JS target after
// each commit (host/web-js/orientationlock.js). It runs the runner's half
// (runner/src/orientation.rs) over the DOM, from the same rules: the holders
// are displayed by the wake lock's rule (wakelock-glue.js `displayed`) and
// outside every `dialog` and popover; the topmost sheet that holds wins, the
// nearest holder wins, and the rest intersect. A browser locks only where it
// allows one (in fullscreen, on Android's Chrome and Firefox); a refusal is
// journaled once by its DOMException name, and the lock is asked again on
// entering fullscreen and on the page becoming visible, since the user agent
// lets it go on leaving either.
import { displayed } from "./wakelock-glue.js";

/** The four orientations, as bits in this order (runner `ORIENTATIONS`). */
export const ORIENTATIONS = ["portrait-primary", "portrait-secondary", "landscape-primary", "landscape-secondary"];
const ALL = 0b1111;
/** The orientations a type names; undefined for `natural`, `current` and anything else. */
export const types = word => ({ portrait: 0b0011, landscape: 0b1100, any: ALL })[word] ?? (ORIENTATIONS.includes(word) ? 1 << ORIENTATIONS.indexOf(word) : undefined);
export const WORDS = ["portrait", "landscape", ...ORIENTATIONS, "natural", "any", "current", "none"];

const sheet = route => route.getAttribute("navigationPresentation") === "modal";

/** Each stack `el` lies in, by the element holding its routes, with the tier
 * of the route it lies under: its index, or the page's for the page and the
 * routes beneath it that show (a split's list), as runner `tiers`. */
function tiers(el, root) {
  const out = [];
  for (let n = el; n && n !== root; n = n.parentElement) {
    const holder = n.parentElement;
    if (!n.hasAttribute("navigationKey") || n.hasAttribute("navigationBack") || !holder) continue;
    const stack = [...holder.children].filter(r => r.hasAttribute("navigationKey") && !r.hasAttribute("data-exiting"));
    const nav = holder.closest("[navigationBack]"), key = nav?.getAttribute("navigationKey");
    const selected = stack.findIndex(r => r.getAttribute("navigationKey") === key), index = stack.indexOf(n);
    if (selected < 0 || index < 0) continue;
    let page = selected; while (page > 0 && sheet(stack[page])) page--;
    out.push([holder, Math.max(index, page)]);
  }
  return out;
}

/** The session's lock under D3's three rules: `{by, types, natural, current}`
 * with `by` the counted holders in tree order, or `{by: []}`. */
export function session(root) {
  if (!root) return { by: [] };
  const holders = [...root.querySelectorAll("[data-exact-orientationlock]")].filter(el => {
    const word = el.getAttribute("data-exact-orientationlock");
    return WORDS.includes(word) && word !== "none" && !el.parentElement?.closest("dialog, [popover]") && el.localName !== "dialog" && !el.hasAttribute("popover") && displayed(el, root);
  });
  // Rule 1: in each stack, only the topmost route that has a holder counts.
  const at = holders.map(el => tiers(el, root)), top = new Map();
  for (const [stack, tier] of at.flat()) top.set(stack, Math.max(top.get(stack) ?? -1, tier));
  const kept = holders.filter((_, i) => at[i].every(([stack, tier]) => top.get(stack) === tier));
  // Rule 2: a holder with a holder among its descendants is overridden.
  const by = kept.filter(el => !kept.some(other => other !== el && el.contains(other)));
  const words = by.map(el => el.getAttribute("data-exact-orientationlock"));
  return { by, words, types: words.reduce((set, w) => set & (types(w) ?? ALL), ALL), natural: words.includes("natural"), current: words.includes("current") };
}

let idOf = el => el.id, want = null, captured = 0, asked = null, holding = false, said = new Set(), listening = false, log = () => {}, applied = "\"unavailable\"", conflictBy = null;
const say = line => { if (!said.has(line)) { said.add(line); log(line); } };
const visible = () => document.visibilityState === "visible";
const now = () => {
  const o = screen.orientation;
  return o && ORIENTATIONS.includes(o.type) ? 1 << ORIENTATIONS.indexOf(o.type) : 0;
};
/** The device's natural orientation as a bit: the one at angle 0. */
const natural = () => {
  const o = screen.orientation, portrait = o?.type?.startsWith("portrait");
  return o ? ((o.angle % 180 === 0) === portrait ? 1 : 4) : 0;
};
const named = bits => ORIENTATIONS.filter((_, i) => bits & (1 << i));
const lockType = bits => bits === ALL ? "any" : bits === 0b0011 ? "portrait" : bits === 0b1100 ? "landscape" : named(bits).length === 1 ? named(bits)[0] : null;

function report(type, held, why) {
  applied = JSON.stringify({ asked: type, held, why: why ?? null, orientation: screen.orientation?.type ?? null });
}

function ask() {
  if (!want || !visible()) return;
  const type = asked;
  if (typeof screen.orientation?.lock !== "function") { report(type, false, "screen.orientation.lock is missing"); return say("orientationlock: unavailable: screen.orientation.lock is missing"); }
  screen.orientation.lock(type).then(() => {
    if (asked !== type) return;
    holding = true; report(type, true);
  }, e => {
    if (asked !== type) return;
    holding = false; report(type, false, e?.name ?? String(e));
    if (visible()) say(`orientationlock: unavailable: ${e?.name ?? e}`);
  });
}

function release() {
  if (holding) { holding = false; try { screen.orientation.unlock(); } catch {} }
}

/** Apply the session's lock (`session(root)`); `journal` takes the lines. */
export function hold(s, journal, id) {
  if (journal) log = journal;
  if (id) idOf = id;
  if (!listening) {
    listening = true;
    document.addEventListener("visibilitychange", () => { if (!visible()) holding = false; else ask(); });
    document.addEventListener("fullscreenchange", () => { if (document.fullscreenElement) ask(); else if (want) { holding = false; report(asked, false, "fullscreen ended"); } });
  }
  const by = s.by.map((el, i) => `view ${idOf(el)} "${s.words[i]}"`);
  if (!s.by.length) {
    want = null; captured = 0; conflictBy = null; asked = null; release(); applied = "\"unavailable\""; return;
  }
  if (!s.current) captured = 0;
  else if (!captured) captured = now() || ALL;
  const bits = s.types & (s.natural ? natural() || ALL : ALL) & (s.current ? captured : ALL);
  const key = JSON.stringify([by, bits]);
  if (key === want) return;
  want = key;
  if (bits === 0) {
    conflictBy = s.by.map((el, i) => [idOf(el), s.words[i]]);
    release(); asked = null; report(null, false, "conflict");
    return log(`orientationlock: conflict: ${by.join(", ")}; none held`);
  }
  conflictBy = null;
  // `natural` alone is the API's own type; otherwise the set names one.
  const type = s.natural && s.types === ALL && !s.current ? "natural" : lockType(bits);
  if (type === asked && holding) return;
  asked = type; holding = false; report(type, false, "asked");
  ask();
}

/** `state.orientationlock`, as the runner says it (runner `state`). */
export function state(root, id) {
  if (id) idOf = id;
  const s = session(root), ids = s.by.map(idOf), held = s.by.length > 0 && !conflictBy && s.types !== 0;
  const lock = held ? { types: s.types, orientations: named(s.types), natural: s.natural, current: s.current, by: ids.map((v, i) => [v, s.words[i]]) } : null;
  const conflict = s.by.length && s.types === 0 ? ids.map((v, i) => [v, s.words[i]]) : conflictBy;
  return { held, by: ids, words: s.words ?? [], lock, conflict: conflict ?? null, applied: JSON.parse(applied) };
}

/** A navigation projection may have shown or hidden a holder: once anything
 * has asked for a lock, it follows. */
export function projected(root) {
  if (listening) hold(session(root));
}

(globalThis.exact ??= {}).orientationLockGlue = { session, hold, state, projected };
