// Dropping across lists on the web (LLP 1094 D5–D9): a grip whose list has
// a `reorderGroup`, for `arrangeController` (motion-glue.js). The page draws
// the lifted row as a ghost in the top layer; the ghost's centre picks the
// grouped list it is over and the gap there (`reorder-preview-into`); the
// drop goes to that list and may hold until the move shows; then the ghost
// springs onto the row (or fades when the row is gone) and the session
// finishes. A grip with no `press`, `key`, `pan` or `pointerdown` of its own
// also takes the keys: Space lifts, the arrows step, Space or Enter drops,
// Escape cancels. The runner hides the row and certifies every gap; the
// page holds no item key. Both web targets load this file.
const SLOP = 8, BAND = 32, SPEED = 720;
// The ghost's look is the host's (D6): a shadow and a slight lift.
const SHADOW = '0 8px 24px rgba(0, 0, 0, 0.25)', LIFT = 1.03;
// What a clone of a row must not carry: it is no node of the app's (D6).
const STRIP = ['id', 'data-view', 'data-exact-on', 'data-agent-view', 'data-testid'];
const STEPS = { ArrowUp: 1, ArrowDown: 2, ArrowLeft: 3, ArrowRight: 4 };

/** The ghost: a deep clone of the row's wrapper, stripped of every identity,
 * in a manual popover (the top layer) where the row is now. Each element
 * carries its computed style inline, since the page's rules for the app's
 * root do not reach the top layer outside it. */
