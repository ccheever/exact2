// Arrange, the reorder drag, on the JS target (LLP 1041 §8.5): the runner's
// half of `host/web/src/reorder_drag.rs`, answering the reorder packets of
// the web host's own `arrangeController` (motion-glue.js, unchanged) over
// the motion engine's holds and the collection's preview (list.js, the
// runner's `reorder.rs`). Identities, the token and the collection's exact
// geometry are checked before any value; no item key crosses from the page:
// the collection certifies the gap and names the keys the action receives.
import { arrangeController } from './motion-glue.js';
import { install } from './reorder.js';

export function arrangeDrags({ w, views, viewId, api, lower, ops, now, applyBatch, authored, holds, held, hooks, After }) {
  const RUNTIME = '1', handles = new Map(), stale = () => ({ accepted: false });
  let active = null, reorder = null;
  // The collection half, installed on list.js's collections at the first grip.
  const L = () => reorder ??= globalThis.exact?.lists?.internals ? install(globalThis.exact.lists, hooks, After) : null;
  const KEYS = ['handle', 'list', 'wrapper', 'root', 'rowEpoch'];
  const GEOMETRY = ['list', 'revision', 'scrollSequence', 'scrollTop', 'portWidth', 'portHeight', 'rowWidth', 'totalExtent'];
  const same = (a, b, keys) => !!a && !!b && keys.every(k => a[k] === b[k]);
  const bindingOf = f => ({ handle: Number(f.handleKey), list: Number(f.listKey), wrapper: Number(f.wrapperKey), root: Number(f.rootKey), rowEpoch: Number(f.rowEpoch) });
  const geometryOf = f => ({ list: Number(f.listKey), revision: Number(f.revision), scrollSequence: Number(f.scrollSequence), scrollTop: f.scrollTop,
    portWidth: f.portWidth, portHeight: f.portHeight, rowWidth: f.rowWidth, totalExtent: f.totalExtent });
  const live = serial => serial != null && w.m_held(serial) > 0;
  const hold = (view, value, t) => {
    const el = views.get(view);
    if (el && !authored.has(view)) authored.set(view, el.style.cssText);
    const s = w.m_begin(view, 0, value[0], value[1], t);
    if (!s) { if (!held(view)) authored.delete(view); return 0; }
    holds.set(s, view);
    return s;
  };
  // The collection's surviving mounted wrappers, as the wasm host's frame JSON.
  const frame = () => {
    const f = active && L()?.frame(active.token);
    return (f?.wrappers ?? []).map(r => ({ view: r.wrapper, key: String(r.wrapper), rootKey: String(r.root), top: r.top, offset: r.offset,
      hold: String(live(active.holds.get(r.wrapper)) ? active.holds.get(r.wrapper) : 0) }));
  };
  const reply = (certified, dispatched, list) => ({ accepted: true, certified, dispatched, runtime: RUNTIME, token: String(active.token),
    terminal: active.terminal, released: active.released, frame: frame(), batch: ops(list) });
  function request(f) {
    if (f.runtime !== RUNTIME || !L()) return stale();
    const op = f.op, b = bindingOf(f), g = geometryOf(f), token = Number(f.token ?? 0), t = f.now / 1000;
    if (op === 'reorder-begin') {
      if (token !== 0 || active || !same(L().binding(b.handle), b, KEYS) || !same(L().geometry(b.list), g, GEOMETRY)) return stale();
    } else {
      if (!active || !same(active.binding, b, KEYS) || active.token !== token) return stale();
      if ((op === 'reorder-preview' || op === 'reorder-terminal') && (active.terminal || !L().has(active.token)
        || !live(active.holds.get(b.wrapper)) || !same(L().geometry(b.list), g, GEOMETRY))) return stale();
      if (op === 'reorder-cancel' && (active.released || active.captured) || op === 'reorder-rebase' && (!active.terminal || active.released)
        || op === 'reorder-finish' && !active.released) return stale();
    }
    const rows = f.rows ?? [], pixel = v => Number.isFinite(v) && Math.abs(v) <= 3.4028234663852886e38;
    if (!(f.now >= 0) || (f.vx ?? 0) !== 0 || (f.vy ?? 0) !== 0) return { error: 'reorder velocity is measured; its slots must be zero' };
    if (![f.contentY ?? 0, f.x ?? 0, f.y ?? 0].every(pixel) || rows.some(r => !r.value?.every(pixel))) return { error: 'invalid reorder sample' };
    if (['reorder-begin', 'reorder-preview', 'reorder-finish'].includes(op) && rows.length) return { error: 'unexpected reorder samples' };
    if (new Set(rows.map(r => String(r.key))).size !== rows.length) return { error: 'duplicate reorder sample' };
    if (op === 'reorder-begin') {
      const started = L().begin(b, g);
      if (!started) return stale();
      const serial = hold(b.wrapper, [f.x ?? 0, f.y ?? 0], t);
      if (!serial) { L().cancel(started.token); L().finish(started.token); return { accepted: false, batch: ops(lower()) }; }
      active = { binding: b, token: started.token, holds: new Map([[b.wrapper, serial]]), terminal: false, captured: false, released: false, velocity: [0, 0] };
      return reply(true, false, [...lower(), { op: 'animate', id: b.wrapper, property: 'translate', delay: 0, duration: 0, values: [] }]);
    }
    if (op === 'reorder-finish') {
      const tk = active.token; active = null;
      L().finish(tk);
      return { accepted: true, batch: ops(lower()) };
    }
    if (op === 'reorder-rebase') {
      // Only surviving original holds may be rebased and released.
      const survivors = [...active.holds].filter(([key, s]) => views.get(key)?.isConnected && live(s));
      const sampled = new Map(rows.map(r => [Number(r.key), Number(r.hold)]));
      if (rows.length !== survivors.length || survivors.some(([k, s]) => sampled.get(k) !== s)) return { error: 'incomplete reorder rebase' };
      for (const r of rows) w.m_update(Number(r.hold), r.value[0], r.value[1], t);
      for (const [key, s] of survivors) {
        const [vx, vy] = key === active.binding.wrapper ? active.velocity : [0, 0];
        if (w.m_end(s, vx, vy, 0, t) === 1) { holds.delete(s); if (!held(key)) authored.delete(key); }
      }
      active.released = true;
      return reply(false, false, lower());
    }
    if (op === 'reorder-terminal' || op === 'reorder-cancel') {
      // A complete capture of the collection's wrappers, each with the hold
      // this owner has on it (0 for none).
      const wrappers = L().frame(active.token)?.wrappers ?? [], keys = new Set(rows.map(r => Number(r.key)));
      if (rows.length !== wrappers.length || wrappers.some(r => !keys.has(r.wrapper))) return { error: 'incomplete reorder capture' };
      for (const r of rows) if ((live(active.holds.get(Number(r.key))) ? active.holds.get(Number(r.key)) : 0) !== Number(r.hold)) return { error: 'stale reorder hold capture' };
    }
    const source = active.holds.get(b.wrapper);
    if (live(source)) w.m_update(source, f.x ?? 0, f.y ?? 0, t);
    if (op === 'reorder-cancel') return terminal(f, false, t);
    const progress = L().preview(active.token, g, f.contentY ?? 0);
    if (progress === 'stale') return stale();
    const certified = progress === 'accepted';
    if (op === 'reorder-terminal') return terminal(f, certified, t);
    return reply(certified, false, lower());
  }
  // The end of a contact (ReorderDrags::reorder_terminal): every wrapper not
  // yet held is held where it shows, the source's velocity is the engine's,
  // and a certified gap runs the list's `reorderdrop` with the keys the
  // collection names; otherwise the preview is cancelled.
  function terminal(f, certified, t) {
    const extra = [];
    for (const r of f.rows ?? []) {
      const view = Number(r.key);
      if (active.holds.has(view)) continue;
      const s = hold(view, r.value, t);
      if (!s) continue;
      active.holds.set(view, s);
      extra.push({ op: 'animate', id: view, property: 'translate', delay: 0, duration: 0, values: [] });
    }
    active.terminal = active.captured = true;
    let velocity = [0, 0];
    if (certified) { w.m_measured(active.holds.get(active.binding.wrapper), t); velocity = [w.m_scratch(0), w.m_scratch(1)].map(v => Number.isFinite(v) ? v : 0); }
    active.velocity = velocity;
    const dispatched = certified ? L().drop(active.token, geometryOf(f)) : (L().cancel(active.token), false);
    return reply(certified, !!dispatched, [...extra, ...lower()]);
  }
  const controller = arrangeController({ views, collections: new Proxy({}, { get: (_, k) => L()?.controller()?.[k] }), motion: api, request, applyBatch, now, generation: () => 0, inert: el => !!el.closest('[inert]') });
  return {
    request, controller,
    // After each commit: each handle's binding, published when it changed,
    // and the live preview's state (ReorderDrags::emit_reorder_drags).
    reconcile() {
      if (!L()) return;
      for (const [id, h] of handles) {
        // A virtualized row is built detached and inserted after: its grip
        // waits for the document, and leaves only by `gone` (stocks diary
        // #1: a grip registered while its row was detached was dropped here
        // and never heard of again, so no listener ever reached it).
        if (!h.el.isConnected) continue;
        const b = L().binding(id);
        if (same(b, h.published, KEYS) || !b && h.published === null) continue;
        h.published = b ?? null;
        controller.binding({ id, runtime: RUNTIME, handleKey: String(id), list: b?.list ?? null, listKey: b ? String(b.list) : null,
          wrapper: b?.wrapper ?? null, wrapperKey: b ? String(b.wrapper) : null, rootKey: b ? String(b.root) : null, rowEpoch: String(b?.rowEpoch ?? 0) });
      }
      if (active) {
        const f = L().frame(active.token);
        active.terminal ||= !f || f.terminal;
        controller.state({ runtime: RUNTIME, token: String(active.token), terminal: active.terminal, released: active.released, frame: frame() });
      }
      controller.commit();
    },
    handle(el) { handles.set(viewId(el), { el, published: undefined }); },
    gone(id) { handles.delete(id); controller.destroy(id); },
  };
}
