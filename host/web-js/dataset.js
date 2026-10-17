// An element's `data-*` words on the JS target (LLP 1075.003 §3.3; rt.js
// re-exports it). A read also sees literal attributes emitted with the node.
import { inertAncestor } from './navigation.js';

const opened = new WeakMap();
let order = 0;
// Only a hatch's script can open another presentation inside beforetoggle.
// That reentrant order is not tracked: the outer opening is stamped first
// but enters the top layer after the inner one.
globalThis.document?.addEventListener?.('beforetoggle', e => {
  if (e.newState === 'open' && (e.target.localName === 'dialog' || e.target.hasAttribute('popover'))) opened.set(e.target, ++order);
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
  const top = [...document.querySelectorAll(':modal,:popover-open')];
  const modals = top.filter(el => el.matches(':modal'));
  const front = Math.max(0, ...modals.map(el => opened.get(el) ?? 0));
  const live = modals.length ? top.filter(el => (opened.get(el) ?? 0) >= front) : null;
  return [...root.querySelectorAll(selector)].filter(el => {
    if (!el.id || inertAncestor(el) || el.closest("[data-exiting]")) return false;
    if (live && !live.some(layer => layer.contains(el))) return false;
    for (let at = el; at && at !== root; at = at.parentElement) if (getComputedStyle(at).display === "none") return false;
    return true;
  }).map(el => el.id);
};
