// @ref LLP 1125 D11 — `wakelock="screen"` on the web: the Screen Wake Lock
// API (https://w3c.github.io/screen-wake-lock/), held while any displayed
// element under the app's root says so. Both web targets load it once an
// element carries `data-exact-wakelock`: glue.js after each batch, the JS
// target after each commit (host/web-js/wakelock.js). A hidden page loses its
// lock (the spec releases it on `visibilitychange`), so it is asked again
// when the page is visible while still wanted. Where the API is missing or a
// request rejects, the journal says so once.

let want = false, sentinel = null, asking = false, said = false, listening = false, log = () => {};

const unavailable = why => { if (!said) { said = true; log(`wakelock: unavailable: ${why}`); } };

const visible = () => document.visibilityState === "visible";

function acquire() {
  if (sentinel?.released) sentinel = null; // a release whose event has not run yet
  if (!want || sentinel || asking || !visible()) return;
  if (typeof navigator.wakeLock?.request !== "function") return unavailable("navigator.wakeLock is missing");
  asking = true;
  navigator.wakeLock.request("screen").then(s => {
    asking = false;
    // Not wanted now, or granted to a page that has since hidden (which
    // loses it): let it go; a visible page that still wants it asks again.
    if (!want || !visible() || s.released) { s.release().catch(() => {}); return; }
    sentinel = s;
    s.addEventListener("release", () => { if (sentinel === s) { sentinel = null; acquire(); } });
  }, e => {
    asking = false;
    // A hidden page is refused by the spec; it asks again when visible.
    if (visible()) unavailable(`the request was refused (${e?.name ?? e})`);
  });
}

/** Hold the lock or let it go; `journal` takes the one line said where it cannot be held. */
export function hold(on, journal) {
  if (journal) log = journal;
  if (!listening) { listening = true; document.addEventListener("visibilitychange", acquire); }
  want = on;
  if (on) acquire();
  else if (sentinel) { const s = sentinel; sentinel = null; s.release().catch(() => {}); }
}

/** Whether `el` is displayed: neither it nor an ancestor up to `root` is
 * `display: none`, leaving (an exit animation's ghost, presence-glue.js), or
 * a route or tab panel its navigation root hides (navigation.js `project`;
 * the route a sheet covers stays visible), as the runner's rule
 * (runner/src/wakelock.rs). */
function displayed(el, root) {
  for (let n = el; n; n = n === root ? null : n.parentElement) {
    if (n.hasAttribute("data-exiting") || getComputedStyle(n).display === "none") return false;
    if (n.style.visibility === "hidden" && (n.hasAttribute("navigationKey") || n.getAttribute("role") === "tabpanel")) return false;
  }
  return true;
}

/** The displayed elements under `root` that say `wakelock="screen"`, in tree order. */
export function holders(root) {
  return root ? [...root.querySelectorAll('[data-exact-wakelock="screen"]')].filter(el => displayed(el, root)) : [];
}

/** Whether the lock is held now (a sentinel granted and not released): tests and diagnostics. */
export const granted = () => sentinel != null;

// glue.js loads host pieces as scripts and reads them here.
(globalThis.exact ??= {}).wakeLockGlue = { hold, holders, granted };
