// Virtualized collections' DOM controller (@ref LLP 1010 §6), the list
// engine's browser half: an after-paint piece, fetched when a batch first
// commits a collection (LLP 1047 D5); navigation.js stands in until then.
export function collectionBytes(facts) {
  const valid = n => Number.isFinite(n) && n >= 0 && n <= 3.4028234663852886e38;
  const id = n => Number.isInteger(n) && n > 0 && n <= 0xffffffff;
  const u64 = n => { if (typeof n === 'number' && !Number.isSafeInteger(n)) throw Error('unsafe collection identity'); const v = BigInt(n); if (v < 0n || v > 0xffffffffffffffffn) throw Error('invalid collection identity'); return v; };
  const rows = facts.measurements, seen = new Set();
  if (!id(facts.view) || ![facts.scroll_top, facts.port_width, facts.port_height, facts.row_width].every(valid)
      || [facts.focus_view, facts.interaction_view].some(n => n != null && !id(n))) throw Error('invalid collection geometry');
  for (const row of rows) {
    if (!id(row.view) || seen.has(row.view) || !valid(row.height)) throw Error('invalid collection row');
    seen.add(row.view);
  }
  const bytes = new Uint8Array(68 + rows.length * 20), d = new DataView(bytes.buffer);
  d.setUint32(0, 1, true); d.setUint32(4, facts.view, true);
  d.setBigUint64(8, u64(facts.revision), true); d.setBigUint64(16, u64(facts.scroll_sequence), true);
  [facts.scroll_top, facts.port_width, facts.port_height, facts.row_width].forEach((n, i) => d.setFloat64(24 + i * 8, n, true));
  d.setUint32(56, facts.focus_view ?? 0, true); d.setUint32(60, facts.interaction_view ?? 0, true);
  d.setUint32(64, rows.length, true);
  rows.forEach((row, i) => {
    d.setUint32(68 + i * 20, row.view, true); d.setBigUint64(72 + i * 20, u64(row.epoch), true);
    d.setFloat64(80 + i * 20, row.height, true);
  });
  return bytes;
}

export function applyCollectionFeedback(batch, applyBatch) {
  applyBatch(batch);
  return batch.accepted === true;
}

