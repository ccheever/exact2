// Exit animation and layout transition on the web (LLP 1063). Fetched after
// paint, when a batch first carries either row (`presenceLoader` in
// navigation.js holds a batch that needs it before it arrives); the browser
// plays both.
//
// An `exit` op names the one view of a removed subtree that leaves (the
// kernel's choice) and the CSS `animation` list it leaves with, its
// `@keyframes` already in the page. This module keeps it, out of flow at its
// last box, inert and unnamed, until those animations end; they follow the
// element's own `animation` list, which keeps playing.
//
// `--exact-layout-transition` is `<ms> <delay ms> <easing>`, the easing a
// CSS one or `spring(stiffness, damping, mass)`. A view that declares it is
// measured before and after each batch; a move within the box it is placed
// in plays back from where it was as an additive `translate`, so it composes
// with the authored one and a moving parent carries it. A size change is the
// surface's alone, as native frame animation shows it: the element keeps its
// laid-out box, so its content and children never scale, and a stand-in
// behind it (the element's background, border, radius and shadow, which it
// stops painting for the while) grows or shrinks from its top-left corner;
// an element that clips its children clips them to the shown box. A spring
// is lowered here, per move, from its displacement and velocity in points,
// on the grid and rest threshold the engine settles on natively
// (`exact_motion::spring`).
const LAYOUT = '--exact-layout-transition';
const PAINT = ['backgroundColor', 'backgroundImage', 'backgroundSize', 'backgroundPosition', 'backgroundRepeat', 'backgroundOrigin', 'backgroundClip',
  'borderTop', 'borderRight', 'borderBottom', 'borderLeft', 'borderRadius', 'boxShadow', 'opacity', 'translate', 'rotate', 'scale', 'transform', 'transformOrigin'];
const CLEAR = { backgroundColor: 'transparent', backgroundImage: 'none', borderTopColor: 'transparent', borderRightColor: 'transparent',
  borderBottomColor: 'transparent', borderLeftColor: 'transparent', boxShadow: 'none' };

// A 2D matrix of an element's own transforms about its origin, from its
// computed style: `translate`, `rotate`, `scale`, then `transform`.
function own(cs, w, h) {
  const length = (v, of) => v.endsWith('%') ? parseFloat(v) / 100 * of : parseFloat(v);
  const [ox, oy] = cs.transformOrigin.split(' ').map(parseFloat);
  const m = new DOMMatrix().translateSelf(ox, oy);
  if (cs.translate !== 'none') { const [x, y = '0px'] = cs.translate.split(' '); m.translateSelf(length(x, w), length(y, h)); }
  if (cs.rotate !== 'none') {
    const parts = cs.rotate.split(' '), angle = parseFloat(parts.pop());
    const axis = parts.length === 3 ? parts.map(Number) : { x: [1, 0, 0], y: [0, 1, 0] }[parts[0]] ?? [0, 0, 1];
    m.rotateAxisAngleSelf(...axis, angle);
  }
  if (cs.scale !== 'none') { const [sx, sy = sx] = cs.scale.split(' ').map(Number); m.scaleSelf(sx, sy); }
  if (cs.transform !== 'none') m.multiplySelf(new DOMMatrix(cs.transform));
  return m.translateSelf(-ox, -oy);
}

// The linear part (rotation, scale, skew) of a matrix.
const linear = m => new DOMMatrix([m.a, m.b, m.c, m.d, 0, 0]);

// An element's border-box size from its computed style, sub-pixel and free
// of transforms; null for a box CSS gives no single size (an inline box).
function size(cs) {
  const edges = (a, b) => ['padding', 'border'].reduce((n, e) => n + parseFloat(cs[`${e}${a}${e === 'border' ? 'Width' : ''}`]) + parseFloat(cs[`${e}${b}${e === 'border' ? 'Width' : ''}`]), 0);
  let w = parseFloat(cs.width), h = parseFloat(cs.height);
  if (!(w >= 0 && h >= 0)) return null;
  if (cs.boxSizing !== 'border-box') { w += edges('Left', 'Right'); h += edges('Top', 'Bottom'); }
  return [w, h];
}

