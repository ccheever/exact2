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
    if (event.key === "Tab") { radioStops(root); tabStops(root); } // a radio group's one stop, and a tablist's, before the focus moves
    if (event.isComposing || !ready() || event.defaultPrevented) return;
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
      if (!el.isConnected || !el.getClientRects().length || inertAncestor(el) || getComputedStyle(el).visibility !== "visible") continue;
      if (!(el.getAttribute("aria-keyshortcuts") ?? "").split(/\s+/).some(matches)) continue;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (!event.repeat && !el.disabled) el.click();
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
    if (event.defaultPrevented || event.isComposing || event.metaKey || event.ctrlKey || event.altKey || !ready()) return;
    if (radioArrow(root, event) || roveArrow(root, event) || event.repeat) return;
    // @ref LLP 1115.000 D4.14 — Space holds a `<button>` down until its keyup,
    // when the browser clicks it. A press it declares shows meanwhile, as under
    // a pointer: a native button's highlight, a `-exact-press-scale`, a pressed paint. (The
    // unsaid dim needs nothing here: it is the browser's own `:active`.)
    if (event.key === " " && el?.matches?.("button:enabled") && root.contains(el) && (el.hasAttribute("data-button-style") || declaresPress(el))) return keyed(el);
    if (typeof el?.matches !== "function" || !root.contains(el) || !pressesByKey(el)) return;
    if (el.closest(":disabled,[disabled]:not([disabled='false'])")) return; // a disabled box takes no press by key either (LLP 1121 D8)
    if (event.key !== "Enter" && !(event.key === " " && el.getAttribute("role") !== "link")) return;
    event.preventDefault();
    el.click();
    if (event.key === " " && !surface(el)) keyed(el);
  });
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
  // Space held on a pressable this piece presses by key shows its press (its
  // `-exact-press-scale` and pressed paint, else the shell's dim) until the key comes up or the
  // focus leaves, as the browser holds a `<button>` `:active` under Space; so
  // does a `<button>` whose press is declared.
  // The press a key holds, which leaving the window or the page ends too.
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
  // handler of its own, as a UIButton highlights (the shell dims its face).
  const feedback = new WeakMap();
  // @ref LLP 1061 D7 — the author's pressed paint, shown by the same rule
  // and on the same ease as the scale. The shell's important rules
  // (index.html) paint it from the node's own properties, read live, `--exact-
  // press-paint` of the way (which this piece eases) over the node's look
  // beneath, which this piece reads again whenever the node's style or class
  // changes, the scheme flips, or the press turns. Its `color` reaches the
  // subtree as CSS's does.
  // `:active`'s opacity is the press's absolute target (`--exact-press-opacity-to`,
  // LLP 1121 D8), eased from what shows at the press, a hover look's included.
  const PAINT = ["--exact-press-opacity", "--exact-press-opacity-to", "--exact-press-background-color", "--exact-press-color"];
  const declaresPress = el => !!el.style.getPropertyValue("--exact-press") || PAINT.some(p => el.style.getPropertyValue(p));
  const paints = new Map();
  const EASE = { duration: 120, easing: "cubic-bezier(0.16,1,0.3,1)", fill: "both" };
  // Start the ease toward `state.to` from where it shows, over `duration`,
  // on the node's look read now with the paint off: at the press, as it
  // shows; later (the release, a change of the node's style or class, the
  // scheme), a style change like any other, so a transition the node has on
  // a painted property times the paint's move to a look that changed.
  const paint = (el, state, duration) => {
    const from = Number(getComputedStyle(el).getPropertyValue("--exact-press-paint")) || 0;
    el.removeAttribute("data-exact-painting");
    const beneath = getComputedStyle(el);
    const base = { "--exact-press-base-opacity": beneath.opacity, "--exact-press-base-background": beneath.backgroundColor, "--exact-press-base-color": beneath.color };
    state.animation?.cancel();
    const animation = state.animation = el.animate([{ ...base, "--exact-press-paint": from }, { ...base, "--exact-press-paint": state.to }], { ...EASE, duration: agentMode ? 0 : duration });
    el.setAttribute("data-exact-painting", "");
    // A finish already queued when a re-press replaced the ease ends nothing.
    if (state.to === 0) animation.onfinish = () => { if (state.animation === animation) endPaint(el, state); };
    if (agentMode) animation.finish();
  };
  const endPaint = (el, state) => {
    if (paints.get(el) !== state) return;
    paints.delete(el);
    if (!paints.size) sheets.disconnect();
    state.observer.disconnect();
    state.animation.cancel();
    el.removeAttribute("data-exact-painting");
  };
  // The look beneath changed: the ease goes on from where it is, over the rest of its time.
  const repaint = el => {
    const state = paints.get(el);
    if (!state) return;
    const t = state.animation.effect.getComputedTiming();
    paint(el, state, Math.max(0, (t.endTime ?? 0) - (t.localTime ?? 0)));
  };
  globalThis.matchMedia?.("(prefers-color-scheme: dark)").addEventListener?.("change", () => { for (const el of paints.keys()) repaint(el); });
  // So does a live edit that reaches it through a sheet (a state look's rule, the theme's) or the root's
  // modes and tokens (the JS target's), which change no style of the node's own (LLP 1121 D8).
  const sheets = new MutationObserver(() => { for (const el of paints.keys()) repaint(el); });
  // The JS target writes its tokens and tuples through the CSSOM, which no observer sees: it calls this (modes.js).
  (globalThis.exact ??= {}).repaintPress = () => { for (const el of paints.keys()) repaint(el); };
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
      paints.set(el, state);
    }
    state.to = down ? 1 : 0;
    paint(el, state, EASE.duration);
    if (!down && (now || !el.isConnected)) endPaint(el, state);
  };
  const showPress = (el, down, now = false) => {
    el.toggleAttribute("data-pressed", down);
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
  const canHover = matchMedia("(hover: hover)");
  // The focused node's `:focus-visible` as last seen: a focus look's edge is its flipping. (`:hover` can't be read
  // so: a browser moves it for the style it computes next, so an edge is the pointer crossing a node's border.)
  const focusSeen = new WeakMap();
  // The edges pointer events crossed whose compatibility mouse events are still owed (they come after: a pointer's
  // out and over, then the mouse's); a mouse event alone, Firefox's for a scroll under a still pointer, is its own.
  const pointerEdges = [];
  const elementOf = (n) => (n instanceof Element ? n : n?.parentElement ?? null);
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
      // A hover look holds only where the pointer can hover (looks.rs's media query), on an enabled node; entering or
      // leaving it, not moving within it.
      if (!canHover.matches) return;
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
  const release = (now = false) => { if (press) showPress(press.el, false, now); press = null; };
  root.addEventListener("pointerdown", e => {
    if (e.button !== 0 || !e.isPrimary) return;
    release(); // a press whose release never reached the page
    surface(e.target);
    // A finger holds the unsaid dim too (index.html): a touch's `:active` is the engine's choice, which Safari makes
    // only for an element with a touch listener ("Safari Web Content Guide", Handling Events) and Chrome only after a
    // tap gesture a synthesized touch never makes.
    const touch = e.pointerType === "touch";
    const el = e.target.closest?.(touch ? "[data-exact-on~=press],button,a[data-view][href]" : "[data-exact-on~=press],button[data-button-style]");
    if (!el || !root.contains(el) || el.closest(":disabled,[disabled]:not([disabled='false'])")) return; // a box says `disabled` (LLP 1121 D8)
    // @ref LLP 1077 D14 — `-exact-press-haptic` plays at the press, as Apple's does:
    // `navigator.vibrate` where the browser has it (not desktop, not iOS
    // Safari), with `haptic()`'s two lengths (workout F4).
    const haptic = el.style.getPropertyValue("--exact-press-haptic").trim();
    if (haptic && haptic !== "none") navigator.vibrate?.(haptic === "selection" ? 5 : 12);
    if (!(declaresPress(el) || el.matches("button[data-button-style]") || touch)) return;
    const r = unpressedBox(el);
    press = { el, id: e.pointerId, box: [r.left, r.top, r.right, r.bottom] };
    showPress(el, true);
  }, true);
  document.addEventListener("pointermove", e => {
    if (e.pointerId !== press?.id) return;
    // The button came up where this document did not hear it (outside the window).
    if (e.buttons === 0) return release();
    const [l, t, r, b] = press.box;
    showPress(press.el, e.clientX >= l && e.clientX <= r && e.clientY >= t && e.clientY <= b);
  }, true);
  for (const type of ["pointerup", "pointercancel"]) document.addEventListener(type, e => { if (e.pointerId === press?.id) release(); }, true);
  // Leaving the page, the window or the tab may take the release with it, as the pointer dispatch below assumes: the
  // press, a key's too, ends at once. A gesture that claims the contact (motion-glue's swipe) ends the press as it
  // claims it, as a pan cancels a touch.
  const leave = () => { release(true); keyHold?.(null, true); };
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
  // ring is the browser's own `:focus-visible`: none after a pointer's press, one after a key's (`focusVisible` says
  // which, where the engine takes it). The stack is read from the attributes navigation.js projects from.
  let shown = null, pressed = null;
  const origins = new WeakMap();
  // The press is noted as it begins (the pointer going down; Enter or Space, in the keys' listener above), before the
  // click any listener navigates by: a link to a route is followed from the document's capture phase (above, and the
  // JS target's router.js), so its commit arrives before a listener here would hear that click. A click with neither
  // before it (a script's, the agent's) is noted as it is heard.
  const note = (e, key) => { if (root.contains(e.target)) pressed = { el: e.target?.closest?.("button, a[href], [tabindex]") ?? null, focus: document.activeElement, key, at: performance.now() }; };
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
    const focus = () => to.focus({ preventScroll: true, ...(by ? { focusVisible: by.key } : {}) }); focus(); if (document.activeElement?.closest("[data-exiting]")) focus(); // from a frame in a shadow root Firefox lands the focus on the second call
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

// A same-origin guest joins `tree` as a compact, bounded outline. Access to
// a sandboxed or cross-origin document is simply absent (@ref LLP 1020 D4).
export function guestOutline(frame) {
  let doc;
  try { doc = frame.contentDocument; } catch { return null; }
  if (!doc) return null;
  const outline = [];
  const visit = (el, depth) => {
    if (depth > 4 || outline.length >= 32) return;
    const id = el.id || undefined;
    const testId = el.getAttribute("data-testid") ?? el.getAttribute("testId") ?? undefined;
    const text = [...el.childNodes]
      .filter((n) => n.nodeType === Node.TEXT_NODE)
      .map((n) => n.textContent.trim())
      .filter(Boolean)
      .join(" ")
      .replace(/\s+/g, " ")
      .slice(0, 160) || undefined;
    if (id || testId || text) {
      outline.push({ guest: true, depth, tag: el.localName, ...(id ? { id } : {}), ...(testId ? { testId } : {}), ...(text ? { text } : {}) });
    }
    for (const child of el.children) visit(child, depth + 1);
  };
  for (const child of doc.body?.children ?? []) visit(child, 0);
  return outline;
}
function guestDocument(frame) {
  try {
    const document = frame.contentDocument;
    return document ? { document } : { error: "guest is cross-origin" };
  } catch {
    return { error: "guest is cross-origin" };
  }
}
export function guestTap(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const x = Number.isFinite(request.x) ? request.x : guest.innerWidth / 2;
  const y = Number.isFinite(request.y) ? request.y : guest.innerHeight / 2;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, malformed: true, error: "guest tap has an invalid selector" }; }
  target ||= document.elementFromPoint(x, y) || document.body;
  if (!target) return { guest: true, error: "guest tap found no target" };
  // Script input is intentionally untrusted (@ref LLP 1020 D4;
  // exact1 20260806-webview-frame-guest-click-delivery).
  target.dispatchEvent(new guest.PointerEvent("pointerdown", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 1 }));
  target.dispatchEvent(new guest.PointerEvent("pointerup", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 0 }));
  target.dispatchEvent(new guest.MouseEvent("click", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0 }));
  return { tapped: request.id, guest: true };
}
export function guestType(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const active = document.activeElement;
  const editable = active?.matches?.("input,textarea,[contenteditable]") ? active : null;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, malformed: true, error: "guest type has an invalid selector" }; }
  target ||= editable || document.querySelector("input,textarea,[contenteditable]");
  if (!target) return { guest: true, error: "guest type found no target" };
  // These are the Apple guest script's event shapes, including focus and
  // isTrusted:false (@ref LLP 1020 D4).
  target.focus();
  if (request.key != null) {
    const key = String(request.key);
    target.dispatchEvent(new guest.KeyboardEvent("keydown", { key, bubbles: true, composed: true }));
    target.dispatchEvent(new guest.KeyboardEvent("keyup", { key, bubbles: true, composed: true }));
    return { typed: request.id, guest: true, key, value: guestValue(target) };
  }
  const text = String(request.text ?? "");
  if ("value" in target) target.value = text; else target.textContent = text;
  target.dispatchEvent(new guest.InputEvent("input", { data: text, inputType: "insertText", bubbles: true, composed: true }));
  target.dispatchEvent(new guest.Event("change", { bubbles: true, composed: true }));
  return { typed: request.id, guest: true, value: guestValue(target) };
}
// What a guest field holds, as the reply shows it: a password's is a fixed mark, whatever its length (#134).
const guestValue = (target) => "value" in target ? (target.type === "password" && target.value ? "•••" : target.value) : target.textContent;

