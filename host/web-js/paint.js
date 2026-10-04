// Mirrors kernel/src/paint_order.rs (LLP 1083.000): own facts, potentials,
// left-to-right decisions, then contributions. Templates supply facts only;
// every region arm and each copy is decided on its actual instance tree.
const dirty = new Set();
const has = (e, n) => e.hasAttribute("data-exact-" + n);
const get = (e, n) => e.getAttribute("data-exact-" + n);
const fact = (e, n) => e.getAttributeNames().some(k => k === "data-exact-" + n || k.startsWith("data-exact-" + n + "-"));
const record = e => e.$paint ??= { z: false, level0: false, isolated: false, cz: false, c0: false };

/** A structural edit can change both a list and its holder's policy. */
export function paintList(p) {
  if (p?.nodeType !== 1) return;
  dirty.add(p);
  if (p.parentElement && p.id !== "exact-root") dirty.add(p.parentElement);
}
/** Position, stacking rows, z-index and display all affect the two lists. */
export function paintFacts(e) { paintList(e); }

function own(e, parent, exclusion) {
  const root = has(e, "root"), position = get(e, "position");
  const positioned = root || position !== null;
  const zi = get(e, "zi"), z = zi !== null && (positioned || has(parent, "flex")) ? Number(zi) : null;
  const authored = fact(e, "stack") || has(e, "own-isolation") || position === "sticky" || z !== null;
  const kind = get(e, "kind"), button = kind === "control" && get(e, "type") === "button";
  const style = get(e, "button-style") ?? "bordered";
  const policy = root || kind === "canvas"
    || (kind === "image" && (has(e, "tint") || get(e, "source")?.startsWith("symbol:")))
    || (button && (style.endsWith("glass") || (get(e, "disabled") === "true" && !["bordered", "gray"].includes(style))))
    || has(e, "material") || get(e, "navigation") === "modal"
    || fact(e, "motion") || has(e, "layout") || has(e, "exit")
    || (kind === "text" && exclusion) || Array.from(e.children).some(c => has(c, "layout"));
  return { positioned, stacks: authored || !!policy, policy: !!policy && !authored, z,
    outside: !kind || has(e, "outside") || get(e, "semantic") === "dialog" || has(e, "popover"), root };
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
    const p = list.pop(), children = Array.from(p.children);
    const exclusion = children.some(c => get(c, "position") === "absolute" && has(c, "wrap"));
    let layered = false, leaked = false, z = false, level0 = false, changed = false;
    for (const c of children) {
      const r = record(c), o = own(c, p, exclusion), free = !o.stacks, stat = !o.positioned;
      const isolated = !o.root && !o.outside && free && (r.z || (stat && r.level0 && (layered || leaked)) || (stat && leaked));
      const leaks = stat && free && !isolated && r.level0;
      if (!o.outside) { layered ||= o.positioned || o.stacks || isolated || leaks; leaked ||= leaks; }
      c.toggleAttribute("data-exact-policy", o.policy);
      const iso = isolated || o.policy || has(c, "own-isolation");
      if (iso) { if (c.style.isolation !== "isolate") c.style.isolation = "isolate"; }
      else if (c.style.isolation === "isolate") c.style.removeProperty("isolation");
      const open = !o.stacks && !isolated, nonzero = o.z !== null && o.z !== 0;
      const cz = !o.outside && (nonzero || (open && r.z));
      const c0 = !o.outside && (((o.positioned || o.stacks) && !nonzero) || isolated || (open && stat && r.level0));
      changed ||= r.isolated !== isolated || r.policy !== o.policy || r.cz !== cz || r.c0 !== c0;
      Object.assign(r, { own: o, isolated, policy: o.policy, cz, c0 });
      z ||= cz; level0 ||= c0;
    }
    const r = record(p);
    changed ||= r.z !== z || r.level0 !== level0;
    r.z = z; r.level0 = level0;
    if (changed && p.id !== "exact-root") queue(p.parentElement);
  }
}
