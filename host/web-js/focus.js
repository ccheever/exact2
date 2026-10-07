// `autofocus` at mount on the JS target (LLP 1035.000 D9; LLP 1102 §3.19), as the wasm host's
// focusController (host/web/navigation.js): after each commit, the first `[autofocus]` not yet
// offered that is shown, enabled and not inert is offered once, and takes the focus when the focus
// is on the body or on the control just pressed (a `when` that mounts a field after a tap).
// At boot the rest are marked offered too: a carried restart (checkpoint.js, which holds `focus`
// while it rebuilds) must not have its rebuilt fields take the focus later.
const offered = new WeakSet();
let pressed = null;

/** The control a press is dispatching on, until the microtasks after it. */
export function press(el) {
  pressed = el;
  queueMicrotask(() => { if (pressed === el) pressed = null; });
}

export function autofocus(root, boot = false) {
  if (!root) return;
  for (const el of root.querySelectorAll('[autofocus]')) {
    if (offered.has(el) || !el.getClientRects().length || el.closest('[inert]') || el.matches(':disabled') || getComputedStyle(el).visibility !== 'visible') continue;
    offered.add(el);
    const active = document.activeElement;
    if (!active || active === document.body || active === pressed) el.focus({ preventScroll: true });
    break;
  }
  if (boot) for (const el of root.querySelectorAll('[autofocus]')) offered.add(el);
}