// The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a
// target whose middle is out of view is scrolled to the middle of its
// nearest scroll containers, then of the page (across, only as far as it
// takes) — Playwright's actionability scroll, the web's own `scrollIntoView`. A scroll event reaches the app as a
// person's scroll would. `scrolled` is where the middle moved from and to;
// a middle already in view moves nothing.
// Both agents supply their modal-aware inert check; standalone callers default to DOM ancestry.
export function reveal(el, id, inert = el => el.closest("[inert]")) {
  if (!el?.isConnected) return { error: `no view ${id}` };
  const middle = () => { const b = viewBox(el); return [b.left + b.width / 2, b.top + b.height / 2]; };
  const [x, y] = middle();
  let seen = x >= 0 && y >= 0 && x < innerWidth && y < innerHeight;
  for (let a = el.parentElement; seen && a && a.id !== "exact-root"; a = a.parentElement) {
    const cs = getComputedStyle(a), b = a.getBoundingClientRect();
    if ((cs.overflowX !== "visible" || cs.overflowY !== "visible") && !(x >= b.left && x < b.right && y >= b.top && y < b.bottom)) seen = false;
  }
  if (seen || inert(el) || ["hidden", "collapse"].includes(getComputedStyle(el).visibility)) return { revealed: id, scrolled: false }; // nor what its tap refuses (hidden or inert): a refusal leaves the page as it was
  (folded(el) ? el.parentElement : el).scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" });
  return { revealed: id, scrolled: true, from: [x, y], to: middle() };
}