export function ghostOf(row, reduced) {
  const r = row.getBoundingClientRect(), clone = row.cloneNode(true), from = [row, ...row.querySelectorAll('*')];
  [clone, ...clone.querySelectorAll('*')].forEach((el, i) => {
    const cs = getComputedStyle(from[i]);
    el.style.cssText = Array.from(cs, p => `${p}:${cs.getPropertyValue(p)}`).join(';');
    for (const a of STRIP) el.removeAttribute(a);
    el.removeAttribute('class');
  });
  clone.style.translate = 'none'; clone.style.transition = 'none'; clone.style.visibility = 'visible';
  clone.style.position = 'static'; clone.style.width = '100%'; clone.style.height = '100%';
  const host = row.ownerDocument.createElement('div');
  host.setAttribute('popover', 'manual');
  host.dataset.exactGhost = '';
  Object.assign(host.style, { position: 'fixed', inset: 'auto', left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`,
    height: `${r.height}px`, margin: '0', padding: '0', border: 'none', background: 'transparent', overflow: 'visible',
    pointerEvents: 'none', boxShadow: SHADOW, scale: reduced ? '1' : String(LIFT), translate: '0px 0px' });
  host.append(clone);
  row.ownerDocument.body.append(host);
  host.showPopover?.();
  return { el: host, left: r.left, top: r.top, width: r.width, height: r.height, dx: 0, dy: 0 };
}

export function groupController({ views, collections, request, applyBatch, now, ready, inert, root, gripOf, viewOf = el => Number(el.dataset.view), log = () => {} }) {
  const doc = root?.ownerDocument ?? document;
  const reduced = () => !!globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  let cur = null, pending = null, pump = null, pumpTime = null;
  const lists = group => [...(root ?? doc).querySelectorAll('[data-reordergroup]')].filter(el => el.dataset.reordergroup === group && views.get(viewOf(el)) === el);
  const binding = b => ({ runtime: b.runtime, handleKey: b.handleKey, listKey: b.listKey, wrapperKey: b.wrapperKey, rootKey: b.rootKey, rowEpoch: b.rowEpoch });
  const geometry = m => ({ revision: m.revision, scrollSequence: m.scrollSequence, scrollTop: m.scrollTop, portWidth: m.portWidth,
    portHeight: m.portHeight, rowWidth: m.rowWidth, totalExtent: m.totalExtent, contentY: m.contentY });
  const call = (d, op, extra = {}) => {
    const r = request({ ...binding(d.b), op, token: d.token ?? 0, now: now(), ...extra });
    if (r?.batch) applyBatch(r.batch);
    return r ?? { accepted: false };
  };
  const centre = d => [d.ghost.left + d.ghost.dx + d.ghost.width / 2, d.ghost.top + d.ghost.dy + d.ghost.height / 2];
  const inside = (el, [x, y]) => { const r = el.getBoundingClientRect(); return x >= r.left && x < r.right && y >= r.top && y < r.bottom; };

  // The grouped list whose port holds the ghost's centre (D7), its mapping
  // at that centre; none over a header, a gutter or the quick-add.
  function over(d) {
    const c = centre(d);
    for (const el of lists(d.group)) {
      const m = collections.reorderMapping(viewOf(el), c[1]);
      if (m && inside(m.port, c)) return { view: viewOf(el), el, m };
    }
    return null;
  }
  // One sample: only a centre inside a grouped port moves the gap (D5).
  function sample(d) {
    if (cur !== d || d.phase !== 'active') return;
    const t = over(d);
    if (!t) return;
    const r = call(d, 'reorder-preview-into', { targetView: t.view, ...geometry(t.m) });
    if (r.accepted === true) d.target = r.target ?? d.target;
    else if (r.accepted === false && !r.error) cancel(d);
  }
  // Autoscroll is geometric (D7): the target's port, then each scroll
  // ancestor of the target whose box holds the centre, innermost first, the
  // first that can still move toward its edge band on its own axis.
  function scrollers(d) {
    const target = views.get(d.target), out = [];
    if (!target) return out;
    const m = collections.reorderMapping(d.target, centre(d)[1]);
    if (m?.port) out.push([m.port, 'y']);
    for (let el = (m?.port ?? target).parentElement; el && el !== doc.documentElement; el = el.parentElement) {
      const cs = getComputedStyle(el);
      for (const [axis, overflow, extent, client] of [['x', cs.overflowX, el.scrollWidth, el.clientWidth], ['y', cs.overflowY, el.scrollHeight, el.clientHeight]])
        if (/auto|scroll/.test(overflow) && extent > client && inside(el, centre(d))) out.push([el, axis]);
    }
    return out;
  }
  function band(d) {
    const c = centre(d);
    for (const [el, axis] of scrollers(d)) {
      const r = el.getBoundingClientRect(), at = axis === 'x' ? c[0] - r.left : c[1] - r.top, size = axis === 'x' ? el.clientWidth : el.clientHeight;
      const direction = at < BAND ? -1 : at > size - BAND ? 1 : 0;
      const pos = axis === 'x' ? el.scrollLeft : el.scrollTop, max = axis === 'x' ? el.scrollWidth - el.clientWidth : el.scrollHeight - el.clientHeight;
      if (direction && (direction < 0 ? pos > 0 : pos < max)) return { el, axis, direction };
    }
    return null;
  }
  function stopPump() { if (pump !== null) cancelAnimationFrame(pump); pump = null; pumpTime = null; }
  function edges(d) {
    if (cur !== d || d.phase !== 'active' || pump !== null || !band(d)) return;
    pump = requestAnimationFrame(at => {
      pump = null;
      if (cur !== d || d.phase !== 'active' || !ready()) { stopPump(); return; }
      const dt = pumpTime === null ? 0 : Math.min(32, Math.max(0, at - pumpTime)); pumpTime = at;
      const b = band(d);
      if (!b) { stopPump(); return; }
      if (dt) {
        const key = b.axis === 'x' ? 'scrollLeft' : 'scrollTop';
        b.el[key] += b.direction * SPEED * dt / 1000;
        sample(d); // the gap re-certifies after a scroll
      }
      edges(d);
    });
  }

  // The lift: past the slop in any direction (D6), the ghost made before
  // the begin's batch hides the row, the pin retained as the in-list drag's.
  function lift(b, e, g, v) {
    const m = collections.reorderMapping(b.list, v.clientY);
    const lease = m && collections.retainInteraction(b.el, e.pointerId);
    if (!m || !lease) return null;
    const ghost = ghostOf(b.row, reduced());
    const d = { b, group: g.group, pointer: e.pointerId, lease, ghost, grab: [e.clientX, e.clientY], phase: 'active', target: b.list, keys: false };
    cur = d;
    const r = call(d, 'reorder-begin', { ...geometry(m), flags: 1 });
    if (r.accepted !== true) { cur = null; ghost.el.remove(); collections.releaseRetainedInteraction(lease); return null; }
    d.token = r.token;
    b.el.setPointerCapture?.(e.pointerId);
    // The ghost follows the contact by where it went down on the grip (D6):
    // the slop it crossed before the lift is not lost.
    return d;
  }
  function move(d, v) {
    d.ghost.dx = v.clientX - d.grab[0]; d.ghost.dy = v.clientY - d.grab[1];
    d.ghost.el.style.translate = `${d.ghost.dx}px ${d.ghost.dy}px`;
    sample(d); edges(d);
  }
  function drop(d) {
    if (cur !== d || d.phase !== 'active') return;
    stopPump();
    const view = d.target ?? d.b.list, m = collections.reorderMapping(view, centre(d)[1]);
    const r = m ? call(d, 'reorder-terminal', { targetView: view, ...geometry(m) }) : call(d, 'reorder-cancel');
    after(d, r);
  }
  function cancel(d) {
    if (cur !== d || d.phase !== 'active') return;
    stopPump();
    after(d, call(d, 'reorder-cancel'));
  }
  // The answer to a drop or a cancel: a hold keeps the ghost where it is
  // (D8) until a batch's `reorder-state` says the hold ended.
  function after(d, r) {
    if (r?.phase === 'holding') { d.phase = 'holding'; return; }
    if (r?.phase === 'settling' || r?.phase === 'cancelling') land(d, r);
  }
  // The ending (D8): the ghost springs onto the row wherever it is (landed,
  // timeout, cancel), or fades (gone); then the session finishes, which
  // shows the row and releases the pin.
  function land(d, r) {
    if (cur !== d || d.phase === 'landing') return;
    d.phase = 'landing'; stopPump();
    const row = r.row != null ? views.get(r.row) : null, g = d.ghost;
    const done = () => finish(d, r);
    d.landNow = done; // a new drag ends the landing at once (LLP 1102 §3.18): the move has shown
    if (d.keys) { done(); return; }
    let animation;
    if (row && r.ending !== 'gone') {
      const to = row.getBoundingClientRect(), dx = to.left - g.left, dy = to.top - g.top;
      animation = g.el.animate([{ translate: `${g.dx}px ${g.dy}px`, scale: g.el.style.scale }, { translate: `${dx}px ${dy}px`, scale: '1' }],
        { duration: reduced() ? 0 : 250, easing: 'cubic-bezier(0.2, 0.9, 0.3, 1.05)', fill: 'forwards' });
    } else {
      animation = g.el.animate([{ opacity: 1 }, { opacity: 0 }], { duration: reduced() ? 0 : 200, fill: 'forwards' });
    }
    animation.finished.then(done, done);
  }
  function finish(d, r) {
    if (cur !== d) return;
    cur = null;
    d.ghost?.el.remove();
    for (const [el, name, fn] of d.listeners ?? []) el.removeEventListener(name, fn);
    const f = request({ ...binding(d.b), op: 'reorder-finish', token: d.token, now: now() });
    collections.releaseRetainedInteraction(d.lease);
    if (f?.batch) applyBatch(f.batch);
    // Focus follows the keys (D9): the moved row's grip where it landed,
    // else back to the grip it started from.
    if (!d.keys) return;
    const row = r.ending === 'landed' && r.row != null ? views.get(r.row) : null;
    // The landed row's grip may not be bound yet (its row is not measured):
    // it is found in the row, and made focusable as `bind` will make it.
    const grip = row && (gripOf(r.row) ?? row.querySelector('[data-reorderfor]')) || (d.b.el.isConnected ? d.b.el : null);
    if (grip && !grip.hasAttribute('tabindex')) grip.tabIndex = 0;
    grip?.focus({ preventScroll: true });
  }

  function swallowClick() {
    const stop = c => { c.preventDefault(); c.stopPropagation(); };
    doc.addEventListener('click', stop, { capture: true, once: true });
    setTimeout(() => doc.removeEventListener('click', stop, { capture: true }), 0);
  }
  // A drag refused while the last drop's session holds, said once a session (LLP 1102 §3.17):
  // a drive's reply reads like a success otherwise.
  let refusedFor = null;
  const refused = () => {
    if (refusedFor === cur) return;
    refusedFor = cur;
    log('reorder: a drag refused: the last drop is held until its move shows (LLP 1094 D8); a person waits for the card to land; a drive waits with `clock settle` before the next drag');
  };
  // The scroll container under the contact, short of the grip, where `touch-action` stops.
  const scroller = (el, grip) => {
    for (let n = el; n && n !== grip; n = n.parentElement) {
      const s = getComputedStyle(n);
      if (/(auto|scroll|hidden)/.test(s.overflowX + s.overflowY)) return n;
    }
    return null;
  };
  function down(b, e, g) {
    if (cur?.phase === 'landing') cur.landNow();
    if (cur && !inert(b.el)) refused();
    if (cur || inert(b.el)) return; // no lift while a session holds (D8)
    pending?.();
    collections.reorderContact(b.el, e.pointerId);
    const start = [e.clientX, e.clientY], events = [];
    let d = null;
    const on = (el, name, fn) => { el.addEventListener(name, fn); events.push([el, name, fn]); };
    const cleanup = () => { for (const [el, name, fn] of events.splice(0)) el.removeEventListener(name, fn); if (pending === cleanup) pending = null; if (!d) collections.releaseInteraction(e.pointerId); };
    pending = cleanup;
    on(doc, 'pointermove', v => {
      if (v.pointerId !== e.pointerId) return;
      if (!d) {
        if (Math.hypot(v.clientX - start[0], v.clientY - start[1]) < SLOP) return;
        d = lift(b, e, g, v);
        if (!d) { cleanup(); return; }
        d.listeners = events; pending = null;
      }
      v.preventDefault(); v.stopPropagation(); move(d, v);
    });
    // A child's capture loss while the grip takes capture is no cancel (habits F10).
    const up = v => {
      if (v.pointerId !== e.pointerId || v.type === 'lostpointercapture' && v.target !== b.el) return;
      if (!d) {
        // The browser took a finger before the lift (LLP 1102 §3.17): say why, as the pan glue does.
        if (v.type === 'pointercancel' && e.pointerType !== 'mouse') {
          const s = scroller(e.target, b.el), named = s?.dataset?.testid ? ` ("${s.dataset.testid}")` : '';
          log(`reorder: the browser took the ${e.pointerType || 'pointer'} contact on a grip to scroll before it lifted${s ? `: a scroll container under the grip${named} (overflow-x or overflow-y hidden, as an ellipsis title) ends touch-action there; give it touch-action="none", or start the grip off it` : '; give the grip touch-action="none"'}`);
        }
        cleanup(); return;
      }
      // A lifted drag is no press: the click its release makes is swallowed.
      if (v.type === 'pointerup') { swallowClick(); move(d, v); drop(d); } else cancel(d);
    };
    for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) on(doc, name, up);
    on(doc, 'keydown', k => { if (d && k.key === 'Escape' && d.phase === 'active') { k.preventDefault(); cancel(d); } });
    on(doc.defaultView ?? globalThis, 'blur', () => { if (d) cancel(d); });
  }

  // The keys (D9): one listener on the document while a key session lives,
  // since the row hides when another list is the target.
  function keydown(b, e) {
    if (e.target !== b.el || e.key !== ' ' || e.repeat) return;
    if (cur?.phase === 'landing') cur.landNow();
    if (cur) { refused(); return; }
    // The keys' contact takes the row's interaction pin as a finger's does;
    // the mapping reports it before the pin is retained.
    collections.reorderContact(b.el, -1);
    const r0 = b.el.getBoundingClientRect(), m = collections.reorderMapping(b.list, r0.top + r0.height / 2);
    const lease = m && collections.retainInteraction(b.el, -1);
    if (!m || !lease) { collections.releaseInteraction(-1); return; }
    e.preventDefault();
    const d = { b, group: b.group, lease, ghost: null, phase: 'active', target: b.list, keys: true, listeners: [] };
    cur = d;
    const r = call(d, 'reorder-begin', { ...geometry(m), flags: 2 });
    if (r.accepted !== true) { cur = null; collections.releaseRetainedInteraction(lease); return; }
    d.token = r.token;
    const key = k => {
      if (cur !== d || d.phase !== 'active' || k.repeat && !STEPS[k.key]) return;
      if (STEPS[k.key]) {
        k.preventDefault();
        const s = call(d, 'reorder-step', { flags: STEPS[k.key] });
        if (s.accepted === true) d.target = s.target ?? d.target;
      } else if (k.key === ' ' || k.key === 'Enter') {
        k.preventDefault();
        const t = views.get(d.target), m2 = t && collections.reorderMapping(d.target, 0);
        after(d, m2 ? call(d, 'reorder-terminal', { targetView: d.target, ...geometry(m2) }) : call(d, 'reorder-cancel'));
      } else if (k.key === 'Escape') { k.preventDefault(); after(d, call(d, 'reorder-cancel')); }
    };
    doc.addEventListener('keydown', key, true);
    d.listeners.push([doc, 'keydown', key]);
  }

  return {
    down,
    /** A grouped grip's group and key admission (D1, D9). */
    bind(b, g) {
      b.group = g.group;
      if (!g.group || !g.keys || b.keyed) return;
      b.keyed = e => keydown(b, e);
      if (!b.el.hasAttribute('tabindex')) { b.el.tabIndex = 0; b.tabbed = true; }
      if (!b.el.hasAttribute('role')) { b.el.setAttribute('role', 'button'); b.roled = true; }
      b.el.addEventListener('keydown', b.keyed);
    },
    release(b) {
      if (!b.keyed) return;
      b.el.removeEventListener('keydown', b.keyed); b.keyed = null;
      if (b.tabbed) b.el.removeAttribute('tabindex');
      if (b.roled) b.el.removeAttribute('role');
    },
    /** A batch's `reorder-state`: the hold's ending, or the runner's own
     * cancel (the grip went), reaches the ghost here, after the batch. */
    state(op) {
      const d = cur;
      if (!d || op.token !== String(d.token)) return;
      if (op.target != null) d.target = op.target;
      // At once, in the commit that ended the hold, so the agent's clock
      // registers the ghost's return with the commit's own animations.
      if (op.phase === 'settling' || op.phase === 'cancelling') land(d, op);
    },
    commit() { if (cur?.phase === 'active' && cur.ghost) edges(cur); },
    reset() {
      const d = cur;
      if (!d) return;
      stopPump();
      if (d.phase === 'active') cancel(d);
      if (cur === d) finish(d, {});
    },
  };
}

if (globalThis.exact) globalThis.exact.groupGlue = { groupController, ghostOf };
