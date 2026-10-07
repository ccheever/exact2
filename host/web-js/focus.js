// `autofocus` at mount on the JS target (LLP 1035.000 D9; LLP 1102 §3.19), as the wasm host's
// focusController (host/web/navigation.js): after each commit, the first `[autofocus]` not yet
// offered that is shown, enabled and not inert is offered once, and takes the focus when the focus
// is on the body or on the control just pressed (a `when` that mounts a field after a tap).
// A carried restart (checkpoint.js, which holds `focus` while it rebuilds) marks every field it
// rebuilt offered (`offerAll`), so none takes the focus later; a fresh boot's hidden or disabled
// field may still take it when it is shown.
const offered = new WeakSet();

/** The press a pointer is dispatching (a click with `detail` > 0: a key's activation keeps the
 * focus where it is, as the wasm host's), held while its own work runs: the dispatch's synchronous
 * commits (a due `then` the event runs first among them) and a tree update it handed to a view
 * transition (shared.js `hold`). A field those mount may take the focus from the pressed control,
 * as the wasm host's `pointerTarget` lets one during `press()`; once they have run, or once a later
 * pointer or key interaction supersedes it, none may (Charlie, 2026-10-07: no wall-clock window). */
let current = null;
const release = (token) => () => { if (--token.n === 0 && current === token) current = null; };
// A key always supersedes; a pointer does unless it presses the same control again.
const supersede = (ev) => { if (current && (ev.type === 'keydown' || !current.el.contains?.(ev.target))) current = null; };
let listening = false;
export function press(el) {
  if (!listening && globalThis.document?.addEventListener) {
    listening = true;
    for (const kind of ['pointerdown', 'keydown']) document.addEventListener(kind, supersede, true);
  }
  current = { el, n: 1 };
  return release(current);
}
/** Keep the press dispatching now for work it deferred; the returned function lets it go. */
export function hold() {
  if (!current) return () => {};
  current.n++;
  return release(current);
}

const rootOf = () => globalThis.document?.getElementById?.('exact-root');

export function autofocus(root = rootOf()) {
  const by = current?.el ?? null;
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
