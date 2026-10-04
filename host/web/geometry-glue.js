// Geometry reads for actions (LLP 1051.000 D1, D4, D5): where layout put an
// element, and its border box at `height: auto`, answered synchronously to
// the runner's one import, `exact_geometry.read(op, view, out)`. Loaded after
// first paint when the app's wasm imports it; until then glue.js answers 0
// (unavailable).
//
// `frame` writes nothing: LLP 1063's layout-box measure, composed up to the
// root, free of every transform, with every scroll offset above it applied
// and placed in the viewport, as `getBoundingClientRect` places a box (the
// kernel's hosts subtract the offsets their presenters note; kanban diary
// F4). `measure` writes one inline
// `height` and restores it within the call, so no rendering step sees it; it
// never goes through the host's `apply`, which commits.
import { measure as layoutBoxes, size } from './presence-glue.js';

// Reply bits: answered, provisional.
const ANSWERED = 1, PROVISIONAL = 2;
// Animated properties that leave layout alone (keyframe keys, kebab-cased).
const PAINT_ONLY = new Set(['transform', 'translate', 'scale', 'rotate', 'opacity', 'filter', 'backdrop-filter',
  'color', 'background-color', 'box-shadow', 'clip-path', 'visibility',
  'offset', 'easing', 'composite', 'computed-offset']);
const kebab = name => name.replace(/[A-Z]/g, c => '-' + c.toLowerCase());

// An animation that moves layout, running or held: a CSS transition names its
// property; a keyframe effect names every property it animates.
function movesLayout(animation) {
  if (animation.playState !== 'running' && animation.playState !== 'paused') return false;
  const names = typeof CSSTransition !== 'undefined' && animation instanceof CSSTransition
    ? [animation.transitionProperty]
    : (animation.effect?.getKeyframes?.() ?? []).flatMap(frame => Object.keys(frame));
  return names.some(name => !PAINT_ONLY.has(kebab(name)));
}

// An inline declaration back as it was, or gone.
function restore(style, [name, value, priority]) {
  if (value) style.setProperty(name, value, priority); else style.removeProperty(name);
}

