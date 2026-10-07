// Arrange's collection half on the JS target (LLP 1041 §8.5): the runner's
// `instance/collection/reorder.rs` (the preview: the source row's gap,
// certified by measured rows, each wrapper's absolute target) and
// `runner/reorder.rs` (a grip's binding, one owner, the list's `reorderdrop`
// with the keys the collection names), installed on list.js's collections
// by the motion piece when a plan has a reorder drag, so a list without one
// carries none of it.
import { clock, every, commit, journal } from "./rt.js";

// The runner's journal line (rt.js `say`).
const say = line => journal.push(`t=${clock.now} ${line}`);

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
      this.preview = { binding, token, source, before: this.index.order[p + 1] ?? null, height: this.index.h[p], terminal: false, pinOwned: true, handle, certified: null,
        outgoing: false, holding: false, stepped: false, grouped: false };
      this.emitPreview();
      return true;
    },
    movePreview(geometry, y) {
      const p = this.preview;
      p.certified = null; p.stepped = false;
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
      if (!p || p.terminal || p.holding || !(p.stepped || p.certified && GEOMETRY.every(k => p.certified[k] === g[k]))) return null;
      if (!this.index.pos.has(p.source) || (p.before !== null && !this.index.pos.has(p.before))) return null;
      const drop = [p.source.slice(2), p.before === null ? null : p.before.slice(2)];
      this.endPreview();
      return drop;
    },
    // A hold is ended only by its own endings (LLP 1094 D8, `endHold`).
    endPreview() {
      if (!this.preview) return;
      if (!this.preview.holding) { this.preview.terminal = true; this.preview.certified = null; }
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
    /** A source row whose height changes ends the preview; a remeasure of
     * the translated row that differs by float noise is not a change
     * (`HEIGHT_NOISE`, runner `check_preview_height`). */
    checkPreviewHeight() {
      const p = this.preview;
      if (p && !p.terminal && !p.holding && !(Math.abs(this.index.h[this.index.pos.get(p.source)] - p.height) <= HEIGHT_NOISE)) this.endPreview();
    },
    // The source's own preview (`Outgoing` when another list is the
    // target: the row keeps its slot and the rows after it close the gap),
    // else a target's `Incoming` (reorder_group.rs; LLP 1094 D4).
    offsets() {
      const p = this.preview;
      if (p && !p.terminal) {
        const source = this.index.pos.get(p.source);
        if (source === undefined) return null;
        if (p.outgoing) return r => r > source ? -p.height : 0;
        const before = p.before === null ? this.index.len : this.index.pos.get(p.before);
        if (before === undefined) return null;
        const shift = this.index.prefix(before) - (before > source ? p.height : 0) - this.index.prefix(source);
        return r => r === source ? shift : before <= r && r < source ? p.height : source < r && r < before ? -p.height : 0;
      }
      const inc = this.incoming;
      if (!inc || inc.closed || inc.before === undefined || this.index.pos.has(inc.item)) return null;
      const at = inc.before === null ? this.index.len : this.index.pos.get(inc.before) ?? this.index.len;
      return r => r >= at ? inc.extent : 0;
    },
    previewFrame(token) {
      const p = this.preview;
      if (!p || p.token !== token) return null;
      const at = this.offsets();
      return { terminal: p.terminal, wrappers: this.mounted.map(m => ({ wrapper: m.view, root: m.root, top: this.index.prefix(m.position), offset: at ? at(m.position) : 0 })) };
    },
    /** Each mounted wrapper's absolute target (views.rs's style op): the motion
     * engine springs it there (`translate -exact-spring(300,30,1)`). */
    // Offsets that move with their rows in one commit apply at once (LLP
    // 1094 D8); the dragged row hides while a ghost stands for it (D6).
    emitPreview() {
      if (!this.preview && !this.incoming && this.hidden == null && !this.mounted.some(m => m.previewHidden)) return;
      const at = this.offsets(), hidden = this.hidden == null ? undefined : this.index.pos.get(this.hidden);
      const transition = this.instant ? "" : "translate -exact-spring(300,30,1)";
      for (const m of this.mounted) {
        const offset = at ? at(m.position) : 0;
        if (m.previewTarget !== offset) {
          m.previewTarget = offset;
          const translate = `0px ${num(offset)}px`;
          if (!hooks.style?.(m.wrapper, "translate", translate)) m.wrapper.style.translate = translate;
          hooks.observe?.(m.view, [translate, 1, 0, 1, transition]);
        }
        const hide = hidden === m.position;
        if (!!m.previewHidden !== hide) {
          m.previewHidden = hide;
          const v = hide ? "hidden" : "visible";
          if (!hooks.style?.(m.wrapper, "visibility", v)) m.wrapper.style.visibility = v;
        }
      }
    },
    // ---------------------------------------------- a grouped list's half (reorder_group.rs)
    preparedMove() { if (this.incoming && !this.incoming.holding) { this.incoming.certified = null; this.incoming.stepped = false; } },
    previewItem() { const p = this.preview; return p && this.index.pos.has(p.source) ? p.source : null; },
    setOutgoing(on) { const p = this.preview; if (p && p.outgoing !== on) { p.outgoing = on; p.certified = null; p.stepped = false; } this.emitPreview(); },
    openIncoming(item, extent) { this.incoming = { item, extent, before: undefined, certified: null, stepped: false, holding: false, closed: false }; },
    closeIncoming(instant) {
      if (!this.incoming) return;
      this.incoming.closed = true;
      const was = this.instant; this.instant = instant; this.emitPreview(); this.instant = was;
      this.incoming = null;
    },
    uncertify() { for (const x of [this.preview, this.incoming]) if (x) { x.certified = null; x.stepped = false; } },
    retargetHome(geometry, y) { const p = this.preview, was = p.outgoing; p.outgoing = false; const ok = this.movePreview(geometry, y); if (!ok) p.outgoing = was; return ok; },
    /** The measured gap at `y`, nothing excluded; an empty list with a port certifies gap 0 (D4). */
    moveIncoming(geometry, y) {
      const inc = this.incoming;
      if (!inc) return false;
      inc.certified = null; inc.stepped = false;
      const at = this.index.len ? gap(this.index, y, -1) : geometry.portHeight > 0 ? 0 : null;
      if (at === null) return false;
      const top = this.index.prefix(at);
      if (top < geometry.scrollTop || top > geometry.scrollTop + geometry.portHeight) return false;
      if (at < this.index.len && !this.index.order[at].startsWith("s:")) return false;
      inc.before = this.index.order[at] ?? null; inc.certified = geometry;
      this.emitPreview();
      return true;
    },
    incomingDrop(g) {
      const inc = this.incoming;
      if (!inc || inc.holding || inc.closed || inc.before === undefined || !(inc.stepped || inc.certified && GEOMETRY.every(k => inc.certified[k] === g[k]))) return undefined;
      if (inc.before !== null && !this.index.pos.has(inc.before)) return undefined;
      inc.holding = true; inc.certified = null;
      return inc.before === null ? null : inc.before.slice(2);
    },
    holdDrop(g) {
      const p = this.preview;
      if (!p || p.terminal || p.holding || !(p.stepped || p.certified && GEOMETRY.every(k => p.certified[k] === g[k]))) return null;
      if (!this.index.pos.has(p.source) || (p.before !== null && !this.index.pos.has(p.before))) return null;
      p.holding = true; p.certified = null;
      return [p.source.slice(2), p.before === null ? null : p.before.slice(2)];
    },
    holdOutgoing() { if (this.preview) { this.preview.holding = true; this.preview.certified = null; } },
    endHold(instant) {
      const p = this.preview;
      if (p) { p.holding = false; p.terminal = true; p.certified = null; }
      const was = this.instant; this.instant = instant; this.emitPreview(); this.instant = was;
    },
    setHidden(ident) { this.hidden = ident; this.emitPreview(); if (ident == null) delete this.hidden; },
    placeOf(ident) { const at = this.index.pos.get(ident); return at === undefined ? null : [this.index.order[at - 1] ?? null, this.index.order[at + 1] ?? null]; },
    wrapperOf(ident) { const at = this.index.pos.get(ident); return this.mounted.find(m => m.position === at)?.view ?? null; },
    stepSlot() {
      const p = this.preview;
      if (p && !p.outgoing) {
        const source = this.index.pos.get(p.source), len = this.index.len - 1;
        if (source === undefined) return null;
        if (p.before === null) return [len, len];
        const b = this.index.pos.get(p.before);
        return b === undefined ? null : [b > source ? b - 1 : b, len];
      }
      const inc = this.incoming;
      if (!inc) return null;
      const len = this.index.len;
      return [inc.before == null ? len : this.index.pos.get(inc.before) ?? len, len];
    },
    stepTo(slot) {
      const s = this.stepSlot();
      if (!s) return false;
      slot = Math.max(0, Math.min(slot, s[1]));
      const p = this.preview;
      if (p && !p.outgoing) {
        const source = this.index.pos.get(p.source);
        p.before = this.index.order[slot < source ? slot : slot + 1] ?? null; p.certified = null; p.stepped = true;
      } else {
        const inc = this.incoming;
        inc.before = this.index.order[slot] ?? null; inc.certified = null; inc.stepped = true;
      }
      this.emitPreview();
      return true;
    },
    gapBefore(source) {
      if (source) { const p = this.preview; return p && !p.outgoing ? p.before : null; }
      return this.incoming?.before ?? null;
    },
    stepNeighbour() {
      const p = this.preview, before = p && !p.outgoing ? p.before : this.incoming?.before;
      return before ?? this.index.order.at(-1) ?? null;
    },
  });
  // A row's measured border box comes through its transform: a row the
  // preview translates remeasures a few float32 ulps off (76 as 75.99998)
  // when a relayout reports it mid-drag (habits F10). Layout itself moves in
  // 1/64 (Chrome, WebKit) or 1/60 (Firefox) pixels, so a hundredth is noise.
  const HEIGHT_NOISE = 0.01;
  const GEOMETRY = ["list", "revision", "scrollSequence", "scrollTop", "portWidth", "portHeight", "rowWidth", "totalExtent"];
  let ReorderOwner = null, ReorderSerial = 0;
  /** A grip's binding: its `reorderFor` list (the strict ancestor the
   * compiler resolved) with a `reorderdrop`, every box on the way live. */
  function reorderBinding(handle) {
    const el = Views.get(handle), list = el?.$reorderList ?? named(el), c = list?.$list;
    if (!c || !list.$reorderdrop || !el.isConnected || el.closest("[inert],[disabled]") || !el.getClientRects().length) return null;
    return c.reorderBinding(handle);
  }
  // A computed `reorderFor` names its list at run time: the one strict
  // ancestor with that `id` (runner `reorder_binding`).
  function named(el) {
    const name = el?.dataset.reorderfor;
    if (!name) return null;
    let found = null;
    for (let a = el.parentElement; a; a = a.parentElement) if (a.id === name) { if (found) return null; found = a; }
    return found;
  }
  const sameBinding = (a, b) => !!a && !!b && ["handle", "list", "wrapper", "root", "rowEpoch"].every(k => a[k] === b[k]);
  const sameGeometry = (a, b) => !!a && !!b && GEOMETRY.every(k => a[k] === b[k]);
  const hasReorder = token => { const p = ReorderOwner?.preview; return !!p && p.token === token && !p.terminal && !p.holding && p.pinOwned && sameBinding(reorderBinding(p.binding.handle), p.binding); };
  // ------------------------------------------------ a session across grouped lists (runner/reorder_group.rs)
  // The source's token and owner for the whole gesture, a target and a phase
  // (LLP 1094 D4); a drop may hold until the move shows (D8).
  const HOLD_MS = 1000;
  let Session = null;
  const groupOf = c => c?.el?.dataset.reordergroup || null;
  const groupLists = group => [...Lists.values()].filter(c => groupOf(c) === group && c.el.isConnected)
    .sort((a, b) => a.el.compareDocumentPosition(b.el) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1);
  const listId = c => c?.el?.id ?? "";
  const grouped = token => Session && Session.token === token && Session.group ? Session : null;
  function leaveTarget(s, current) {
    // Without a ghost the row stays shown: it is the focused grip's (D9).
    if (current === s.source) s.source.setOutgoing(true);
    else current?.closeIncoming(false);
  }
  /** Close the previews and enter the last phase (reorder_group.rs `end_session`). */
  function endSession(s, ending, instant) {
    const dropped = s.phase === "holding";
    if (Lists.get(s.source.view) === s.source) s.source.endHold(instant);
    if (s.target !== s.source) s.target.closeIncoming(instant);
    for (const c of s.touched) if (Lists.get(c.view) === c) c.setHidden(s.ghost && c.index.pos.has(s.item) ? s.item : null);
    s.phase = dropped || ending ? "settling" : "cancelling"; s.ending = ending ?? null; s.deadline = null;
    if (s.timer) { const i = clock.timers.indexOf(s.timer); if (i >= 0) clock.timers.splice(i, 1); s.timer = null; }
  }
  function holdOutcome(s) {
    const at = s.target.placeOf(s.item);
    if (at && at[1] === s.before) return "landed";
    for (const c of groupLists(s.group)) {
      const place = c.placeOf(s.item);
      if (!place) continue;
      return c === s.place[0] && place[0] === s.place[1] && place[1] === s.place[2] ? null : "landed";
    }
    return "gone";
  }
  /** The session's commit-end check, before the controller hears its state. */
  function reconcileGroup() {
    const s = Session;
    if (!s || !s.group) return;
    if (!Lists.has(s.source.view)) { Session = null; return; }
    if (s.phase === "holding") { const ending = holdOutcome(s); if (ending) endSession(s, ending, true); return; }
    if (s.phase !== "active") return;
    if (!hasReorder(s.token)) { endSession(s, null, false); return; }
    if (s.target !== s.source && Lists.get(s.target.view) !== s.target) { s.source.setOutgoing(false); s.target = s.source; }
  }
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
    frame(token) {
      const f = ReorderOwner?.previewFrame(token) ?? null, s = Session;
      if (!f || !s || s.token !== token) return f;
      const lists = [s.target, s.source, ...(s.group ? groupLists(s.group) : [])];
      let row = null;
      for (const c of lists) if (Lists.get(c.view) === c && (row = c.wrapperOf(s.item)) != null) break;
      return { ...f, phase: s.phase, ending: s.ending, target: s.target.view, row };
    },
    finish(token) {
      if (!ReorderOwner) return false;
      if (!ReorderOwner.finishPreview(token)) return false;
      ReorderOwner = null;
      // Every row a grouped session hid shows again, and a gap left open closes (D6).
      if (Session?.token === token) { for (const c of Session.touched) if (Lists.get(c.view) === c) { c.closeIncoming(false); c.setHidden(null); } Session = null; }
      settled();
      return true;
    },
    controller: () => controller(),
    // ---------------------------------------------- grouped (LLP 1094)
    /** `begin_group_reorder`: `ghost` when the page draws one, so the row hides until `finish`. */
    beginGroup(binding, geometry, ghost) {
      const c = Lists.get(binding.list), group = groupOf(c);
      if (!group) return null;
      const started = Reorder.begin(binding, geometry);
      if (!started) return null;
      const item = c.previewItem();
      c.preview.grouped = true;
      if (ghost) c.setHidden(item);
      Session = { token: started.token, source: c, target: c, group, phase: "active", ending: null, deadline: null, item, key: item.slice(2),
        before: null, place: null, ghost, touched: new Set([c]), warned: false, timer: null };
      return started;
    },
    grouped: token => !!grouped(token),
    /** `preview_reorder_into`: the gap in whichever grouped list the page names. */
    into(token, view, geometry, y) {
      const s = grouped(token), t = Lists.get(view);
      if (!s || s.phase !== "active" || !hasReorder(token) || !t || !sameGeometry(t.reorderGeometry(), geometry)) return "stale";
      if (!isFinite(y)) throw new Error("invalid reorder coordinate");
      if (t !== s.source) {
        if (groupOf(t) !== s.group) return "stale";
        if (t.index.pos.has(s.item)) {
          if (!s.warned) { s.warned = true; say(`reorderdrop: ${listId(t)} already holds the dragged row, so it is not a target`); }
          return "accepted";
        }
      }
      const current = s.target;
      let ok;
      if (t === current) ok = t === s.source ? t.movePreview(geometry, y) : t.moveIncoming(geometry, y);
      else if (t === s.source) {
        ok = t.retargetHome(geometry, y);
        if (ok) { current.closeIncoming(false); }
      } else {
        t.openIncoming(s.item, s.source.preview.height);
        ok = t.moveIncoming(geometry, y);
        if (!ok) t.incoming = null; else leaveTarget(s, current);
      }
      if (ok) { s.target = t; s.touched.add(t); } else if (t !== current) current.uncertify();
      return ok ? "accepted" : "needs";
    },
    /** `reorder_step` (D9): a row up or down, or the same index in the previous or next grouped list. */
    step(token, n) {
      const s = grouped(token);
      if (!s || s.phase !== "active" || !hasReorder(token)) return false;
      const current = s.target, slot = current.stepSlot();
      if (!slot) return false;
      let t = current;
      if (n <= 2) current.stepTo(slot[0] + (n === 1 ? -1 : 1));
      else {
        const lists = groupLists(s.group), at = lists.indexOf(current);
        if (at < 0) return false;
        const ahead = n === 4 ? lists.slice(at + 1) : lists.slice(0, at).reverse();
        t = ahead.find(c => c === s.source || !c.index.pos.has(s.item));
        if (!t) return false;
        if (t === s.source) { t.setOutgoing(false); t.stepTo(slot[0]); current.closeIncoming(false); }
        else { t.openIncoming(s.item, s.source.preview.height); t.stepTo(slot[0]); leaveTarget(s, current); }
        s.target = t; s.touched.add(t);
      }
      // The gap kept in view, `nearest` (scrollIntoView, list.js).
      const near = t.stepNeighbour();
      if (near) globalThis.exact?.lists?.into(t.view, near.slice(2), "nearest", "nearest");
      return true;
    },
    /** A grouped drop (D2, D8): the target's `reorderdrop` once, with its `ReorderEvent`; the hold armed first. */
    dropGroup(token, geometry) {
      const s = grouped(token), t = s?.target;
      if (!s || s.phase !== "active" || !hasReorder(token) || !sameGeometry(t.reorderGeometry(), geometry)) return false;
      let item, before;
      if (t === s.source) { const d = t.holdDrop(geometry); if (!d) return false; [item, before] = d; }
      else { before = t.incomingDrop(geometry); if (before === undefined) return false; item = s.key; s.source.holdOutgoing(); }
      s.before = before === null ? null : `s:${before}`;
      s.place = [s.source, ...(s.source.placeOf(s.item) ?? [null, null])];
      s.phase = "holding"; s.deadline = clock.now + HOLD_MS;
      every(HOLD_MS, () => { if (Session === s && s.phase === "holding") return commit(() => { say("reorderdrop: the move did not show within 1 s"); endSession(s, "timeout", false); }, "reorderdrop hold"); return true; }, true);
      s.timer = clock.timers.at(-1);
      t.el.$reorderdrop(item, before, [listId(s.source), listId(t)]);
      return true;
    },
    cancelGroup(token) { const s = grouped(token); if (s && s.phase === "active") endSession(s, null, false); },
    reconcileGroup,
    session: () => Session,
    /** `state.reorder` (D12). */
    json() {
      const s = Session;
      if (!s) return null;
      const before = s.phase === "active" ? s.target.gapBefore(s.target === s.source) : s.before;
      return { item: s.key, from: listId(s.source), to: listId(s.target), before: before ? before.slice(2) : null, phase: s.phase, ending: s.ending };
    },
  };
  // A preview whose binding is gone ends (runner reconcile_reorder).
  After.push(() => {
    if (ReorderOwner && !Lists.has(ReorderOwner.view)) { ReorderOwner = null; Session = null; return; }
    if (Session?.group) return; // the motion piece's flush asks first (`reconcileGroup`)
    const p = ReorderOwner?.preview;
    if (p && !p.terminal && p.pinOwned && !hasReorder(p.token)) ReorderOwner.endPreview();
  });
  return Reorder;
}
