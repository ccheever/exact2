// The JS target's motion capability (LLP 1071 §7): springs, holds, swipes
// and a pan's release velocity. A chunk fetched after the first painted
// frame by a plan that uses motion, as the wasm host's motion piece is:
// the engine is `motion.wasm` (host/web-js/motion, the one `exact_motion`
// Engine), the page's half is the web host's own `motion-glue.js`, unchanged,
// and this file is the runner's half between them: the kernel's motion seam
// (each registered node's targets and `transition` at every commit, rt.js
// `mo`), the holds `motion-glue.js` asks for, and the actions they end in.
import { motionController } from './motion-glue.js';
import { transformDrags } from './transform.js';

export async function engine({ clock, wall, views, viewId, hooks, say }) {
  const bytes = globalThis.__files ? globalThis.__files('motion.wasm') : await fetch(new URL('motion.wasm', document.baseURI)).then(r => r.arrayBuffer());
  const made = await WebAssembly.instantiate(bytes, {}), w = (made.instance ?? made).exports;
  const enc = new TextEncoder();
  const put = text => { const b = enc.encode(text), p = w.m_in(b.length); new Uint8Array(w.memory.buffer, p, b.length).set(b); return b.length; };
  // The page's clock: the driver's under the agent, else the runtime's wall.
  const now = () => clock.agent ? clock.now : Math.max(clock.now, wall());
  const PROPS = ['translate', 'scale', 'rotate', 'opacity', 'height'];
  // The engine's lowered ops (Motion::lower's numbers) as the wasm host's
  // batch writes them: a spring's frames (`animate` with `at`, ms), or a cancel.
  const lower = () => {
    const n = w.m_lower(now() / 1000), f = new Float64Array(w.memory.buffer, w.m_out(), n), out = [];
    for (let i = 0; i < n;) {
      const [kind, id, p] = [f[i], f[i + 1], PROPS[f[i + 2]]];
      if (kind === 2) { out.push({ op: 'animate', id, property: p, delay: 0, duration: 0, values: [] }); i += 3; continue; }
      const k = f[i + 6], values = [];
      for (let j = 0; j < k; j++) values.push(p === 'translate' ? [f[i + 7 + 2 * j], f[i + 8 + 2 * j]] : f[i + 7 + 2 * j]);
      out.push({ op: 'animate', id, property: p, at: f[i + 3] * 1000, delay: f[i + 4] * 1000, duration: f[i + 5] * 1000, values });
      i += 7 + 2 * k;
    }
    return out;
  };
  // Each registered node's last facts: its transition text and targets, and
  // the authored inline style while motion-glue holds it (`authored`).
  const nodes = new Map(), authored = new Map(), holds = new Map();
  const held = view => [...holds.values()].includes(view);
  const ended = serial => { const view = holds.get(serial); holds.delete(serial); if (!held(view)) authored.delete(view); };
  // `[translate, scale, rotate, opacity, transition]` as the plan binds them:
  // translate is one or two pixel lengths (kernel `parse_translate`), rotate
  // degrees; a value the row refuses is the row's initial one.
  const num = (v, d) => { const n = typeof v === 'number' ? v : parseFloat(v); return Number.isFinite(n) ? n : d; };
  const observe = (id, [tr, scale, rotate, opacity, transition]) => {
    let n = nodes.get(id);
    if (!n) nodes.set(id, n = {});
    const [x, y = 0] = typeof tr === 'string' ? tr.trim().split(/\s+/).map(p => num(p, 0)) : [0, 0];
    if (n.transition !== transition) { n.transition = transition; if (!w.m_transitions(id, put(transition ?? '')) && transition) say(`motion: transition ${JSON.stringify(transition)} refused`); }
    if (!w.m_observe(id, x, y, num(scale, 1), num(rotate, 0), num(opacity, 1), now() / 1000)) say(`motion: targets for #${id} refused`);
  };
  const ops = list => ({ ops: list, timers: false });
  const applyBatch = batch => {
    for (const op of batch?.ops ?? []) {
      if (op.op === 'animate') api.animate(op);
      else if (op.op === 'retire-motion') api.retire(op.id, op.property, op.token, op.runtime);
    }
  };
  const serial = token => Number(token ?? 0);
  const replyEnd = (facts, mode) => {
    const r = w.m_end(serial(facts.token), facts.x ?? 0, facts.y ?? 0, mode, facts.now / 1000);
    if (r === 2) return { error: 'invalid release' };
    if (r !== 2) { ended(serial(facts.token)); T.ended(serial(facts.token)); }
    return { accepted: r === 1, batch: ops(lower()) };
  };
  // What `motion-glue.js` asks (`host/web/src/abi.rs` `motion`, answered here).
  const request = facts => {
    switch (facts.op) {
      case 'gesture': return { knee: w.m_gesture(0), resistance: w.m_gesture(1), edge: w.m_gesture(2), slop: w.m_gesture(3) };
      case 'begin': {
        const p = PROPS.indexOf(facts.property), el = views.get(facts.view);
        if (el && !authored.has(facts.view)) authored.set(facts.view, el.style.cssText);
        const s = w.m_begin(facts.view, p, facts.x, facts.y, facts.now / 1000);
        if (!s) { if (!held(facts.view)) authored.delete(facts.view); return { accepted: false }; }
        holds.set(s, facts.view);
        // Taking over cancels whatever played the property (Host::begin_hold).
        return { token: String(s), value: [w.m_scratch(0), w.m_scratch(1)], batch: ops([...lower(), { op: 'animate', id: facts.view, property: facts.property, delay: 0, duration: 0, values: [] }]) };
      }
      case 'move': return w.m_update(serial(facts.token), facts.x, facts.y, facts.now / 1000) ? { accepted: true, batch: ops(lower()) } : { accepted: false };
      case 'track': return { accepted: !!w.m_track(serial(facts.token), facts.x, facts.y, facts.now / 1000) };
      case 'live': return { accepted: w.m_held(serial(facts.token)) > 0 };
      case 'release': return replyEnd(facts, 0);
      case 'cancel': return replyEnd(facts, 1);
      case 'release-measured': return replyEnd(facts, 2);
      // A swipe past the knee: the held node's `swiperight`, while its hold
      // still owns the presentation (Host::dispatch_held).
      case 'action': {
        const node = w.m_held(serial(facts.token));
        if (!node || w.m_scratch(0) !== 0 || !(facts.now >= 0)) return { accepted: false };
        const el = views.get(node);
        if (!el?.$swipe) return { accepted: false };
        el.$swipe();
        return { accepted: true, committed: true, batch: ops(lower()) };
      }
      // A height drag (host/web/src/height_drag.rs): the handle's binding
      // names the owner it holds; its action runs at the height shown and
      // the engine's velocity over the heights shown (LLP 1057.001 §3).
      case 'height-begin': {
        const b = handles.get(facts.view);
        if (!b || b.target == null || b.target !== owner) return { accepted: false };
        const el = views.get(b.target);
        if (el && !authored.has(b.target)) authored.set(b.target, el.style.cssText);
        const s = w.m_begin(b.target, 4, facts.x, 0, facts.now / 1000);
        if (!s) { if (!held(b.target)) authored.delete(b.target); return { accepted: false }; }
        holds.set(s, b.target);
        return { token: String(s), target: b.target, value: [w.m_scratch(0), w.m_scratch(1)], batch: ops([...lower(), { op: 'animate', id: b.target, property: 'height', delay: 0, duration: 0, values: [] }]) };
      }
      case 'height-action': {
        const b = handles.get(facts.view), s = serial(facts.token);
        if (!b || w.m_held(s) !== b.target || w.m_scratch(0) !== 4 || !b.el.$heightrelease) return { accepted: false };
        if (!w.m_update(s, facts.x, 0, facts.now / 1000)) return { accepted: false };
        w.m_measured(s, facts.now / 1000);
        b.el.$heightrelease(facts.x, w.m_scratch(0));
        return { accepted: true, batch: ops(lower()) };
      }
      case 'height-owner': case 'clear-height-owner': return { accepted: false };
      case 'transform-geometry': case 'transform-invalidate': case 'transform-begin': case 'transform-move': case 'transform-action': return T.request(facts);
      case 'pan-sample': w.m_pan_sample(facts.view, facts.token ? 1 : 0, facts.x, facts.y, facts.now); return { accepted: true };
      case 'pan-velocity': w.m_pan_release(facts.view, facts.now); return { vx: w.m_scratch(0), vy: w.m_scratch(1) };
      default: return { error: `${facts.op} is not in the JS target's motion` };
    }
  };
  // Height drags: each handle and the owner its `heightDragFor` names, and
  // the one owner the engine follows (Host::reconcile_height_drags: the
  // first valid target, never stolen by a second).
  const handles = new Map(), heights = new Map();
  let owner = null;
  const eligible = el => el?.isConnected && !el.closest('[inert],[disabled]') && el.getClientRects().length > 0;
  function reconcileHeights() {
    const valid = t => heights.has(t) && typeof heights.get(t)[0] === 'number' && eligible(views.get(t)) && getComputedStyle(views.get(t)).boxSizing === 'border-box';
    const targets = [...handles.values()].map(b => b.target).filter(t => t != null && valid(t));
    const next = owner != null && targets.includes(owner) ? owner : targets[0] ?? null;
    if (next !== owner) {
      if (owner != null && w.m_unheight(owner)) api.retire(owner, 'height');
      owner = next;
      if (owner != null) observeHeight(owner, heights.get(owner));
    }
    for (const [id, b] of handles) {
      const target = b.target != null && b.target === owner && eligible(b.el) ? b.target : null;
      if (b.published !== target) { b.published = target; api.heightBinding({ id, target, handleKey: id, targetKey: target }); }
    }
  }
  function observeHeight(id, [h, transition] = []) {
    if (id !== owner || typeof h !== 'number') return;
    let n = nodes.get(id);
    if (!n) nodes.set(id, n = {});
    if (n.transition !== transition) { n.transition = transition; w.m_transitions(id, put(transition ?? '')); }
    w.m_height(id, h, now() / 1000);
  }
  const api = motionController({ views, now, generation: () => 0, request, applyBatch, inert: el => !!el.closest('[inert]') });
  const T = transformDrags({ w, views, viewId, api, lower, ops, now, authored, holds, held, eligible });
  // A dynamic style row on a held node goes to the authored text the hold
  // restores, as the wasm host's `style` op does (motion-glue `style`).
  hooks.style = (e, prop, v) => {
    const id = viewId(e);
    if (!authored.has(id)) return false;
    const s = document.createElement('div').style;
    s.cssText = authored.get(id);
    s.removeProperty(prop); if (v != null) s.setProperty(prop, v);
    authored.set(id, s.cssText);
    api.style(id, s.cssText);
    return true;
  };
  globalThis.exact.settleAt = () => { const t = w.m_settle(); return t < 0 ? null : t * 1000; };
  globalThis.exact.synced = () => api.followTimelines();
  return {
    api, request, now,
    observe,
    // After each commit's tree: lower what it changed, then retire what no
    // longer qualifies (glue.js `applyBatch`'s tail).
    flush() { reconcileHeights(); applyBatch(ops([...T.reconcile(), ...lower()])); api.commit(); },
    transformDrag(el, target, clip) { T.attach(el, target, clip); T.reconcile(); },
    height(id, v) { heights.set(id, v); observeHeight(id, v); },
    heightDrag(el, target) {
      const id = viewId(el);
      handles.set(id, { el, target: target ? viewId(target) : null, published: undefined });
      api.attachHeightDrag(el, id, (t, g) => el.addEventListener(t, g));
      reconcileHeights();
    },
    gone(id) { T.gone(id); if (handles.delete(id) | heights.delete(id)) reconcileHeights(); nodes.delete(id); authored.delete(id); for (const [k, v] of holds) if (v === id) holds.delete(k); w.m_remove(id); api.destroy(id); },
    swipe(el) { const id = viewId(el); api.attachSwipe(el, id, (t, g) => el.addEventListener(t, g)); },
    pan: { sample: (id, x, y, t, first) => w.m_pan_sample(id, first ? 1 : 0, x, y, t), velocity: (id, t) => (w.m_pan_release(id, t), [w.m_scratch(0), w.m_scratch(1)]) },
  };
}
