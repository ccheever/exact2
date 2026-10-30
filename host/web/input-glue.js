// Input-only glue: loaded after the baked first pixel, independently of data readiness.
import "./prompt-glue.js"; // a prompt's Return
import "./alert-message.js"; // a grouped alert's message box
/** An element whose `press` the keyboard reaches only through its tabindex:
 * not one the browser activates itself (the wasm host's handler list, or the
 * JS target's `data-exact-on`). */
const pressesByKey = el => !el.matches("button, a[href], input, select, textarea, summary")
  && (el.exactHandlers ?? el.dataset.exactOn?.split(" "))?.includes("press") === true;
const shortcutKeys = new Set(["Enter", "Tab", "Escape", "Backspace", "Delete", "Insert", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"]);
/** A shortcut's key when `e.key` is one character outside ASCII and the Latin script (ㅠ on KeyB under Korean 2-Set,
 * Russian, Greek): what its physical key (`code`) types on a US layout, with Shift, as web apps match (the macOS host
 * asks the Mac's ASCII-capable layout instead; the web has only `code`). A Latin character is the key wherever it is
 * (AZERTY, Dvorak, German's ö), so no chord fires twice; so are Option's characters (the macOS host's rule). */
const usCodes = "Backquote Digit1 Digit2 Digit3 Digit4 Digit5 Digit6 Digit7 Digit8 Digit9 Digit0 Minus Equal BracketLeft BracketRight Backslash Semicolon Quote Comma Period Slash".split(" ");
const physicalKey = e => {
  if (e.altKey || typeof e.key !== "string" || [...e.key].length !== 1 || !/[^\x00-\x7f]/.test(e.key) || /\p{Script=Latin}/u.test(e.key)) return null;
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  const at = usCodes.indexOf(e.code);
  return at < 0 ? null : (e.shiftKey ? "~!@#$%^&*()_+{}|:\"<>?" : "`1234567890-=[]\\;',./")[at];
};
/** The modifiers an event holds, as a chord prefix (a pointer record's last field; glue.js's press writes the same). */
const modifiers = e => (e.shiftKey ? "Shift+" : "") + (e.ctrlKey ? "Control+" : "") + (e.altKey ? "Alt+" : "") + (e.metaKey ? "Meta+" : "");
/** The `PointerEvent` line of `e` at `el`: the point from its content box in its own CSS px (a scale undone), the buttons, pressure, device, id, the viewport point (LLP 1094 D11) and modifiers. */
function pointerLine(el, e, lifted = false) {
  const r = el.getBoundingClientRect(), cs = getComputedStyle(el);
  const sx = el.offsetWidth ? r.width / el.offsetWidth : 1, sy = el.offsetHeight ? r.height / el.offsetHeight : 1;
  const left = parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft), top = parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop);
  const type = e.pointerType === "pen" || e.pointerType === "touch" ? e.pointerType : "mouse";
  return `${(e.clientX - r.left) / (sx || 1) - left},${(e.clientY - r.top) / (sy || 1) - top},${lifted ? 0 : e.buttons},${lifted ? 0 : Math.min(1, Math.max(0, e.pressure || 0))},${type},${e.pointerId ?? 1},${e.clientX},${e.clientY},${modifiers(e)}`;
}
/** @ref LLP 1116.004 D5.2 — a `radiogroup` of `radio`s, a value chosen from a few (the segmented
 * control the shell draws), keyed as WAI-ARIA's radio group and a native one: one Tab stop, its
 * checked radio or else its first, set as the Tab key goes down, before the browser moves the
 * focus; and the arrows move the choice, to the next or previous radio, wrapping and past
 * disabled ones (Left and Right mirrored right to left), focused and pressed. */
const radiosOf = group => Array.from(group.children ?? []).filter(el => el.getAttribute?.("role") === "radio");
const radioUsable = el => !el.disabled && el.getAttribute("aria-disabled") !== "true" && el.getClientRects().length > 0;
const rove = (all, stop) => { for (const r of all) r.tabIndex = r === stop ? 0 : -1; };
function radioStops(root) {
  for (const group of root.querySelectorAll('[role="radiogroup"]')) {
    const all = radiosOf(group);
    rove(all, all.find(r => r.getAttribute("aria-checked") === "true" && radioUsable(r)) ?? all.find(radioUsable));
  }
}
/** An arrow on a radio group's radio moves its choice; whether it did. */
function radioArrow(root, event) {
  const el = event.target, group = el?.parentElement;
  if (el?.getAttribute?.("role") !== "radio" || group?.getAttribute?.("role") !== "radiogroup" || !root.contains(group)) return false;
  const rtl = getComputedStyle(group).direction === "rtl";
  const step = { ArrowDown: 1, ArrowUp: -1, ArrowRight: rtl ? -1 : 1, ArrowLeft: rtl ? 1 : -1 }[event.key];
  if (!step) return false;
  event.preventDefault();
  const all = radiosOf(group), live = all.filter(r => r === el || radioUsable(r));
  const next = live[(live.indexOf(el) + step + live.length) % live.length];
  if (next !== el) { rove(all, next); next.focus(); next.click(); }
  return true;
}
/** @ref LLP 1115.000 D4.5 — a `tablist`'s tabs and a `menu`'s items (a chooser's too, drawn as a menu) move by the
 * arrow keys, as WAI-ARIA's patterns give the roles and a web developer expects of them: Left and Right along a tablist
 * (Up and Down when it is vertical: its `aria-orientation`, or a root tab bar the shell draws as a sidebar), Up and Down
 * in a menu, Home and End to its ends (into the one on top: Up and Down from outside, all four from a menu beneath),
 * wrapping, past disabled ones. The focus moves and nothing is pressed: a tab's press is the app's action (a fetch, in
 * Shelf's Discover), so Enter or Space chooses, as a `<button>` takes them. A tablist is one Tab stop: its selected tab,
 * or the tab the arrows left the focus on. */
