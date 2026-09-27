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
// measured before and after each batch; a move or resize within the box it
// is placed in plays back from where it was as an additive `translate` and
// `scale` from its top-left corner, so it composes with the authored ones and
// a moving parent carries it. A spring is lowered here, per move, from its
// displacement and velocity in points, on the grid and rest threshold the
// engine settles on natively (`exact_motion::spring`).
const LAYOUT = '--exact-layout-transition';

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

// Where layout put an element in the box it is placed in, `[x, y, w, h]` in
// that box's own points: sub-pixel, and free of every transform. Its own
// (the authored ones and this module's) come off its bounding rect through
// its computed style; the scale its ancestors draw it at is divided out. A
// windowed list's row wrapper (it carries `data-listitemkey`) only positions its
// row, so a row's root is placed in the list content. Null for a box CSS
// gives no single size (an inline box).
function place(el) {
  let parent = el.parentElement;
  if (parent?.hasAttribute('data-listitemkey')) parent = parent.parentElement;
  if (!parent || !el.isConnected) return null;
  const cs = getComputedStyle(el), r = el.getBoundingClientRect(), p = parent.getBoundingClientRect();
  const edges = (a, b) => ['padding', 'border'].reduce((n, e) => n + parseFloat(cs[`${e}${a}${e === 'border' ? 'Width' : ''}`]) + parseFloat(cs[`${e}${b}${e === 'border' ? 'Width' : ''}`]), 0);
  let w = parseFloat(cs.width), h = parseFloat(cs.height);
  if (!(w >= 0 && h >= 0)) return null;
  if (cs.boxSizing !== 'border-box') { w += edges('Left', 'Right'); h += edges('Top', 'Bottom'); }
  const m = own(cs, w, h), corners = [[0, 0], [w, 0], [0, h], [w, h]].map(([x, y]) => m.transformPoint({ x, y }));
  const [minX, maxX, minY, maxY] = ['x', 'y'].flatMap(k => [Math.min(...corners.map(c => c[k])), Math.max(...corners.map(c => c[k]))]);
  const kx = maxX > minX ? r.width / (maxX - minX) : 1, ky = maxY > minY ? r.height / (maxY - minY) : 1;
  return [(r.left - p.left) / kx - minX + parent.scrollLeft, (r.top - p.top) / ky - minY + parent.scrollTop, w, h];
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
  const flips = new WeakMap(); // element -> { animation, d, v, config, start }
  let first = new Map();

  // What a running move still has to go, `[dx, dy, dw, dh]` from the laid-out
  // box, and how fast it goes there (a spring's velocity; an easing has none,
  // as CSS gives it none).
  function going(el) {
    const f = flips.get(el), still = [[0, 0, 0, 0], [0, 0, 0, 0]];
    if (!f || f.animation.playState === 'finished' || f.animation.playState === 'idle') return still;
    if (f.config) {
      const t = (document.timeline.currentTime - f.start) / 1000;
      if (t < 0) return [f.d, f.v];
      const at = f.d.map((d, i) => spring(f.config, d, f.v[i], t));
      return [at.map(s => s[0]), at.map(s => s[1])];
    }
    const p = f.animation.effect.getComputedTiming().progress ?? 1;
    return [f.d.map(d => d * (1 - p)), still[1]];
  }

  // Play `el` back from `d` (and velocity `v`) to its laid-out box `w` × `h`.
  function move(el, d, v, w, h) {
    const [ms, delay, ...rest] = el.style.getPropertyValue(LAYOUT).trim().split(' ');
    const easing = rest.join(' '), config = /^spring\((.*)\)$/.exec(easing)?.[1].split(',').map(Number);
    // A box `d` from its place, as a translate and scale about the element's
    // origin: scaling from its top-left corner is scaling about the origin
    // and moving the origin with it.
    const [ox, oy] = getComputedStyle(el).transformOrigin.split(' ').map(parseFloat);
    const frame = ([dx, dy, dw, dh]) => {
      const sx = w > 0 ? (w + dw) / w : 1, sy = h > 0 ? (h + dh) / h : 1;
      return { translate: `${dx + (sx - 1) * ox}px ${dy + (sy - 1) * oy}px`, scale: `${sx} ${sy}` };
    };
    flips.get(el)?.animation.cancel();
    let frames = [frame(d), frame([0, 0, 0, 0])], duration = Number(ms);
    if (config) {
      duration = Math.max(...d.map((x, i) => x || v[i] ? settle(config, x, v[i]) : 0));
      if (duration <= 0) { flips.delete(el); return; }
      const n = Math.ceil(duration * 60);
      frames = Array.from({ length: n }, (_, i) => ({ ...frame(d.map((x, j) => spring(config, x, v[j], i / 60)[0])), offset: i / 60 / duration }));
      frames.push({ ...frame([0, 0, 0, 0]), offset: 1 });
      duration *= 1000;
    }
    const animation = el.animate(frames, { duration, delay: Number(delay), easing: config ? 'linear' : easing, fill: 'backwards', composite: 'add' });
    flips.set(el, { animation, d, v, config, start: document.timeline.currentTime + Number(delay) });
  }

  return {
    // Before a batch's ops: the leaving views' boxes (nothing has moved yet)
    // and the place of every view that declares the row or gains it here,
    // with what it has still to go: a view that gains the row moves from
    // where it was, as a CSS transition gained with its change runs.
    before(batch, views) {
      first = new Map();
      const gains = (batch.ops ?? []).filter(op => op.op === 'style' && op.css?.includes(LAYOUT)).map(op => views.get(op.id));
      for (const el of new Set([...tracked, ...gains])) {
        if (!el?.isConnected) { tracked.delete(el); continue; }
        const at = place(el);
        if (at) first.set(el, [at, going(el)]);
      }
      for (const op of batch.ops ?? []) if (op.op === 'exit') this.exit(views.get(op.id), op.css);
    },
    exit(el, css) {
      if (!el?.isConnected || !css) return;
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
    // whose box moved plays back from where it was.
    after(batch, views) {
      for (const op of batch.ops ?? []) {
        if (op.op !== 'create' && op.op !== 'style') continue;
        const el = views.get(op.id);
        if (!el) continue;
        if (el.style.getPropertyValue(LAYOUT)) tracked.add(el);
        else { tracked.delete(el); flips.get(el)?.animation.cancel(); flips.delete(el); }
      }
      for (const [el, [was, [d, v]]] of first) {
        if (!tracked.has(el)) continue;
        const at = place(el);
        if (!at || at.every((x, i) => Math.abs(x - was[i]) < 0.01)) continue;
        move(el, at.map((x, i) => was[i] + d[i] - x), v, at[2], at[3]);
      }
      first = new Map();
    },
  };
}

if (globalThis.exact) globalThis.exact.presence = createPresence;
