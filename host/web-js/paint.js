// The sibling rule of the page's painting order (LLP 1074; layers.rs is the
// live web host's): a static box that follows, among its siblings, something
// that paints with the positioned — a positioned box or stacking context, or
// one holding such a box — is isolated, so it paints over it in tree order
// as the kernel paints. A CSS `~` over `:has()` said it, and re-matched every
// later sibling whenever a list changed (a 1,000-row swap: 43 → 88 ms); this
// decides it once per changed sibling list, after the commit's tree update.
let Own = null;
const dirty = new Set();
/** The selector of what paints with the positioned (style.rs `paint_own`). */
export function paintOwn(own) { Own = own; }
/** `p`'s children changed: decide its list again. */
export function paintList(p) { if (Own && p && p.nodeType === 1) dirty.add(p); }
/** `e`'s own paint facts changed: its children's list (a flex box layers a
 * child's z-index) and every list it is in or under. */
export function paintFacts(e) { if (!Own) return; dirty.add(e); for (let a = e; a?.parentElement && a.id !== "exact-root"; a = a.parentElement) dirty.add(a.parentElement); }
const holds = e => e.matches(Own) || e.querySelector(Own) !== null;
export function paintFlush() {
  if (!dirty.size) return;
  for (const p of dirty) {
    // Its children, not its subtree (a selector would walk every row of a 10,000-row list).
    let b = p?.isConnected ? p.firstElementChild : null;
    while (b && !b.hasAttribute("data-exact-box")) b = b.nextElementSibling;
    if (!b) continue;
    let after = false;
    for (let c = p.firstElementChild; c; c = c.nextElementSibling) {
      if (c.hasAttribute("data-exact-box") && !c.hasAttribute("data-exact-own-isolation")) {
        const iso = after && !c.matches(Own);
        if ((c.$iso ?? c.style.isolation === "isolate") !== iso) iso ? c.style.isolation = "isolate" : c.style.removeProperty("isolation");
        c.$iso = iso;
      }
      if (!after) after = holds(c);
    }
  }
  dirty.clear();
}
