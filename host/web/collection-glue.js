// Virtualized collections' DOM controller (@ref LLP 1010 §6), the list
// engine's browser half: an after-paint piece, fetched when a batch first
// commits a collection (LLP 1047 D5); navigation.js stands in until then.
// `fill` (LLP 1050.000 §6): `velocity` in CSS px/s toward the end, and
// `limit`, the rows past what the report owes it may build (null: any);
// `ancestorMoving`, an enclosing list is moving (LLP 1070 F2). Wire v3:
// offset, port and row sizes on the list's main and cross axes.
export function collectionBytes(facts, fill = {}) {
  const valid = n => Number.isFinite(n) && n >= 0 && n <= 3.4028234663852886e38;
  const id = n => Number.isInteger(n) && n > 0 && n <= 0xffffffff;
  const u64 = n => { if (typeof n === 'number' && !Number.isSafeInteger(n)) throw Error('unsafe collection identity'); const v = BigInt(n); if (v < 0n || v > 0xffffffffffffffffn) throw Error('invalid collection identity'); return v; };
  const rows = facts.measurements, seen = new Set();
  if (!id(facts.view) || ![facts.offset, facts.port_main, facts.port_cross, facts.cross].every(valid)
      || [facts.focus_view, facts.interaction_view].some(n => n != null && !id(n))) throw Error('invalid collection geometry');
  for (const row of rows) {
    if (!id(row.view) || seen.has(row.view) || !valid(row.size)) throw Error('invalid collection row');
    seen.add(row.view);
  }
  const velocity = Number.isFinite(fill.velocity) ? fill.velocity : 0;
  const limit = Number.isInteger(fill.limit) && fill.limit >= 0 ? Math.min(fill.limit, 0xfffffffe) : 0xffffffff;
  const bytes = new Uint8Array(84 + rows.length * 20), d = new DataView(bytes.buffer);
  d.setUint32(0, 3, true); d.setUint32(4, facts.view, true);
  d.setBigUint64(8, u64(facts.revision), true); d.setBigUint64(16, u64(facts.scroll_sequence), true);
  [facts.offset, facts.port_main, facts.port_cross, facts.cross].forEach((n, i) => d.setFloat64(24 + i * 8, n, true));
  d.setUint32(56, facts.focus_view ?? 0, true); d.setUint32(60, facts.interaction_view ?? 0, true);
  d.setFloat64(64, velocity, true); d.setUint32(72, limit, true);
  d.setUint32(76, fill.ancestorMoving ? 1 : 0, true); d.setUint32(80, rows.length, true);
  rows.forEach((row, i) => {
    d.setUint32(84 + i * 20, row.view, true); d.setBigUint64(88 + i * 20, u64(row.epoch), true);
    d.setFloat64(96 + i * 20, row.size, true);
  });
  return bytes;
}

// The reader's own hand on a port (LLP 1070.000 §2.5: it cancels a request).
const INPUT = ['wheel', 'touchstart', 'keydown', 'pointerdown'];
// A list's main axis (LLP 1070 H1): `y` for a block list, `x` for a flex row.
// Every main-axis read and write goes through one of these.
const AXES = {
  y: { offset: 'scrollTop', client: 'clientHeight', crossClient: 'clientWidth', scrollSize: 'scrollHeight', overflow: 'overflowY',
    start: 'top', end: 'bottom', size: 'height', clientStart: 'clientTop', padStart: 'paddingTop', crossPads: ['paddingLeft', 'paddingRight'] },
  x: { offset: 'scrollLeft', client: 'clientWidth', crossClient: 'clientHeight', scrollSize: 'scrollWidth', overflow: 'overflowX',
    start: 'left', end: 'right', size: 'width', clientStart: 'clientLeft', padStart: 'paddingLeft', crossPads: ['paddingTop', 'paddingBottom'] },
};

export function applyCollectionFeedback(batch, applyBatch) {
  applyBatch(batch);
  return batch.accepted === true;
}