export function collectionController({ root, views, report, settled=()=>{},
  requestFrame = fn => requestAnimationFrame(fn), cancelFrame = id => cancelAnimationFrame(id) }) {
  const states = new Map(), dirty = new Set(), waiting = new Set(), rowOwners = new WeakMap(), doc = root.ownerDocument;
  let frame = null, delivering = false, interaction = null, reportsLeft = 4, notification=false;
  const number = text => Number.parseFloat(text) || 0;
  const size = el => { const r = el.getBoundingClientRect(); return `${r.width},${r.height}`; };
  const portOf = el => {
    for (let p = el; p && p !== doc.body; p = p.parentElement) {
      if (/^(auto|scroll|hidden)$/.test(getComputedStyle(p).overflowY)) return p;
    }
    return doc.scrollingElement;
  };
  const viewport = port => port === doc.scrollingElement
    ? { top: 0, width: doc.documentElement.clientWidth, height: doc.documentElement.clientHeight }
    : { top: port.getBoundingClientRect().top + port.clientTop, width: port.clientWidth, height: port.clientHeight };
  function geometry(s) {
    if (!s.el.isConnected || !s.el.getClientRects().length) return null;
    const css = getComputedStyle(s.el), port = viewport(s.port);
    const origin = s.el.getBoundingClientRect().top + s.el.clientTop + number(css.paddingTop)
      - (s.el === s.port ? s.port.scrollTop : 0);
    return { raw: port.top - origin, width: port.width, height: port.height,
      rowWidth: s.el.clientWidth - number(css.paddingLeft) - number(css.paddingRight) };
  }
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
    const top = s.port.scrollTop;
    if (top === s.scrollTop) return false;
    s.scrollTop = top; s.sequence++;
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
  function flush(beforePaint = false) {
    if (!beforePaint) { frame = null; reportsLeft = 4; }
    const attempted = new Set();
    for (let pass = 0; pass < 4 && reportsLeft > 0; pass++) {
      const releases = [...states.values()].filter(retiring);
      const candidates = [...dirty].filter(s => !attempted.has(s));
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
      const measuredSizes = new Map();
      const visible = g && g.height > 0 && g.rowWidth > 0
        && s.rows.every(row => row.el.isConnected && row.el.getClientRects().length);
      if (visible) {
        measurements = s.rows.map(row => {
          const rect = row.el.getBoundingClientRect();
          measuredSizes.set(row.el, `${rect.width},${rect.height}`);
          return { view: row.view, epoch: row.epoch, height: rect.height };
        });
      } else if (releases.includes(s)) {
        g = { raw: old.scroll_top, width: old.port_width, height: old.port_height, rowWidth: old.row_width };
      } else continue;
      scrollChanged(s);
      const dimensions = `${g.width},${g.height},${g.rowWidth}`;
      if (s.dimensions !== null && dimensions !== s.dimensions) s.sequence++;
      s.dimensions = dimensions;
      const facts = { view: s.snapshot.view, revision: s.snapshot.revision, scroll_sequence: s.sequence,
        scroll_top: Math.max(0, g.raw), port_width: g.width, port_height: g.height, row_width: g.rowWidth,
        focus_view: pins[0], interaction_view: pins[1], measurements };
      const signature = [facts.scroll_top, facts.scroll_sequence, dimensions, ...pins,
        ...measurements.flatMap(r => [r.view, r.epoch, r.height])].join('|');
      for (const [el, value] of measuredSizes) if (s.observed.has(el)) s.observed.set(el, value);
      if (s.signature === signature) continue;
      let bytes;
      try { bytes = collectionBytes(facts); } catch { continue; }
      s.budget--; reportsLeft--;
      for (const el of s.observed.keys()) if (!measuredSizes.has(el)) s.observed.set(el, size(el));
      measuredSizes.clear();
      delivering = true;
      let accepted;
      try { accepted = report(bytes) !== false; } finally { delivering = false; }
      if (accepted) {
        s.signature = signature; s.lastFacts = facts;
        if(!notification){notification=true;queueMicrotask(()=>{notification=false;settled();});}
        if (releases.includes(s)) for (const held of waiting) enqueue(held);
      }
    }
    schedule();
  }
  function detach(s) {
    dirty.delete(s); waiting.delete(s); s.observer.disconnect();
    for (const row of s.rows) if (row.el) rowOwners.delete(row.el);
    s.port.removeEventListener('scroll', s.scrolled);
    s.el.style.overflowAnchor = s.anchor;
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
      const s=states.get(view); if(!s?.valid||portOf(s.el)!==s.port)return null;
      if(scrollChanged(s))enqueue(s,true);
      enqueue(s);if(!delivering)flush(true);
      const g=geometry(s),f=s.lastFacts,p=viewport(s.port);
      if(!g||!f||f.scroll_top!==Math.max(0,g.raw)||f.port_width!==g.width||f.port_height!==g.height
        ||f.row_width!==g.rowWidth||BigInt(f.scroll_sequence)!==s.sequence)return undefined;
      return {revision:s.snapshot.revision,scrollSequence:String(s.sequence),scrollTop:f.scroll_top,
        portWidth:g.width,portHeight:g.height,rowWidth:g.rowWidth,totalExtent:s.snapshot.totalExtent,
        contentY:y-p.top+g.raw,raw:g.raw,port:s.port,portTop:p.top};
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
        ||!g||old?.interaction_view!==lease.view||old.scroll_top!==Math.max(0,g.raw)
        ||old.port_width!==g.width||old.port_height!==g.height||old.row_width!==g.rowWidth
        ||s.port.scrollTop!==s.scrollTop) return null;
      const facts={...old,revision:s.snapshot.revision,interaction_view:view,measurements:[]};
      const bytes=collectionBytes(facts);reportsLeft--;delivering=true;let accepted;
      try { accepted=report(bytes)!==false; } finally { delivering=false; }
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
        const port = portOf(el);
        let s = states.get(snapshot.view);
        if (s && (s.el !== el || s.port !== port)) { detach(s); s = null; }
        if (s && BigInt(snapshot.revision) < BigInt(s.snapshot.revision)) continue;
        if (!s) {
          s = { el, port, snapshot, rows: [], valid: false, observed: new Map(), budget: 2,
            sequence: BigInt(snapshot.scrollSequence), scrollTop: port.scrollTop,
            dimensions: null, signature: null, lastFacts: null, corrected: null, anchor: el.style.overflowAnchor };
          s.scrolled = () => { if (scrollChanged(s)) enqueue(s, true); };
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
        const correction = snapshot.correction;
        if (correction && s.corrected !== snapshot.revision
            && BigInt(correction.scrollSequence) === s.sequence
            && Number.isFinite(correction.scrollTop) && correction.scrollTop >= 0) {
          const g = geometry(s);
          if (g && (s.dimensions === null || s.dimensions === `${g.width},${g.height},${g.rowWidth}`)) {
            s.corrected = snapshot.revision;
            // Relative conversion also handles a list below siblings in its port.
            port.scrollTop += correction.scrollTop - g.raw;
            s.scrollTop = port.scrollTop; // consume the programmatic scroll echo
          }
        }
        observe(s);
        enqueue(s, !delivering || freshRows);
      }
      if (!dirty.size && frame !== null && !delivering) { cancelFrame(frame); frame = null; }
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
