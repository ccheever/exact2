// An element's `data-*` words on the JS target (LLP 1075.003 §3.3; rt.js
// re-exports it). A read also sees literal attributes emitted with the node.
import { inertAncestor } from './navigation.js';

const opened = new WeakMap();
const opening = [];
let order = 0;
// Stamp the inner opening, then its still-dispatching callers, innermost first.
// An outer target already open aborts its opening, so keeps its existing stamp.
globalThis.addEventListener?.('beforetoggle', e => {
  for (let i = opening.length - 1; i >= 0; i--) if (opening[i].eventPhase === Event.NONE) opening.splice(i, 1);
  if (e.newState !== 'open' || (e.target.localName !== 'dialog' && !e.target.hasAttribute('popover'))) return;
  opened.set(e.target, ++order);
  for (let i = opening.length - 1; i >= 0; i--) {
    const target = opening[i].target;
    if (!target.matches('dialog:modal,:popover-open')) opened.set(target, ++order);
  }
  opening.push(e);
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
  const top = [...document.querySelectorAll('dialog:modal,:popover-open')];
  const modals = top.filter(el => el.matches('dialog:modal'));
  const front = Math.max(0, ...modals.map(el => opened.get(el) ?? 0));
  const live = modals.length ? top.filter(el => (opened.get(el) ?? 0) >= front) : null;
  return [...root.querySelectorAll(selector)].filter(el => {
    if (!el.id || inertAncestor(el) || el.closest("[data-exiting]")) return false;
    if (live && !live.some(layer => layer.contains(el))) return false;
    for (let at = el; at && at !== root; at = at.parentElement) if (getComputedStyle(at).display === "none") return false;
    return true;
  }).map(el => el.id);
};
