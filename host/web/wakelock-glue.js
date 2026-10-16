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

function acquire() {
  if (!want || sentinel || asking || document.visibilityState !== "visible") return;
  if (typeof navigator.wakeLock?.request !== "function") return unavailable("navigator.wakeLock is missing");
  asking = true;
  navigator.wakeLock.request("screen").then(s => {
    asking = false;
    if (!want) { s.release().catch(() => {}); return; }
    sentinel = s;
    s.addEventListener("release", () => { if (sentinel === s) sentinel = null; });
  }, e => { asking = false; unavailable(`the request was refused (${e?.name ?? e})`); });
}

/** Hold the lock or let it go; `journal` takes the one line said where it cannot be held. */
export function hold(on, journal) {
  if (journal) log = journal;
  if (!listening) { listening = true; document.addEventListener("visibilitychange", acquire); }
  want = on;
  if (on) acquire();
  else if (sentinel) { const s = sentinel; sentinel = null; s.release().catch(() => {}); }
}

/** Whether `el` and every ancestor up to `root` are displayed (none is `display: none`). */
function displayed(el, root) {
  for (let n = el; n; n = n === root ? null : n.parentElement) if (getComputedStyle(n).display === "none") return false;
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
