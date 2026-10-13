// Only percentage row insets need a browser layout result. One observer uses
// contentRect.x (resolved left padding), with no synchronous geometry reads.
const rows = new WeakMap(), parents = new WeakMap(), pending = new Set();
let observer, queued = false;
const resolved = '--exact-grouped-inset-resolved';
function write(el, x) {
  const value = `${x}px`, row = rows.get(el);
  if (row) row.value = value;
  if (el.style.getPropertyValue(resolved) !== value) el.style.setProperty(resolved, value);
}
function detach(el, parent) {
  const children = parents.get(parent);
  if (!children) return;
  children.delete(el);
  if (!children.size) { parents.delete(parent); if (!rows.has(parent)) observer.unobserve(parent); }
}
function bind(el) {
  const row = rows.get(el), parent = el.parentElement;
  if (!row || row.parent === parent) return;
  if (row.parent) detach(el, row.parent);
  row.parent = parent;
  if (parent) {
    let children = parents.get(parent);
    if (!children) { parents.set(parent, children = new Set()); observer.observe(parent); }
    children.add(el);
  }
}
function arrive(el) {
  pending.add(el);
  if (queued) return;
  queued = true;
  queueMicrotask(() => { queued = false; for (const el of pending) bind(el); pending.clear(); });
}
export function observeGroupedInset(el, raw) {
  if (!String(raw).includes('%')) { releaseGroupedInset(el); return; }
  if (typeof ResizeObserver !== 'function' || globalThis.__exactRender) return;
  observer ??= new ResizeObserver(entries => {
    // Deliver parent changes first: even a row whose content width stays zero
    // gets a fresh padding origin when its containing block changes width.
    const delivered = new Set(entries.map(e => e.target));
    for (const { target } of entries) for (const el of parents.get(target) ?? []) if (!delivered.has(el)) {
      observer.unobserve(el); observer.observe(el);
    }
    for (const { target, contentRect } of entries) if (rows.has(target) && target.isConnected) {
      bind(target); write(target, contentRect.x);
    }
  });
  const row = rows.get(el);
  if (!row) { rows.set(el, { raw, parent: null, value: null }); observer.observe(el); }
  else {
    if (row.value != null) write(el, parseFloat(row.value));
    if (row.raw !== raw) { row.raw = raw; observer.unobserve(el); observer.observe(el); }
  }
  arrive(el);
}
export function releaseGroupedInset(el) {
  const row = rows.get(el);
  if (!row) return;
  pending.delete(el); rows.delete(el);
  if (!parents.has(el)) observer.unobserve(el);
  if (row.parent) detach(el, row.parent);
  el.style.removeProperty(resolved);
}
export function reparentGroupedInset(el) { if (rows.has(el)) arrive(el); }