// One measure of the page: every element's computed style, size and own
// transform, and the linear map its ancestors draw it with, each read once.
function measure() {
  const known = new Map();
  const of = el => {
    let m = known.get(el);
    if (m) return m;
    const cs = getComputedStyle(el), [w, h] = size(cs) ?? [0, 0], parent = el.parentElement;
    const above = parent && parent !== document.documentElement ? of(parent) : null;
    m = { cs, w, h, own: own(cs, w, h), above: above ? above.above.multiply(linear(above.own)) : new DOMMatrix() };
    // Where its untransformed top-left lands on the page: its bounding rect
    // is the box of its corners through its own and its ancestors' maps.
    const r = el.getBoundingClientRect(), map = m.above.multiply(m.own);
    const corners = [[0, 0], [w, 0], [0, h], [w, h]].map(([x, y]) => map.transformPoint({ x, y }));
    m.at = { x: r.left - Math.min(...corners.map(c => c.x)), y: r.top - Math.min(...corners.map(c => c.y)) };
    m.map = map;
    known.set(el, m);
    return m;
  };
  // Where layout put `el` in `parent`, `[x, y, w, h]` in the parent's own
  // points: sub-pixel and free of every transform, however its ancestors
  // turn or scale it. A virtualized list's row wrapper (it carries
  // `data-listitemkey`) only positions its row, so by default a row's root
  // is placed in the list content.
  return (el, parent = el.parentElement?.hasAttribute('data-listitemkey') ? el.parentElement.parentElement : el.parentElement) => {
    if (!parent || !el.isConnected || !size(getComputedStyle(el))) return null;
    const e = of(el), p = of(parent);
    // The page point of its top-left is the parent's local point through
    // the parent's map: undo that map, then the parent's scroll.
    const d = p.map.inverse().transformPoint({ x: e.at.x - p.at.x, y: e.at.y - p.at.y });
    return [d.x + parent.scrollLeft, d.y + parent.scrollTop, e.w, e.h];
  };
}

// `exact_motion::spring`: displacement and velocity at `t` seconds after a
// release at `d` moving at `v`, closed form in all three regimes.
function spring([k, c, m], d, v, t) {
  const a = c / (2 * m), w2 = k / m, disc = a * a - w2, eps = w2 * 1e-12, decay = Math.exp(-a * t);
  if (disc < -eps) {
    const wd = Math.sqrt(-disc), b = (v + a * d) / wd, co = Math.cos(wd * t), si = Math.sin(wd * t);
    return [decay * (d * co + b * si), decay * ((-a * d + b * wd) * co + (-a * b - d * wd) * si)];
  }
  if (Math.abs(disc) <= eps) { const b = v + a * d; return [decay * (d + b * t), decay * (b - a * (d + b * t))]; }
  const root = Math.sqrt(disc), r1 = -a + root, r2 = -a - root, c1 = (v - r2 * d) / (r1 - r2), c2 = d - c1;
  const e1 = Math.exp(r1 * t), e2 = Math.exp(r2 * t);
  return [c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2];
}
// Seconds until it first samples at rest on the 240 Hz grid, at most ten.
function settle(config, d, v) {
  for (let n = 0; ; n++) {
    const t = n / 240;
    if (t >= 10) return 10;
    const [x, s] = spring(config, d, v, t);
    if (Math.abs(x) < 1e-3 && Math.abs(s) < 1e-3) return t;
  }
}

