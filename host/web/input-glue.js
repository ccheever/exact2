// Input-only glue: loaded after the baked first pixel, independently of data readiness.
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
/** @ref LLP 1115.000 D4.5 — a `tablist`'s tabs and a `menu`'s items move by the arrow keys, as WAI-ARIA's patterns give
 * the roles and a web developer expects of them: Left and Right along a tablist (Up and Down when it is vertical: its
 * `aria-orientation`, or a root tab bar the shell draws as a sidebar), Up and Down in a menu (and into an open one from
 * outside it), Home and End to the ends, wrapping, past disabled ones. The focus moves and nothing is pressed: a tab's
 * press is the app's action (a fetch, in Shelf's Discover), so Enter or Space chooses, as a `<button>` takes them. A
 * tablist is one Tab stop: its selected tab, or the tab the arrows left the focus on. */
const ROVES = '[role="tablist"], [role="menu"]';
const roversOf = group => [...group.querySelectorAll(group.getAttribute("role") === "menu" ? 'button, a[href], [role^="menuitem"]' : '[role="tab"]')].filter(el => el.closest(ROVES) === group);
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
  const open = !within && /^Arrow(Up|Down)$/.test(key) && !el?.matches?.("input, textarea, select, [contenteditable]") ? root.querySelector('[role="menu"]:popover-open') : null;
  const group = open ?? within;
  if (!group || !root.contains(group)) return false;
  const menu = group.getAttribute("role") === "menu", all = roversOf(group), live = all.filter(item => item === el || radioUsable(item));
  if (!open && !all.includes(el)) return false;
  const vertical = menu || group.getAttribute("aria-orientation") === "vertical" || group.parentElement?.hasAttribute("data-exact-sidebar"), rtl = getComputedStyle(group).direction === "rtl";
  const step = (vertical ? { ArrowDown: 1, ArrowUp: -1 } : { ArrowRight: rtl ? -1 : 1, ArrowLeft: rtl ? 1 : -1 })[key];
  const next = open ? live.at(key === "ArrowDown" ? 0 : -1) : step ? live[(live.indexOf(el) + step + live.length) % live.length] : key === "Home" ? live[0] : key === "End" ? live.at(-1) : null;
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
    const { wasm, writeIn, navigate, go } = globalThis.exact;
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
    // a pointer: a native button's highlight, a `-exact-press-scale`. (The
    // unsaid dim needs nothing here: it is the browser's own `:active`.)
    if (event.key === " " && el?.matches?.("button:enabled") && root.contains(el) && (el.hasAttribute("data-button-style") || el.style.getPropertyValue("--exact-press"))) return keyed(el);
    if (typeof el?.matches !== "function" || !root.contains(el) || !pressesByKey(el)) return;
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
  // `-exact-press-scale`, else the shell's dim) until the key comes up or the
  // focus leaves, as the browser holds a `<button>` `:active` under Space; so
  // does a `<button>` whose press is declared.
  const keyed = el => {
    const up = e => {
      if (e.type === "keyup" && e.key !== " ") return;
      showPress(el, false);
      removeEventListener("keyup", up, true); el.removeEventListener("blur", up);
    };
    showPress(el, true);
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
  const showPress = (el, down) => {
    el.toggleAttribute("data-pressed", down);
    const to = down ? Number(el.style.getPropertyValue("--exact-press") || 1) : 1;
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
    if (agentMode) animation.finish();
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
  const release = () => { if (press) showPress(press.el, false); press = null; };
  root.addEventListener("pointerdown", e => {
    if (e.button !== 0 || !e.isPrimary) return;
    release(); // a press whose release never reached the page
    surface(e.target);
    const el = e.target.closest?.("[data-exact-on~=press],button[data-button-style]");
    if (!el || !root.contains(el) || el.closest(":disabled,[disabled='true']")) return;
    // @ref LLP 1077 D14 — `-exact-press-haptic` plays at the press, as Apple's does:
    // `navigator.vibrate` where the browser has it (not desktop, not iOS
    // Safari), with `haptic()`'s two lengths (workout F4).
    const haptic = el.style.getPropertyValue("--exact-press-haptic").trim();
    if (haptic && haptic !== "none") navigator.vibrate?.(haptic === "selection" ? 5 : 12);
    if (!(el.style.getPropertyValue("--exact-press") || el.matches("button[data-button-style]"))) return;
    const r = unpressedBox(el);
    press = { el, id: e.pointerId, box: [r.left, r.top, r.right, r.bottom] };
    showPress(el, true);
  }, true);
  document.addEventListener("pointermove", e => {
    if (e.pointerId !== press?.id) return;
    const [l, t, r, b] = press.box;
    showPress(press.el, e.clientX >= l && e.clientX <= r && e.clientY >= t && e.clientY <= b);
  }, true);
  for (const type of ["pointerup", "pointercancel"]) document.addEventListener(type, e => { if (e.pointerId === press?.id) release(); }, true);
  // @ref LLP 1115.000 D4.2 — a route is a page, so the focus follows it, as a careful web app moves it when it
  // navigates. On a push (or a replace) it goes to the new route's header's heading, else to the route itself; on
  // Back, to the control whose press pushed the route that left (the row, the link, the button that opened a sheet),
  // else as on a push. A focus the commit set is left alone: an `autofocus` field, a `focus()` command, anything
  // now inside the route, or anywhere that is neither the pressed control nor where the focus was as the press began
  // (so a route that arrives with no press never takes the focus from where a person put it). Another
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
  const lands = el => !!el?.isConnected && !el.closest("[inert]") && el.getClientRects().length > 0 && !el.matches(":disabled");
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
    for (const route of nav?.querySelectorAll("[navigationPresentation], [data-exact-dialog]") ?? []) dialog(route);
    const top = key == null ? null : [...nav.querySelectorAll("[navigationKey]")].find(r => r.getAttribute("navigationKey") === key && r.parentElement.closest("[navigationKey]") === nav);
    if (!top) return;
    const routes = [...top.parentElement.children].filter(r => r.hasAttribute("navigationKey")), stack = routes.slice(0, routes.indexOf(top) + 1);
    const was = shown, active = document.activeElement, by = pressed && performance.now() - pressed.at < 2000 ? pressed : null;
    shown = stack;
    if (!was || was[0] !== stack[0] || was.at(-1) === top) return;
    const back = was.includes(top);
    if (!back) origins.set(top, was.length === stack.length ? origins.get(was.at(-1)) : by?.el); // a replace keeps where its route came from
    pressed = null;
    if (active && active !== document.body && (top.contains(active) || ![by?.el, by?.focus].includes(active))) return;
    const origin = back ? origins.get(was[was.indexOf(top) + 1]) : null;
    const to = lands(origin) ? origin : titleOf(top) ?? top;
    if (!to.hasAttribute("tabindex") && to.tabIndex < 0) { to.tabIndex = -1; to.addEventListener("blur", () => to.removeAttribute("tabindex"), { once: true }); }
    to.focus({ preventScroll: true, ...(by ? { focusVisible: by.key } : {}) });
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
    // scroll), files dropped from outside (38, each minted a `doc:` handle,
    // documents-glue.js) and the window's `beforeunload` (36), as the JS
    // target's rt.js and files.js hear them. `fire(kind, line)` dispatches.
    mouse(el, kind, e, fire) {
      if (!ready()) return;
      if (kind === "beforeunload") return el.isConnected && fire(36, "");
      if (el.matches(":disabled") || inertAncestor(el)) return;
      if (kind === "wheel") return e.exactStopped || fire(37, `${pointerLine(el, e).split(",").slice(0, 2)},${e.deltaX},${e.deltaY},${e.deltaMode},${modifiers(e)}`);
      if (kind === "dragover") { if (e.dataTransfer?.types?.includes("Files")) { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; } return; }
      if (kind === "drop") {
        const dt = e.dataTransfer;
        if (!dt?.files?.length) return;
        e.preventDefault(); e.stopPropagation();
        const at = `${pointerLine(el, e).split(",").slice(0, 2)},${modifiers(e)}`, files = [...dt.files];
        const handles = [...dt.items].filter(i => i.kind === "file").map(i => i.getAsFileSystemHandle?.().catch(() => null) ?? null);
        return documents?.().then(async (glue) => {
          const found = await glue.dropped(await Promise.all(handles), files);
          if (found.length) fire(38, `${at}\n${found.join("\n")}`); else log("drop: refused: no file of a type this app declares");
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

if (globalThis.exact) globalThis.exact.createInputHandlers = createInputHandlers;
