// @ref LLP 1005, Intersection visibility. Loaded only after first paint.
// One observer per root/margin preserves the browser's entry order. The
// mutation observer follows mount/removal and changes of scroll ownership;
// the browser itself tracks geometry, clipping, transforms and scrolling.
const targets = new Map(), groups = new Map();
let mutations = null;

function rootOf(el) {
  for (let p = el.parentElement; p; p = p.parentElement) {
    if (p === document.scrollingElement) return null;
    const s = getComputedStyle(p);
    if ([s.overflowX, s.overflowY].some(v => /^(auto|scroll|hidden)$/.test(v))) return p;
  }
  return null;
}

function release(t) {
  const g = t.group;
  if (!g) return;
  g.observer.unobserve(t.el);
  if (!--g.count) {
    g.observer.disconnect();
    const byMargin = groups.get(g.root);
    byMargin.delete(g.margin);
    if (!byMargin.size) groups.delete(g.root);
  }
  t.group = null;
}

function sync() {
  for (const t of targets.values()) {
    if (!t.el.isConnected) { t.stop(); continue; }
    const root = rootOf(t.el), margin = t.el.getAttribute('data-intersectmargin') ?? '0px';
    if (t.group?.root === root && t.group.margin === margin) continue;
    release(t);
    let byMargin = groups.get(root);
    if (!byMargin) groups.set(root, byMargin = new Map());
    let g = byMargin.get(margin);
    if (!g) {
      g = { root, margin, count: 0, observer: null };
      try {
        g.observer = new IntersectionObserver(entries => {
          for (const entry of entries) {
            const current = targets.get(entry.target);
            // An earlier action can retire another entry in this same batch.
            if (current?.group === g && entry.target.isConnected) current.deliver(entry);
          }
        }, { root, rootMargin: margin, threshold: 0 });
      } catch (error) {
        if (!byMargin.size) groups.delete(root);
        t.stop();
        console.warn('intersect: observation failed', error);
        continue;
      }
      byMargin.set(margin, g);
    }
    t.group = g;
    g.count++;
    g.observer.observe(t.el);
  }
}

export function observeIntersect(el, deliver) {
  targets.get(el)?.stop();
  if (!el.isConnected) return () => {};
  const t = { el, deliver, group: null, stop: null };
  t.stop = () => {
    if (targets.get(el) !== t) return;
    release(t);
    targets.delete(el);
    if (!targets.size) {
      mutations?.disconnect(); mutations = null;
      removeEventListener('resize', sync);
    }
  };
  targets.set(el, t);
  if (!mutations) {
    mutations = new MutationObserver(sync);
    mutations.observe(document.documentElement, { subtree: true, childList: true, attributes: true,
      attributeFilter: ['class', 'style', 'data-intersectmargin'] });
    addEventListener('resize', sync);
  }
  sync();
  return t.stop;
}

if (globalThis.exact) globalThis.exact.observeIntersect = observeIntersect;