function createPresence(root) {
  // Every view that declared the row before this module arrived.
  const tracked = new Set([...root.querySelectorAll('[style*="--exact-layout-transition"]')]);
  // element -> { animation, parts, surface, d, v, config }: the move,
  // the animations that show its size, and the surface's stand-in.
  const flips = new WeakMap();
  const surfaces = new Set();
  let frame = null;
  let first = new Map();

  // Read the element's presented paint without our transparent surface or
  // layout offset. Paint animations keep their clocks: only their painted
  // values are hidden while the stand-in draws them. This includes CSS
  // transitions, which outrank the transparent Web Animation in the cascade.
  function reveal(f) {
    const sf = f.surface;
    if (!sf) return;
    sf.hide.effect.target = null;
    for (const [a, frames] of sf.hidden) a.effect.setKeyframes(frames);
    sf.hidden.clear();
  }

  function sync(placed = false) {
    for (const el of surfaces) {
      const f = flips.get(el), sf = f.surface;
      if (!el.isConnected || ['idle', 'finished'].includes(f.animation.playState)) { stop(el); continue; }
      reveal(f);
      f.animation.effect.target = null;
      const cs = getComputedStyle(el);
      for (const k of PAINT) sf.box.style[k] = cs[k];
      if (placed) {
        const place = measure(), [x, y] = place(el, el.parentElement), [ax, ay] = place(sf.stand, el.parentElement);
        sf.box.style.left = `${x - ax}px`; sf.box.style.top = `${y - ay}px`;
      }
      for (const a of el.getAnimations()) {
        if (f.parts.includes(a)) continue;
        const frames = a.effect.getKeyframes();
        if (!frames.some(frame => Object.keys(CLEAR).some(k => k in frame))) continue;
        sf.hidden.set(a, frames);
        a.effect.setKeyframes(frames.map(frame => Object.fromEntries(Object.entries(frame).map(([k, v]) => [k, CLEAR[k] ?? v]))));
      }
      f.animation.effect.target = el;
      sf.hide.effect.target = el;
    }
    if (surfaces.size && frame === null) frame = requestAnimationFrame(() => { frame = null; sync(); });
  }

  // What a running move still has to go, `[dx, dy, dw, dh]` from the laid-out
  // box, and how fast it goes there (a spring's velocity; an easing has none,
  // as CSS gives it none).
  function going(el) {
    const f = flips.get(el), still = [[0, 0, 0, 0], [0, 0, 0, 0]];
    if (!f || f.animation.playState === 'finished' || f.animation.playState === 'idle') return still;
    if (f.config) {
      const t = ((f.animation.currentTime ?? 0) - f.animation.effect.getTiming().delay) / 1000;
      if (t < 0) return [f.d, f.v];
      const at = f.d.map((d, i) => spring(f.config, d, f.v[i], t));
      return [at.map(s => s[0]), at.map(s => s[1])];
    }
    const p = f.animation.effect.getComputedTiming().progress ?? 1;
    return [f.d.map(d => d * (1 - p)), still[1]];
  }

  function stop(el) {
    const f = flips.get(el);
    if (!f) return;
    flips.delete(el);
    reveal(f);
    for (const a of [f.animation, ...f.parts]) a.cancel();
    f.surface?.stand.remove();
    surfaces.delete(el);
    if (!surfaces.size && frame !== null) { cancelAnimationFrame(frame); frame = null; }
  }

  // The element's surface, standing in for it behind it while its size
  // moves: a zero-size anchor before it (so the size never moves the
  // anchor, whatever its parent's alignment), and in it a box with the
  // element's background, border, radius, shadow and own transforms, placed
  // over the element's laid-out box. Behind its content in the parent's
  // stacking context, which the parent isolates for the while.
  function surface(el, place) {
    const cs = getComputedStyle(el), stand = document.createElement('div'), box = document.createElement('div');
    stand.setAttribute('data-exiting', ''); stand.setAttribute('aria-hidden', 'true'); stand.inert = true;
    stand.style.cssText = 'position:absolute;width:0;height:0;margin:0;padding:0;border:0;z-index:-1;pointer-events:none';
    el.before(stand);
    const [x, y] = place(el, el.parentElement), [ax, ay] = place(stand, el.parentElement);
    box.style.cssText = `position:absolute;left:${x - ax}px;top:${y - ay}px;box-sizing:border-box;margin:0`;
    for (const k of PAINT) box.style[k] = cs[k];
    stand.append(box);
    return { stand, box, hidden: new Map(), clips: cs.overflowX !== 'visible' || cs.overflowY !== 'visible', clipPath: cs.clipPath, radius: cs.borderRadius };
  }

  // Play `el` back from `d` (and velocity `v`) to its laid-out box `w` × `h`.
  function move(el, d, v, w, h, place) {
    const [ms, delay, ...rest] = el.style.getPropertyValue(LAYOUT).trim().split(' ');
    const easing = rest.join(' '), config = /^spring\((.*)\)$/.exec(easing)?.[1].split(',').map(Number);
    stop(el);
    // Each sample `d` from the laid-out box: its offset, and its size.
    let samples = [[d, 0], [[0, 0, 0, 0], 1]], duration = Number(ms);
    if (config) {
      duration = Math.max(...d.map((x, i) => x || v[i] ? settle(config, x, v[i]) : 0));
      if (duration <= 0) return;
      samples = Array.from({ length: Math.ceil(duration * 60) }, (_, i) => [d.map((x, j) => spring(config, x, v[j], i / 60)[0]), i / 60 / duration]);
      samples.push([[0, 0, 0, 0], 1]);
      duration *= 1000;
    }
    const timing = { duration, delay: Number(delay), easing: config ? 'linear' : easing, fill: 'backwards' };
    const frames = f => samples.map(([s, offset]) => ({ ...f(s), offset }));
    const moved = frames(([dx, dy]) => ({ translate: `${dx}px ${dy}px` })), parts = [];
    // Its stand-in copies the element's transforms before the move adds to them.
    const sf = samples.some(([s]) => s[2] || s[3]) ? surface(el, place) : null;
    const animation = el.animate(moved, { ...timing, composite: 'add' });
    if (sf) {
      const size = ([, , dw, dh]) => [Math.max(0, w + dw), Math.max(0, h + dh)];
      const hold = { ...timing, fill: 'both' };
      sf.hide = el.animate([CLEAR, CLEAR], hold);
      parts.push(
        sf.stand.animate(moved, timing),
        sf.box.animate(frames(s => { const [bw, bh] = size(s); return { width: `${bw}px`, height: `${bh}px` }; }), timing),
        sf.hide,
        el.parentElement.animate([0, 1].map(offset => ({ isolation: 'isolate', offset })), hold));
      // A box that clips its children clips them to the shown box.
      if (sf.clips && sf.clipPath === 'none') {
        parts.push(el.animate([{ overflow: 'visible' }, { overflow: 'visible' }], hold),
          el.animate(frames(s => { const [bw, bh] = size(s); return { clipPath: `inset(0 ${w - bw}px ${h - bh}px 0 round ${sf.radius})` }; }), timing));
      }
      surfaces.add(el);
    }
    const f = { animation, parts, surface: sf, d, v, config };
    flips.set(el, f);
    animation.finished.then(() => { if (flips.get(el) === f) stop(el); }, () => {});
  }

  return {
    sync,
    // Agent state reads the painted surface, including unnamed exit ghosts.
    // The ordinary layout box still describes the content's final geometry.
    observation() {
      return [...root.querySelectorAll('[data-view]')].filter(el =>
        el.style.getPropertyValue(LAYOUT) || el.style.getPropertyValue('--exact-exit-animation') || el.hasAttribute('data-exiting')
      ).map(el => {
        const shown = flips.get(el)?.surface?.box ?? el, r = shown.getBoundingClientRect();
        return { id: Number(el.dataset.view), x: r.x, y: r.y, w: r.width, h: r.height,
          opacity: Number(getComputedStyle(el).opacity), exiting: el.hasAttribute('data-exiting') };
      });
    },
    // Before a batch's ops: the place of every view that declares the row or
    // gains it here, with what it has still to go (a view that gains the row
    // moves from where it was, as a CSS transition gained with its change
    // runs), then the leaving views, before any op moves them.
    before(batch, views) {
      first = new Map();
      for (const el of surfaces) reveal(flips.get(el));
      if (batch.presenceSnap) { for (const el of tracked) stop(el); }
      else {
        const place = measure();
        const gains = (batch.ops ?? []).filter(op => op.op === 'style' && op.css?.includes(LAYOUT)).map(op => views.get(op.id));
        for (const el of new Set([...tracked, ...gains])) {
          if (!el?.isConnected) { tracked.delete(el); continue; }
          const at = place(el);
          if (at) first.set(el, [at, going(el)]);
        }
      }
      for (const op of batch.ops ?? []) if (op.op === 'exit') this.exit(views.get(op.id), op.css);
    },
    exit(el, css) {
      if (!el?.isConnected || !css) return;
      stop(el);
      const cs = getComputedStyle(el), s = el.style;
      const box = [el.offsetLeft - parseFloat(cs.marginLeft), el.offsetTop - parseFloat(cs.marginTop), el.offsetWidth, el.offsetHeight];
      // No input, no accessibility, no focus, no ids: a view re-created with
      // the same key while this one leaves is the only one with its names.
      el.setAttribute('data-exiting', '');
      el.setAttribute('aria-hidden', 'true');
      el.inert = true;
      for (const n of [el, ...el.querySelectorAll('[id]')]) n.removeAttribute('id');
      tracked.delete(el);
      // Its authored transitions do not animate it out of flow: the ghost
      // takes its box at once (and a transition still running ends there).
      s.transition = 'none';
      s.position = 'absolute'; s.left = `${box[0]}px`; s.top = `${box[1]}px`;
      s.width = `${box[2]}px`; s.height = `${box[3]}px`; s.boxSizing = 'border-box'; s.pointerEvents = 'none';
      // Above its old siblings, as native brings it to front: one that moves
      // (a transform) would otherwise paint over it in tree order.
      s.zIndex = '1';
      // After its own animations, which keep playing: a name it already
      // plays is a second entry, which starts now.
      const playing = new Set(el.getAnimations()), list = s.animation;
      s.animation = list && list !== 'none' ? `${list}, ${css}` : css;
      const leaving = el.getAnimations().filter(a => a.animationName !== undefined && !playing.has(a));
      const done = () => el.remove();
      Promise.all(leaving.map(a => a.finished)).then(done, done);
    },
    // Whether a destroyed view stays in the page: a leaving one and its subtree.
    keeps(el) { return el.closest('[data-exiting]') !== null; },
    // After a batch's ops: which views declare the row now, and every one
    // whose box moved plays back from where it was. All are measured before
    // any starts, so no move reads another's first frame.
    after(batch, views) {
      for (const op of batch.ops ?? []) {
        if (op.op !== 'create' && op.op !== 'style') continue;
        const el = views.get(op.id);
        if (!el) continue;
        if (el.style.getPropertyValue(LAYOUT)) tracked.add(el);
        else { tracked.delete(el); stop(el); }
      }
      const place = measure(), moves = [];
      for (const [el, [was, [d, v]]] of first) {
        if (!tracked.has(el)) continue;
        const at = place(el);
        if (at && at.some((x, i) => Math.abs(x - was[i]) >= 0.01)) moves.push([el, at.map((x, i) => was[i] + d[i] - x), v, at]);
      }
      for (const [el, d, v, at] of moves) move(el, d, v, at[2], at[3], measure());
      first = new Map();
      sync(true);
    },
  };
}

if (globalThis.exact) globalThis.exact.presence = createPresence;
