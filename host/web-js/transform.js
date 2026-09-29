// The photo pair's transform drag on the JS target (LLP 1041 §8.5, LLP
// 1057.001 §4): the runner's half of `host/web/src/transform_drag.rs`,
// answering `motion-glue.js`'s transform packets (`transform-geometry`,
// `-invalidate`, `-begin`, `-move`, `-action`) over the motion engine's pair
// holds. Handles are bound to the one owner target (the first valid, never
// stolen); a geometry sequence orders the page's observations; stale
// identities, sequences and tokens are refused before any value is used.
export function transformDrags({ w, views, viewId, api, lower, ops, now, authored, holds, held, eligible, inflight }) {
  const RUNTIME = '1', handles = new Map(), pairs = new Map();
  let owner = null;
  const stale = () => ({ accepted: false });
  const binding = b => b.target != null && b.target === owner && b.el.$tgeom && b.el.$trelease
    && eligible(b.el) && eligible(views.get(b.target)) && eligible(views.get(b.clip)) ? b : null;
  // Independent original-token cleanup (Host::retire_transform_pair): a
  // replacement is never ended.
  function retire(serial, out) {
    const p = pairs.get(serial); pairs.delete(serial);
    for (const [s, property] of [[serial, 'translate'], [p.scale, 'scale']]) {
      if (w.m_held(s)) w.m_end(s, 0, 0, 1, now() / 1000);
      holds.delete(s);
      out.push({ op: 'retire-motion', id: p.target, property, runtime: RUNTIME, token: String(s) });
    }
    if (!held(p.target)) authored.delete(p.target);
  }
  // The owner, and the pairs no longer valid retired (Host::reconcile_transform_drags).
  function pairsValid() {
    const valid = [...handles.values()].filter(b => b.target != null && b.el.$tgeom && b.el.$trelease && eligible(b.el) && eligible(views.get(b.target)) && eligible(views.get(b.clip)));
    if (!valid.some(b => b.target === owner)) owner = valid[0]?.target ?? null;
    const out = [];
    for (const [serial, p] of [...pairs]) {
      const b = handles.get(p.handle);
      const live = [serial, p.scale].map(s => w.m_held(s) > 0);
      if (!b || !binding(b) || !b.geometry.ready || b.geometry.sequence !== p.sequence || !(p.ending ? live.some(Boolean) : live.every(Boolean))) retire(serial, out);
    }
    return out;
  }
  // After a commit: that, then each handle's binding published when it
  // changed (Host::emit_transform_drags).
  function reconcile() {
    const out = pairsValid();
    for (const [id, b] of handles) {
      const bound = binding(b) ? b.target : null;
      if (b.published === bound) continue;
      b.published = bound; b.geometry = { sequence: b.geometry.sequence, dims: null, ready: false };
      // The page reports a new binding's geometry in a frame after it: in
      // flight until it does (or three frames pass), so `clock settle` waits.
      if (bound != null && !b.awaiting) {
        b.awaiting = true; inflight.n++;
        b.reported = () => { if (b.awaiting) { b.awaiting = false; inflight.n--; } };
        requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => b.reported())));
      }
      api.transformBinding({ id, runtime: RUNTIME, handleKey: String(id), target: bound, targetKey: bound == null ? null : String(bound),
        clip: bound == null ? null : b.clip, clipKey: bound == null ? null : String(b.clip) });
    }
    return out;
  }
  function request(f) {
    const id = Number(f.handleKey), b = handles.get(id), sequence = Number(f.geometrySequence), v = f.values, t = f.now / 1000;
    if (f.runtime !== RUNTIME || !b) return stale();
    if (!binding(b) || String(b.target) !== f.targetKey || String(b.clip) !== f.clipKey) return { accepted: false, batch: ops([...pairsValid(), ...lower()]) };
    if (!sequence || sequence < b.geometry.sequence) return stale();
    if (['transform-begin', 'transform-move', 'transform-action'].includes(f.op) && (sequence !== b.geometry.sequence || !b.geometry.ready)) return stale();
    if (!Array.isArray(v) || v.length !== 6 || !v.every(Number.isFinite) || !(f.now >= 0)) return { error: 'invalid transform values' };
    if (f.op === 'transform-geometry' || f.op === 'transform-invalidate') {
      b.reported?.();
      const geometry = f.op === 'transform-geometry', dims = v.slice(0, 4), prev = b.geometry;
      if (sequence === prev.sequence) {
        if (geometry && prev.dims?.every((d, i) => d === dims[i]) && prev.ready === dims.every(d => d > 0) || !geometry && !prev.ready) return { accepted: true, batch: ops([]) };
        return { error: 'geometry sequence cannot change its facts' };
      }
      const changed = geometry && !prev.dims?.every((d, i) => d === dims[i]);
      b.geometry = { sequence, ready: geometry && dims.every(d => d > 0), dims: geometry ? dims : prev.dims };
      // A mapping change ends the old pair before the feedback runs.
      const out = [];
      for (const [serial, p] of [...pairs]) if (p.target === b.target) retire(serial, out);
      if (changed) b.el.$tgeom(...dims);
      return { accepted: true, batch: ops([...out, ...lower()]) };
    }
    if (f.op === 'transform-begin') {
      const el = views.get(b.target);
      if (el && !authored.has(b.target)) authored.set(b.target, el.style.cssText);
      const serial = w.m_pair_begin(b.target, v[0], v[1], v[2], t);
      if (!serial) { if (!held(b.target)) authored.delete(b.target); return stale(); }
      const scale = w.m_scratch(3), value = [w.m_scratch(0), w.m_scratch(1), w.m_scratch(2)];
      pairs.set(serial, { handle: id, target: b.target, sequence, scale, fired: false, ending: false });
      holds.set(serial, b.target); holds.set(scale, b.target);
      return { accepted: true, runtime: RUNTIME, geometrySequence: String(sequence), translateToken: String(serial), scaleToken: String(scale), value,
        batch: ops([...lower(), ...['translate', 'scale'].map(property => ({ op: 'animate', id: b.target, property, delay: 0, duration: 0, values: [] }))]) };
    }
    const serial = Number(f.translateToken), p = pairs.get(serial);
    if (!p || p.handle !== id || p.sequence !== sequence || String(p.scale) !== f.scaleToken || p.fired || p.ending) return stale();
    if (!w.m_pair_update(serial, v[0], v[1], v[2], t)) return { accepted: false, batch: ops([...pairsValid(), ...lower()]) };
    if (f.op === 'transform-move') return { accepted: true, batch: ops(lower()) };
    // The release: the engine's velocities over every value the pair was
    // given (LLP 1057.001 §3), the action run while both still hold.
    p.fired = true;
    w.m_measured(serial, t); const [vx, vy] = [w.m_scratch(0), w.m_scratch(1)];
    w.m_measured(p.scale, t); const vs = w.m_scratch(0);
    const velocity = [vx, vy, vs].every(Number.isFinite) ? [vx, vy, vs] : [0, 0, 0];
    b.el.$trelease(v[0], v[1], v[2], ...velocity);
    return { accepted: true, dispatched: true, committed: true, velocity, batch: ops(lower()) };
  }
  return {
    request, reconcile,
    // A pair's member ended through motion-glue's generic end.
    ended(serial) {
      for (const [key, p] of pairs) if (key === serial || p.scale === serial) {
        p.ending = true;
        if (!w.m_held(key) && !w.m_held(p.scale)) pairs.delete(key);
      }
    },
    attach(el, target, clip) {
      const id = viewId(el);
      handles.set(id, { el, target: target ? viewId(target) : null, clip: clip ? viewId(clip) : null, geometry: { sequence: 0, dims: null, ready: false }, published: undefined });
      api.attachTransformDrag(el, id, (t, g) => el.addEventListener(t, g));
    },
    gone(id) { handles.delete(id); },
  };
}