// A text folded into its box's content (LLP 1007.001): `display: contents`, or an inline box under a box that restricts
// touch. A paragraph's inline runs are not `data-exact-text`; only the paragraph is.
const folded = el => { const d = getComputedStyle(el).display; return d === "contents" || d === "inline" && el.hasAttribute("data-exact-text"); };

// A view's box as the agent reports it. A folded text has no block box of its own: its box is the anonymous block its
// text is, as a style-less block child's was — its line boxes along the main axis, and its box's content box across
// when the box stretches its items (a block, or a flex or grid box that stretches).
export function viewBox(el) {
  if (!folded(el)) return el.getBoundingClientRect();
  const range = document.createRange();
  range.selectNodeContents(el);
  const t = range.getBoundingClientRect(), p = el.parentElement;
  if (!p) return t;
  const cs = getComputedStyle(p), b = p.getBoundingClientRect(), n = s => parseFloat(cs[s]) || 0;
  const left = b.left + n("borderLeftWidth") + n("paddingLeft"), right = b.right - n("borderRightWidth") - n("paddingRight");
  const top = b.top + n("borderTopWidth") + n("paddingTop"), bottom = b.bottom - n("borderBottomWidth") - n("paddingBottom");
  const flex = /flex|grid/.test(cs.display), row = flex && cs.display.includes("flex") && !cs.flexDirection.startsWith("column");
  const stretch = !flex || /normal|stretch/.test(cs.alignItems);
  if (row) return stretch ? new DOMRect(t.x, top, t.width, bottom - top) : t;
  return stretch ? new DOMRect(left, t.y, right - left, t.height) : t;
}

