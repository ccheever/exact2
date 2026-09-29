// Arrange's collection half on the JS target (LLP 1041 §8.5): the runner's
// `instance/collection/reorder.rs` (the preview: the source row's gap,
// certified by measured rows, each wrapper's absolute target) and
// `runner/reorder.rs` (a grip's binding, one owner, the list's `reorderdrop`
// with the keys the collection names), installed on list.js's collections
// by the motion piece when a plan has a reorder drag, so a list without one
// carries none of it.
export function install({ internals: { Collection, Lists, Views, find, minEpoch, settled, controller } }, hooks, After) {
  const num = n => { const f = Math.fround(n); if (Number.isInteger(f) && Math.abs(f) < 1e9) return String(f); for (let p = 1; p < 10; p++) { const s = Number(f.toPrecision(p)); if (Math.fround(s) === f) return String(s); } return String(f); };
  /** The measured gap nearest content offset `y`, the source row collapsed
   * (gaps.rs `certified_gap_excluding`); null while a row it rests on, or
   * a zero row beside it, lacks a current measurement. */
  function gap(ix, y, source) {
    if (!isFinite(y)) throw new Error("invalid reorder coordinate");
    if (!ix.len) return null;
    y = Math.max(0, Math.min(y, ix.total));
    const i = ix.rowAt(y);
    let raw = ix.len;
    if (i !== null) {
      if (!ix.measured(ix.order[i])) return null;
      raw = source === i ? i : y < ix.prefix(i) + ix.h[i] / 2 ? i : i + 1;
    }
    const top = ix.prefix(raw);
    let right = ix.rowAt(top) ?? ix.len, left = top > 0 ? find(ix.t, top, true) ?? 0 : 0;
    if (right === source) right = ix.rowAt(ix.prefix(source + 1)) ?? ix.len;
    if (left === source) { const before = ix.prefix(source); left = before > 0 ? find(ix.t, before, true) ?? 0 : 0; }
    return minEpoch(ix.t, left, Math.min(right + 1, ix.len)) >= ix.epoch ? right : null;
  }
  Object.assign(Collection.prototype, {
    reorderGeometry() {
      const g = this.geometry;
      return g && { list: this.view, revision: this.revision, scrollSequence: g.scroll_sequence, scrollTop: g.offset,
        portWidth: g.port_cross, portHeight: g.port_main, rowWidth: g.cross, totalExtent: this.index.total };
    },
    /** A grip's source binding: its mounted row, string-keyed and measured. */
    reorderBinding(handle) {
      const source = this.pin(handle), m = source && this.mounted.find(m => m.key === source);
      if (!m || !source.startsWith("s:") || !this.index.measured(source) || !(this.index.h[m.position] > 0)) return null;
      return { handle, list: this.view, wrapper: m.view, root: m.root, rowEpoch: m.epoch };
    },
    beginPreview(binding, token, handle) {
      const g = this.geometry;
      if (this.preview || !g || g.interaction_view !== handle || !(g.port_cross > 0 && g.port_main > 0 && g.cross > 0)) return false;
      const source = this.pin(handle);
      if (!source) return false;
      const p = this.index.pos.get(source);
      this.preview = { binding, token, source, before: this.index.order[p + 1] ?? null, height: this.index.h[p], terminal: false, pinOwned: true, handle, certified: null };
      this.emitPreview();
      return true;
    },
    movePreview(geometry, y) {
      const p = this.preview;
      p.certified = null;
      const at = gap(this.index, y, this.index.pos.get(p.source));
      if (at === null) return false;
      const top = this.index.prefix(at);
      if (top < geometry.scrollTop || top > geometry.scrollTop + geometry.portHeight) return false;
      if (at < this.index.len && !this.index.order[at].startsWith("s:")) return false;
      p.before = this.index.order[at] ?? null;
      p.certified = geometry;
      this.emitPreview();
      return true;
    },
    previewDrop(g) {
      const p = this.preview;
      if (!p || p.terminal || !p.certified || !GEOMETRY.every(k => p.certified[k] === g[k])) return null;
      if (!this.index.pos.has(p.source) || (p.before !== null && !this.index.pos.has(p.before))) return null;
      const drop = [p.source.slice(2), p.before === null ? null : p.before.slice(2)];
      this.endPreview();
      return drop;
    },
    endPreview() {
      if (!this.preview) return;
      this.preview.terminal = true; this.preview.certified = null;
      this.emitPreview();
    },
    losePreviewPin() { if (this.preview) { this.preview.pinOwned = false; this.endPreview(); } },
    finishPreview(token) {
      const p = this.preview;
      if (!p || p.token !== token || !p.terminal) return false;
      const release = p.pinOwned && this.geometry?.interaction_view === p.handle;
      this.preview = null;
      if (release) this.releasePins([false, true]);
      return true;
    },
    checkPreviewHeight() {
      const p = this.preview;
      if (p && !p.terminal && this.index.h[this.index.pos.get(p.source)] !== p.height) this.endPreview();
    },
    offsets() {
      const p = this.preview;
      if (!p || p.terminal) return null;
      const source = this.index.pos.get(p.source), before = p.before === null ? this.index.len : this.index.pos.get(p.before);
      if (source === undefined || before === undefined) return null;
      const shift = this.index.prefix(before) - (before > source ? p.height : 0) - this.index.prefix(source);
      return r => r === source ? shift : before <= r && r < source ? p.height : source < r && r < before ? -p.height : 0;
    },
    previewFrame(token) {
      const p = this.preview;
      if (!p || p.token !== token) return null;
      const at = this.offsets();
      return { terminal: p.terminal, wrappers: this.mounted.map(m => ({ wrapper: m.view, root: m.root, top: this.index.prefix(m.position), offset: at ? at(m.position) : 0 })) };
    },
    /** Each mounted wrapper's absolute target (views.rs's style op): the motion
     * engine springs it there (`translate spring(300,30,1)`). */
    emitPreview() {
      if (!this.preview) return;
      const at = this.offsets();
      for (const m of this.mounted) {
        const offset = at ? at(m.position) : 0;
        if (m.previewTarget === offset) continue;
        m.previewTarget = offset;
        const translate = `0px ${num(offset)}px`;
        if (!hooks.style?.(m.wrapper, "translate", translate)) m.wrapper.style.translate = translate;
        hooks.observe?.(m.view, [translate, 1, 0, 1, "translate spring(300,30,1)"]);
      }
    },
  });
  const GEOMETRY = ["list", "revision", "scrollSequence", "scrollTop", "portWidth", "portHeight", "rowWidth", "totalExtent"];
  let ReorderOwner = null, ReorderSerial = 0;
  /** A grip's binding: its `reorderFor` list (the strict ancestor the
   * compiler resolved) with a `reorderdrop`, every box on the way live. */
  function reorderBinding(handle) {
    const el = Views.get(handle), list = el?.$reorderList, c = list?.$list;
    if (!c || !list.$reorderdrop || !el.isConnected || el.closest("[inert],[disabled]") || !el.getClientRects().length) return null;
    return c.reorderBinding(handle);
  }
  const sameBinding = (a, b) => !!a && !!b && ["handle", "list", "wrapper", "root", "rowEpoch"].every(k => a[k] === b[k]);
  const sameGeometry = (a, b) => !!a && !!b && GEOMETRY.every(k => a[k] === b[k]);
  const hasReorder = token => { const p = ReorderOwner?.preview; return !!p && p.token === token && !p.terminal && p.pinOwned && sameBinding(reorderBinding(p.binding.handle), p.binding); };
  const Reorder = {
    binding: reorderBinding,
    geometry: view => Lists.get(view)?.reorderGeometry() ?? null,
    begin(binding, geometry) {
      const c = Lists.get(binding.list);
      if (ReorderOwner || !c || !sameBinding(reorderBinding(binding.handle), binding) || !sameGeometry(c.reorderGeometry(), geometry)) return null;
      const token = ++ReorderSerial;
      if (!c.beginPreview(binding, token, binding.handle)) return null;
      ReorderOwner = c;
      return { token };
    },
    has: hasReorder,
    preview(token, geometry, y) {
      if (!hasReorder(token) || !sameGeometry(ReorderOwner.reorderGeometry(), geometry)) return "stale";
      return ReorderOwner.movePreview(geometry, y) ? "accepted" : "needs";
    },
    /** The list's `reorderdrop` with the item's key and the one it now goes before. */
    drop(token, geometry) {
      if (!hasReorder(token) || !sameGeometry(ReorderOwner.reorderGeometry(), geometry)) return false;
      const drop = ReorderOwner.previewDrop(geometry);
      if (!drop) return false;
      ReorderOwner.el.$reorderdrop(drop[0], drop[1]);
      return true;
    },
    cancel(token) { const p = ReorderOwner?.preview; if (p && p.token === token && !p.terminal && p.pinOwned) ReorderOwner.endPreview(); },
    frame: token => ReorderOwner?.previewFrame(token) ?? null,
    finish(token) {
      if (!ReorderOwner) return false;
      if (!ReorderOwner.finishPreview(token)) return false;
      ReorderOwner = null;
      settled();
      return true;
    },
    controller: () => controller(),
  };
  // A preview whose binding is gone ends (runner reconcile_reorder).
  After.push(() => {
    if (ReorderOwner && !Lists.has(ReorderOwner.view)) { ReorderOwner = null; return; }
    const p = ReorderOwner?.preview;
    if (p && !p.terminal && p.pinOwned && !hasReorder(p.token)) ReorderOwner.endPreview();
  });
  return Reorder;
}
