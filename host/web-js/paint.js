// Mirrors kernel/src/paint_order.rs (LLP 1083.000): own facts, potentials,
// left-to-right decisions, then contributions. Templates supply facts only;
// every region arm and each copy is decided on its actual instance tree.
export function paintUse() { return { list: paintList, facts: paintFacts, flush: paintFlush, wait: paintWait }; }
// The compiler replaces these constants; unused branches leave the bundle.
const Z = true, FLOW = true, LAYOUT = true, BUTTON = true;
const dirty = new Set();
const get = (e, n) => e.getAttribute("data-exact-" + n);
const record = e => e.$paint ??= { z: false, level0: false, isolated: false, cz: false, c0: false };
// src/paint.rs folds literals and emits only each binding's contribution.
// A row clearing its flag never clears another row's stacking contribution.
const facts = e => e.$paintWaiting ? record(e).bits : e.$paintFacts ??= e.getAttributeNames().reduce((bits, k) =>
  k === "data-exact-f" || k.startsWith("data-exact-f-") ? bits | Number(e.getAttribute(k)) : bits, 0);

/** Keep authored inputs and descendant potentials while bindings wait for
 * adoption. Parent display and sibling exclusions remain live inputs. */
export function paintWait(e) {
  const [bits, zi] = JSON.parse(get(e, "paint"));
  Object.assign(record(e), { z: Z && !!(bits & 16), level0: !!(bits & 32), bits, zi });
  e.$paintWaiting = true;
}

const layout = e => LAYOUT && !!(facts(e) & 256);
const excludes = e => FLOW && (facts(e) & 1536) === 1536;
// A waiting row's children are opaque; its summary keeps this policy bit.
const holdsLayout = e => LAYOUT && !e.$paintWaiting && Array.from(e.children).some(c => !c.hasAttribute("data-exiting") && layout(c));

/** A structural edit can change both a list and its holder's policy. */
export function paintList(p) {
  if (p?.nodeType !== 1) return;
  dirty.add(p);
  if (p.parentElement && p.id !== "exact-root") dirty.add(p.parentElement);
}
/** Position, stacking rows, z-index and display all affect the two lists. */
export function paintFacts(e) { e.$paintFacts = null; paintList(e); }

function own(e, parent, exclusion) {
  const bits = facts(e), root = !!(bits & 128), positioned = !!(bits & 1);
  const zi = Z ? (e.$paintWaiting ? record(e).zi : get(e, "zi")) : null, z = zi !== null && (positioned || (facts(parent) & 4096)) ? Number(zi) : null;
  const authored = !!(bits & 2) || z !== null;
  const policy = !!(bits & 4)
    || (BUTTON && (bits & 24576) === 24576 && ((bits & 32768) || (bits & 196608) === 196608))
    || (FLOW && (bits & 2048) && exclusion)
    || holdsLayout(e);
  return { positioned, stacks: authored || !!policy, policy: !!policy && !authored, z,
    isolation: !!(bits & 64), outside: (!e.$paintWaiting && !e.hasAttribute("data-exact-f")) || !!(bits & 8), root };
}

export function paintFlush() {
  // Buckets keep the pass deepest first, including newly dirtied ancestors.
  // A wide list is queued once; no repeated sorting or subtree queries.
  const buckets = [], queued = new Set();
  const queue = p => {
    if (!p?.isConnected || p.nodeType !== 1 || queued.has(p)) return;
    let depth = 0;
    for (let a = p; a && a.id !== "exact-root"; a = a.parentElement) depth++;
    (buckets[depth] ??= []).push(p); queued.add(p);
  };
  for (const p of dirty) queue(p);
  dirty.clear();
  while (buckets.length) {
    const list = buckets.at(-1);
    if (!list?.length) { buckets.pop(); continue; }
    const p = list.pop(), children = Array.from(p.children).filter(c => !c.hasAttribute("data-exiting"));
    const exclusion = children.some(excludes);
    let layered = false, leaked = false, z = false, level0 = false, changed = false;
    for (const c of children) {
      const r = record(c), o = own(c, p, exclusion), free = !o.stacks, stat = !o.positioned;
      const isolated = !o.root && !o.outside && free && ((Z && r.z) || (stat && r.level0 && (layered || leaked)) || (stat && leaked));
      const leaks = stat && free && !isolated && r.level0;
      if (!o.outside) { layered ||= o.positioned || o.stacks || isolated || leaks; leaked ||= leaks; }
      c.toggleAttribute("data-exact-policy", o.policy);
      const iso = isolated || o.policy || o.isolation;
      // Presence restores this current decision after the last ghost. A
      // sliced adoption may flush without a presence before/after pair.
      if (c.$ghostIsolation) Object.assign(c.$ghostIsolation, { value: iso ? "isolate" : "", priority: "", released: false });
      if (iso || c.$ghostIsolation) { if (c.style.isolation !== "isolate") c.style.isolation = "isolate"; }
      else if (c.style.isolation === "isolate") c.style.removeProperty("isolation");
      const open = !o.stacks && !isolated, nonzero = Z && o.z !== null && o.z !== 0;
      const cz = Z && !o.outside && (nonzero || (open && r.z));
      const c0 = !o.outside && (((o.positioned || o.stacks) && !nonzero) || isolated || (open && stat && r.level0));
      changed ||= r.isolated !== isolated || r.policy !== o.policy || r.cz !== cz || r.c0 !== c0;
      Object.assign(r, { own: o, isolated, policy: o.policy, cz, c0 });
      if (globalThis.__exactRender) {
        // Same compact summary as the Rust document emitter.
        const bits = facts(c) | (+r.z << 4) | (+r.level0 << 5) | (+holdsLayout(c) << 2);
        c.setAttribute("data-exact-paint", JSON.stringify([bits, Z ? get(c, "zi") : null]));
      }
      z ||= cz; level0 ||= c0;
    }
    const r = record(p);
    changed ||= r.z !== z || r.level0 !== level0;
    r.z = z; r.level0 = level0;
    if (changed && p.id !== "exact-root") queue(p.parentElement);
  }
}
