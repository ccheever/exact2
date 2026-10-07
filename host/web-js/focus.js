// `autofocus` at mount on the JS target (LLP 1035.000 D9; LLP 1102 §3.19), as the wasm host's
// focusController (host/web/navigation.js): after each commit, the first `[autofocus]` not yet
// offered that is shown, enabled and not inert is offered once, and takes the focus when the focus
// is on the body or on the control just pressed (a `when` that mounts a field after a tap).
// A carried restart (checkpoint.js, which holds `focus` while it rebuilds) marks every field it
// rebuilt offered (`offerAll`), so none takes the focus later; a fresh boot's hidden or disabled
// field may still take it when it is shown.
const offered = new WeakSet();
let pressed = null;

/** The control a pointer's press is dispatching on (a click with `detail` > 0: a key's activation
 * keeps the focus where it is, as the wasm host's), for a moment after it: the commits it causes,
 * including a due `then` that runs first and one a view transition defers (shared.js), may hand
 * its focus to a field they mount. */
const PRESS_MS = 1000;
let pressedAt = 0;
export function press(el) {
  pressed = el;
  pressedAt = performance.now();
}

const rootOf = () => globalThis.document?.getElementById?.('exact-root');

export function autofocus(root = rootOf()) {
  const by = performance.now() - pressedAt <= PRESS_MS ? pressed : null;
  if (!root?.querySelectorAll) return;
  for (const el of root.querySelectorAll('[autofocus]')) {
    if (offered.has(el) || !el.getClientRects().length || el.closest('[inert]') || el.matches(':disabled') || getComputedStyle(el).visibility !== 'visible') continue;
    offered.add(el);
    const active = document.activeElement;
    if (!active || active === document.body || active === by) el.focus({ preventScroll: true });
    break;
  }
}

/** A carried restart's rebuilt fields: none takes the focus later. */
export function offerAll(root = rootOf()) {
  if (root?.querySelectorAll) for (const el of root.querySelectorAll('[autofocus]')) offered.add(el);
}