const MENUS = '[role="menu"], [data-exact-chooser]', ROVES = `[role="tablist"], ${MENUS}`;
const roversOf = group => [...group.querySelectorAll(group.matches(MENUS) ? 'button, a[href], [role^="menuitem"]' : '[role="tab"]')].filter(el => el.closest(ROVES) === group);
// The open menu on top: the last to open (navigation.js stamps `exactOpened`), which is the top layer's order and not
// the document's (a chooser a menu's row opens stands over the menu, wherever each is written).
const topMenu = root => [...root.querySelectorAll(`:is(${MENUS}):popover-open`)].sort((a, b) => (a.exactOpened ?? 0) - (b.exactOpened ?? 0)).at(-1);
function tabStops(root) {
  for (const list of root.querySelectorAll('[role="tablist"]')) {
    const all = roversOf(list), at = document.activeElement;
    rove(all, all.includes(at) ? at : all.find(t => t.getAttribute("aria-selected") === "true" && radioUsable(t)) ?? all.find(radioUsable));
  }
}
/** An arrow, Home or End among a tablist's tabs or a menu's items moves the focus; whether it did. */
function roveArrow(root, event) {
  const el = event.target, key = event.key, within = el?.closest?.(ROVES);
  if (!el || !(root.contains(el) || el === document.body)) return false;
  const top = /^(Arrow(Up|Down)|Home|End)$/.test(key) && !el?.matches?.("input, textarea, select, [contenteditable]") ? topMenu(root) : null;
  const open = top && !top.contains(el) && (within ? within.matches(MENUS) : /^Arrow/.test(key)) ? top : null; // from a menu beneath it, or by an arrow from outside
  const group = open ?? within;
  if (!group || !root.contains(group)) return false;
  const menu = group.matches(MENUS), all = roversOf(group), live = all.filter(item => item === el || radioUsable(item));
  if (!open && !all.includes(el)) return false;
  const vertical = menu || group.getAttribute("aria-orientation") === "vertical" || group.parentElement?.hasAttribute("data-exact-sidebar"), rtl = getComputedStyle(group).direction === "rtl";
  const step = (vertical ? { ArrowDown: 1, ArrowUp: -1 } : { ArrowRight: rtl ? -1 : 1, ArrowLeft: rtl ? 1 : -1 })[key];
  const next = open ? live.at(key === "ArrowDown" || key === "Home" ? 0 : -1) : step ? live[(live.indexOf(el) + step + live.length) % live.length] : key === "Home" ? live[0] : key === "End" ? live.at(-1) : null;
  if (!next) return false;
  event.preventDefault();
  if (next !== el) { if (!menu) rove(all, next); next.focus(); }
  return true;
}
export function createInputHandlers({ root, views, retiredViews, ready, inertAncestor, editMenu, dispatch, release: dispatchRelease = () => {}, velocity = {}, agentMode = false, log = () => {}, documents = null }) {
  // @ref LLP 1005 §Events — a pointer's down goes along the event's own path,
  // innermost first, to each node's `exactPointerDown` (`pointer` below), from
  // this one listener at the root's capture, before any element's: a
  // recognizer that stops the event (a pan taking its contact) takes nothing
  // from the pointer events, as on the native hosts.
  // Every holder is fixed before any down runs, as the native hosts fix the
  // path: a node a handler further in disables or removes still hears the up.
  root.addEventListener("pointerdown", e => { const path = e.composedPath(); for (const n of path) n.exactPointerDown?.(e, true); for (const n of path) n.exactPointerDown?.(e, false); }, true);
  // @ref LLP 1038 §7 — a plain click on a same-origin link to a declared
  // route stays in this document: a link with its own `press` navigates by
  // it; any other goes to the root's `navigate` handler, as popstate does.
  // With no `navigate` handler it is the runner's own `go` (LLP 1115.000
  // D4.3), as Back is its own `back`: a link to a route never reloads the page.
  // A modified or other-button click, a `target` or `download`, is the
  // browser's alone — a pressing link's press does not also run — and so are
  // other origins, fragments of this page and undeclared paths (a file).
  // The page's own `exact` names the route table and the root's handler.
  document.addEventListener("click", event => {
    const a = event.target.closest?.("a[href]");
    if (!a || !root.contains(a) || event.defaultPrevented || !ready()) return;
    const press = a.exactHandlers?.includes("press");
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
      || (a.target && a.target !== "_self") || a.hasAttribute("download")) { if (press) event.stopPropagation(); return; }
    const url = new URL(a.href), to = url.pathname + url.search, here = to === location.pathname + location.search || to === globalThis.history?.state?.url;
    // (A JS-target page with no GPU surface writes nothing in: its route table is its own.)
    const { wasm, writeIn = () => 0, navigate, go } = globalThis.exact;
    if (url.origin !== location.origin || (here && url.hash) || wasm.exact_route_match(writeIn(to)) !== 1) return;
    const nav = root.firstElementChild, heard = nav?.hasAttribute("navigationBack") && nav.exactHandlers?.includes("navigate");
    event.preventDefault();
    if (press || here) return;
    if (!(heard ? navigate(to) : go(to))?.ops?.some(op => op.op === "router")) wasm.exact_log(writeIn(`history: link ${JSON.stringify(to)} refused`));
  }, true);
  document.addEventListener("keydown", (event) => {
    touched = true; // any key, Tab included: a route's focus shows the ring to a keyboard's user (`arrive`, below)
    if (event.key === "Tab") { radioStops(root); tabStops(root); } // a radio group's one stop, and a tablist's, before the focus moves
    if (event.isComposing || event.keyCode === 229 || event.key === "Process" || !ready() || event.defaultPrevented) return; // an input method's key, WebKit's committing Enter included (rt.js `onKey`)
    if (event.key === "Enter" || event.key === " ") note(event, true); // a key's press begins (`arrive`, below)
    // A commit may restore the authored runs: refresh their Tab stops and press origin above.
    // Shortcuts already heard the source event.
    if (event.exactFlowCapture) return;
    const editing = event.composedPath().some(el => el?.isContentEditable || el?.matches?.("input, textarea, [role=textbox], [role=searchbox], [role=combobox]"));
    const physical = physicalKey(event);
    const matches = (chord) => {
      const parts = chord.split("+");
      let key;
      if (chord === "+") { key = "Plus"; parts.length = 0; }
      else if (parts.length >= 3 && parts.slice(-2).every(p => p === "")) { key = "Plus"; parts.splice(-2); }
      else key = parts.pop();
      if (key === "Plus") key = "+";
      if (key === "Space") key = " ";
      const modifiers = new Set(parts);
      if (editing && key !== "Escape" && !modifiers.has("Meta") && !modifiers.has("Control")) return false;
      return (key?.length === 1 || shortcutKeys.has(key) || /^F([1-9]|[12][0-9]|3[0-5])$/.test(key))
        && [...modifiers].every(m => ["Meta", "Control", "Alt", "Shift"].includes(m))
        && event.metaKey === modifiers.has("Meta") && event.ctrlKey === modifiers.has("Control")
        && event.altKey === modifiers.has("Alt") && event.shiftKey === modifiers.has("Shift")
        && (event.key?.toLowerCase() === key.toLowerCase() || physical === key.toLowerCase());
    };
    // Nothing behind the frontmost modal — a modal `dialog`, or the last
    // shown `aria-modal` view (gallery F22) — and never Enter or Space while
    // the focus is a control they activate (onboarding F27).
    const modal = document.activeElement.closest("dialog:modal") ?? [...root.querySelectorAll('[aria-modal="true"]')].findLast(m => m.getClientRects().length && !inertAncestor(m));
    const focus = document.activeElement, activates = focus?.matches?.("button, a[href], summary, input[type=checkbox], input[type=radio], [data-exact-on~=press]");
    for (const el of root.querySelectorAll("button[aria-keyshortcuts]")) {
      if (modal && !modal.contains(el)) continue;
      if (activates && focus !== el && (event.key === "Enter" || event.key === " ") && !(event.metaKey || event.ctrlKey || event.altKey || event.shiftKey)) continue;
      // A hidden Back control takes its shortcut through the visible bar button, in a header or in content.
      const target = backButton(el) ?? el;
      if (!target.isConnected || !(target.getClientRects().length || target.hasAttribute("data-exact-bar-back")) || inertAncestor(el) || getComputedStyle(target).visibility !== "visible") continue;
      if (!(el.getAttribute("aria-keyshortcuts") ?? "").split(/\s+/).some(matches)) continue;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (!event.repeat && !el.disabled) target.click();
      return;
    }
  }, true);
  // A pressable that is not a button or a link (`tabindex="0"`, written
  // where its handlers are) activates as one, as it does natively: Enter, or
  // Space unless it is a link, after the key's handlers, unless one
  // prevented it — the bubble phase at the document is after them all (chat F14).
  // A radio group's arrows move its choice, held or not, after the key's own handlers too; a tablist's and a menu's
  // move the focus.
  document.addEventListener("keydown", (event) => {
    const el = event.target;
    if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.key === "Process" || event.metaKey || event.ctrlKey || event.altKey || !ready()) return;
    if (radioArrow(root, event) || roveArrow(root, event) || event.repeat) return;
    // @ref LLP 1115.000 D4.14 — Space holds a `<button>` down until its keyup,
    // when the browser clicks it. Its press shows meanwhile, as under a pointer:
    // the unsaid dim, a native button's highlight, a `-exact-press-scale`, a pressed paint.
    if (event.key === " " && el?.matches?.("button:enabled") && root.contains(el)) return keyed(el);
    if (typeof el?.matches !== "function" || !root.contains(el) || !pressesByKey(el)) return;
    if (el.closest(":disabled,[disabled]:not([disabled='false'])")) return; // a disabled box takes no press by key either (LLP 1121 D8)
    if (event.key !== "Enter" && !(event.key === " " && el.getAttribute("role") !== "link")) return;
    event.preventDefault();
    el.click();
    if (event.key === " " && !surface(el)) keyed(el);
  });
  // Hold the default dim for at least 0.1 s after it appears, then fade over 0.2 s (LLP 1115.000 D6;
  // PressFeedback.swift `dimHold`, `dimFade`). Only animate a node that showed the dim; immediate cancellation
  // and the agent's clock skip the fade.
  const DIM = { opacity: "0.65", hold: 100, fade: 200 };
  const dimStarts = new WeakMap(), dimFades = new WeakMap();
  const markPressed = (el, down, now = false) => {
    if (down !== el.hasAttribute("data-pressed")) {
      if (down) {
        dimFades.get(el)?.cancel();
        dimStarts.set(el, performance.now());
      } else if (!now && !agentMode && el.isConnected && getComputedStyle(el).opacity === DIM.opacity) {
        const hold = Math.max(0, (dimStarts.get(el) ?? 0) + DIM.hold - performance.now());
        // No closing keyframe: the fade ends at whatever lies beneath the dim.
        dimFades.set(el, el.animate([{ opacity: DIM.opacity }, { opacity: DIM.opacity, offset: hold / (hold + DIM.fade), easing: "ease-out" }], { duration: hold + DIM.fade }));
      }
    }
    el.toggleAttribute("data-pressed", down);
  };
  // @ref LLP 1115.000 D6 — a pressable covering the window is a surface (a
  // scrim that closes a sheet, a viewer that toggles its bars) and shows no
  // press, as on iOS (PressFeedback.swift `platformPressDims`). CSS cannot
  // measure a box against the window, so each press measures the innermost
  // pressable under it and marks it for the shell's rule, before a frame.
  const surface = at => {
    const el = at.closest?.('button, a[href], [data-exact-on~="press"]');
    if (!el || !root.contains(el)) return false;
    const r = el.getBoundingClientRect(), page = document.documentElement;
    return el.toggleAttribute("data-exact-surface", r.width >= page.clientWidth && r.height >= page.clientHeight * .9);
  };
  // Hold keyboard feedback until Space lifts, focus leaves, or the window or page loses the press
  let keyHold = null;
  const keyed = el => {
    const up = (e, now = false) => {
      if (e?.type === "keyup" && e.key !== " ") return;
      showPress(el, false, now);
      removeEventListener("keyup", up, true); el.removeEventListener("blur", up);
      if (keyHold === up) keyHold = null;
    };
    keyHold?.(null, true);
    showPress(el, true);
    up.el = el;
    keyHold = up;
    addEventListener("keyup", up, true); el.addEventListener("blur", up);
  };
  // @ref LLP 1061 D3 — press feedback by UIKit's rule, not `:active`'s: the
  // innermost node with a `press` handler takes the press (its pressable
  // ancestors, which `:active` would also match, do not), and shows it only
  // while the pointer is inside the box it had when pressed — leaving
  // releases it, coming back presses again; a pan or a cancel ends it. The
  // browser eases a separate factor, multiplied into CSS `scale`, so
  // authored transforms, transitions and keyframes keep their values. A
  // native button is pressed by the same rule with or without a row or a
  // handler of its own, as a UIButton highlights (neutral.css draws the
  // highlight: a wash, or a plain button's fading face).
  const feedback = new WeakMap();
  // @ref LLP 1061 D7 — the author's pressed paint, shown by the same rule
  // and on the same ease as the scale: an important rule of the node's own
  // (`paintRule`) paints its pressed values `--exact-press-paint` of the way
  // (which this piece eases) over its look beneath, read again whenever the
  // node's style or class changes, the scheme flips, or the press turns. It
  // paints on images (a background layer, a mask), which no transition
  // animates, so the node's authored transitions run as authored, beneath
  // the paint. (An earlier form wrote the paint into opacity, background-
  // color and color through custom properties only its animation supplied,
  // which the style a transition is computed from lacks: every press started
  // the node's transitions toward the properties' initial values.) Its
  // `color` reaches the subtree as CSS's does.
  // `:active`'s opacity is the press's absolute target (`--exact-press-opacity-to`,
  // LLP 1121 D8), eased from what shows at the press, a hover look's included.
  const PAINT = ["--exact-press-opacity", "--exact-press-opacity-to", "--exact-press-background-color", "--exact-press-color"];
  const paints = new Map();
  const EASE = { duration: 120, easing: "cubic-bezier(0.16,1,0.3,1)", fill: "both" };
  // Start the ease toward `state.to` from where it shows, over `duration`,
  // on the node's look beneath, read now (`paintRule`): at the press, as it
  // shows; later (the release, a change of the node's style or class, the
  // scheme) with the paint on, its kept declarations in place.
  const paint = (el, state, duration) => {
    const from = Number(getComputedStyle(el).getPropertyValue("--exact-press-paint")) || 0;
    // The paint stays on while it is read again (`paintRule`): taking it off would itself be a style change.
    const rule = paintRule(el, state);
    state.animation?.cancel();
    const animation = state.animation = el.animate([{ "--exact-press-paint": from }, { "--exact-press-paint": state.to }], { ...EASE, duration: agentMode ? 0 : duration });
    const written = new Set(el.getAnimations());
    el.setAttribute("data-exact-painting", hold(el, state, rule));
    // Writing a kept declaration back is the glue's own change, which Firefox (its transition styles see the
    // animation) would time with the node's transition: one it starts is none of the node's.
    if (Object.keys(state.kept ?? {}).length) { getComputedStyle(el).opacity; for (const a of el.getAnimations()) if (!written.has(a) && a.transitionProperty in state.kept) a.cancel(); }
    // A finish already queued when a re-press replaced the ease ends nothing.
    if (state.to === 0) animation.onfinish = () => { if (state.animation === animation) endPaint(el, state); };
    if (agentMode) animation.finish();
  };
  const endPaint = (el, state) => {
    if (paints.get(el) !== state) return;
    paints.delete(el);
    if (!paints.size) { sheets.disconnect(); gone.disconnect(); }
    state.observer.disconnect();
    state.animation.cancel();
    el.removeAttribute("data-exact-painting");
    drop(state);
  };
  // Whether the node declares a transition on `property` (by its name, `all`, or a shorthand holding it): the last
  // entry naming it decides, by its combined duration (duration + delay, which may be negative) above 0.
  const HOLDS = { "background-color": ["background"], "background-position": ["background"], "background-position-x": ["background", "background-position"], "background-position-y": ["background", "background-position"], "background-size": ["background"],
    "mask-position": ["mask"], "mask-position-x": ["mask", "mask-position"], "mask-position-y": ["mask", "mask-position"], "mask-size": ["mask"], color: [], opacity: [] };
  const transitioned = (cs, property) => {
    const list = s => s.split(/\s*,\s*/), names = list(cs.transitionProperty), times = list(cs.transitionDuration), delays = list(cs.transitionDelay);
    let on = false;
    names.forEach((name, i) => { if (name === "all" || name === property || HOLDS[property]?.includes(name)) on = Math.max(0, parseFloat(times[i % times.length]) || 0) + (parseFloat(delays[i % delays.length]) || 0) > 0; });
    return on;
  };
  // An authored transition running (or paused, as the agent's clock pauses it) on `property`.
  const PAINTED_KEY = { opacity: "opacity", color: "color" };
  const running = (el, property) => el.getAnimations().find(a => a.transitionProperty === property && (a.playState === "running" || a.playState === "paused"));
  // A layered list (a background's, a mask's) with the paint's own layer first: each of the node's lists cycled out
  // to its images' count, as CSS matches them, then `first` before it.
  const items = said => { const out = []; let depth = 0, from = 0, quote = ""; for (let i = 0; i < said.length; i++) { const c = said[i]; if (quote) { if (c === "\\") i++; else if (c === quote) quote = ""; } else if (c === '"' || c === "'") quote = c; else if (c === "(") depth++; else if (c === ")") depth--; else if (c === "," && !depth) { out.push(said.slice(from, i).trim()); from = i + 1; } } out.push(said.slice(from).trim()); return out; };
  const layered = (said, count, first) => { const list = items(said); return [first, ...Array.from({ length: count }, (_, i) => list[i % list.length])].join(", "); };
  const LAYER = { background: { position: "0% 0%", size: "auto", repeat: "repeat", origin: "padding-box", clip: "border-box", attachment: "scroll" },
    mask: { position: "0% 0%", size: "auto", repeat: "repeat", origin: "border-box", clip: "no-clip", composite: "intersect", mode: "alpha" } };
  // The paint's layer over a node's own (`kind`: background or mask): its images, then every per-layer list shifted
  // with them. Null where that would move the node's own layers under a transition (a position or size list
  // changing length interpolates): that paint is left out.
  const layer = (cs, kind, mine) => {
    const read = name => cs.getPropertyValue(`${kind}-${name}`), images = read("image"), count = images === "none" ? 0 : items(images).length;
    const out = [`${kind}-image: ${count ? `${mine}, ${images}` : mine} !important`];
    for (const [name, value] of Object.entries(LAYER[kind])) {
      const theirs = read(name);
      // With no images of its own, a node's background clip and origin are its background colour's: the layer
      // takes them (a mask's own no-clip still applies, so a shadow fades with the node).
      if (!count && ((kind === "background" && (name === "clip" || name === "origin")) || items(theirs)[0] === value)) continue;
      if ((name === "position" || name === "size") && [`${kind}-${name}`, ...(name === "position" ? [`${kind}-position-x`, `${kind}-position-y`] : [])].some(n => transitioned(cs, n))) return null;
      out.push(`${kind}-${name}: ${count ? layered(theirs, count, value) : value} !important`);
    }
    return out.join("; ");
  };
  // The node's important paint (LLP 1061 D7), its own pressed values `--exact-press-paint` of the way over its look
  // beneath: the pressed colour as a background-image layer over the node's own background-color (its own images
  // beneath; a colour at alpha p over another, which is CSS's premultiplied mix for an opaque pressed colour),
  // images changing discretely, so starting no transition; the pressed opacity as the property where the engine
  // hands its own number (computedStyleMap; a float, exact) and elsewhere (Firefox) a uniform mask, which
  // multiplies as opacity does (WebKit clips a mask to the border box, a shadow with it, so it takes the number);
  // the pressed text colour as the property. Where the node declares a transition on a property the rule writes,
  // the rule is exactly the look beneath at 0 (else a transition starts and, above every declaration, hides the
  // paint), written only from a look whose numbers round-trip (an opaque rgb(), the engine's opacity), left out
  // while a transition of the property runs (added when it ends), and, once written, kept as it is for the press:
  // re-read with the paint on, so the paint's turn starts nothing (Firefox's transition styles see the animation).
  const paintRule = (el, state) => {
    if (!el.closest("#exact-root")) return "";
    // What is kept holds only while the node's own inputs (its inline style: its opacity and colour, its pressed
    // values; its class) are as they were when it was written, and no sheet, scheme or token changed: else the look
    // beneath is read again (a base change, which the node's transition times, D8).
    const inputs = `${el.getAttribute("style")}|${el.getAttribute("class")}`, was = state.kept ?? {}, before = new Set(el.getAnimations());
    if (state.inputs !== inputs || state.stale) state.kept = {};
    state.inputs = inputs; state.stale = false;
    const kept = state.kept ?? {};
    // Re-read with only the kept declarations on, under the same key: the look beneath of what isn't kept, and the
    // node's own images and masks.
    if (state.rule) hold(el, state, Object.values(kept).join("; "));
    const cs = getComputedStyle(el), own = name => el.style.getPropertyValue(name) ? cs.getPropertyValue(name).trim() : "";
    // A transition the read itself started (Firefox's transition styles see the animation, so taking a kept
    // declaration off is a change there) toward the very look beneath the kept one was written from: none of the
    // node's, cancelled at once. One toward another look is the base's own move, the node's to time (D8).
    for (const a of el.getAnimations()) if (!before.has(a) && a.transitionProperty in was && String(a.effect.getKeyframes().at(-1)?.[PAINTED_KEY[a.transitionProperty]]) === state.keptBase?.[a.transitionProperty]) a.cancel();
    const p = "var(--exact-press-paint)", out = [], next = {};
    const number = (name, copy) => { if (!own(name)) return null; const v = parseFloat(cs.getPropertyValue(copy)); return Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : null; };
    const settle = () => { if (paints.get(el) === state) repaint(el); };
    const later = property => { const a = running(el, property); if (a && !state.waits?.has(a)) { (state.waits ??= new Set()).add(a); a.finished.then(settle, settle); } return !!a; };
    if (own("--exact-press-background-color")) {
      const bg = layer(cs, "background", `linear-gradient(color-mix(in srgb, var(--exact-press-background-color) calc(${p} * 100%), transparent) 0 0)`);
      if (bg) out.push(bg);
      state.background = !!bg;
    }
    const to = number("--exact-press-opacity-to", "--exact-press-opacity-to-number"), share = number("--exact-press-opacity", "--exact-press-opacity-number");
    const held = el.computedStyleMap?.().get("opacity")?.value, base = Number.isFinite(held) ? held : Number(cs.opacity);
    state.factor = null;
    if (kept.opacity) next.opacity = kept.opacity;
    else if ((to != null || share != null) && Number.isFinite(base) && !later("opacity")) {
      if (Number.isFinite(held) || (to != null && to > base && !transitioned(cs, "opacity"))) {
        const d = `opacity: ${to != null ? `calc(${held ?? base} + ${to - base} * ${p})` : `calc(${held ?? base} * (1 - ${p} * ${1 - share}))`} !important`;
        if (transitioned(cs, "opacity")) next.opacity = d; else out.push(d);
      } else if (to == null || (base > 0 && to <= base)) {
        const factor = to != null ? `calc((${base} + ${to - base} * ${p}) / ${base})` : `calc(1 - ${p} * ${1 - share})`;
        const mask = layer(cs, "mask", `linear-gradient(rgb(0 0 0 / ${factor}) 0 0)`);
        if (mask) { out.push(mask); state.factor = paint => to != null ? (base + (to - base) * paint) / base : 1 - paint * (1 - share); }
      }
    }
    if (kept.color) next.color = kept.color;
    else if (own("--exact-press-color") && !later("color")) {
      const m = /^rgba?\((\d+), (\d+), (\d+)\)$/.exec(cs.color), x = /^color\(srgb ([-\d.e]+) ([-\d.e]+) ([-\d.e]+)(?: \/ ([\d.]+))?\)$/.exec(cs.getPropertyValue("--exact-press-color-in-srgb").trim());
      if (m && x) {
        const b = m.slice(1, 4).map(Number), v = x.slice(1, 4).map(Number), a = x[4] == null ? 1 : Number(x[4]);
        // Opaque beneath: premultiplied over it is (b + (v·a − b)·p) / (1 + (a − 1)·p), exactly b at 0 (a pressed
        // colour fully transparent at 1: no colour to divide, held off 0).
        const alpha = `max(1 + ${a - 1} * ${p}, 0.000001)`;
        const c = b.map((bi, i) => a === 1 ? `calc(${bi} + ${v[i] * 255 - bi} * ${p})` : `calc((${bi} + ${v[i] * 255 * a - bi} * ${p}) / ${alpha})`);
        const d = `color: rgb(${c.join(" ")}${a === 1 ? "" : ` / calc(1 + ${a - 1} * ${p})`}) !important`;
        if (transitioned(cs, "color")) next.color = d; else out.push(d);
      } else if (!transitioned(cs, "color")) out.push(`color: color-mix(in srgb, var(--exact-press-color) calc(${p} * 100%), ${cs.color}) !important`);
    }
    state.kept = next;
    // The look each kept declaration was written from: carried with it while it is kept (a re-read with it on sees
    // the painted look, not that one), read now only where it was written now.
    state.keptBase = { opacity: next.opacity ? (kept.opacity ? state.keptBase?.opacity : String(held ?? base)) : undefined, color: next.color ? (kept.color ? state.keptBase?.color : cs.color) : undefined };
    return [...out, ...Object.values(next)].join("; ");
  };
  // What a painting node shows, as the page can't read it from the node's computed style (the paint is on images):
  // its opacity times the mask, its background-color with the pressed colour over it (as pixels, 8-bit; its own
  // images and masks aren't in it), its colour. Without OffscreenCanvas, the background-color beneath.
  // The agent's node detail reports it (scripts/agent-inspect.mjs), and the press tests read it.
  (globalThis.exact ??= {}).pressShown = el => {
    const state = paints.get(el);
    if (!state) return null;
    const cs = getComputedStyle(el), paint = Number(cs.getPropertyValue("--exact-press-paint")) || 0;
    const shown = { paint, opacity: String(+(Number(cs.opacity) * (state.factor?.(paint) ?? 1)).toPrecision(6)), color: cs.color, backgroundColor: cs.backgroundColor };
    if (state.background && typeof OffscreenCanvas === "function") {
      const g = new OffscreenCanvas(1, 1).getContext("2d");
      if (!g) return shown;
      g.fillStyle = cs.backgroundColor; g.fillRect(0, 0, 1, 1);
      g.globalAlpha = Math.min(1, Math.max(0, paint)); g.fillStyle = cs.getPropertyValue("--exact-press-background-in-srgb").trim(); g.fillRect(0, 0, 1, 1);
      const [r, gr, b, a] = g.getImageData(0, 0, 1, 1).data;
      shown.backgroundColor = a === 255 ? `rgb(${r}, ${gr}, ${b})` : `rgba(${r}, ${gr}, ${b}, ${Math.round(a / 255 * 1000) / 1000})`;
    }
    return shown;
  };
  // Each painting node's rule, in one sheet on its root (a document or a shadow root; a frame's own), keyed by
  // the value of its data-exact-painting, an attribute the paint sets anyway, so no observer sees anything new.
  const paintSheets = new WeakMap();
  let paintKeys = 0;
  const sheetOf = el => {
    const root = el.getRootNode?.(), Sheet = (root?.ownerDocument ?? root)?.defaultView?.CSSStyleSheet;
    if (!Sheet || !root || !("adoptedStyleSheets" in root)) return null;
    let sheet = paintSheets.get(root);
    try {
      if (!sheet) paintSheets.set(root, sheet = new Sheet());
      // Adopted again if the page replaced the list since.
      if (!root.adoptedStyleSheets.includes(sheet)) root.adoptedStyleSheets = [...root.adoptedStyleSheets, sheet];
    } catch { return null; }
    return sheet;
  };
  const hold = (el, state, declarations) => {
    drop(state);
    const sheet = declarations && sheetOf(el);
    if (!sheet) return "";
    state.key ??= String(++paintKeys);
    const at = sheet.insertRule(`#exact-root [data-exact-painting="${state.key}"][data-exact-painting] { ${declarations} }`, sheet.cssRules.length);
    state.rule = { sheet, rule: sheet.cssRules[at] };
    return state.key;
  };
  const drop = state => {
    if (!state.rule) return;
    const { sheet, rule } = state.rule, at = [...sheet.cssRules].indexOf(rule);
    state.rule = null;
    if (at >= 0) sheet.deleteRule(at);
  };
  // The look beneath changed: the ease goes on from where it is, over the rest of its time.
  // `stale`: a sheet, the scheme or a token changed, which no attribute of the node's says.
  const repaint = (el, stale = false) => {
    const state = paints.get(el);
    if (!state) return;
    if (stale) state.stale = true;
    const t = state.animation.effect.getComputedTiming();
    paint(el, state, Math.max(0, (t.endTime ?? 0) - (t.localTime ?? 0)));
  };
  globalThis.matchMedia?.("(prefers-color-scheme: dark)").addEventListener?.("change", () => { for (const el of paints.keys()) repaint(el, true); });
  // So does a live edit that reaches it through a sheet (a state look's rule, the theme's) or the root's
  // modes and tokens (the JS target's), which change no style of the node's own (LLP 1121 D8).
  const sheets = new MutationObserver(() => { for (const el of paints.keys()) repaint(el, true); });
  // A painting node gone from the page (removed, re-rendered) ends its paint and lets go of its rule at once.
  const gone = new MutationObserver(() => { for (const [el, state] of paints) if (!el.isConnected) endPaint(el, state); });
  // The JS target writes its tokens and tuples through the CSSOM, which no observer sees: it calls this (modes.js).
  (globalThis.exact ??= {}).repaintPress = () => { for (const el of paints.keys()) repaint(el, true); };
  // `now`: the press ends at once (the window or the page went), as it does
  // for a node no longer in the page.
  const showPaint = (el, down, now = false) => {
    let state = paints.get(el);
    if ((state?.to ?? 0) === (down ? 1 : 0)) return;
    if (!state && !PAINT.some(p => el.style.getPropertyValue(p))) return;
    if (!state) {
      state = { observer: new MutationObserver(() => repaint(el)) };
      state.observer.observe(el, { attributes: true, attributeFilter: ["style", "class"] });
      if (!paints.size) {
        sheets.observe(document.head, { childList: true, subtree: true, characterData: true });
        sheets.observe(document.documentElement, { attributes: true });
      }
      gone.observe(el.ownerDocument, { childList: true, subtree: true }); // a frame's node, its own document
      paints.set(el, state);
    }
    state.to = down ? 1 : 0;
    paint(el, state, EASE.duration);
    if (!down && (now || !el.isConnected)) endPaint(el, state);
  };
  // Keep the release style only until its transitions end. Immediate cancellation and the agent's clock skip it.
  const releases = new WeakMap(), RELEASED = new Set(["opacity", "background-color", "--exact-wash"]);
  const showPress = (el, down, now = false) => {
    const native = el.matches("button[data-button-style]");
    const eased = native && !down && !now && !agentMode && el.hasAttribute("data-pressed");
    const running = eased ? new Set(el.getAnimations({ subtree: true })) : null;
    markPressed(el, down, now);
    if (native && down) {
      // Flush the pressed style so a tap released before the next frame still shows its highlight (LLP 1061 D2)
      el.removeAttribute("data-exact-released");
      getComputedStyle(el).opacity;
    } else if (eased) {
      el.setAttribute("data-exact-released", "");
      // Leave an author's running transitions independent of the release marker
      const ease = el.getAnimations({ subtree: true }).filter(a => !running.has(a) && RELEASED.has(a.transitionProperty) && (a.effect.target === el || a.effect.target.parentElement === el));
      const ends = Promise.allSettled(ease.map(a => a.finished));
      releases.set(el, ends);
      ends.then(() => { if (releases.get(el) === ends) el.removeAttribute("data-exact-released"); });
    }
    showPaint(el, down, now);
    // A design token's scale is its property's `var()`: the computed value
    // is the token's (LLP 1121 D7).
    const press = el.style.getPropertyValue("--exact-press");
    const to = down ? Number((press.includes("var(") ? getComputedStyle(el).getPropertyValue("--exact-press") : press) || 1) : 1;
    const old = feedback.get(el);
    if (old?.to === to || !old && to === 1) return;
    const progress = old?.animation.effect.getComputedTiming().progress ?? 0;
    const from = old ? old.from + (old.to - old.from) * progress : 1;
    old?.animation.cancel();
    const animation = el.animate([{ "--exact-press-factor": from }, { "--exact-press-factor": to }], {
      duration: agentMode ? 0 : 120, easing: "cubic-bezier(0.16,1,0.3,1)", fill: "both",
    });
    const state = { animation, from, to };
    feedback.set(el, state);
    animation.onfinish = () => {
      if (to === 1 && feedback.get(el) === state) { animation.cancel(); feedback.delete(el); }
    };
    if (agentMode || now) animation.finish();
  };
  // A re-press may catch the release easing out. Neutralize only our effect
  // for this synchronous measurement, then restore it before a frame. The
  // browser undoes exactly that factor about transform-origin, including
  // SVG's reference box and transformed ancestors; no transform is guessed.
  const unpressedBox = el => {
    const effect = feedback.get(el)?.animation.effect;
    const frames = effect?.getKeyframes();
    effect?.setKeyframes([{ "--exact-press-factor": 1 }]);
    const box = el.getBoundingClientRect();
    if (effect) effect.setKeyframes(frames);
    return box;
  };
  // A press's target as `inside` tests it: its unpressed box's edges.
  const boxOf = el => { const r = unpressedBox(el); return [r.left, r.top, r.right, r.bottom]; };
  let press = null;
  // @ref LLP 1122 D8, LLP 1121 stage 8 E — a held state, shown as the real one: `active` the press of the node the
  // press resolves to (its paint and scale, eased as a real press, at once on the agent's clock), `hover` and
  // `focus-visible` that node's look (`data-exact-held`, which the looks rules take beside `:hover` and
  // `:focus-visible`; it snaps, an authored transition or not), handlers never told. One at a time; a disabled node
  // refuses one; real input on the node, its going or its disabling lets it go (`dropped(reason)`). A real press on
  // the node as it is held ends there: the hold has it (its release unpaints nothing).
  const DISABLED = ":disabled,[disabled]:not([disabled='false'])";
  let held = null;
  // (A leaving view, `data-exiting`, is gone as it starts to leave.)
  const watch = new MutationObserver(() => { if (!held?.el.isConnected || held.el.closest("[data-exiting]")) letGo("gone"); else if (held.el.closest(DISABLED)) letGo("disabled"); });
  // (Held still as a look's edge is, below: `data-exact-snap`, which the press paint's observer doesn't watch.)
  const snap = (el, change) => { el.setAttribute("data-exact-snap", ""); change(); getComputedStyle(el).opacity; el.removeAttribute("data-exact-snap"); };
  // `reason` null: let go unsaid (a dev loop's new generation, whose tree goes and whose hold the server replays).
  const letGo = (reason) => {
    if (!held) return;
    const h = held;
    held = null;
    watch.disconnect();
    if (h.state === "active") showPress(h.el, false); else snap(h.el, () => h.el.removeAttribute("data-exact-held"));
    if (reason) h.dropped?.(reason);
  };
  // (Announced once it is: a hold sent while the glue loads, after paint, waits for it — `exact-hold`.)
  queueMicrotask(() => globalThis.dispatchEvent?.(new Event("exact-hold")));
  (globalThis.exact ??= {}).hold = (el, state, dropped) => {
    if (state != null && !["active", "hover", "focus-visible"].includes(state)) return { error: "holdState is active, hover, focus-visible or null", malformed: true };
    letGo("replaced");
    if (state == null) return { held: null };
    // `active`: the innermost node with a press handler, as a real press's (theme2's 07:48); none there is `gone`.
    const node = state === "active" ? el?.closest?.("[data-exact-on~=press]") : el;
    if (!node?.isConnected || !root.contains(node) || node.closest("[data-exiting]")) return { reason: "gone", error: state === "active" ? "no press there: the node presses nothing, nor does any around it" : "the node is not on the page" };
    if (node.closest(DISABLED)) return { reason: "disabled", error: "a disabled node holds no state" };
    held = { el: node, asked: el, state, dropped };
    watch.observe(root, { subtree: true, childList: true, attributes: true, attributeFilter: ["disabled", "data-exiting"] });
    if (state === "active") {
      if (press?.el === node) press = null;
      if (keyHold?.el === node) { const up = keyHold; keyHold = null; removeEventListener("keyup", up, true); node.removeEventListener("blur", up); }
      showPress(node, true);
    } else snap(node, () => node.setAttribute("data-exact-held", state));
    return { held: state, el: node };
  };
  globalThis.exact.holdQuiet = () => letGo(null);
  // What is held on `el` (or on the press it resolved to, `el` named), for `layout` and inspect (LLP 1122 D8): its
  // state and the rows that state shows, by their
  // wire names: a press's paint rows (`:active` lowered to them), else the state look's rows in the looks sheet
  // (its `[data-exact-held~=…]` rule). The rows' places are the server's to join, from the source map.
  const WIRE = { "--exact-press": "press_scale", "--exact-press-background-color": "press_background_color", "--exact-press-color": "press_color", "--exact-press-opacity": "press_opacity", "--exact-press-opacity-to": "press_opacity_to" };
  // `ask`: the runner's agent (the wasm page): the rows as `layout` says the press's own (its D7 rows, `:active`
  // lowered to them) or its look's, each with the token it read (LLP 1121 D8, stage 9 E). Without one (the JS
  // target) they are the CSS the page holds, a varying token named where modes.js knows it. Either names the node
  // the state is held on (the press it resolved to, when named on a node inside it).
  globalThis.exact.heldOf = (el, ask = null) => {
    if (!held || (held.el !== el && held.asked !== el)) return null;
    el = held.el;
    // (The wasm page numbers its elements `data-view`; the JS target its own way, a link's `data-view` an empty marker.)
    const view = /^\d+$/.test(el.dataset?.view ?? "") ? Number(el.dataset.view) : globalThis.$exactViewId?.(el) ?? null;
    const node = ask && view != null ? ask({ op: "node", id: view, plan: true }) : null;
    if (node && !node.error) {
      const own = held.state === "active" ? Object.fromEntries(Object.entries(node.style ?? {}).filter(([name]) => Object.values(WIRE).includes(name)))
        : node.looks?.[held.state === "hover" ? "hover" : "focus_visible"] ?? {};
      const rows = Object.entries(own).map(([name, row]) => ({ name, value: row.value, ...(row.token ? { token: row.token } : {}) }));
      return { held: { state: held.state, node: view, site: node.site ?? null, instance: node.instance ?? [], rows } };
    }
    const rows = [];
    if (held.state === "active") for (const [prop, name] of Object.entries(WIRE)) { const value = el.style.getPropertyValue(prop).trim(); if (value) rows.push({ name, value, prop }); }
    else for (const sheet of document.styleSheets) {
      let list; try { list = [...sheet.cssRules]; } catch { continue; }
      for (const rule of list.flatMap(r => r instanceof CSSMediaRule ? [...r.cssRules] : [r])) {
        if (!rule.selectorText?.includes(`[data-exact-held~="${held.state}"]`) && !rule.selectorText?.includes(`[data-exact-held~=${held.state}]`)) continue;
        if (!el.matches(rule.selectorText)) continue;
        // A custom property is the look's own (`--exact-hover-wash`), not a row.
        for (const prop of rule.style) if (!prop.startsWith("--")) rows.push({ name: prop.replace(/^border-(top|right|bottom|left)-color$/, "border_color_$1").replace(/-/g, "_"), value: rule.style.getPropertyValue(prop).trim(), prop });
      }
    }
    // A row a token or a role writes (`var(…)`, `light-dark(…)`) says the value it resolves to here, and the token
    // where modes.js names it; a token reference (`var(--x-…)`) it can't name leaves the answer
    // `provenance: "incomplete"` (a literal pair or a host's own property names none to miss).
    let incomplete = false;
    const cs = getComputedStyle(el);
    for (const row of rows) {
      const prop = row.prop;
      delete row.prop;
      if (!/\b(var|light-dark)\(/.test(row.value)) continue;
      row.computed = cs.getPropertyValue(prop).trim();
      const token = globalThis.exact.modes?.token?.(el, prop, row.value);
      if (token) row.token = token;
      // Each token reference on its own: one modes.js can't name leaves the answer incomplete, whatever else it named.
      const refs = new Set([...row.value.matchAll(/var\((--x-[\w-]+)/g)].map(m => m[1]));
      if ([...refs].some(ref => !globalThis.exact.modes?.token?.(el, prop, `var(${ref})`))) incomplete = true;
    }
    const site = el.dataset?.site != null ? Number(el.dataset.site) : null;
    // (Its instance path is the inspect table's, where the page has one: `exact.instanceOf`, inspect-dev.js.)
    const instance = site != null ? globalThis.exact.instanceOf?.(el) ?? null : null;
    const answer = { held: { state: held.state, node: view, site, ...(instance ? { instance } : {}), rows, ...(incomplete ? { provenance: "incomplete" } : {}) } };
    // (The element, unsaid in JSON: a host that numbers its views another way names it from this.)
    Object.defineProperty(answer.held, "el", { value: el, enumerable: false });
    return answer;
  };
  // Real input on the held node: a press, a key, the pointer coming over it (a hover's), the focus (a focus's).
  for (const type of ["pointerdown", "keydown"]) root.addEventListener(type, e => { if (held?.el.contains(e.target)) letGo("input"); }, true);
  // The pointer or the focus coming or going: its leaving the node, not moving within it.
  const crosses = (e) => held.el.contains(e.target) !== held.el.contains(e.relatedTarget);
  for (const type of ["pointerover", "pointerout"]) root.addEventListener(type, e => { if (held?.state === "hover" && crosses(e)) letGo("input"); }, true);
  for (const type of ["focusin", "focusout"]) root.addEventListener(type, e => { if (held?.state === "focus-visible" && crosses(e)) letGo("input"); }, true);
  // @ref LLP 1121 D8 — a look snaps, an authored transition or not. The browser has already moved `:hover` and
  // `:focus-visible` when these events come, so an enabled node with a look for the state whose edge this is (the
  // pointer entering or leaving it where it can hover; its `:focus-visible` turning on or off) takes that style change
  // with its transitions off (`data-exact-snap`, an attribute the press paint's observer doesn't watch), one forced
  // style for all of them. A base change still eases as authored; a transition of that node's running across the edge
  // ends there (CSS can't hold one property still: theme2's 19:55 (b)). A look: the JS target's `data-exact-looks`,
  // whose rules name their states, or a view the wasm host's looks sheet has rules for. (Not heard: hover that moves
  // with no event: LLP 1121 §10.)
  if (!document.getElementById("exact-snap")) document.head.append(Object.assign(document.createElement("style"), { id: "exact-snap", textContent: "[data-exact-snap]{transition:none!important}" }));
  // A JS-target look's states, by its attribute's value, from its rules; read again when a sheet comes or goes.
  const lookStates = new Map();
  new MutationObserver(() => lookStates.clear()).observe(document.head, { childList: true, subtree: true, characterData: true });
  const looks = (el, sheet) => {
    if (el.dataset.view != null) { const css = sheet?.exactRules?.get(Number(el.dataset.view)); if (css) return css; }
    const at = el.getAttribute("data-exact-looks");
    if (at == null) return "";
    if (!lookStates.has(at)) {
      let text = "";
      const sel = `[data-exact-looks="${at}"]`, walk = (rules) => { for (const r of rules) { if (r.selectorText?.includes(sel)) text += r.selectorText; if (r.cssRules) walk(r.cssRules); } };
      for (const s of document.styleSheets) { try { walk(s.cssRules); } catch { /* another origin's */ } }
      if (!text) return "";
      lookStates.set(at, text);
    }
    return lookStates.get(at);
  };
  // Held still before any style is read: a read under the authored transition would start the one it holds off. (A
  // node with no transition of its own takes this as nothing.)
  const stillThrough = (els) => {
    if (!els.length) return;
    for (const el of els) el.setAttribute("data-exact-snap", "");
    getComputedStyle(els[0]).opacity;
    for (const el of els) el.removeAttribute("data-exact-snap");
  };
  // In the page: under `root`, through any shadow root's host.
  const inPage = (el) => { while (el && !root.contains(el)) el = el.getRootNode?.().host; return !!el; };
  // The pointer last used, as `data-exact-input` on the page, which every hover look reads (neutral.css, looks.rs): a
  // finger's tap leaves the browser's `:hover` on what it touched, which no platform shows. Writing the mark restyles
  // the whole page, so a finger's is written as it lifts, before the `:hover` its tap leaves, and not as it lands,
  // where the restyle would hold up the scroll the finger may start. The page starts with its primary pointer's.
  const canHover = matchMedia("(any-hover: hover)"), html = document.documentElement;
  html.dataset.exactInput ??= matchMedia("(pointer: coarse)").matches ? "touch" : "mouse";
  const touching = () => html.dataset.exactInput === "touch";
  const used = e => { if (html.dataset.exactInput !== e.pointerType) html.dataset.exactInput = e.pointerType; };
  for (const type of ["pointerover", "pointerdown", "pointermove"]) addEventListener(type, e => { if (e.pointerType !== "touch") used(e); }, true);
  for (const type of ["pointerup", "pointercancel"]) addEventListener(type, e => { if (e.pointerType === "touch") used(e); }, true);
  // The focused node's `:focus-visible` as last seen: a focus look's edge is its flipping. (`:hover` can't be read
  // so: a browser moves it for the style it computes next, so an edge is the pointer crossing a node's border.)
  const focusSeen = new WeakMap();
  // The edges pointer events crossed whose compatibility mouse events are still owed (they come after: a pointer's
  // out and over, then the mouse's); a mouse event alone, Firefox's for a scroll under a still pointer, is its own.
  const pointerEdges = [];
  const elementOf = (n) => (n instanceof Element ? n : n?.parentElement ?? null);
  // @ref LLP 1115.000 D6 — under a mouse or a pen a hover wash shows at once and fades out over 0.15 s as the pointer
  // leaves, as a Mac's hover highlight does. Every engine has dropped `:hover` by `pointerout` (Firefox and WebKit
  // have restyled the node by then, so no transition can start there), so the wash's color (`--exact-wash`,
  // neutral.css) is read as the pointer comes over, and the fade is the node's ground easing from it to nothing: the
  // same pixels on a node with no fill of its own, which is the only one it plays on. These snap instead: an author's
  // hover look (LLP 1121 D8), a grouped row's wash (it lies over the row's separator), a node with a fill of its own,
  // and anything under the agent's clock.
  const washes = new WeakMap(), fadesOut = new WeakMap();
  // `<html>` holds the registered property's initial value, which is no wash.
  let noWash;
  const hovered = e => { const el = e.pointerType !== "touch" && elementOf(e.target)?.closest('button, a[data-view][href], [data-exact-on~="press"]'); return el && root.contains(el) ? el : null; };
  addEventListener("pointerover", e => {
    const el = hovered(e);
    if (!el) return;
    fadesOut.get(el)?.cancel();
    const s = getComputedStyle(el), wash = s.getPropertyValue("--exact-wash");
    noWash ??= getComputedStyle(html).getPropertyValue("--exact-wash");
    const shows = s.backgroundImage !== "none" && s.getPropertyValue("--exact-hover-wash").trim() !== "none" && wash !== noWash;
    washes.set(el, shows && !el.matches('[data-groupedrowseparator], [data-exact-said~="background-color"]') ? wash : null);
  }, true);
  addEventListener("pointerout", e => {
    const el = hovered(e), wash = el && washes.get(el);
    if (!wash || el.contains(elementOf(e.relatedTarget))) return;
    washes.delete(el);
    // Pressed, or kept pressed while its popover is open (menu-rows.js): the press paints over any wash.
    if (agentMode || el.matches("[data-pressed], [data-exact-open]") || !el.isConnected) return;
    const ground = getComputedStyle(el).backgroundColor;
    if (ground !== "rgba(0, 0, 0, 0)") return; // a fill of its own
    fadesOut.set(el, el.animate([{ backgroundColor: wash }, { backgroundColor: ground }], { duration: 150, easing: "ease-out" }));
  }, true);
  // In a menu the pointer and the keyboard move one highlight (neutral.css), as NSMenu's: the row under a mouse or a pen
  // takes the focus, which the arrows move on from, and the pointer leaving the rows gives it back to what held it
  // before (the menu's button, where Escape leaves it too), so the arrows enter the menu again from its ends. What
  // held it is noted as the focus comes into the menu, by the pointer or by the arrows.
  const ROW = 'button, a[href], [role^="menuitem"]';
  let focusBefore = null;
  root.addEventListener("focusin", e => { const menu = e.target.closest?.(MENUS); if (menu && !menu.contains(e.relatedTarget)) focusBefore = e.relatedTarget; }, true);
  root.addEventListener("pointerover", e => {
    const row = e.pointerType !== "touch" && elementOf(e.target)?.closest(ROW), menu = row && row.closest(ROVES);
    if (!menu || menu !== topMenu(root) || !radioUsable(row) || document.activeElement === row) return;
    row.focus({ preventScroll: true, focusVisible: false });
  }, true);
  root.addEventListener("pointerout", e => {
    const row = e.pointerType !== "touch" && elementOf(e.target)?.closest(ROW), menu = row && row.closest(MENUS);
    if (!menu || row !== document.activeElement || elementOf(e.relatedTarget)?.closest(ROW)?.closest(MENUS) === menu) return;
    if (focusBefore?.isConnected && focusBefore !== document.body) focusBefore.focus({ preventScroll: true, focusVisible: false });
    else row.blur();
    focusBefore = null;
  }, true);
  const deepFocus = () => { let el = document.activeElement; while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement; return el; };
  { const el = deepFocus(); if (el instanceof Element) focusSeen.set(el, el.matches(":focus-visible")); }
  const focusLook = (el) => el instanceof Element && inPage(el) && !el.matches(DISABLED) && looks(el, document.getElementById("exact-looks")).includes("focus-visible");
  const lookEdges = (e) => {
    const sheet = document.getElementById("exact-looks"), els = [];
    if (e.type === "focusin" || e.type === "focusout") {
      // `:focus-visible` is the focused node's own (inside a shadow root too). Coming in, it had none.
      const el = e.composedPath()[0];
      if (!(el instanceof Element) || !inPage(el)) return;
      const was = e.type === "focusin" ? false : focusSeen.get(el) ?? false, now = e.type === "focusin" && el.matches(":focus-visible");
      focusSeen.set(el, now);
      if (was !== now && focusLook(el)) els.push(el);
    } else {
      // (A mouse event's target can be a text node: its element's edge.)
      const over = e.type.endsWith("over"), target = elementOf(e.target), related = elementOf(e.relatedTarget);
      if (e.type.startsWith("pointer")) { pointerEdges.push({ over, target, related }); if (pointerEdges.length > 4) pointerEdges.shift(); }
      else {
        // Its pointer event's twin, taken once; any other mouse edge is its own (and no twin is still owed).
        const twin = pointerEdges.findIndex(p => p.over === over && p.target === target && p.related === related);
        if (twin >= 0) { pointerEdges.splice(0, twin + 1); return; }
        pointerEdges.length = 0;
      }
      // A hover look holds only where a pointer can hover and the last one used was no finger (looks.rs), on an
      // enabled node; entering or leaving it, not moving within it.
      if (!canHover.matches || touching()) return;
      for (let el = target; el instanceof Element && root.contains(el); el = el.parentElement) {
        if (el.contains(related)) break;
        if (!el.matches(DISABLED) && looks(el, sheet).includes(":hover")) els.push(el);
      }
    }
    stillThrough(els);
  };
  for (const type of ["pointerover", "pointerout", "mouseover", "mouseout", "focusin", "focusout"]) root.addEventListener(type, lookEdges, true);
  // A key turning `:focus-visible` on for the node already focused (a click, then a key): Blink and WebKit move it
  // before the key comes, others in its default action, so it is looked at now and in the frame's style change (at
  // the window's capture, before a shortcut can take the key).
  const keyTurned = () => {
    const el = deepFocus();
    if (!focusLook(el) || focusSeen.get(el) || !el.matches(":focus-visible")) return false;
    focusSeen.set(el, true);
    stillThrough([el]);
    return true;
  };
  addEventListener("keydown", () => { if (focusLook(deepFocus()) && !keyTurned()) requestAnimationFrame(keyTurned); }, true);
  // Delay touch and pen feedback only where a scroll can move the target. A quick tap flashes on release;
  // a scroll cancels it before it shows (UIKit's `delaysContentTouches`, LLP 1061 D2).
  const TOUCH_DELAY = 150;
  // Only a person's scroll ends a touch's press: one that follows a touch or a pen the browser took (`pointercancel`)
  // or a wheel (a trackpad's momentum is wheels too), in a scroller holding what that input landed on, until its
  // scrolls rest for the delay. Any other scroll is the page moving itself (an app's `scrollTop`, scroll anchoring,
  // `scrollIntoView`), which, like UIKit's `setContentOffset`, cancels no touch: the press goes on, measured where its
  // target now is.
  let person = { until: -Infinity, from: null };
  const personInput = e => { person = { until: performance.now() + TOUCH_DELAY, from: e.target }; };
  // Whether `node` is `el` or inside it, through any shadow root's host.
  const holds = (el, node) => { while (node && node !== el) node = node.parentNode ?? node.host; return !!node; };
  addEventListener("pointercancel", e => { if (e.pointerType === "touch" || e.pointerType === "pen") personInput(e); }, true);
  addEventListener("wheel", personInput, { capture: true, passive: true });
  // It ends with its scroller's `scrollend`, which comes after the glide: what moves that scroller next is the page.
  document.addEventListener("scrollend", e => { if (holds(e.target, person.from)) person.until = -Infinity; }, { capture: true, passive: true });
  const scrolled = new WeakMap();
  const touchScroll = el => {
    const since = performance.now() - TOUCH_DELAY;
    let delay = false, moving = false;
    for (let node = el; node; node = node.parentNode ?? node.host) {
      if (!(node instanceof Element)) {
        continue;
      }
      const style = getComputedStyle(node);
      delay ||= /auto|scroll|overlay/.test(style.overflowY) && node.scrollHeight > node.clientHeight
        || /auto|scroll|overlay/.test(style.overflowX) && node.scrollWidth > node.clientWidth;
      moving ||= scrolled.get(node) > since;
      // Stop at a bar that stays in place while an outer scroller moves
      if (style.position === 'fixed' || style.position === 'sticky') {
        return { delay, moving };
      }
    }
    const page = document.scrollingElement;
    delay ||= page.scrollHeight > page.clientHeight || page.scrollWidth > page.clientWidth;
    moving ||= scrolled.get(document) > since;
    return { delay, moving };
  };
  document.addEventListener("scroll", e => {
    const now = performance.now();
    if (now <= person.until && holds(e.target, person.from)) {
      person.until = now + TOUCH_DELAY;
      scrolled.set(e.target, now);
      if (press?.touch && touchScroll(press.el).moving) return release();
    }
    if (!press || !holds(e.target, press.el)) return;
    press.box = boxOf(press.el);
    if (!press.delay) showHeld(press, stillUnder(press));
  }, { capture: true, passive: true });
  const inside = (p, e) => { const [l, t, r, b] = p.box; return e.clientX >= l && e.clientX <= r && e.clientY >= t && e.clientY <= b; };
  // Whether the finger, where it last was, is over the target as the page last moved it.
  const stillUnder = p => inside(p, { clientX: p.at[0], clientY: p.at[1] });
  // Only mark a mouse-held link without a `press` handler: it dims unless it declares feedback, in which case
  // it shows nothing (LLP 1121's `lower-state-dead` lint). Buttons and touch presses show authored feedback.
  const showHeld = (p, down, now = false) => p.dimOnly ? markPressed(p.el, down, now) : showPress(p.el, down, now);
  // Play the haptic when feedback appears, where `navigator.vibrate` is available (LLP 1077 D14)
  const begin = p => {
    // Discard delayed feedback if the target became unavailable before its timer fired
    if (!p.el.isConnected || p.el.closest(DISABLED) || inertAncestor(p.el)) {
      if (press === p) {
        press = null;
      }
      return false;
    }
    showHeld(p, true);
    if (p.haptic) {
      navigator.vibrate?.(p.haptic);
    }
    return true;
  };
  // Keep a quick tap's feedback through its ease-in without releasing a later press on the same node
  const lifts = new Map();
  const unlift = (el, now = false) => {
    const p = lifts.get(el);
    lifts.delete(el);
    clearTimeout(p.lift);
    if (press?.el !== el || press.delay) showHeld(p, false, now);
  };
  // `lifted`: the pointer came up inside the box, so a tap still waiting out the delay shows its press now.
  const release = (now = false, lifted = false) => {
    const p = press;
    press = null;
    if (!p) return;
    if (!p.delay) return showHeld(p, false, now);
    clearTimeout(p.delay);
    if (!lifted) return;
    if (!begin(p)) {
      if (lifts.has(p.el)) {
        unlift(p.el);
      }
      return;
    }
    clearTimeout(lifts.get(p.el)?.lift);
    p.lift = setTimeout(() => unlift(p.el), EASE.duration);
    lifts.set(p.el, p);
  };
  root.addEventListener("pointerdown", e => {
    if (e.button !== 0 || !e.isPrimary) return;
    release(); // a press whose release never reached the page
    surface(e.target);
    // The innermost pressable takes the press under every pointer; the shell's dim reads this mark, not `:active`
    // (index.html). A field, or a link that is no view's, in a pressable takes it from it.
    const el = e.target.closest?.("[data-exact-on~=press],button,a[href],input,select,textarea");
    if (!el?.matches("[data-exact-on~=press],button,a[data-view][href]") || !root.contains(el) || el.closest(DISABLED)) return; // a box says `disabled` (LLP 1121 D8)
    const touch = e.pointerType === "touch" || e.pointerType === "pen";
    const scroll = touch && !agentMode ? touchScroll(el) : {};
    if (scroll.moving) return;
    const haptic = el.style.getPropertyValue("--exact-press-haptic").trim();
    const p = press = {
      el, id: e.pointerId, box: boxOf(el), at: [e.clientX, e.clientY], touch,
      haptic: haptic && haptic !== "none" ? (haptic === "selection" ? 5 : 12) : 0,
      dimOnly: !touch && el.matches('a:not([data-exact-on~=press])'),
    };
    // The delay over, the press shows if its target is still under the finger: the page may have moved it away.
    if (scroll.delay) p.delay = setTimeout(() => { if (press !== p) return; p.delay = 0; if (stillUnder(p)) begin(p); }, TOUCH_DELAY);
    else begin(p);
  }, true);
  document.addEventListener("pointermove", e => {
    if (e.pointerId !== press?.id) return;
    // The button came up where this document did not hear it (outside the window).
    if (e.buttons === 0) return release();
    press.at = [e.clientX, e.clientY];
    // Inside the delay the browser decides: a scroll cancels the touch, a tap lifts it.
    if (press.delay) return;
    showHeld(press, inside(press, e));
  }, true);
  // A touch belongs to the target it began on, as UIKit hit-tests a touch once: lifted outside that target's box as it
  // stands now (the app may have moved it from under the finger), it presses nothing, though the browser aims its
  // `click` at whatever is under the finger then. Only the click that lift makes is swallowed: a trusted one a pointer
  // made (`detail` above 0, where a key's or assistive technology's activation has 0), within a second, before any
  // other press or key. `exact.strayClick` says so to a listener that may have heard it first (menu-rows.js, which
  // defers a row's action).
  let strayUntil = -Infinity;
  const stray = e => e.isTrusted && e.detail > 0 && performance.now() <= strayUntil;
  (globalThis.exact ??= {}).strayClick = stray;
  addEventListener("click", e => {
    if (!stray(e)) return;
    strayUntil = -Infinity;
    e.preventDefault(); e.stopImmediatePropagation();
  }, true);
  for (const type of ["pointerdown", "keydown"]) addEventListener(type, () => { strayUntil = -Infinity; }, true);
  document.addEventListener("pointerup", e => {
    if (e.pointerId !== press?.id) return;
    press.box = boxOf(press.el);
    const lifted = inside(press, e);
    if (press.touch && !lifted) strayUntil = performance.now() + 1000;
    release(false, lifted);
  }, true);
  document.addEventListener("pointercancel", e => { if (e.pointerId === press?.id) release(); }, true);
  // Leaving the page, the window or the tab may take the release with it, as the pointer dispatch below assumes: the
  // press, a key's too, ends at once. A gesture that claims the contact (motion-glue's swipe) ends the press as it
  // claims it, as a pan cancels a touch.
  const leave = () => { release(true); for (const el of lifts.keys()) unlift(el, true); keyHold?.(null, true); };
  addEventListener("blur", e => { if (e.target === window) leave(); });
  addEventListener("pagehide", leave);
  document.addEventListener("visibilitychange", () => { if (document.hidden) leave(); });
  (globalThis.exact ??= {}).pressClaimed = id => { if (id === press?.id) release(); };
  // @ref LLP 1115.000 D4.2 — a route is a page, so the focus follows it, as a careful web app moves it when it
  // navigates. On a push (or a replace) it goes to the new route's header's heading, else to the route itself; on
  // Back, to the control whose press pushed the route that left (the row, the link, the button that opened a sheet),
  // else as on a push. A focus the commit set is left alone: an `autofocus` field, a `focus()` command, anything
  // now inside the route, or anywhere that is neither the pressed control nor where the focus was as the press began
  // (so a route that arrives with no press never takes the focus from where a person put it), but for one still in a
  // leaving element (`[data-exiting]`: a sheet sliding away or a presence view leaving), which is no one's. Another
  // tab's stack is no arrival (the focus stays on its tab), nor is the first sight of a page. Nothing scrolls, and the
  // ring is the browser's own `:focus-visible`: none after a pointer's press, one after a key's, and none before the
  // person has pressed a key or the pointer, as for a page's first route (`focusVisible` says which, where the engine
  // takes it). The stack is read from the attributes navigation.js projects from.
  let shown = null, pressed = null, touched = false;
  const origins = new WeakMap();
  // The press is noted as it begins (the pointer going down; Enter or Space, in the keys' listener above), before the
  // click any listener navigates by: a link to a route is followed from the document's capture phase (above, and the
  // JS target's router.js), so its commit arrives before a listener here would hear that click. A click with neither
  // before it (a script's, the agent's) is noted as it is heard.
  const note = (e, key) => { if (!root.contains(e.target)) return; touched = true; pressed = { el: e.target?.closest?.("button, a[href], [tabindex]") ?? null, focus: document.activeElement, key, at: performance.now() }; };
  document.addEventListener("pointerdown", e => note(e, false), true);
  document.addEventListener("click", e => { if (!pressed?.el?.contains?.(e.target) || performance.now() - pressed.at > 2000) note(e, e.detail === 0); }, true);
  const lands = el => !!el?.isConnected && !inertAncestor(el) && el.getClientRects().length > 0 && !el.matches(":disabled");
  const titleOf = route => [...route.children].find(c => c.localName === "header")?.querySelector("h1, h2, h3, h4, h5, h6, [role=heading]");
  // @ref LLP 1115.000 D4.5 — a modal route (a sheet) is a dialog to assistive technology: `role="dialog"` and
  // `aria-modal`, named by its header's heading, and the focus moves into it and returns on close as it does for any
  // route (`arrive`). The host's marks (`data-exact-dialog`: "named" when the name is its own too) are not the app's:
  // a role or a label the author wrote is left as written, and the agent's tree leaves the marks out (agent.js).
  const dialog = route => {
    const mark = route.getAttribute("data-exact-dialog"), modal = /^(modal|fullscreen)$/.test(route.getAttribute("navigationPresentation") ?? "") && (mark != null || !route.hasAttribute("role"));
    if (!modal) { if (mark != null) for (const name of ["data-exact-dialog", "role", "aria-modal", ...(mark ? ["aria-labelledby"] : [])]) route.removeAttribute(name); return; }
    const title = titleOf(route), names = !!title && !route.hasAttribute("aria-label") && (!!mark || !route.hasAttribute("aria-labelledby"));
    if (names) { title.id ||= `exact-title-${route.getAttribute("navigationKey")}`; if (route.getAttribute("aria-labelledby") !== title.id) route.setAttribute("aria-labelledby", title.id); }
    else if (mark) route.removeAttribute("aria-labelledby");
    if (mark !== (names ? "named" : "")) { route.setAttribute("data-exact-dialog", names ? "named" : ""); route.setAttribute("role", "dialog"); route.setAttribute("aria-modal", "true"); }
  };
  const arrive = () => {
    const nav = root.querySelector("[navigationBack]"), key = nav?.getAttribute("navigationKey");
    for (const route of nav?.querySelectorAll(":is([navigationPresentation], [data-exact-dialog]):not([data-exiting])") ?? []) dialog(route); // a sheet sliding away is no route (navigation.js `sheetLeaves`)
    const top = key == null ? null : [...nav.querySelectorAll("[navigationKey]")].find(r => r.getAttribute("navigationKey") === key && r.parentElement.closest("[navigationKey]") === nav);
    if (!top) return;
    const routes = [...top.parentElement.children].filter(r => r.hasAttribute("navigationKey") && !r.hasAttribute("data-exiting")), stack = routes.slice(0, routes.indexOf(top) + 1);
    const was = shown, active = document.activeElement, by = pressed && performance.now() - pressed.at < 2000 ? pressed : null;
    shown = stack;
    if (!was || was[0] !== stack[0] || was.at(-1) === top) return;
    const back = was.includes(top);
    if (!back) origins.set(top, was.length === stack.length ? origins.get(was.at(-1)) : by?.el); // a replace keeps where its route came from
    pressed = null;
    if (active && active !== document.body && !active.closest("[data-exiting]") && (top.contains(active) || ![by?.el, by?.focus].includes(active))) return; // a focus a leaving element still holds (in a frame or a shadow root, where its blur does not reach) is no one's
    const origin = back ? origins.get(was[was.indexOf(top) + 1]) : null;
    const to = lands(origin) ? origin : titleOf(top) ?? top;
    if (!to.hasAttribute("tabindex") && to.tabIndex < 0) { to.tabIndex = -1; to.addEventListener("blur", () => to.removeAttribute("tabindex"), { once: true }); }
    const focus = () => to.focus({ preventScroll: true, ...(by ? { focusVisible: by.key } : touched ? {} : { focusVisible: false }) }); focus(); if (document.activeElement?.closest("[data-exiting]")) focus(); // from a frame in a shadow root Firefox lands the focus on the second call
  };
  return {
    arrive, // once the piece is made (the stack as it stands), then after each commit's tree, its focus commands and its `autofocus`
    // @ref LLP 1005 §3 — `pointerdown`/`pointerup`, DOM's own: the primary
    // button or a touch going down on the node, then up or cancelled (a
    // cancel is an up), bubbling to every enabled ancestor that hears either
    // (Charlie, 2026-10-15). The up is heard on the document, so it arrives
    // wherever the pointer lifts; a pointer capture would also retarget the
    // click there, a press the platforms do not make. LLP 1056 §3 stage 3 —
    // `pointermove`: a free pointer over the node (no button down), or the
    // held one anywhere, at most once a frame, the latest. Each carries the
    // `PointerEvent` record (`offsetX,offsetY,buttons,pressure,pointerType,
    // pointerId,` the modifiers held, from the content box): `fire(29, r)` is down, 30 up, 31 a
    // move. The JS target's pointer.js is the same rule.
    pointer(el, on, fire) {
      let held = null, owner = false, last = null, move = null, frame = 0, downOf = null;
      const wants = kind => el.exactHandlers?.includes(kind);
      const record = (e, lifted = false) => pointerLine(el, e, lifted);
      const flush = () => {
        cancelAnimationFrame(frame); frame = 0;
        const m = move; move = null;
        if (m && wants("pointermove") && ready()) fire(31, record(m));
      };
      const moved = e => { last = e; move = e; if (!frame) frame = requestAnimationFrame(flush); };
      // While held: the owner's moves, and every holder's latest point,
      // which a blur's up reports.
      const heldMove = e => { if (e.pointerId !== held) return; last = e; if (owner) moved(e); };
      // The end: an up or cancel anywhere in the document, or the pointer
      // leaving it (out of the window, into a frame) or the window losing
      // focus, which this document never hears the up from.
      const ends = [["pointerup", document], ["pointercancel", document], ["pointerout", document], ["blur", window]];
      const up = e => {
        if (e.type === "blur" ? e.target !== window : e.pointerId !== held || (e.type === "pointerout" && e.relatedTarget && e.relatedTarget.localName !== "iframe")) return;
        held = null; downOf = null;
        for (const [type, target] of ends) target.removeEventListener(type, up, target === document);
        document.removeEventListener("pointermove", heldMove, true);
        flush();
        // Never for a removed node, nor past a handler further in that
        // stopped this up (`exactStopped`, glue.js).
        if (wants("pointerup") && ready() && el.isConnected && !e.exactStopped) fire(30, record(e.type === "blur" ? last : e, true), e);
      };
      const disabled = () => el.matches(":disabled") || el.hasAttribute("disabled") || inertAncestor(el);
      // A free pointer over it: the innermost node hearing moves takes them;
      // one that hears only down or up lets them by to an ancestor that
      // hears them, as pointer.js does.
      on("pointermove", e => {
        if (e.exactPointerMover || !wants("pointermove")) return;
        e.exactPointerMover = el;
        if (held === null && e.buttons === 0 && !disabled()) moved(e);
      });
      // `hold`: the walk's first pass, which fixes the holders; then the down.
      return (e, hold) => {
        // Its down, unless a handler further in removed or disabled it, or
        // stopped this down (`exactStopped`, glue.js): it still holds the up and the moves.
        if (!hold) { if (downOf === e && el.isConnected && !disabled() && wants("pointerdown") && ready() && !e.exactStopped) fire(29, record(e), e); return; }
        // Any button, as the DOM's (studio diary R22); `buttons` says which.
        if (!e.isPrimary || held !== null || !el.isConnected || disabled()) return;
        // The innermost enabled pointer node holds its moves (the walk
        // reaches it first and marks the event); an ancestor holds only its up.
        owner = !e.exactPointerOwner;
        if (!owner && !wants("pointerdown") && !wants("pointerup")) return;
        e.exactPointerOwner = el;
        held = e.pointerId; last = e; downOf = e;
        for (const [type, target] of ends) target.addEventListener(type, up, target === document);
        document.addEventListener("pointermove", heldMove, true);
        flush();
      };
    },
    // Studio diary R22, R3, R19, R17: a `contextmenu` with its point (10),
    // `dblclick` (11), a `wheel` (37, its record; a prevented one does not
    // scroll), files and folders dropped from outside (38, each minted a
    // `doc:` handle, documents-glue.js), a file drag's enter and leave (55, 56, documents-glue.js
    // `hearDrags`) and the window's `beforeunload` (36), as the JS target's
    // rt.js and files.js hear them. `fire(kind, line)` dispatches.
    mouse(el, kind, e, fire) {
      if (!ready()) return;
      if (kind === "beforeunload") return el.isConnected && fire(36, "");
      if (el.matches(":disabled") || inertAncestor(el)) return;
      if (kind === "wheel") return e.exactStopped || fire(37, `${pointerLine(el, e).split(",").slice(0, 2)},${e.deltaX},${e.deltaY},${e.deltaMode},${modifiers(e)}`);
      if (kind === "dragover") { if (e.dataTransfer?.types?.includes("Files")) { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; } return; }
      if (kind === "dragenter" || kind === "dragleave") return fire(kind === "dragenter" ? 55 : 56, `${pointerLine(el, e).split(",").slice(0, 2)},${modifiers(e)}`);
      if (kind === "drop") {
        const dt = e.dataTransfer;
        if (!dt?.types?.includes("Files")) return;
        e.preventDefault(); e.stopPropagation();
        const at = `${pointerLine(el, e).split(",").slice(0, 2)},${modifiers(e)}`, files = [...dt.files], items = [...dt.items].filter(i => i.kind === "file");
        const handles = items.map(i => i.getAsFileSystemHandle?.().catch(() => null) ?? null), entries = items.map(i => i.webkitGetAsEntry?.() ?? null);
        return documents?.().then(async (glue) => {
          const found = await glue.dropped(await Promise.all(handles), files, entries);
          if (found.length) fire(38, `${at}\n${found.join("\n")}`); else log("drop: refused: the browser handed over no file");
        });
      }
      // A field keeps the browser's own double-click, and a text field its edit menu (navigation.js `editMenu`); a control's input has no edit menu, so its node's `contextmenu` runs (#487).
      if (kind === "contextmenu" ? editMenu(e.target) : e.target.closest("input,textarea,[contenteditable]")) return;
      e.preventDefault(); e.stopPropagation();
      fire(kind === "contextmenu" ? 10 : 11, kind === "contextmenu" ? pointerLine(el, e) : "");
    },
    pan(el, id, on) {
      // @ref LLP 1043.000 §3 D8: one coalesced action per display frame.
      // LLP 1057.001 §1: an inner swipe that is still deciding (its pointer
      // 'pending' in exact.contacts) goes first; the pan waits for its verdict,
      // and a pan that began cancels the click (rule 4).
      let contact = null, frame = 0, suppressClick = false;
      const contacts = () => (globalThis.exact ??= {}).contacts ??= new Map();
      const live = () => views.get(id) === el && !retiredViews.has(el) && ready() && !inertAncestor(el) && !el.closest(":disabled,[disabled='true']");
      // Under a nested press the pan captures nothing until it begins, so until
      // then the window hears the contact's moves and its end (capture phase),
      // wherever they happen: a release outside this node ends it here too.
      const watched = { pointermove: e => move(e, true), pointerup: e => up(e, true), pointercancel: e => { if (e.pointerId === contact?.pointer) cancelled(e); } };
      const watch = on => { for (const t in watched) (on ? addEventListener : removeEventListener)(t, watched[t], true); };
      const drop = () => { if (contact?.watching) watch(false); contact = null; };
      const flush = () => {
        cancelAnimationFrame(frame); frame = 0;
        if (!contact || !live()) { drop(); return; }
        const [x,y] = contact.to, [px,py] = contact.from;
        // exact_motion::gesture::SLOP; a pan-only plan links no motion export to ask.
        if (!contact.active && Math.max(Math.abs(x-px),Math.abs(y-py)) <= 4) return;
        if (!contact.active) release(); // a pan ends a press, as it cancels a touch
        // The button's tap is over: the drag is the pan's, captured, so this node hears the rest.
        if (contact.watching) { watch(false); contact.watching = false; el.setPointerCapture(contact.pointer); }
        contact.active = true; contact.from = [x,y];
        if (x !== px || y !== py) dispatch(id, `${x-px},${y-py}`);
      };
      const waiting = e => {
        if (!contact.deferred) return false;
        const state = contacts().get(e.pointerId);
        if (state === "pending") return true;
        if (state === "claimed") { drop(); return true; }
        contact.deferred = false; el.setPointerCapture(e.pointerId); return false;
      };
      // @ref LLP 1057 §10.6 — a pan that began ends with one `panrelease`:
      // the engine's tracker over the contact's samples, at each event's own
      // timestamp (motion-glue's `pan`); a cancelled contact releases at rest.
      const sample = (e, first = false) => velocity.sample?.(id, e.clientX, e.clientY, e.timeStamp, first);
      const released = (payload) => { if (el.exactHandlers?.includes("panrelease") && live()) dispatchRelease(id, payload); };
      const cancel = () => { cancelAnimationFrame(frame); frame=0; const began = contact?.active; drop(); if (began) released("0,0"); };
      // The browser took the contact (pointercancel): it scrolls or zooms by a
      // finger this node does not claim. Silent, it was a drag that died after
      // two moves (files diary F10); the journal says why and what claims it.
      const cancelled = e => {
        if (contact?.pointer === e.pointerId) log(`pan cancelled: the browser took the ${e.pointerType || "pointer"} contact to scroll or zoom; give the dragged node touch-action="none", or "pan-y" or "pan-x" to leave the browser the other axis`);
        cancel();
      };
      // A watched contact is the window's alone (`watched`); this node's listeners take it once captured.
      const move = (e, outside = false) => {
        if (contact?.pointer !== e.pointerId || !!contact.watching !== outside || waiting(e)) return;
        if (e.buttons === 0 && e.pointerType !== "touch") return cancel(); // its button came up where no one heard it
        sample(e); contact.to=[e.clientX,e.clientY]; if (!frame) frame=requestAnimationFrame(flush);
      };
      const up = (e, outside = false) => {
        if (contact?.pointer === e.pointerId && !!contact.watching !== outside) return;
        if (contact?.pointer !== e.pointerId || contact.deferred && contacts().get(e.pointerId) === "claimed") { if (contact?.pointer === e.pointerId) drop(); return; }
        sample(e); contact.to=[e.clientX,e.clientY]; flush(); suppressClick = !!contact?.active;
        const began = contact?.active; drop();
        if (began) { const [vx, vy] = velocity.velocity?.(id, e.timeStamp) ?? [0, 0]; released(`${vx},${vy}`); }
      };
      on("pointermove", e => move(e));
      on("pointerup", e => up(e));
      on("pointercancel", cancelled);
      // A child's implicit touch capture, lost when a deferred pan takes it, bubbles here.
      on("lostpointercapture", e => { if (e.target === el) cancel(); });
      el.addEventListener("click", e => { if (suppressClick) { suppressClick = false; e.preventDefault(); e.stopImmediatePropagation(); } }, true);
      el.addEventListener("dragstart", e => { if (contact?.nested) e.preventDefault(); }); // a link's own drag is not the pan's
      return e => {
        // Only the click right after a pan is suppressed; a drag makes none.
        suppressClick = false;
        if (!live() || !e.isPrimary || e.button !== 0 || contact || e.exactPan || el.matches("input,textarea,[contenteditable]")) return;
        // A control or editor between the contact and this node keeps it (rule 3).
        const inner = e.target.closest("input,textarea,select,[contenteditable]");
        if (inner && inner !== el && el.contains(inner)) return;
        // A press handler between keeps it only within the slop, as a draggable
        // element hears a drag that starts on a button inside it (kanban F6):
        // nothing is captured or prevented until the pan begins, so a tap stays the button's.
        const press = e.target.closest("button,a[href],[data-exact-on~='press']"), nested = !!press && press !== el && el.contains(press);
        const deferred = contacts().get(e.pointerId) === "pending";
        e.exactPan = true; // the innermost pan takes the contact (rule 3)
        if (!nested) { e.preventDefault(); e.stopPropagation(); if (!deferred) el.setPointerCapture(e.pointerId); }
        contact = {pointer:e.pointerId,from:[e.clientX,e.clientY],to:[e.clientX,e.clientY],active:false,deferred,nested,watching:nested};
        if (nested) watch(true);
        velocity.sample?.(id, e.clientX, e.clientY, e.timeStamp, true);
      };
    },
  };
}

// The visible Back that stands for a hidden route control. The hiding mark belongs only to bar items;
// author-hidden controls keep their styles. Input uses only the selected route; geometry can read a covered route.
export function backButton(el, selectedOnly = true) {
  if (!el?.id || el.checkVisibility?.({ visibilityProperty: true }) !== false) return null;
  const route = el.closest("[navigationkey]"), nav = route?.parentElement?.closest("[navigationback]");
  if (!nav || nav.getAttribute("navigationBack") !== el.id || (selectedOnly && nav.getAttribute("navigationKey") !== route.getAttribute("navigationKey"))
    || route.querySelector(`[id="${CSS.escape(el.id)}"]`) !== el) return null;
  const back = route.querySelector(":scope > header")?.shadowRoot?.querySelector('button[part="back"]:not([hidden])');
  return back?.checkVisibility({ visibilityProperty: true }) ? back : null;
}
if (globalThis.exact) Object.assign(globalThis.exact, { createInputHandlers, backButton });
