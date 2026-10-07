// `autofocus` at mount on the JS target (LLP 1035.000 D9; LLP 1102 §3.19), as the wasm host's
// focusController (host/web/navigation.js): after each commit, the first `[autofocus]` not yet
// offered that is shown, enabled and not inert is offered once, and takes the focus when the focus
// is on the body or on the control just pressed (a `when` that mounts a field after a tap).
// A carried restart (checkpoint.js, which holds `focus` while it rebuilds) marks every field it
// rebuilt offered (`offerAll`), so none takes the focus later; a fresh boot's hidden or disabled
// field may still take it when it is shown.
const offered = new WeakSet();

/** The press a pointer is dispatching (a click with `detail` > 0: a key's activation keeps the
 * focus where it is, as the wasm host's): its own work may hand the pressed control's focus to a
 * field it mounts — the dispatch's synchronous commits (a due `then` the event runs first among
 * them) and a tree update it handed to a view transition (shared.js holds it and runs it `within`
 * that press). Nothing else may: not after that work has run, not an unrelated commit meanwhile,
 * and not once a later key, or a pointer on another control, supersedes the press, as the wasm
 * host's `pointerTarget` lasts only through `press()` (Charlie, 2026-10-07: no wall-clock window). */
let current = null; // the newest press, until a later interaction supersedes it
let dispatching = null; // the press whose synchronous dispatch is running
let running = null; // the press a held tree update runs for
const release = (token) => () => { if (--token.n === 0 && current === token) current = null; };
// A key always supersedes; a pointer does unless it presses the same control again.
const supersede = (ev) => { if (current && (ev.type === 'keydown' || !current.el.contains?.(ev.target))) { current.dead = true; current = null; } };
let listening = false;
export function press(el) {
  if (!listening && globalThis.document?.addEventListener) {
    listening = true;
    for (const kind of ['pointerdown', 'keydown']) document.addEventListener(kind, supersede, true);
  }
  const token = current = { el, n: 1, dead: false }, was = dispatching, done = release(token);
  dispatching = token;
  return () => { dispatching = was; done(); };
}
/** The press whose work is running now, kept for a tree update that work deferred; null when no
 * press's work is running (an unrelated commit holds nothing). */
export function hold() {
  const token = running ?? dispatching;
  if (!token || token.dead) return null;
  token.n++;
  return { token, release: release(token) };
}
/** Run a deferred tree update with the press that deferred it, then let that press go. */
export function within(held, f) {
  if (!held) return f();
  const was = running;
  running = held.token;
  try { return f(); } finally { running = was; held.release(); }
}

const rootOf = () => globalThis.document?.getElementById?.('exact-root');

export function autofocus(root = rootOf()) {
  const token = running ?? dispatching, by = token && !token.dead ? token.el : null;
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
