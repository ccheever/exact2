// Geometry reads for actions (LLP 1051.000 D1, D4, D5): where the viewer
// sees an element, and its border box at `height: auto`, answered synchronously to
// the runner's one import, `exact_geometry.read(op, view, out)`. Loaded after
// first paint when the app's wasm imports it; until then glue.js answers 0
// (unavailable).
//
// `frame` writes nothing: the border box where the viewer sees it,
// `getBoundingClientRect`'s, through every transform and with every scroll
// offset above it applied (LLP 1051.000 D1; the kernel's hosts compose the
// same from their rows and the offsets their presenters note). `measure`
// writes one inline `height` and restores it within the call, so no
// rendering step sees it; it never goes through the host's `apply`, which
// commits.
import { size } from './presence-glue.js';

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

// Fields with text being composed: a range read then answers nothing.
const composing = new WeakSet();
addEventListener('compositionstart', e => composing.add(e.target), true);
addEventListener('compositionend', e => composing.delete(e.target), true);

function createGeometry(root) {
  // Where the viewer sees `el`'s border box: its client rect.
  const frame = el => {
    const r = el.getBoundingClientRect();
    return [r.x, r.y, r.width, r.height];
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

  // `elementFromPoint(x, y)` (LLP 1094 D10): the page's own hit test, DOM's
  // `elementsFromPoint`, front to back — through every transform, as `frame`
  // reads boxes (LLP 1051.000 D1, changed 2026-10-04), an overflow clip
  // cutting descendants, `pointer-events: none` and a hidden `visibility`
  // passed over — less what is not the app's own: a leaving node, the host's
  // drag ghost and anything else outside the root. The runner's kernel walk
  // answers the same natively.
  const points = (x, y) => document.elementsFromPoint(x, y)
    .filter(el => el !== root && root.contains(el) && !el.closest('[data-exiting]'));
  const point = (x, y) => points(x, y)[0] ?? null;
  const namedPoints = (x, y) => {
    const hits = points(x, y), seen = new Set();
    return hits.flatMap(el => {
      let named = el;
      if (!named.id) {
        named = el.closest('[id]:not([id=""])');
        if (!named || hits.includes(named)) return [];
        const box = named.getBoundingClientRect();
        if (x >= box.left && x < box.right && y >= box.top && y < box.bottom) return [];
      }
      if (named === root || seen.has(named)) return [];
      seen.add(named);
      return [named.id];
    });
  };

  // Whether `offset` is on a line with no text (an empty field's too): a
  // textarea's value breaks at LF (CR and CR LF become it) and at the other
  // forced breaks a line keeps (CSS Text's BK and NL).
  const breaks = '\n\u000b\u000c\u0085\u2028\u2029';
  const emptyLine = (value, offset) =>
    (offset === 0 || breaks.includes(value[offset - 1])) && (offset === value.length || breaks.includes(value[offset]));
  // Where a textarea paints its caret at `offset`: a twin of the field,
  // every computed style copied (its font, wrapping, writing mode, box,
  // scrollbars, transform and zoom), standing where the field stands among
  // the same ancestors and scrolled as it is, holding its whole value with
  // a zero-width space at the offset; that space's box is the caret's
  // (the text's height there), read in the viewport, then the twin goes.
  const caretProbe = (el, offset) => {
    const parent = el.parentNode;
    if (!parent) return null;
    const style = getComputedStyle(el), twin = document.createElement('exact-caret-twin');
    // Its `left` and `top` are in its own zoomed px; the offsets are not.
    const zoom = parseFloat(style.zoom) || 1;
    for (const p of style) twin.style.setProperty(p, style.getPropertyValue(p));
    Object.assign(twin.style, {
      position: style.position === 'fixed' ? 'fixed' : 'absolute', left: `${el.offsetLeft / zoom}px`, top: `${el.offsetTop / zoom}px`,
      right: 'auto', bottom: 'auto', margin: '0', visibility: 'hidden', pointerEvents: 'none', display: 'block', float: 'none',
      // It moves, and scrolls, at once.
      transition: 'none', animation: 'none', scrollBehavior: 'auto',
      // The field's border box: a computed content width is less its own
      // scrollbar, which the twin lays out again.
      boxSizing: 'border-box', width: `${el.offsetWidth}px`, height: `${el.offsetHeight}px`,
      minWidth: '0', maxWidth: 'none', minHeight: '0', maxHeight: 'none',
      minInlineSize: '0', maxInlineSize: 'none', minBlockSize: '0', maxBlockSize: 'none',
    });
    // A textarea's editor keeps every space and break whatever its
    // `white-space` (Blink makes `normal` `pre-wrap` and `nowrap` `pre`;
    // `break-spaces` it leaves).
    if (style.whiteSpaceCollapse !== 'break-spaces') twin.style.whiteSpaceCollapse = 'preserve';
    // Never read, reached or focused while it stands (inert takes the
    // focus and the pointer from it and its text).
    twin.setAttribute('aria-hidden', 'true');
    twin.inert = true;
    const mark = document.createElement('exact-caret-mark');
    mark.style.cssText = 'all: unset; display: inline';
    mark.textContent = '\u200b';
    // A last empty line is a line in the field (its editor's placeholder
    // break), as it isn't after a bare break in a box: keep it.
    const last = offset < el.value.length && breaks.includes(el.value.at(-1)) ? '\u200b' : '';
    twin.append(el.value.slice(0, offset), mark, el.value.slice(offset) + last);
    parent.insertBefore(twin, el.nextSibling);
    // Its containing block may not be the field's offset parent (a
    // transformed ancestor, zoom): find where its box moves per px of
    // `left` and `top` in the viewport (a box's bounds move with it under
    // any transform), then move it onto the field's.
    const at = () => { const r = twin.getBoundingClientRect(); return [r.x, r.y]; };
    const [left, top] = [el.offsetLeft / zoom, el.offsetTop / zoom], r0 = at();
    twin.style.left = `${left + 100}px`;
    const r1 = at();
    twin.style.left = `${left}px`;
    twin.style.top = `${top + 100}px`;
    const r2 = at(), e = el.getBoundingClientRect();
    const [ax, ay, bx, by] = [(r1[0] - r0[0]) / 100, (r1[1] - r0[1]) / 100, (r2[0] - r0[0]) / 100, (r2[1] - r0[1]) / 100];
    const [dx, dy, det] = [e.x - r0[0], e.y - r0[1], ax * by - bx * ay];
    if (!det) { twin.remove(); return null; }
    twin.style.left = `${left + (dx * by - bx * dy) / det}px`;
    twin.style.top = `${top + (ax * dy - dx * ay) / det}px`;
    twin.scrollTop = el.scrollTop;
    twin.scrollLeft = el.scrollLeft;
    const r = mark.getBoundingClientRect();
    twin.remove();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  };

  return {
    point, points, namedPoints,
    // The painted caret as `caretProbe` finds it, for the tests that hold it
    // to Chrome's own carets on lines with text.
    caretAt(el, offset) { return caretProbe(el, offset); },
    // `rangeFrame` (LLP 1114.007): a plain textarea's range through
    // `createValueRange` (OpaqueRange), the Markdown editor's through its own
    // source mapping; a collapsed range is the caret's rectangle. Offsets are
    // truncated toward zero; a bad range, one past the text, a hidden field
    // and composition answer nothing.
    range(el, start, end, out) {
      if (!el?.isConnected || !root.contains(el) || !el.getClientRects().length || getComputedStyle(el).visibility === 'hidden') return 0;
      [start, end] = [Math.trunc(start), Math.trunc(end)];
      // A Markdown reader answers from its atoms' pills (LLP 1045.000 D7, atoms.js), its offsets the source's.
      const reader = !el.exactMarkup && !('value' in el) && el.getAttribute('markup') === 'markdown';
      if (!(start >= 0 && start <= end && end <= (reader ? el.$source?.length ?? Infinity : el.value?.length ?? 0)) || composing.has(el) || el.exactComposing) return 0;
      let box = null;
      if (reader) box = globalThis.exact?.atoms?.rangeFrame(el, start, end) ?? null;
      else if (el.exactMarkup) box = el.exactMarkup.rangeFrame?.(start, end) ?? null;
      else if (el instanceof HTMLTextAreaElement && typeof el.createValueRange === 'function') {
        // Chrome puts a caret on an empty line at the end of the text before
        // it, and an empty field's at the viewport's origin, where it paints
        // the caret on that line: the grammar's answer is the painted caret.
        if (start === end && emptyLine(el.value, start)) box = caretProbe(el, start);
        else try { const range = el.createValueRange(start, end); box = range.getBoundingClientRect(); range.disconnect?.(); } catch { return 0; }
      }
      if (!box) return 0;
      out.set([box.x, box.y, box.width, box.height]);
      return ANSWERED | (provisional(el) ? PROVISIONAL : 0);
    },
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