// How far a scroller's offset travels on each axis from one end to the other (LLP 1012 `rx`/`ry`), whether or not the
// person can scroll it that way: `overflow: hidden` keeps a programmatic range.
export function scrollRange(el) {
  const r2 = (x) => Math.round(x * 100) / 100;
  return { rx: r2(Math.max(0, el.scrollWidth - el.clientWidth)), ry: r2(Math.max(0, el.scrollHeight - el.clientHeight)) };
}

// The agent reports attached views in viewport coordinates, including folded text.
export function layoutBoxes(views, gpu) {
  const r2 = (x) => Math.round(x * 100) / 100;
  const nodes = [];
  for (const [id, el] of [...views].sort((a, b) => a[0] - b[0])) {
    if (!el.isConnected) continue;
    const r = gpu?.placementHidden(el) ? new DOMRect() : viewBox(el);
    const n = { id, x: r2(r.x), y: r2(r.y), w: r2(r.width), h: r2(r.height) };
    if (el instanceof HTMLIFrameElement) {
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      n.hit = hit === el;
    }
    if (el.dataset.scroll === "true") { n.sx = r2(el.scrollLeft); n.sy = r2(el.scrollTop); Object.assign(n, scrollRange(el)); }
    nodes.push(n);
  }
  return nodes;
}
// The agent's select-all before it types, as a person's ⌘A. HTML's "set the selection range" queues `select` only
// when the selection changes (and Firefox fires none at a disabled control); the promise is that event's, so the
// driver's next input cannot overtake it and leave its handler reading the selection after the input. Its 2 s bound
// only stops a hang where no `select` comes (a control disabled before it is dispatched).
export function selectAll(el) {
  // Email and number fields keep a selection that `select()` sets and their getters do not show (HTML: `select()`
  // applies to them, `selectionStart` does not); Chrome's document selection shows a focused field's. Elsewhere
  // (Firefox's document selection does not) the two reads agree and nothing is awaited.
  const at = () => {
    try { if (el.selectionStart != null) return `${el.selectionStart},${el.selectionEnd},${el.selectionDirection}`; } catch {}
    if (document.activeElement !== el) return null;
    const s = getSelection();
    return `${s.type}:${s.direction}:${s}`;
  }, was = at();
  let heard, timer;
  const event = new Promise(resolve => {
    heard = () => { clearTimeout(timer); el.removeEventListener("select", heard); resolve(); };
    el.addEventListener("select", heard);
  });
  el.select?.();
  const now = at();
  if (el.matches(":disabled") || (was !== null && now === was)) {
    el.removeEventListener("select", heard);
    return null;
  }
  timer = setTimeout(heard, 2000);
  // An email or number field not focused before or after says nothing either way: a changed selection's `select`
  // is dispatched by the next frame (Chromium queues it for one), so it is awaited for two.
  if (was === null || now === null) requestAnimationFrame(() => requestAnimationFrame(heard));
  return event;
}
export const agentInput = { guestOutline, guestTap, guestType, reveal, viewBox, layoutBoxes, scrollRange, selectAll };
if (globalThis.exact) Object.assign(globalThis.exact, { createInputHandlers, agentInput });
