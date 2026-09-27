// Exit animation and layout transition on the web (LLP 1063). Loaded after
// paint, when a batch first carries either row; the browser plays both.
//
// The host writes each row as a custom property the page never inherits from
// (only the element's own declaration is read): `--exact-exit-animation` is
// the CSS `animation` list the node plays as it leaves, its `@keyframes`
// already in the page; `--exact-layout-transition` is `<ms> <delay ms>
// <easing>`. An `exit` op names the one view of a removed subtree that leaves
// (the kernel's choice); this module keeps it, out of flow at its last box,
// inert and unnamed, until its animations end. A view with a layout
// transition is measured before and after each batch; a move within its
// parent plays back from the old place as an additive `translate`, so it
// composes with the authored one and a moving parent carries it.
const EXIT = '--exact-exit-animation', LAYOUT = '--exact-layout-transition';

// Layout origin in the parent, transform-free (CSSOM offsets ignore every
// transform, the authored ones and this module's alike).
function origin(el) {
  let x = 0, y = 0;
  for (let n = el; n; n = n.offsetParent) { x += n.offsetLeft; y += n.offsetTop; }
  return [x, y];
}
function place(el) {
  const parent = el.parentElement;
  if (!parent || !el.isConnected) return null;
  const [x, y] = origin(el), [px, py] = origin(parent);
  return [x - px, y - py];
}

function createPresence(root) {
  // Every view that declared the row before this module arrived.
  const tracked = new Set([...root.querySelectorAll('[style*="--exact-layout-transition"]')]);
  const flips = new WeakMap(); // element -> { animation, dx, dy }
  let first = new Map();

  // What is still to travel of a running move: its offset times one minus
  // its eased progress (Web Animations' iteration progress is post-easing).
  function residual(el) {
    const f = flips.get(el);
    if (!f || f.animation.playState === 'finished' || f.animation.playState === 'idle') return [0, 0];
    const p = f.animation.effect.getComputedTiming().progress ?? 1;
    return [f.dx * (1 - p), f.dy * (1 - p)];
  }

  function move(el, dx, dy) {
    const [ms, delay, ...easing] = el.style.getPropertyValue(LAYOUT).trim().split(' ');
    flips.get(el)?.animation.cancel();
    const animation = el.animate([{ translate: `${dx}px ${dy}px` }, { translate: '0px 0px' }],
      { duration: Number(ms), delay: Number(delay), easing: easing.join(' '), fill: 'backwards', composite: 'add' });
    flips.set(el, { animation, dx, dy });
  }

  return {
    // Before a batch's ops: the leaving views' boxes (nothing has moved yet)
    // and every tracked view's place, with what it has still to travel.
    before(batch, views) {
      first = new Map();
      for (const el of tracked) {
        if (!el.isConnected) { tracked.delete(el); continue; }
        const at = place(el);
        if (at) first.set(el, [at, residual(el)]);
      }
      for (const op of batch.ops ?? []) if (op.op === 'exit') this.exit(views.get(op.id));
    },
    exit(el) {
      const css = el?.isConnected && el.style.getPropertyValue(EXIT);
      if (!css) return;
      const cs = getComputedStyle(el);
      const box = [el.offsetLeft - parseFloat(cs.marginLeft), el.offsetTop - parseFloat(cs.marginTop), el.offsetWidth, el.offsetHeight];
      // No input, no accessibility, no focus, no ids: a view re-created with
      // the same key while this one leaves is the only one with its names.
      el.setAttribute('data-exiting', '');
      el.setAttribute('aria-hidden', 'true');
      el.inert = true;
      for (const n of [el, ...el.querySelectorAll('[id]')]) n.removeAttribute('id');
      tracked.delete(el);
      const s = el.style;
      s.position = 'absolute'; s.left = `${box[0]}px`; s.top = `${box[1]}px`;
      s.width = `${box[2]}px`; s.height = `${box[3]}px`; s.boxSizing = 'border-box'; s.pointerEvents = 'none';
      // Above its old siblings, as native brings it to front: one that moves
      // (a transform) would otherwise paint over it in tree order.
      s.zIndex = '1';
      // `none` first: an exit naming the keyframes an entry played restarts.
      s.animation = 'none';
      void cs.animationName;
      s.animation = css;
      const leaving = el.getAnimations().filter(a => a.animationName !== undefined);
      const done = () => el.remove();
      Promise.all(leaving.map(a => a.finished)).then(done, done);
    },
    // Whether a destroyed view stays in the page: a leaving one and its subtree.
    keeps(el) { return el.closest('[data-exiting]') !== null; },
    // After a batch's ops: which views declare the row now, and every one
    // that moved within its parent plays back from where it was.
    after(batch, views) {
      for (const op of batch.ops ?? []) {
        if (op.op !== 'create' && op.op !== 'style') continue;
        const el = views.get(op.id);
        if (!el) continue;
        if (el.style.getPropertyValue(LAYOUT)) tracked.add(el);
        else { tracked.delete(el); flips.get(el)?.animation.cancel(); flips.delete(el); }
      }
      for (const [el, [was, [rx, ry]]] of first) {
        if (!tracked.has(el)) continue;
        const at = place(el);
        if (!at || (Math.abs(at[0] - was[0]) < 0.5 && Math.abs(at[1] - was[1]) < 0.5)) continue;
        move(el, was[0] + rx - at[0], was[1] + ry - at[1]);
      }
      first = new Map();
    },
  };
}

if (globalThis.exact) globalThis.exact.presence = createPresence;