function createGeometry(root) {
  // Where the viewer sees `el`'s laid-out border box: in the viewport,
  // scrolled but untransformed. Each step is LLP 1063's box of an element in
  // its parent (a virtualized row's root in the list content, since its
  // wrapper only positions it) less the parent's scroll, composed up; the
  // root's own place in the viewport carries the page's scroll.
  const frame = el => {
    const box = layoutBoxes();
    let x = 0, y = 0, at = el, own = null;
    while (at && at !== root) {
      const parent = at.parentElement?.hasAttribute('data-listitemkey') ? at.parentElement.parentElement : at.parentElement;
      const b = box(at, parent);
      if (!b) return null;
      own ??= b;
      x += b[0] - parent.scrollLeft; y += b[1] - parent.scrollTop;
      at = parent;
    }
    if (at !== root || !own) return null;
    const page = root.getBoundingClientRect();
    return [x + page.left, y + page.top, own[2], own[3]];
  };

  // Its border box at `height: auto`, every other style kept (D4): refused
  // while it or an ancestor animates layout, since an inline write would
  // cancel a CSS transition and lose to a running WAAPI one. Every scroller
  // the write can move is put back: an ancestor, which anchoring may shift,
  // and the node itself or a scroller inside it, whose offset is clamped
  // when it grows.
  const auto = el => {
    for (let a = el; a && a !== document.documentElement; a = a.parentElement) {
      if (a.getAnimations().some(movesLayout)) return null;
    }
    const style = el.style;
    const saved = ['height', 'transition'].map(name => [name, style.getPropertyValue(name), style.getPropertyPriority(name)]);
    const scrollers = [];
    const hold = a => {
      scrollers.push([a, a.scrollLeft, a.scrollTop, ['overflow-anchor', a.style.getPropertyValue('overflow-anchor'), a.style.getPropertyPriority('overflow-anchor')]]);
      a.style.setProperty('overflow-anchor', 'none', 'important');
    };
    for (let a = el.parentElement; a; a = a.parentElement) {
      if (a.scrollHeight > a.clientHeight || a.scrollWidth > a.clientWidth) hold(a);
    }
    for (const a of [el, ...el.querySelectorAll('*')]) if (a.scrollTop || a.scrollLeft) hold(a);
    // Transitions off first, so neither write starts one.
    style.setProperty('transition', 'none', 'important');
    style.setProperty('height', 'auto', 'important');
    const measured = size(getComputedStyle(el));
    restore(style, saved[0]);
    void getComputedStyle(el).height; // the height is back before transitions are
    restore(style, saved[1]);
    // Offsets land at once, since `scroll-behavior: smooth` animates an
    // assignment (CSSOM View §7), and only where one moved, so no snap
    // restarts and no pan is cancelled.
    for (const [a, left, top] of scrollers) {
      if (a.scrollLeft !== left || a.scrollTop !== top) a.scrollTo({ left, top, behavior: 'instant' });
    }
    for (const [a, , , anchor] of scrollers) restore(a.style, anchor);
    return measured;
  };

  // An image or video inside that waits for its natural size, or fonts still
  // loading: the box may change (D5).
  const provisional = el => {
    const media = [el, ...el.querySelectorAll('img, video')];
    return media.some(m => (m instanceof HTMLImageElement && (!m.complete || m.naturalWidth === 0))
      || (m instanceof HTMLVideoElement && m.readyState < 1))
      || (document.fonts && document.fonts.status !== 'loaded');
  };

  // `elementFromPoint(x, y)` (LLP 1094 D10): the front-most element whose
  // `frame` box holds the viewport point, as the runner's kernel walk finds
  // it natively: children front to back (LLP 1083.000's rank from the
  // computed style, then tree order) before their parent, an overflow clip
  // cutting descendants to the padding box, `pointer-events: none` and a
  // hidden `visibility` passed over, `display: none` and a leaving node's
  // subtree skipped. A row wrapper only positions its row: its children are
  // placed in the list content, as `frame` places them.
  const point = (x, y) => {
    const box = layoutBoxes(), page = root.getBoundingClientRect(), placed = new Map([[root, [page.left, page.top, page.width, page.height]]]);
    const at = el => {
      if (placed.has(el)) return placed.get(el);
      const parent = el.parentElement?.hasAttribute('data-listitemkey') ? el.parentElement.parentElement : el.parentElement;
      const b = parent && box(el, parent), q = b && at(parent);
      const p = q ? [q[0] + b[0] - parent.scrollLeft, q[1] + b[1] - parent.scrollTop, b[2], b[3]] : null;
      placed.set(el, p);
      return p;
    };
    const inside = b => b[0] <= x && x < b[2] && b[1] <= y && y < b[3];
    const rank = (cs, item) => {
      const positioned = cs.position !== 'static', z = (positioned || item) && cs.zIndex !== 'auto' ? parseInt(cs.zIndex) : null;
      if (z) return 2 * z;
      return positioned || z === 0 || cs.isolation === 'isolate' || +cs.opacity < 1 || cs.filter !== 'none'
        || ['transform', 'translate', 'scale', 'rotate'].some(p => cs[p] !== 'none') ? 1 : 0;
    };
    const hit = (el, clip) => {
      const cs = getComputedStyle(el);
      if (cs.display === 'none' || el.hasAttribute('data-exiting')) return null;
      const wrapper = el.hasAttribute('data-listitemkey'), b = wrapper || cs.display === 'contents' ? null : at(el);
      let inner = clip;
      if (b && (cs.overflowX !== 'visible' || cs.overflowY !== 'visible')) {
        const pad = [b[0] + parseFloat(cs.borderLeftWidth), b[1] + parseFloat(cs.borderTopWidth),
          b[0] + b[2] - parseFloat(cs.borderRightWidth), b[1] + b[3] - parseFloat(cs.borderBottomWidth)];
        inner = clip ? [Math.max(clip[0], pad[0]), Math.max(clip[1], pad[1]), Math.min(clip[2], pad[2]), Math.min(clip[3], pad[3])] : pad;
      }
      const item = /flex|grid/.test(cs.display);
      const kids = [...el.children].map((c, i) => [rank(getComputedStyle(c), item), i, c]).sort((a, b) => a[0] - b[0] || a[1] - b[1]);
      for (let i = kids.length - 1; i >= 0; i--) { const h = hit(kids[i][2], inner); if (h) return h; }
      return b && inside([b[0], b[1], b[0] + b[2], b[1] + b[3]]) && (!clip || inside(clip))
        && cs.pointerEvents !== 'none' && cs.visibility === 'visible' ? el : null;
    };
    return hit(root, null);
  };

  return {
    point,
    read(op, el, out) {
      if (!el?.isConnected || !root.contains(el)) return 0;
      const box = frame(el);
      if (!box) return 0;
      if (op === 1) {
        const measured = auto(el);
        if (!measured) return 0;
        [box[2], box[3]] = measured;
      }
      out.set(box);
      return ANSWERED | (provisional(el) ? PROVISIONAL : 0);
    },
  };
}

if (globalThis.exact) globalThis.exact.geometry = createGeometry;