// `report(bytes, facts, fill)`: the wire bytes for the wasm runner, the
// same facts as values for the JS target's (host/web-js/list.js).
export function collectionController({ root, views, report, settled=()=>{},
  requestFrame = fn => requestAnimationFrame(fn), cancelFrame = id => cancelAnimationFrame(id), now = () => performance.now() }) {
  const states = new Map(), dirty = new Set(), waiting = new Set(), rowOwners = new WeakMap(), doc = root.ownerDocument;
  let settling = false, frame = null, delivering = false, interaction = null, reportsLeft = 4, notification=false, reported = 0;
  // LLP 1050.000 stage 1. The browser scrolls on its own thread and never
  // waits for rows (the declared deviation), so each frame's reports share a
  // slice of time: every row a report owes (what shows, the pins), then as
  // many more as the measured per-row cost fits, or the next two frames of
  // travel uncover. A report's reply that leaves rows unbuilt (`pending`)
  // continues in the next frame. Authored jumps build first, then move (§6).
  let interval = 1000 / 60, lastFrame = null, deadline = 0;
  const jumps = [];
  const slice = () => Math.min(4, Math.max(1, interval * 0.24));
  // A `scrollIntoView` under way (LLP 1070.000 §2.5): the port's moves are
  // its own and the browser's clamps, never travel, until the reader's input.
  const authored = s => s.seeking && !s.input;
  const velocity = s => !authored(s) && s.travel && now() - s.travel.time < 150 ? s.travel.velocity : 0;
  function sample(s) {
    const A = AXES[s.axis], time = now(), at = s.port[A.offset], t = s.travel;
    if (s.clampAt === at) { s.travel = { at, time, velocity: 0 }; return; }
    // A step longer than the port is a jump, not travel: nothing to lead.
    if (!t || Math.abs(at - t.at) > s.port[A.client]) { s.travel = { at, time, velocity: 0 }; return; }
    const delta = at - t.at, elapsed = time - t.time;
    if (delta === 0 || elapsed <= 0) return;
    const speed = delta * 1000 / Math.max(elapsed, interval / 2);
    t.velocity = elapsed > 150 || speed * t.velocity <= 0 ? speed : t.velocity * 0.5 + speed * 0.5;
    t.at = at; t.time = time;
  }
  function fits(s, remaining) {
    if (remaining <= 0) return 0;
    if (s.perRow == null) return 1;
    return Math.max(0, Math.min(s.lastRows * 2, Math.floor(remaining * 0.9 / s.perRow)));
  }
  // Rows past the mounted run the port needs once it travels `ahead` px,
  // at the mounted rows' mean size, from rects this pass already read.
  function rowsToCover(s, rects, g, ahead) {
    if (!ahead || !rects.length) return 0;
    const A = AXES[s.axis], count = s.snapshot.count;
    const rows = s.rows.map((row, i) => ({ index: row.index, top: rects[i][A.start], bottom: rects[i][A.end] }))
      .sort((a, b) => a.top - b.top);
    const mean = rows.reduce((n, r) => n + r.bottom - r.top, 0) / rows.length;
    if (!(mean > 0)) return 0;
    const top = viewport(s.port, A).start, bottom = top + g.portMain;
    let shortfall;
    if (ahead > 0) {
      if (rows.at(-1).index === count - 1) return 0;
      let reached = top;
      for (const r of rows) if (r.top <= reached + 0.5) reached = Math.max(reached, r.bottom);
      shortfall = bottom + ahead - reached;
    } else {
      if (rows[0].index === 0) return 0;
      let reached = bottom;
      for (const r of rows.toReversed()) if (r.bottom >= reached - 0.5) reached = Math.min(reached, r.top);
      shortfall = reached - (top + ahead);
    }
    return shortfall > 0 ? Math.min(64, Math.ceil(shortfall / mean)) : 0;
  }
  const number = text => Number.parseFloat(text) || 0;
  const size = el => { const r = el.getBoundingClientRect(); return `${r.width},${r.height}`; };
  const portOf = (el, axis) => {
    for (let p = el; p && p !== doc.body; p = p.parentElement) {
      if (/^(auto|scroll|hidden)$/.test(getComputedStyle(p)[AXES[axis].overflow])) return p;
    }
    return doc.scrollingElement;
  };
  // The port's start edge (viewport px) and its size on each axis.
  const viewport = (port, A) => port === doc.scrollingElement
    ? { start: 0, main: doc.documentElement[A.client], cross: doc.documentElement[A.crossClient] }
    : { start: port.getBoundingClientRect()[A.start] + port[A.clientStart], main: port[A.client], cross: port[A.crossClient] };
  // `raw`: the port's start edge from the list's content origin; `cross`: the
  // rows' available cross size, after the list's cross-axis padding.
  function geometry(s) {
    if (!s.el.isConnected || !s.el.getClientRects().length) return null;
    const A = AXES[s.axis], css = getComputedStyle(s.el), port = viewport(s.port, A);
    const origin = s.el.getBoundingClientRect()[A.start] + s.el[A.clientStart] + number(css[A.padStart])
      - (s.el === s.port ? s.port[A.offset] : 0);
    return { raw: port.start - origin, portMain: port.main, portCross: port.cross,
      cross: s.el[A.crossClient] - number(css[A.crossPads[0]]) - number(css[A.crossPads[1]]) };
  }
  const dimensionsOf = g => `${g.portCross},${g.portMain},${g.cross}`;
  function liveView(s, element) {
    let owner;
    for (let at = element; at && at !== root; at = at.parentElement) {
      if ((owner = rowOwners.get(at))) break;
    }
    if (owner !== s) return null;
    for (let at = element; at && at !== s.el; at = at.parentElement) {
      const id = Number(at.dataset?.view);
      if (id && views.get(id) === at) return id;
    }
    return null;
  }
  function schedule() {
    if (frame === null && dirty.size) frame = requestFrame(() => flush());
  }
  function enqueue(s, stimulus = false) {
    if (stimulus) s.budget = 2;
    if (s.budget > 0) { dirty.add(s); schedule(); }
  }
  function scrollChanged(s) {
    const A = AXES[s.axis], at = s.port[A.offset];
    if (at === s.offset) return false;
    // The browser clamping the port to an extent a row laid out smaller than
    // its estimate shortened: not a scroll. The runner's coordinates are the
    // painted frame's, where the port was still where it was (LLP 1070.000).
    const max = s.port[A.scrollSize] - s.port[A.client];
    if (at < s.offset && s.offset > max && Math.abs(at - max) <= 1) {
      s.clamp = { from: s.clamp?.from ?? s.offset, at }; s.offset = s.clampAt = at;
      return false;
    }
    s.clamp = s.clampAt = null; s.offset = at; s.sequence++;
    return true;
  }
  function desired(s) {
    const pinned = interaction?.lease;
    const held = pinned?.state === s && s.rows.some(r => r.el === pinned.wrapper && r.el.isConnected);
    return [liveView(s, doc.activeElement), held ? pinned.view : liveView(s, interaction?.element)];
  }
  function retiring(s) {
    const next = desired(s), old = s.lastFacts;
    return old && ((old.focus_view != null && old.focus_view !== next[0])
      || (old.interaction_view != null && old.interaction_view !== next[1]));
  }
  function flush(beforePaint = false, only = null) {
    if (!beforePaint) {
      frame = null; reportsLeft = 4;
      const time = now();
      if (lastFrame !== null && time - lastFrame > 4 && time - lastFrame < 50) interval = time - lastFrame;
      lastFrame = time; deadline = time + slice();
    }
    const attempted = new Set();
    // The agent's settle visits every list each round, not four a frame.
    for (let pass = 0; pass < (settling ? states.size + 4 : 4) && (reportsLeft > 0 || only?.jump != null); pass++) {
      const releases = [...states.values()].filter(retiring);
      const candidates = [...dirty].filter(s => !attempted.has(s) && (!only || s === only));
      const s = candidates.find(s => releases.includes(s)) ?? candidates[0];
      if (!s) break;
      dirty.delete(s); attempted.add(s);
      if (!states.has(s.snapshot.view) || s.budget <= 0) continue;
      const next = desired(s), old = s.lastFacts;
      const pins = releases.length ? next.map((pin, i) =>
        pin === (i === 0 ? old?.focus_view : old?.interaction_view) ? pin : null) : next;
      if (pins.some((pin, i) => pin !== next[i])) waiting.add(s);
      else waiting.delete(s);
      let g = s.valid ? geometry(s) : null;
      let measurements = [];
      const measuredSizes = new Map(), rects = [], jump = s.jump;
      const A = AXES[s.axis];
      const visible = g && g.portMain > 0 && g.cross > 0
        && s.rows.every(row => row.el.isConnected && row.el.getClientRects().length);
      if (visible) {
        measurements = s.rows.map(row => {
          const rect = row.el.getBoundingClientRect();
          rects.push(rect);
          measuredSizes.set(row.el, `${rect.width},${rect.height}`);
          return { view: row.view, epoch: row.epoch, size: rect[A.size] };
        });
      } else if (releases.includes(s)) {
        g = { raw: old.offset, portCross: old.port_cross, portMain: old.port_main, cross: old.cross };
      } else continue;
      scrollChanged(s);
      // The jump's target, clamped as the browser will: reported before it
      // is assigned, so its rows exist before any frame shows it.
      if (jump != null) {
        g = { ...g, raw: g.raw + Math.max(0, Math.min(jump, s.port[A.scrollSize] - s.port[A.client])) - s.port[A.offset] };
        s.sequence++; s.jumpedAt = s.sequence;
      }
      const dimensions = dimensionsOf(g);
      if (s.dimensions !== null && dimensions !== s.dimensions) { s.sequence++; s.jumpedAt = s.sequence; }
      s.dimensions = dimensions;
      const facts = { view: s.snapshot.view, revision: s.snapshot.revision, scroll_sequence: s.sequence,
        offset: Math.max(0, g.raw + (s.clamp?.at === s.port[A.offset] ? s.clamp.from - s.clamp.at : 0)), port_main: g.portMain, port_cross: g.portCross, cross: g.cross,
        focus_view: pins[0], interaction_view: pins[1], measurements };
      const signature = [facts.offset, facts.scroll_sequence, dimensions, ...pins,
        ...measurements.flatMap(r => [r.view, r.epoch, r.size])].join('|');
      for (const [el, value] of measuredSizes) if (s.observed.has(el)) s.observed.set(el, value);
      if (s.signature === signature && jump == null && !s.snapshot.pending) continue;
      const v = jump == null && !settling ? velocity(s) : 0;
      // While its outer list moves, an inner list builds only what it owes
      // (LLP 1070 F2); its pending reply continues the fill at rest.
      const outer = s.snapshot.parent == null ? null : states.get(s.snapshot.parent);
      // The agent's settle ends motion and builds all a report owes at once,
      // as the native pumps do.
      const fill = { velocity: v, ancestorMoving: !settling && !!outer && velocity(outer) !== 0, limit: settling ? null : jump != null ? 2
        : Math.max(1, fits(s, deadline - now()), rowsToCover(s, rects, g, v * interval * 2 / 1000)) };
      let bytes;
      try { bytes = collectionBytes(facts, fill); } catch { continue; }
      s.budget--; reportsLeft--; reported++;
      for (const el of s.observed.keys()) if (!measuredSizes.has(el)) s.observed.set(el, size(el));
      measuredSizes.clear();
      delivering = true;
      let accepted;
      const before = new Set(s.rows.map(row => row.view)), at = s.offset, started = now(), clamp = s.clamp;
      try { accepted = report(bytes, facts, fill) !== false; } finally { delivering = false; }
      const created = s.rows.filter(row => !before.has(row.view)).length;
      if (created > 0) {
        const cost = Math.max(0.001, (now() - started) / created);
        s.perRow = Math.max(cost, (s.perRow ?? cost) * 0.75 + cost * 0.25); s.lastRows = created;
      }
      // An anchor correction in the reply already moved the port there.
      if (jump != null) { s.jump = null; if (s.offset === at) move(s, jump); }
      if (accepted) {
        s.signature = signature; s.lastFacts = facts;
        if (s.clamp === clamp) s.clamp = null; // reported; one in its own commit is the next report's
        if(!notification){notification=true;queueMicrotask(()=>{notification=false;settled();});}
        if (releases.includes(s)) for (const held of waiting) enqueue(held);
      }
    }
    schedule();
    if (!delivering) for (const [s, at] of jumps.splice(0)) jumpTo(s, at);
  }
  function move(s, at) {
    const name = AXES[s.axis].offset;
    s.port[name] = at; s.offset = s.port[name]; s.travel = null;
  }
  function jumpTo(s, at) {
    if (!states.has(s.snapshot.view)) return;
    if (delivering) { jumps.push([s, at]); return; }
    s.jump = at; enqueue(s, true); flush(true, s);
    // Not reportable (hidden, partial): move now; its rows follow a frame later.
    if (s.jump != null) { s.jump = null; move(s, at); scrollChanged(s); enqueue(s, true); }
  }
  function detach(s) {
    dirty.delete(s); waiting.delete(s); s.observer.disconnect();
    for (const row of s.rows) if (row.el) rowOwners.delete(row.el);
    s.port.removeEventListener('scroll', s.scrolled);
    s.el.style.overflowAnchor = s.anchor;
    for (const name of INPUT) s.port.removeEventListener(name, s.touched);
    states.delete(s.snapshot.view);
    for (const held of waiting) enqueue(held);
  }
  function observe(s) {
    const elements = new Set([s.el, s.port, ...s.rows.map(r => r.el)]);
    for (const el of s.observed.keys()) if (!elements.has(el)) { s.observer.unobserve(el); s.observed.delete(el); }
    for (const el of elements) {
      if (!s.observed.has(el)) s.observer.observe(el);
      // The queued measurement supplies row baselines. Port/list geometry stays
      // current for pre-paint resize feedback. At the final dependent commit no
      // pass remains: read now so its own notification cannot renew the budget.
      const deferred = el !== s.el && el !== s.port && (!delivering || s.budget > 0);
      s.observed.set(el, deferred ? null : size(el));
    }
  }
  function focusChanged() { for (const s of states.values()) enqueue(s, true); }
  function pointerDown(event) {
    if (interaction?.lease) return; // Terminal source still owns the single slot.
    interaction = { element: event.target, pointer: event.pointerId };
    focusChanged();
  }
  function pointerUp(event) {
    if (event.pointerId !== interaction?.pointer || interaction?.lease) return;
    // A child-to-ancestor capture transfer keeps the same contact and row pin.
    if (event.type === 'lostpointercapture'
        && event.composedPath().some(node => node.hasPointerCapture?.(event.pointerId))) return;
    interaction = null; focusChanged();
  }
  root.addEventListener('focusin', focusChanged, true);
  root.addEventListener('focusout', focusChanged, true);
  root.addEventListener('pointerdown', pointerDown, true);
  for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) doc.addEventListener(name, pointerUp);
  return {
    releaseInteraction(pointer) { pointerUp({pointerId:pointer}); },
    reporting() { return delivering; },
    reorderContact(element,pointer) { pointerDown({target:element,pointerId:pointer}); },
    reorderMapping(view,y) {
      const s=states.get(view); if(!s?.valid||s.axis!=='y'||portOf(s.el,s.axis)!==s.port)return null;
      if(scrollChanged(s))enqueue(s,true);
      enqueue(s);if(!delivering)flush(true);
      const g=geometry(s),f=s.lastFacts,p=viewport(s.port,AXES.y);
      if(!g||!f||f.offset!==Math.max(0,g.raw)||f.port_cross!==g.portCross||f.port_main!==g.portMain
        ||f.cross!==g.cross||BigInt(f.scroll_sequence)!==s.sequence)return undefined;
      return {revision:s.snapshot.revision,scrollSequence:String(s.sequence),scrollTop:f.offset,
        portWidth:g.portCross,portHeight:g.portMain,rowWidth:g.cross,totalExtent:s.snapshot.totalExtent,
        contentY:y-p.start+g.raw,raw:g.raw,port:s.port,portTop:p.start};
    },
    retainInteraction(element, pointer) {
      if (interaction?.lease || interaction?.pointer !== pointer) return null;
      for (const state of states.values()) {
        const view = liveView(state, element), wrapper = state.rows.find(r => r.el.contains(element))?.el;
        if (view == null || !wrapper || state.lastFacts?.interaction_view !== view) continue;
        const lease = Object.freeze({ state, wrapper, view });
        interaction = { element, pointer, lease }; return lease;
      }
      return null;
    },
    releaseRetainedInteraction(lease) {
      if (!lease || interaction?.lease !== lease) return false;
      interaction = null; focusChanged(); return true;
    },
    transferRetainedInteraction(lease, element, pointer) {
      // A descendant -> wrapper -> descendant handoff keeps the SAME row in
      // the existing slot. A null-pin report between these would unmount it.
      if (!lease || interaction?.lease !== lease || delivering || reportsLeft <= 0) return null;
      const s=lease.state,view=liveView(s,element),g=geometry(s),old=s.lastFacts;
      if (!states.has(s.snapshot.view)||!lease.wrapper.isConnected||!lease.wrapper.contains(element)||view==null
        ||!g||old?.interaction_view!==lease.view||old.offset!==Math.max(0,g.raw)
        ||old.port_cross!==g.portCross||old.port_main!==g.portMain||old.cross!==g.cross
        ||s.port[AXES[s.axis].offset]!==s.offset) return null;
      const facts={...old,revision:s.snapshot.revision,interaction_view:view,measurements:[]};
      const bytes=collectionBytes(facts);reportsLeft--;delivering=true;let accepted;
      try { accepted=report(bytes,facts)!==false; } finally { delivering=false; }
      if(!accepted)return null;
      const next=Object.freeze({state:s,wrapper:lease.wrapper,view});
      interaction={element,pointer,lease:next};s.lastFacts=facts;s.signature=null;enqueue(s,true);
      return next;
    },
    // A full O(W) snapshot list, after all ordinary extent/children ops. No DOM
    // mutation is deferred with feedback; corrections happen before this paint.
    commit(snapshots) {
      const active = new Set(snapshots.map(s => s.view));
      for (const s of states.values()) if (!active.has(s.snapshot.view)) detach(s);
      for (const snapshot of snapshots) {
        const el = views.get(snapshot.view);
        if (!el?.isConnected) continue;
        const axis = snapshot.axis === 'x' ? 'x' : 'y', port = portOf(el, axis);
        let s = states.get(snapshot.view);
        if (s && (s.el !== el || s.port !== port || s.axis !== axis)) { detach(s); s = null; }
        if (s && BigInt(snapshot.revision) < BigInt(s.snapshot.revision)) continue;
        if (!s) {
          s = { el, port, axis, snapshot, rows: [], valid: false, observed: new Map(), budget: 2, travel: null, perRow: null, lastRows: 1, jump: null,
            sequence: BigInt(snapshot.scrollSequence), offset: port[AXES[axis].offset],
            dimensions: null, signature: null, lastFacts: null, corrected: null, anchor: el.style.overflowAnchor };
          s.scrolled = () => { const changed = scrollChanged(s); sample(s); if (changed) enqueue(s, true); };
          s.touched = () => { s.input = true; };
          for (const name of INPUT) port.addEventListener(name, s.touched, { passive: true });
          s.observer = new ResizeObserver(entries => {
            let changed = false, resizedPort = false, pending = false;
            for (const { target } of entries) {
              const next = size(target);
              if (s.observed.has(target) && s.observed.get(target) !== next) {
                if (s.observed.get(target) === null) pending = true;
                else changed = true;
                if (target === s.port) resizedPort = true;
              }
              if (s.observed.has(target)) s.observed.set(target, next);
            }
            if (changed) {
              enqueue(s, true);
              // Port growth can expose a spacer before the next rAF. Spend the
              // remaining shared report budget now, after layout/before paint.
              // Keep the queued frame: it replenishes the budget once and
              // handles any deferred work. Own row resizes cannot spin here.
              if (resizedPort && !delivering) flush(true);
            } else if (pending) enqueue(s); // Coalesce an own notification, never replenish.
          });
          port.addEventListener('scroll', s.scrolled, { passive: true });
          states.set(snapshot.view, s);
        }
        // A new wrapper/epoch is new measurement work, including membership
        // committed by an edge action during delivery. Give it a bounded pass
        // on the next callback; revision/height-only refinements keep their cap.
        const epochs = new Map(s.rows.map(row => [row.view, row.epoch]));
        const freshRows = snapshot.rows.some(row => epochs.get(row.view) !== row.epoch);
        s.snapshot = snapshot;
        s.sequence = s.sequence > BigInt(snapshot.scrollSequence) ? s.sequence : BigInt(snapshot.scrollSequence);
        for (const row of s.rows) if (row.el) rowOwners.delete(row.el);
        s.rows = snapshot.rows.map(row => ({ ...row, el: views.get(row.view) }));
        // A partially attached batch is not valid geometry; retry only when a
        // later commit has all wrappers, never by spinning an animation frame.
        s.valid = s.rows.every(row => row.el && el.contains(row.el));
        if (!s.valid) { if (retiring(s)) enqueue(s, !delivering); else dirty.delete(s); continue; }
        for (const row of s.rows) rowOwners.set(row.el, s);
        el.style.overflowAnchor = 'none';
        scrollChanged(s); // catches new user scroll before its scroll event runs
        if (snapshot.seeking && !s.seeking) s.input = false;
        s.seeking = snapshot.seeking === true;
        const correction = snapshot.correction;
        // An anchor's correction (`from`) is relative: the rows before the
        // anchor moved by `offset - from` in this commit, so the port moves
        // with them before it paints, whatever was scrolled since the report
        // (a fling's frames). Once: a later revision with the same anchor's
        // correction owes only what it adds.
        // Not one from before an authored jump or a resize (`jumpedAt`).
        if (correction && Number.isFinite(correction.from) && s.corrected !== snapshot.revision
            && BigInt(correction.scrollSequence) >= (s.jumpedAt ?? 0n)) {
          const g = geometry(s), name = AXES[axis].offset;
          // H4: a row list moving under the user's hand is not corrected
          // (below); a vertical one moves with its rows, mid-fling too.
          const moving = axis === 'x' && velocity(s) !== 0;
          if (g && !moving && (s.dimensions === null || s.dimensions === dimensionsOf(g))) {
            s.corrected = snapshot.revision;
            const last = s.shifted;
            const done = last && last.scrollSequence === correction.scrollSequence && last.from === correction.from ? last.offset : correction.from;
            s.shifted = correction;
            if (correction.offset !== done) {
              const was = port[name];
              port[name] += correction.offset - done;
              s.offset = port[name]; // consume the programmatic scroll echo
              // Not the reader's travel: its velocity reads on from here.
              if (s.travel) s.travel.at += port[name] - was;
            }
          }
        } else
        // A request's correction is authored: the browser's clamps and
        // scroll anchoring since the runner's last report don't void it.
        if (correction && s.corrected !== snapshot.revision
            && (BigInt(correction.scrollSequence) === s.sequence || authored(s))
            && Number.isFinite(correction.offset) && correction.offset >= 0) {
          const g = geometry(s), name = AXES[axis].offset;
          // H4: a row list moving under the user's hand is not corrected;
          // its next report carries the uncorrected offset and the runner
          // re-anchors from that. Vertical lists correct as before.
          const moving = axis === 'x' && velocity(s) !== 0;
          if (g && !moving && (s.dimensions === null || s.dimensions === dimensionsOf(g))) {
            s.corrected = snapshot.revision;
            // Relative conversion also handles a list below siblings in its port.
            port[name] += correction.offset - g.raw;
            s.offset = port[name]; // consume the programmatic scroll echo
            s.travel = null; // a correction is not the reader's travel (LLP 1070.000 §2.5)
          }
        }
        observe(s);
        // A reply that left rows unbuilt owes the next slice a report.
        enqueue(s, !delivering || freshRows || snapshot.pending === true);
      }
      if (!dirty.size && frame !== null && !delivering) { cancelFrame(frame); frame = null; }
    },
    // An authored offset on a collection (glue.js): on the list's own axis
    // (`scrollTop` for y, `scrollLeft` for x) its rows are built at the
    // target, then the port moves, in this task (LLP 1050.000 §6); the other
    // axis is a plain assignment.
    jump(view, at, name = 'scrollTop') {
      const s = states.get(view);
      if (s && s.port === s.el && AXES[s.axis].offset === name) jumpTo(s, at);
      else { const el = views.get(view); if (el) el[name] = at; }
    },
    // The agent's `clock settle` (LLP 1070 G3): report every list until a
    // round sends nothing, reading layout now rather than waiting for the
    // frames a real page would have run, so what shows is built and
    // measured when the agent reads it. Bounded, like the native pumps'.
    settle() {
      settling = true;
      try { for (let round = 0; round < 8; round++) {
        const before = reported;
        reportsLeft = 4 * Math.max(1, states.size);
        for (const s of states.values()) enqueue(s, true);
        if (!delivering) flush(true);
        if (reported === before) break;
      } } finally { settling = false; }
    },
    dataReady() {
      // A refused pre-activation action stays armed. Retry unchanged geometry
      // once after activation, retaining pin reservations until acceptance.
      for (const s of states.values()) { s.signature = null; enqueue(s, true); }
    },
    reset() {
      for (const s of states.values()) detach(s);
      if (frame !== null) cancelFrame(frame);
      frame = null; interaction = null;
    },
    dispose() {
      this.reset();
      root.removeEventListener('focusin', focusChanged, true);
      root.removeEventListener('focusout', focusChanged, true);
      root.removeEventListener('pointerdown', pointerDown, true);
      for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) doc.removeEventListener(name, pointerUp);
    },
  };
}

if (globalThis.exact) globalThis.exact.collectionGlue = { collectionBytes, applyCollectionFeedback, collectionController };
