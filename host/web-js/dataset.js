// An element's `data-*` words on the JS target (LLP 1075.003 §3.3; rt.js
// re-exports it). A read also sees literal attributes emitted with the node.
import { inertAncestor } from './navigation.js';

// A beforetoggle handler can open another presentation before its own enters
// the top layer. Finish nested events first, but leave events still dispatching
// pending: focus handlers run after insertion and can open presentations too.
const opened = new WeakMap();
const opening = [];
let order = 0;
function completedOpenings() {
  for (let i = opening.length - 1; i >= 0; i--) {
    const e = opening[i];
    if (e.eventPhase !== Event.NONE) continue;
    if (!e.defaultPrevented && e.target.matches(':modal,:popover-open')) opened.set(e.target, ++order);
    opening.splice(i, 1);
  }
}
globalThis.document?.addEventListener?.('beforetoggle', e => {
  completedOpenings();
  if (e.newState === 'open' && (e.target.localName === 'dialog' || e.target.hasAttribute('popover'))) opening.push(e);
}, true);

/** The runner's object of strings as one attribute per word; a word the
 * element's last value had and this one lacks goes. A hatched node's hatch
 * hears the change (hatches.js, LLP 1075.003.000). */
export function ds(e, json) {
  const words = JSON.parse(json ?? "{}"), last = e.$ds ?? [];
  let changed = false;
  for (const w of last) if (!Object.hasOwn(words, w)) { e.removeAttribute("data-" + w); changed = true; }
  for (const [w, v] of Object.entries(words)) if (e.getAttribute("data-" + w) !== v) { e.setAttribute("data-" + w, v); changed = true; }
  e.$ds = Object.keys(words);
  if (changed) e.$ht?.();
}

export const x_querySelectorAll = selector => {
  const match = /^\[data-([a-z][a-z0-9]*(?:-[a-z][a-z0-9]*)*)~="([^"\\\n\r]+)"\]$/.exec(selector);
  if (!match) return [];
  const root = document.getElementById("exact-root");
  completedOpenings();
  const top = [...document.querySelectorAll(':modal,:popover-open')];
  // An opening can precede this module or have its event intercepted. A live
  // modal must still block page matches even when its event was not observed.
  for (const el of top) if (!opened.has(el)) opened.set(el, ++order);
  const modals = top.filter(el => el.matches(':modal'));
  const front = Math.max(0, ...modals.map(el => opened.get(el)));
  const live = modals.length ? top.filter(el => opened.get(el) >= front) : null;
  return [...root.querySelectorAll(selector)].filter(el => {
    if (!el.id || inertAncestor(el) || el.closest("[data-exiting]")) return false;
    if (live && !live.some(layer => layer.contains(el))) return false;
    for (let at = el; at && at !== root; at = at.parentElement) if (getComputedStyle(at).display === "none") return false;
    return true;
  }).map(el => el.id);
};
