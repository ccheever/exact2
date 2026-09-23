// @ref LLP 1038 D6/D7/D11 — host projection and the last committed URL.
let last = null;
let refused = new WeakMap();
let written = [], gone = new Set(), cursor = 0, first = null, originIndex = null;
let echo = null, pop = null, draining = false;
const queue = [];
const waiters = new Set();
let root, navigate, log;
const routesOf = nav => nav ? [...nav.children].filter(r => r.hasAttribute("navigationKey")) : [];
const selectedRoute = nav => routesOf(nav).find(r => r.getAttribute("navigationKey") === nav.getAttribute("navigationKey"));
const browserIndex = () => globalThis.navigation?.currentEntry?.index ?? null;
const stamp = (index, op) => ({ exact: index, id: op.top, url: op.url });

function pressBack(nav) {
  const route = selectedRoute(nav);
  if (!route || ["modal", "fullscreen"].includes(route.getAttribute("navigationPresentation")) && route.getAttribute("closedby") === "none") return;
  const control = [...route.querySelectorAll("[id]")].find(node => node.id === nav.getAttribute("navigationBack"));
  if (control && !control.matches(":disabled") && !control.closest("[inert]")
      && control.getClientRects().length && getComputedStyle(control).visibility === "visible") control.click();
}

function go(to, from, finish = () => {}) {
  const index = browserIndex();
  const current = index !== null && originIndex !== null ? index - originIndex : from;
  if (to === current) { finish(); return; }
  echo = { index: to, finish };
  history.go(to - current);
}

function commit(op) {
  for (const id of op.removed) gone.add(id);
  if (first === null) {
    first = 0;
    originIndex = browserIndex();
    written[0] = stamp(0, op);
    history.replaceState(written[0], "", location.origin + op.url);
  } else if (written[cursor]?.id === op.top) {
    if (written[cursor].url !== op.url) {
      written[cursor] = stamp(cursor, op);
      history.replaceState(written[cursor], "", location.origin + op.url);
    }
  } else {
    let j = cursor;
    while (j > first && gone.has(written[j]?.id)) {
      j--;
      if (written[j]?.id === op.top) {
        const from = cursor;
        cursor = j;
        go(j, from);
        return;
      }
    }
    for (const index of Object.keys(written)) if (Number(index) > cursor) delete written[index];
    written.length = Math.max(0, cursor + 1);
    written[++cursor] = stamp(cursor, op);
    history.pushState(written[cursor], "", location.origin + op.url);
  }
}

function popped({ j, state, url }) {
  const entry = written[j];
  const owned = entry && state?.exact === j && state.id === entry.id && state.url === entry.url;
  const target = owned ? entry.url : url;
  const nav = root.querySelector("[navigationBack]");
  const routes = routesOf(nav), selected = routes.indexOf(selectedRoute(nav));
  const back = owned && j === cursor - 1 && selected > 0
    && routes[selected - 1].getAttribute("navigationKey") === String(entry.id);
  pop = {};
  try {
    if (back) pressBack(nav);
    else navigate(target);
    const accepted = back ? last?.top === entry.id : pop.op?.url === target;
    if (accepted) {
      cursor = j ?? cursor;
      first = Math.min(first, cursor);
      written[cursor] = stamp(cursor, last);
      go(cursor, j ?? cursor, () => history.replaceState(written[cursor], "", location.origin + written[cursor].url));
    } else if (pop.op) {
      const op = pop.op;
      if (j !== null && j !== cursor) go(cursor, j, () => commit(op));
      else {
        history.replaceState(written[cursor], "", location.origin + written[cursor].url);
        commit(op);
      }
    } else {
      if (back) log("history: Back refused; restoring the entry");
      else log(`history: navigate ${JSON.stringify(target)} refused; restoring the entry`);
      if (j !== null && j !== cursor) go(cursor, j);
      else history.replaceState(written[cursor], "", location.origin + written[cursor].url);
    }
  } finally { pop = null; }
}

function drain() {
  if (draining || echo !== null) return;
  draining = true;
  try {
    while (queue.length && echo === null) {
      const item = queue.shift();
      if (item.op) commit(item.op);
      else popped(item);
    }
  } finally { draining = false; }
}

function settled() {
  if (echo === null && !queue.length) for (const finish of [...waiters]) finish();
}

export const navigation = {
  connect(hostRoot, dispatch, journal, routeMatches) {
    root = hostRoot; navigate = dispatch; log = journal;
    // @ref LLP 1038 §7 — a plain click on a same-origin link to a declared
    // route stays in this document: a link with its own `press` navigates by
    // it; any other goes to the root's `navigate` handler, as popstate does.
    // Modified, other-button and targeted clicks, downloads, other origins,
    // fragments of this page and undeclared paths stay the browser's.
    root.addEventListener("click", event => {
      const a = event.target.closest?.("a[href]");
      if (!a || event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
        || (a.target && a.target !== "_self") || a.hasAttribute("download")) return;
      const url = new URL(a.href), to = url.pathname + url.search, here = to === location.pathname + location.search;
      if (url.origin !== location.origin || (here && url.hash) || !routeMatches(to)) return;
      const press = a.exactHandlers?.includes("press"), nav = root.firstElementChild;
      if (!press && !(nav?.hasAttribute("navigationBack") && nav.exactHandlers?.includes("navigate"))) return;
      event.preventDefault();
      if (press || here) return;
      const before = last;
      navigate(to);
      if (last === before) log(`history: link ${JSON.stringify(to)} refused`);
    }, true);
    addEventListener("popstate", event => {
      if (!last) return;
      const index = browserIndex();
      const j = index !== null && originIndex !== null ? index - originIndex
        : Number.isInteger(event.state?.exact) ? event.state.exact : null;
      if (echo !== null && j === echo.index) {
        const finish = echo.finish; echo = null; finish(); drain(); settled(); return;
      }
      queue.push({ j, state: event.state, url: location.pathname + location.search });
      if (echo !== null) {
        const pending = echo; echo = null;
        go(pending.index, j ?? cursor, pending.finish);
      }
      drain(); settled();
    });
    document.addEventListener("keydown", event => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (!root.contains(event.target) && event.target !== document.body && event.target !== document.documentElement) return;
      if (document.querySelector("dialog:modal") || [...document.querySelectorAll(":popover-open")].some(p => p.popover === "auto" || p.popover === "hint")) return;
      for (const nav of root.querySelectorAll("[navigationBack]")) {
        if (!["modal", "fullscreen"].includes(selectedRoute(nav)?.getAttribute("navigationPresentation"))) continue;
        event.preventDefault(); pressBack(nav); return;
      }
    });
  },
  reset(op = null) {
    refused = new WeakMap();
    if (op && written[cursor]?.id === op.top) return;
    last = null;
    written = []; gone = new Set(); cursor = 0; first = null; originIndex = null;
    echo = null; pop = null; queue.length = 0;
    for (const finish of [...waiters]) finish();
  },
  apply(op) {
    last = op;
    if (pop) { pop.op = op; for (const id of op.removed) gone.add(id); }
    else { queue.push({ op }); drain(); }
  },
  // @ref LLP 1038 D11 — the agent uses the real browser traversal.
  travel(nav, delta) {
    if (nav !== root.querySelector("[navigationBack]")) return { error: "history target is not the navigation root" };
    if (!Number.isInteger(delta) || delta === 0) return { error: "history must be a nonzero integer" };
    return new Promise(resolve => {
      const timer = setTimeout(finish, 1000);
      function finish() {
        clearTimeout(timer); waiters.delete(finish);
        resolve({ history: delta, delivery: "platform" });
      }
      waiters.add(finish); history.go(delta);
    });
  },
  project(root, log) {
    for (const nav of root.querySelectorAll("[navigationBack]")) {
      const routes = [...nav.children].filter(route => route.hasAttribute("navigationKey"));
      const selected = routes.findIndex(route => route.getAttribute("navigationKey") === nav.getAttribute("navigationKey"));
      if (selected < 0) {
        const key = nav.getAttribute("navigationKey");
        if (refused.get(nav) !== key) {
          refused.set(nav, key);
          log(`navigationKey "${key}" matches no route; the stack is unchanged`);
        }
        continue;
      }
      refused.delete(nav);
      const modal = routes[selected]?.getAttribute("navigationPresentation") === "modal";
      for (const [index, route] of routes.entries()) {
        const active = index === selected;
        if (!active && route.contains(document.activeElement)) document.activeElement.blur();
        route.style.visibility = active || (modal && index === selected - 1) ? "" : "hidden";
        route.inert = !active || !!route.authoredInert;
      }
    }
  },
  observation(root) {
    const nav = root.querySelector("[navigationBack]");
    const routes = nav ? [...nav.children].filter((r) => r.hasAttribute("navigationKey")) : [];
    const key = nav?.getAttribute("navigationKey") ?? null;
    const index = routes.findIndex((r) => r.getAttribute("navigationKey") === key);
    const selected = index >= 0 ? routes[index] : null;
    return {
      url: last?.url ?? null,
      route: key,
      stack: index >= 0 ? routes.slice(0, index + 1).map((r) => r.getAttribute("navigationKey")) : [],
      presentation: ["modal", "fullscreen"].includes(selected?.getAttribute("navigationPresentation")) ? selected.getAttribute("navigationPresentation") : null,
      source: selected?.getAttribute("navigationSource") ?? null,
      closedby: selected?.getAttribute("closedby") ?? null,
      transition: { interactive: false, phase: "idle" },
    };
  },
};

// @ref LLP 1010 §6 — committed DOM projection, alongside navigation above.
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

// The eager scrollFollowEnd projection also belongs to this DOM controller.
export function scrollFollowers(positionContexts) {
// An explicit chat/log policy, not CSS overflow anchoring: keep the end
// visible across resizing only while the reader is already there.
const followedScrolls = new Map();
function rememberScroll(s) {
  s.top = s.el.scrollTop; s.height = s.el.scrollHeight; s.port = s.el.clientHeight;
  s.end = s.top >= s.height - s.port - 1;
}
function settleFollow(s) {
  if (!s.el.isConnected) return;
  // A reader above the end uses the browser's CSS scroll anchoring. Writing
  // the remembered numeric offset here would undo its adjustment when content
  // above the visible message changes.
  if (s.end) s.el.scrollTop = s.el.scrollHeight - s.el.clientHeight;
  rememberScroll(s);
  const children = [...s.el.children];
  if (children.length !== s.children.length || children.some((el, i) => el !== s.children[i])) {
    s.observer.disconnect(); s.observer.observe(s.el);
    for (const child of children) s.observer.observe(child);
    s.children = children;
  }
}
function followScroll(el, enabled) {
  const old = followedScrolls.get(el);
  if (old || !enabled) {
    if (old && !enabled) { old.observer.disconnect(); el.removeEventListener("scroll", old.scrolled); followedScrolls.delete(el); }
    return;
  }
  const s = { el, top: 0, height: 0, port: 0, end: true, children: [] };
  s.scrolled = () => {
    // ResizeObserver settles a changed geometry before a queued scroll
    // notification is allowed to change whether the reader follows the end.
    if (el.scrollHeight === s.height && el.clientHeight === s.port) rememberScroll(s);
  };
  s.observer = new ResizeObserver(() => { settleFollow(s); positionContexts(); });
  s.observer.observe(el); el.addEventListener("scroll", s.scrolled, { passive: true });
  followedScrolls.set(el, s);
}
  return { followedScrolls, followScroll, settleFollow, rememberScroll };
}

// Presentation ownership is a host projection. The shared Engine owns hold
// validity and authored targets; this controller owns actual browser sampling.
export function motionBytes(facts) {
  const {op,view=0,property='translate',token=0,x=0,y=0,now=0}=facts;
  const reorder=['reorder-begin','reorder-preview','reorder-terminal','reorder-cancel','reorder-rebase','reorder-finish'].indexOf(op);
  if(reorder>=0) {
    const rows=facts.rows??[];if(rows.length>4096)throw Error('too many reorder samples');
    const bytes=new Uint8Array(176+32*rows.length),d=new DataView(bytes.buffer);
    const u64=(at,value=0)=>{if(typeof value==='number'&&!Number.isSafeInteger(value))throw Error('unsafe reorder identity');
      const n=BigInt(value);if(n<0n||n>0xffffffffffffffffn)throw Error('invalid reorder identity');d.setBigUint64(at,n,true);};
    d.setUint32(0,3,true);d.setUint32(4,15+reorder,true);
    ['runtime','handleKey','listKey','wrapperKey','rootKey','rowEpoch','token','revision','scrollSequence'].forEach((k,i)=>u64(8+i*8,facts[k]));
    d.setUint32(80,rows.length,true);
    ['scrollTop','portWidth','portHeight','rowWidth','totalExtent','contentY','x','y','vx','vy','now'].forEach((k,i)=>d.setFloat64(88+i*8,facts[k]??0,true));
    rows.forEach((r,i)=>{u64(176+i*32,r.key);u64(184+i*32,r.hold);d.setFloat64(192+i*32,r.value[0],true);d.setFloat64(200+i*32,r.value[1],true);});
    return bytes;
  }
  const transform=['transform-geometry','transform-begin','transform-move','transform-action','transform-invalidate'].indexOf(op);
  if(transform>=0) {
    // Frozen Rust counterpart: v2, 120 LE bytes; serials never pass through Number.
    const bytes=new Uint8Array(120),d=new DataView(bytes.buffer);
    d.setUint32(0,2,true);d.setUint32(4,10+transform,true);
    for(const [i,name] of ['runtime','handleKey','targetKey','clipKey','geometrySequence','translateToken','scaleToken'].entries()) {
      const value=facts[name]??0;
      if(typeof value==='number'&&!Number.isSafeInteger(value))throw Error('unsafe transform identity');
      const n=BigInt(value);if(n<0n||n>0xffffffffffffffffn)throw Error('invalid transform identity');
      d.setBigUint64(8+i*8,n,true);
    }
    if(!Array.isArray(facts.values)||facts.values.length!==6)throw Error('invalid transform tuple');
    for(let i=0;i<6;i++)d.setFloat64(64+i*8,facts.values[i],true);
    d.setFloat64(112,now,true);return bytes;
  }
  const operations=['begin','move','release','cancel','live','action','height-owner','clear-height-owner','height-begin','height-action'];
  const properties=['translate','scale','rotate','opacity','height'];
  const code=operations.indexOf(op), prop=properties.indexOf(property);
  if(code<0||prop<0) throw Error('invalid motion operation');
  const bytes=new Uint8Array(48), d=new DataView(bytes.buffer), serial=BigInt(token);
  if(serial<0n||serial>0xffffffffffffffffn || typeof token==='number'&&!Number.isSafeInteger(token)) throw Error('invalid hold serial');
  d.setUint32(0,1,true); d.setUint32(4,code,true); d.setUint32(8,view,true); d.setUint32(12,prop,true);
  d.setBigUint64(16,serial,true); d.setFloat64(24,x,true); d.setFloat64(32,y,true); d.setFloat64(40,now,true);
  return bytes;
}
export function motionController({views,now,generation,request,applyBatch,inert,releaseInteraction=()=>{},ready=()=>true}) {
  const properties=['translate','scale','rotate','opacity','height'];
  const animations=new Map(), held=new Map(), authored=new Map(), drags=new Map();
  const raised=new Set();
  const active=new Map(), heightBindings=new Map(); let reconciling=false;
  const transformBindings=new Map(), geometryDirty=new Set();
  let geometryFrame=null, geometryDelivering=false, geometrySerial=0n;
  const key=(id,property)=>`${id}/${property}`;
  // CSS height clamps negative interpolated lengths. Keep every spring sample
  // and its timing; only its displayed length changes, not the engine curve.
  const css=(property,value)=>property==='translate'?`${value[0]}px ${value[1]}px`:property==='rotate'?`${value[0]}deg`:property==='height'?`${Math.max(0,value[0])}px`:String(value[0]);
  const call=(op,h,value=[0,0])=>request({op,view:h.view,property:h.property,token:h.token??0,x:value[0],y:value[1],now:now()});
  const local=h=>h && h.generation===generation() && views.get(h.view)===h.el && h.el.isConnected && held.get(key(h.view,h.property))===h;
  const eligible=el=>el?.isConnected&&!el.closest('[disabled]')&&!el.matches(':disabled')&&!inert(el)&&el.getClientRects().length>0&&getComputedStyle(el).visibility==='visible';
  const live=h=>local(h)&&call('live',h).accepted===true;
  function cancelProperty(id,property,el=views.get(id)) {
    const k=key(id,property); animations.get(k)?.cancel(); animations.delete(k);
    // Browser easing is a CSSTransition; other properties continue undisturbed.
    for(const animation of el?.getAnimations()??[]) {
      if(animation.effect?.target===el && animation.transitionProperty===property) animation.cancel();
    }
  }
  function overlay(id) {
    const el=views.get(id); if(!el) return;
    const active=properties.map(p=>held.get(key(id,p))).filter(local);
    if(raised.has(id)){el.style.zIndex='2147483647';if(getComputedStyle(el).position==='static')el.style.position='relative';}
    if(!active.length) return;
    const transition=el.style.transition;
    el.style.transition=[...(transition && transition!=='none'?[transition]:[]), ...active.map(h=>`${h.property} 0s linear 0s`)].join(',');
    for(const h of active) { el.style.setProperty(h.property,css(h.property,h.value)); cancelProperty(id,h.property,el); }
  }
  function restore(id) {
    const el=views.get(id); if(!el) return;
    if(authored.has(id)) el.style.cssText=authored.get(id);
    overlay(id);
    if(!raised.has(id)&&!properties.some(p=>held.has(key(id,p)))) authored.delete(id);
  }
  function sample(el,property) {
    const text=getComputedStyle(el).getPropertyValue(property), numbers=text.trim().split(/\s+/);
    if(property==='translate') {
      if(text==='none') return [0,0];
      const box=el.getBoundingClientRect();
      return [0,1].map(i=>numbers[i]?.endsWith('%')?parseFloat(numbers[i])*[box.width,box.height][i]/100:parseFloat(numbers[i]??'0'));
    }
    if(property==='scale' && numbers.length>1 && Number(numbers[0])!==Number(numbers[1])) return null;
    return [text==='none'?(property==='scale'?1:0):parseFloat(text),0];
  }
  function adopt(view,property,reply,adopted) {
    const el=views.get(view);
    if(!el||!reply.token) return null;
    if(!authored.has(view)) authored.set(view,el.style.cssText);
    const h={view,property,el,token:reply.token,generation:generation(),value:reply.value};
    held.set(key(view,property),h);
    cancelProperty(view,property,el); restore(view);
    // A begin batch can retire its binding synchronously. The recognizer must
    // already own this token when that cancellation is delivered.
    adopted?.(h);
    if(reply.batch) applyBatch(reply.batch);
    return h;
  }
  function adoptPair(b,reply,adopted) {
    if(!reply.translateToken||!reply.scaleToken||reply.runtime!==b.runtime||reply.geometrySequence!==b.sequence)return null;
    const el=b.targetEl,view=b.target;
    if(!authored.has(view))authored.set(view,el.style.cssText);
    const pair=['translate','scale'].map((property,i)=>({view,property,el,token:i?reply.scaleToken:reply.translateToken,
      generation:generation(),runtime:b.runtime,value:i?[reply.value[2],0]:reply.value.slice(0,2)}));
    // BOTH records and recognizer ownership precede any synchronous begin batch.
    for(const h of pair)held.set(key(view,h.property),h);
    for(const h of pair)cancelProperty(view,h.property,el);
    // The admitted target may have a browser-owned pair curve, not only a
    // registered spring. Snapshot admission excludes curves coupled to another
    // property, so this cannot cancel an unrelated opacity/geometry animation.
    for(const a of el.getAnimations())if(a.effect?.target===el&&pairCurve(a).pair)a.cancel();
    restore(view);adopted(pair);
    if(reply.batch)applyBatch(reply.batch);
    return pair;
  }
  const bindingLive=b=>b&&heightBindings.get(b.id)===b&&views.get(b.id)===b.el&&views.get(b.target)===b.targetEl&&eligible(b.el)&&eligible(b.targetEl);
  const maxPixel=3.4028234663852886e38;
  const pixel=v=>Number.isFinite(v)&&Math.abs(v)<=maxPixel;
  const positiveScale=v=>pixel(v)&&v>0&&Math.fround(v)>0;
  const transformLocal=b=>b&&b.generation===generation()&&transformBindings.get(b.id)===b
    &&views.get(b.id)===b.el&&views.get(b.target)===b.targetEl&&views.get(b.clip)===b.clipEl
    &&b.el.isConnected&&b.targetEl.isConnected&&b.clipEl.isConnected;
  const px=text=>/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?px$/i.test(text)?Number(text.slice(0,-2)):NaN;
  function transformSample(cs) {
    const t=cs.translate==='none'?['0px','0px']:cs.translate.trim().split(/\s+/);
    const s=cs.scale==='none'?['1']:cs.scale.trim().split(/\s+/);
    if(t.length>2||s.length>2||!s.length||s.length===2&&Number(s[0])!==Number(s[1]))return null;
    const value=[px(t[0]),px(t[1]??'0px'),Number(s[0])];
    return pixel(value[0])&&pixel(value[1])&&positiveScale(value[2])?value:null;
  }
  function pairCurve(animation) {
    const props=new Set(animation.effect.getKeyframes().flatMap(frame=>Object.keys(frame))
      .filter(p=>!['offset','computedOffset','easing','composite'].includes(p)));
    return {pair:props.has('translate')||props.has('scale'),coupled:[...props].some(p=>p!=='translate'&&p!=='scale')};
  }
  function transformSnapshot(b) {
    if(!transformLocal(b)||!eligible(b.el)||!eligible(b.targetEl)||!eligible(b.clipEl))return null;
    const target=getComputedStyle(b.targetEl),clip=getComputedStyle(b.clipEl),value=transformSample(target);
    const zeroInsets=cs=>['paddingTop','paddingRight','paddingBottom','paddingLeft','borderTopWidth','borderRightWidth','borderBottomWidth','borderLeftWidth'].every(p=>px(cs[p])===0);
    if(!value||target.boxSizing!=='border-box'||!zeroInsets(target)||!zeroInsets(clip)
      ||clip.overflowX!=='hidden'||clip.overflowY!=='hidden'||b.targetEl.parentElement!==b.clipEl)return null;
    const dimensions=[px(target.width),px(target.height),px(clip.width),px(clip.height)];
    if(!dimensions.every(v=>pixel(v)&&v>=0)||!['marginTop','marginRight','marginBottom','marginLeft'].every(p=>px(target[p])===0))return null;
    const close=(x,y)=>Math.abs(x-y)<=Math.max(.02,Math.max(Math.abs(x),Math.abs(y))*Number.EPSILON*8);
    if(!close(dimensions[0],dimensions[2])||!close(dimensions[1],dimensions[3]))return null;
    const origin=target.transformOrigin.split(/\s+/).map(px);
    if(origin.length<2||origin.length>3||!close(origin[0],dimensions[0]/2)||!close(origin[1],dimensions[1]/2)||origin.length===3&&origin[2]!==0)return null;
    const path=[];
    for(let el=b.el;el;el=el.parentElement) {
      path.push(el);const cs=el===b.targetEl?target:el===b.clipEl?clip:getComputedStyle(el),sample=transformSample(cs);
      if(!sample||!['none','0deg'].includes(cs.rotate)||cs.transform!=='none'||cs.perspective!=='none'
        ||cs.transformStyle==='preserve-3d'||!['1','normal',''].includes(cs.zoom))return null;
      if(el!==b.targetEl&&(sample[0]!==0||sample[1]!==0||sample[2]!==1))return null;
      // A current identity crossing/delay is not proof that the ancestor curve
      // stays identity. Refuse unsupported active curves, not decompose them.
      for(const a of el.getAnimations()) {
        if(a.effect?.target!==el||['idle','finished'].includes(a.playState))continue;
        const forbidden=['rotate','transform','transformOrigin','transform-origin','perspective','zoom',...(el===b.targetEl?[]:['translate','scale'])];
        if(forbidden.includes(a.transitionProperty)||a.effect.getKeyframes().some(frame=>forbidden.some(p=>p in frame)))return null;
        const curve=pairCurve(a);if(el===b.targetEl&&curve.pair&&curve.coupled)return null;
      }
    }
    if(!path.includes(b.targetEl)||!path.includes(b.clipEl))return null;
    const tr=b.targetEl.getBoundingClientRect(),cr=b.clipEl.getBoundingClientRect();
    if(![tr.x,tr.y,tr.width,tr.height,cr.x,cr.y,cr.width,cr.height].every(Number.isFinite)
      ||!close(tr.x+tr.width/2-value[0],cr.x+cr.width/2)||!close(tr.y+tr.height/2-value[1],cr.y+cr.height/2))return null;
    return {dimensions,origin:[cr.x,cr.y,b.clipEl.scrollLeft,b.clipEl.scrollTop],path,value};
  }
  const sameGeometry=(a,b)=>a===b||!!(a&&b&&a.dimensions.every((v,i)=>v===b.dimensions[i])
    &&a.origin.every((v,i)=>v===b.origin[i])&&a.path.length===b.path.length&&a.path.every((v,i)=>v===b.path[i]));
  function transformFacts(b,op,values,pair=null,time=now()) {
    return {op,runtime:b.runtime,handleKey:b.handleKey,targetKey:b.targetKey,clipKey:b.clipKey,geometrySequence:b.sequence,
      translateToken:pair?.[0].token??0,scaleToken:pair?.[1].token??0,values,now:time};
  }
  function watchTransformPath(b,path) {
    const next=new Set([...path,window]);
    for(const el of b.observedPath)if(!next.has(el))el.removeEventListener('scroll',b.scrolled);
    for(const el of next)if(!b.observedPath.has(el))el.addEventListener('scroll',b.scrolled,{passive:true});
    b.observedPath=next;
  }
  function enqueueGeometry(b) {
    if(!transformLocal(b)||b.exhausted)return;
    geometryDirty.add(b);
    if(geometryFrame===null)geometryFrame=requestAnimationFrame(()=>{
      geometryFrame=null;if(!ready())return;
      for(const b of [...geometryDirty])flushGeometry(b);
    });
  }
  function checkGeometry(b) {
    if(!transformLocal(b))return null;
    const next=transformSnapshot(b);
    if(!sameGeometry(b.facts,next)) {
      // Retire contact BEFORE any geometry callback; the original Engine pair
      // remains held until the accepted geometry receipt has its latest targets.
      drags.get(b.id)?.suspend?.();
      enqueueGeometry(b);
    }
    return next;
  }
  function flushGeometry(b) {
    if(!transformLocal(b)||!ready()||geometryDelivering||b.exhausted)return false;
    geometryDirty.delete(b);
    const next=transformSnapshot(b);
    if(sameGeometry(b.facts,next)) {
      // Mapping can move away and back before this coalesced callback. Contact
      // already ended on the first change; its original pair still needs cleanup.
      const ends=b.pendingEnds;b.pendingEnds=null;
      for(const h of ends??[])api.end(h,[0,0],true);
      return b.admitted;
    }
    drags.get(b.id)?.suspend?.();
    const ends=b.pendingEnds;b.pendingEnds=null;
    const previous=b.facts;b.facts=next;b.admitted=false;
    if(next)watchTransformPath(b,next.path);
    if(!next&&previous===undefined)return false;
    if(geometrySerial===0xffffffffffffffffn){b.exhausted=true;for(const h of ends??[])api.end(h,[0,0],true);return false;}
    b.sequence=String(++geometrySerial);
    geometryDelivering=true;
    try {
      const reply=request(transformFacts(b,next?'transform-geometry':'transform-invalidate',next?[...next.dimensions,0,0]:[0,0,0,0,0,0]));
      if(reply.batch)applyBatch(reply.batch);
      if(transformLocal(b))b.admitted=reply.accepted===true&&!reply.batch?.error&&!!next&&next.dimensions.every(v=>v>0);
    } finally {
      geometryDelivering=false;
      for(const h of ends??[])api.end(h,[0,0],true);
    }
    if(transformLocal(b)&&!sameGeometry(b.facts,transformSnapshot(b))){b.admitted=false;enqueueGeometry(b);}
    return b.admitted;
  }
  function detachTransform(b) {
    // Remove ownership before cancellation can synchronously publish a binding.
    geometryDirty.delete(b);transformBindings.delete(b.id);
    b.observer.disconnect();for(const el of b.observedPath)el.removeEventListener('scroll',b.scrolled);
    const ends=b.pendingEnds;b.pendingEnds=null;
    drags.get(b.id)?.();
    for(const h of ends??[])api.end(h,[0,0],true);
  }
  const api={
    presentReorder(view,token,value) {
      const h=held.get(key(view,'translate'));if(!local(h)||h.token!==token)return false;
      h.value=value;h.el.style.translate=css('translate',value);return true;
    },
    raiseReorder(id,enabled) {
      const el=views.get(id);if(!el)return;
      if(enabled){if(!authored.has(id))authored.set(id,el.style.cssText);raised.add(id);}else raised.delete(id);
      restore(id);
    },
    captureReorder(frame) {
      return frame.map(row=>{const el=views.get(row.view),v=el&&sample(el,'translate'),r=el?.getBoundingClientRect();
        return v?.every(Number.isFinite)&&r?{...row,el,value:v,visual:[r.x,r.y]}:null;}).filter(Boolean);
    },
    adoptReorder(frame,samples,runtime) {
      const captured=new Map(samples.map(s=>[s.key,s]));
      for(const row of frame){const s=captured.get(row.key);if(!s||!row.hold||row.hold==='0'||views.get(row.view)!==s.el)continue;
        const old=held.get(key(row.view,'translate'));if(old?.token===row.hold)continue;
        const h=adopt(row.view,'translate',{token:row.hold,value:s.value});if(h)h.runtime=runtime;}
    },
    rebaseReorder(samples) {
      const rows=[];
      for(const s of samples){const h=held.get(key(s.view,'translate'));if(!local(h)||h.token!==(s.hold??s.token))continue;
        const r=h.el.getBoundingClientRect();h.value=[h.value[0]+s.visual[0]-r.x,h.value[1]+s.visual[1]-r.y];
        h.el.style.translate=css('translate',h.value);rows.push({...s,value:h.value});}
      return rows;
    },
    releaseReorder(samples) {
      for(const s of samples){const h=held.get(key(s.view,'translate'));if(!local(h)||h.token!==s.hold)continue;
        h.el.getBoundingClientRect();held.delete(key(s.view,'translate'));restore(s.view);}
    },
    reorderSettled(view) {
      return !(views.get(view)?.getAnimations()??[]).some(a=>a.effect?.target===views.get(view)
        &&(a===animations.get(key(view,'translate'))||a.transitionProperty==='translate')
        &&!['idle','finished'].includes(a.playState)&&Number(a.currentTime)<a.effect.getComputedTiming().endTime);
    },
    settleReorder(view,finish,valid=()=>true) {
      const check=()=>{if(!valid())return;if(api.reorderSettled(view)){finish();return;}
        const el=views.get(view),pending=el.getAnimations().filter(a=>a.effect?.target===el
          &&(a===animations.get(key(view,'translate'))||a.transitionProperty==='translate')&&!['idle','finished'].includes(a.playState));
        Promise.allSettled(pending.map(a=>a.finished)).then(check);};check();
    },
    // The Kernel resolves authored IDREFs; DOM code only consumes these exact
    // generational bindings, emitted after the tree's create/attach operations.
    heightBinding(op) {
      const old=heightBindings.get(op.id);
      if(old&&old.target===op.target&&old.handleKey===op.handleKey&&old.targetKey===op.targetKey&&views.get(op.id)===old.el&&views.get(op.target)===old.targetEl) return;
      drags.get(op.id)?.(); heightBindings.delete(op.id);
      if(op.target!==null&&views.has(op.id)&&views.has(op.target)) heightBindings.set(op.id,{...op,el:views.get(op.id),targetEl:views.get(op.target)});
    },
    transformBinding(op) {
      const old=transformBindings.get(op.id);
      if(old&&['runtime','handleKey','target','targetKey','clip','clipKey'].every(k=>old[k]===op[k])&&transformLocal(old))return;
      if(old)detachTransform(old);
      if(transformBindings.has(op.id))return;
      if(op.target===null||op.clip===null||!views.has(op.id)||!views.has(op.target)||!views.has(op.clip))return;
      const b={...op,el:views.get(op.id),targetEl:views.get(op.target),clipEl:views.get(op.clip),generation:generation(),
        sequence:'0',facts:undefined,admitted:false,observedPath:new Set(),pendingEnds:null};
      b.scrolled=()=>checkGeometry(b);
      b.observer=new ResizeObserver(()=>checkGeometry(b));
      transformBindings.set(op.id,b);b.observer.observe(b.targetEl);b.observer.observe(b.clipEl);enqueueGeometry(b);
    },
    setHeightOwner(view=null) {
      const reply=request({op:view===null?'clear-height-owner':'height-owner',view:view??0,property:'height',now:now()});
      if(reply.accepted!==true) return false;
      if(reply.batch) applyBatch(reply.batch);
      return true;
    },
    begin(view,property) {
      const el=views.get(view); if(!eligible(el)) return null;
      const value=sample(el,property); if(!value?.every(Number.isFinite)) return null;
      return adopt(view,property,call('begin',{view,property},value));
    },
    move(h,value) {
      if(!local(h)) return false;
      const reply=call('move',h,value); if(reply.accepted!==true) return false;
      h.value=value; h.el.style.setProperty(h.property,css(h.property,value));
      if(reply.batch) applyBatch(reply.batch);
      return true;
    },
    end(h,velocity=[0,0],cancel=false) {
      if(!local(h)) return false;
      const reply=call(cancel?'cancel':'release',h,velocity);
      if(!local(h)) return false;
      if(reply.accepted!==true) {
        // A receipt may have cancelled Rust's token before its binding removal
        // reaches the DOM. An explicit stale reply retires this exact overlay;
        // a validation error must leave a still-live hold intact.
        if(reply.accepted===false) { held.delete(key(h.view,h.property)); restore(h.view); }
        return false;
      }
      // Flush held presentation before restoring the latest easing/target.
      // A spring batch below installs its first frame before this turn paints.
      h.el.getBoundingClientRect(); held.delete(key(h.view,h.property)); restore(h.view);
      if(reply.batch) applyBatch(reply.batch);
      return true;
    },
    finish(h,value,velocity,commit=false,cancel=false) {
      if(local(h)&&!eligible(h.el)) { api.end(h,[0,0],true); return false; }
      if(!live(h)||!api.move(h,value)) return false;
      if(commit) { const reply=call('action',h); if(reply.batch) applyBatch(reply.batch); }
      // The action may delete the held node. That makes this a harmless no-op.
      api.end(h,velocity,cancel); return true;
    },
    // Only live gestures/holds are visited, never all mounted swipe handlers.
    // Cancel may synchronously apply another batch; retire once before reentry.
    commit() {
      if(reconciling) return;
      reconciling=true;
      try {
        for(const b of transformBindings.values())checkGeometry(b);
        for(const [id,stop] of [...active]) if(!eligible(views.get(id))||(stop.valid&&!stop.valid())) stop();
        for(const h of [...held.values()]) if(local(h)&&!eligible(h.el)) api.end(h,[0,0],true);
      } finally { reconciling=false; }
    },
    style(id,text) {
      const el=views.get(id); if(!el) return;
      if(authored.has(id)) authored.set(id,text);
      el.style.cssText=text; overlay(id);
    },
    animate(op) {
      const {id,property}=op, k=key(id,property);
      cancelProperty(id,property);
      if(!op.values.length || held.has(k)) return;
      const el=views.get(id); if(!el) return;
      const animation=el.animate(op.values.map(value=>({[property]:css(property,property==='translate'?value:[value,0])})),
        {delay:op.delay,duration:op.duration,easing:'linear',fill:'backwards'});
      animations.set(k,animation);
      animation.finished.then(()=>{if(animations.get(k)===animation) animations.delete(k);},()=>{});
    },
    // Authored eligibility can disappear without a dirty Engine frame. Retire
    // only this property, restoring current authoring and other held overlays.
    retire(id,property,token=null,runtime=null) {
      const h=held.get(key(id,property));
      if(token!==null&&(!local(h)||h.token!==token||h.runtime!==runtime))return;
      cancelProperty(id,property);
      held.delete(key(id,property));
      restore(id);
    },
    destroy(id) {
      drags.get(id)?.(); drags.delete(id);
      for(const [handle,b] of [...heightBindings]) if(handle===id||b.target===id) { drags.get(handle)?.(); heightBindings.delete(handle); }
      for(const b of [...transformBindings.values()])if(b.id===id||b.target===id||b.clip===id)detachTransform(b);
      // Detaching cancels CSS transitions. Only our retained WAAPI animations
      // need explicit cancellation; querying getAnimations here flushes styles
      // once per retired list row while the DOM batch is still being applied.
      for(const property of properties) { cancelProperty(id,property,null); held.delete(key(id,property)); }
      raised.delete(id);authored.delete(id);
    },
    reset() {
      for(const stop of drags.values()) stop(); drags.clear();
      heightBindings.clear();
      for(const b of [...transformBindings.values()])detachTransform(b);
      geometryDirty.clear();if(geometryFrame!==null)cancelAnimationFrame(geometryFrame);geometryFrame=null;
      for(const animation of animations.values()) animation.cancel(); animations.clear();
      const ids=[...authored.keys()]; held.clear();raised.clear(); for(const id of ids) restore(id); authored.clear();
    },
    attachTransformDrag(el,id,on) {
      let drag=null,suppressClick=false;
      const interactive=e=>e.target.closest('button,a,input,textarea,select,[contenteditable]');
      const pairLocal=d=>d.pair?.every(local)&&transformLocal(d.binding)&&d.binding.admitted&&d.sequence===d.binding.sequence;
      const finiteTerminal=v=>pixel(v[0])&&pixel(v[1])&&positiveScale(v[2])&&v.slice(3).every(Number.isFinite);
      function clearContact(d) {
        if(el.hasPointerCapture(d.pointer))el.releasePointerCapture(d.pointer);
        releaseInteraction(d.pointer);
      }
      function stop(defer=false) {
        const d=drag;drag=null;active.delete(id);
        if(!d)return;
        if(d.pair) {
          suppressClick=true;
          if(defer)d.binding.pendingEnds=d.pair;
          else for(const h of d.pair)api.end(h,[0,0],true);
        }
        clearContact(d);
      }
      stop.suspend=()=>stop(true);
      stop.valid=()=>!drag||transformLocal(drag.binding)&&drag.binding.admitted&&(!drag.pair||pairLocal(drag));
      drags.set(id,stop);
      const track=(d,t,v)=>{
        d.samples.push([t,...v]);
        while(d.samples.length>2&&(d.samples.length>8||t-d.samples[0][0]>80))d.samples.shift();
      };
      const position=(d,e)=>[d.base[0]+e.clientX-d.x,d.base[1]+e.clientY-d.y,d.base[2]];
      function present(d,v) {
        d.pair[0].value=v.slice(0,2);d.pair[1].value=[v[2],0];
        for(const h of d.pair)if(local(h))h.el.style.setProperty(h.property,css(h.property,h.value));
      }
      on('pointerdown',e=>{
        if(!e.isPrimary||e.button!==0||interactive(e))return;
        const b=transformBindings.get(id);if(!transformLocal(b))return;
        stop();flushGeometry(b);
        if(!transformLocal(b)||!b.admitted||!sameGeometry(b.facts,transformSnapshot(b)))return;
        drag={pointer:e.pointerId,x:e.clientX,y:e.clientY,binding:b};active.set(id,stop);
        el.setPointerCapture(e.pointerId);e.preventDefault();
      });
      const move=e=>{
        const d=drag;if(!d||d.pointer!==e.pointerId)return false;
        checkGeometry(d.binding);
        if(drag!==d)return false;
        if(!stop.valid()){stop();return false;}
        if(!d.pair) {
          if(Math.hypot(e.clientX-d.x,e.clientY-d.y)<8)return false;
          flushGeometry(d.binding);
          if(drag!==d||!d.binding.admitted)return false;
          const snap=transformSnapshot(d.binding),t=now();
          if(!snap||!sameGeometry(d.binding.facts,snap)||!Number.isFinite(t)){stop();return false;}
          const reply=request(transformFacts(d.binding,'transform-begin',[...snap.value,0,0,0],null,t));
          if(reply.accepted!==true){if(reply.batch)applyBatch(reply.batch);stop();return false;}
          adoptPair(d.binding,reply,pair=>{d.pair=pair;d.sequence=d.binding.sequence;});
          if(drag!==d)return false;
          if(!pairLocal(d)){stop();return false;}
          d.x=e.clientX;d.y=e.clientY;d.base=[...d.pair[0].value,d.pair[1].value[0]];d.samples=[];track(d,t,d.base);
          e.preventDefault();e.stopPropagation();return true;
        }
        const value=position(d,e),t=now();
        if(!finiteTerminal([...value,0,0,0])||!Number.isFinite(t)||t<d.samples.at(-1)[0]){stop();return false;}
        const reply=request(transformFacts(d.binding,'transform-move',[...value,0,0,0],d.pair,t));
        if(reply.accepted===true)present(d,value);
        if(reply.batch)applyBatch(reply.batch);
        if(drag!==d)return false;
        if(reply.accepted!==true||!pairLocal(d)){stop();return false;}
        const shown=transformSample(getComputedStyle(d.binding.targetEl));
        if(!shown){stop();return false;}track(d,t,shown);
        e.preventDefault();e.stopPropagation();return true;
      };
      on('pointermove',move);
      const finish=e=>{
        const d=drag;if(!d||d.pointer!==e.pointerId)return;
        if(e.type!=='pointerup'||!d.pair){stop();return;}
        checkGeometry(d.binding);if(drag!==d)return;
        if(!pairLocal(d)){stop();return;}
        // Terminal preflight is whole: do NOT send an ordinary final move before
        // all samples, velocities and time are known valid. Delta is parent-space,
        // never divided by caught Scale. Primary pan has zero scale velocity.
        const v=position(d,e),t=now(),samples=[...d.samples,[t,...v]].slice(-8);
        while(samples.length>2&&t-samples[0][0]>80)samples.shift();
        const first=samples[0],dt=t-first[0],velocity=dt>0?[(v[0]-first[1])*1000/dt,(v[1]-first[2])*1000/dt,0]:[0,0,0];
        const values=[...v,...velocity];
        if(!finiteTerminal(values)||!Number.isFinite(t)||t<d.samples.at(-1)[0]){stop();return;}
        drag=null;active.delete(id);suppressClick=true;
        const reply=request(transformFacts(d.binding,'transform-action',values,d.pair,t));
        if(reply.accepted===true)present(d,v);
        if(reply.batch)applyBatch(reply.batch);
        const cancel=reply.accepted!==true||reply.committed!==true;
        // An action/receipt may replace exactly one property. Independently end
        // each original; local/token checks never retire its replacement.
        api.end(d.pair[0],velocity.slice(0,2),cancel);
        api.end(d.pair[1],[velocity[2],0],cancel);
        clearContact(d);e.preventDefault();e.stopPropagation();
      };
      for(const event of ['pointerup','pointercancel','lostpointercapture'])on(event,finish);
      on('dragstart',e=>{if(transformLocal(transformBindings.get(id))&&!interactive(e))e.preventDefault();});
      on('click',e=>{if(suppressClick){suppressClick=false;e.preventDefault();e.stopPropagation();}});
    },
    attachHeightDrag(el,id,on) {
      let drag=null,suppressClick=false;
      const velocity=d=>{
        const last=d.samples.at(-1), first=d.samples[0], dt=last[0]-first[0];
        return dt>0?(last[1]-first[1])*1000/dt:0;
      };
      const position=d=>sample(d.binding.targetEl,'height')?.[0];
      function stop() {
        const d=drag; drag=null; active.delete(id);
        if(!d) return;
        if(d.h) { suppressClick=true; api.end(d.h,[0,0],true); }
        if(el.hasPointerCapture(d.pointer)) el.releasePointerCapture(d.pointer);
        releaseInteraction(d.pointer);
      }
      stop.valid=()=>!drag||bindingLive(drag.binding)&&(!drag.h||local(drag.h));
      drags.set(id,stop);
      on('pointerdown',e=>{
        const binding=heightBindings.get(id);
        if(!e.isPrimary||e.button!==0||!bindingLive(binding)||e.target.closest('button,a,input,textarea,select,[contenteditable]')) return;
        stop(); drag={pointer:e.pointerId,x:e.clientX,y:e.clientY,binding}; active.set(id,stop);
        // A fast first move can already leave a narrow header. Retain delivery
        // while intent is pending; motion takeover still waits for recognition.
        el.setPointerCapture(e.pointerId); e.preventDefault();
      });
      const move=e=>{
        if(!drag||drag.pointer!==e.pointerId) return false;
        if(!stop.valid()) { stop(); return false; }
        const current=drag;
        const dx=e.clientX-drag.x,dy=e.clientY-drag.y;
        if(!drag.h) {
          if(Math.abs(dx)>8&&Math.abs(dx)>=Math.abs(dy)) { stop(); return false; }
          if(Math.abs(dy)<8||Math.abs(dy)<=Math.abs(dx)) return false;
          const value=position(drag);
          if(!Number.isFinite(value)||value<0) { stop(); return false; }
          const reply=request({op:'height-begin',view:id,property:'height',token:drag.binding.handleKey,x:value,y:0,now:now()});
          if(!reply.token||reply.target!==drag.binding.target) { stop(); return false; }
          adopt(reply.target,'height',reply,h=>{current.h=h;});
          if(drag!==current) return false;
          if(!drag.h) { stop(); return false; }
          drag.origin=e.clientY; drag.base=drag.h.value[0]; drag.samples=[[now(),position(drag)]];
          el.setPointerCapture(e.pointerId);
        }
        const value=Math.max(0,Math.min(3.4028234663852886e38,drag.base-(e.clientY-drag.origin)));
        if(!api.move(drag.h,[value,0])) { stop(); return false; }
        if(drag!==current) return false;
        // CSS min/max may clamp a held sample. Release velocity follows actual
        // displayed height, never the finger's speed beyond that constraint.
        const shown=position(drag),t=now();
        if(!Number.isFinite(shown)) { stop(); return false; }
        drag.samples.push([t,shown]);
        while(drag.samples.length>2&&(drag.samples.length>8||t-drag.samples[0][0]>80)) drag.samples.shift();
        e.preventDefault(); if(e.type==='pointermove') e.stopPropagation(); return true;
      };
      on('pointermove',move);
      const finish=e=>{
        if(!drag||drag.pointer!==e.pointerId) return;
        if(e.type!=='pointerup'||!drag.h) { stop(); return; }
        if(!move(e)) return;
        const d=drag; drag=null; active.delete(id); suppressClick=true;
        const shown=position(d),v=velocity(d);
        // Final constrained sample precedes the synchronous authored snap. The
        // host checks this handle/target/token again before dispatching it.
        if(bindingLive(d.binding)&&live(d.h)&&api.move(d.h,[shown,0])) {
          const reply=request({op:'height-action',view:id,property:'height',token:d.h.token,x:shown,y:v,now:now()});
          if(reply.batch) applyBatch(reply.batch);
          api.end(d.h,[v,0],reply.accepted!==true);
        } else api.end(d.h,[0,0],true);
        if(el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
        releaseInteraction(e.pointerId);
      };
      on('pointerup',finish); on('pointercancel',finish); on('lostpointercapture',finish);
      on('click',e=>{if(suppressClick){suppressClick=false;e.preventDefault();e.stopPropagation();}});
    },
    attachSwipe(el,id,on) {
      let drag=null,suppressClick=false;
      const rubber=x=>Math.abs(x)<=64?x:Math.sign(x)*(64+(Math.abs(x)-64)*0.2);
      const inverse=x=>Math.abs(x)<=64?x:Math.sign(x)*(64+(Math.abs(x)-64)/0.2);
      const progressOf=x=>Math.max(0,Math.min(1,x/64));
      const velocity=h=>{const last=h.samples.at(-1);let i=h.samples.length-2;while(i>0&&h.samples[i][1]===last[1]&&h.samples[i][2]===last[2])i--;const first=h.samples[Math.max(0,i)],dt=last[0]-first[0];return dt>0?[(last[1]-first[1])*1000/dt,(last[2]-first[2])*1000/dt]:[0,0];};
      function track(h,value) {
        const t=now(); h.samples.push([t,...value]);
        while(h.samples.length>2&&(h.samples.length>8||t-h.samples[0][0]>80))h.samples.shift();
        return api.move(h,value);
      }
      function stop() {
        const ended=drag; drag=null; active.delete(id);
        if(ended) {
          if(ended.holds) suppressClick=true;
          for(const h of ended.holds??[]) api.end(h,[0,0],true);
          if(el.hasPointerCapture(ended.pointer)) el.releasePointerCapture(ended.pointer);
          releaseInteraction(ended.pointer);
        }
      }
      drags.set(id,stop);
      on('pointerdown',e=>{
        if(!e.isPrimary||e.button!==0||!eligible(el)||e.target.closest('input,textarea,[contenteditable]'))return;
        stop(); drag={pointer:e.pointerId,x:e.clientX,y:e.clientY}; active.set(id,stop);
      });
      const move=e=>{
        if(!drag||e.pointerId!==drag.pointer)return false;
        if(!eligible(el)){stop();return false;}
        const dx=e.clientX-drag.x,dy=e.clientY-drag.y;
        if(!drag.holds) {
          if(Math.abs(dy)>8&&Math.abs(dy)>=Math.abs(dx)){stop();return false;}
          if(Math.abs(dx)<8||Math.abs(dx)<=Math.abs(dy))return false;
          if(dx<0 && !(sample(el,'translate')?.[0]>0)){stop();return false;}
          const h=api.begin(id,'translate'); if(!h){stop();return false;}
          drag.holds=[h]; drag.origin=e.clientX; drag.base=[...h.value]; drag.raw=inverse(h.value[0]);
          // One authored companion, at most two additional property holds.
          const indicator=[...el.children].find(n=>n.getAttribute('swipeIndicator')==='true');
          if(indicator) for(const property of ['opacity','scale']) {
            const h=api.begin(Number(indicator.dataset.view),property); if(h)drag.holds.push(h);
          }
          for(const h of drag.holds) { h.base=[...h.value]; h.samples=[[now(),...h.value]]; }
          el.setPointerCapture(e.pointerId);
        }
        const h=drag.holds[0], displacement=e.clientX-drag.origin;
        const x=displacement===0?drag.base[0]:rubber(drag.raw+displacement);
        if(!track(h,[x,drag.base[1]])){stop();return false;}
        const progress=progressOf(x), caught=progressOf(drag.base[0]);
        for(const companion of drag.holds.slice(1)) {
          const base=companion.base[0];
          const value=progress===caught?base:progress<caught?base*progress/caught:base+(1-base)*(progress-caught)/(1-caught);
          track(companion,[value,0]);
        }
        e.preventDefault(); if(e.type==='pointermove') e.stopPropagation(); return true;
      };
      on('pointermove',move);
      const finish=e=>{
        if(!drag||drag.pointer!==e.pointerId)return;
        // Taking capture from a text child bubbles that child's capture loss here.
        if(e.type==='lostpointercapture'&&e.target!==el)return;
        if(!drag.holds){stop();return;}
        if(e.type==='pointerup'&&!move(e))return;
        const ended=drag; drag=null; active.delete(id); suppressClick=true;
        const [h,...companions]=ended.holds, cancel=e.type!=='pointerup';
        api.finish(h,h.value,velocity(h),!cancel&&h.value[0]>=64,cancel);
        for(const companion of companions) api.end(companion,velocity(companion),cancel);
        if(el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      };
      on('pointerup',finish); on('pointercancel',finish); on('lostpointercapture',finish);
      on('click',e=>{if(suppressClick){suppressClick=false;e.preventDefault();e.stopPropagation();}});
    },
  };
  return api;
}

// One physical Arrange contact and its settling source; Common owns logical keys.
export function arrangeController({views,collections,motion,request,applyBatch,now,generation,inert,ready=()=>true}) {
  const bindings=new Map();let current=null,edge=null,edgeTime=null,busy=false,pending=null;
  const live=b=>b&&b.generation===generation()&&views.get(b.id)===b.el&&views.get(b.wrapper)===b.row&&b.el.isConnected;
  function mapping(b,y) {
    if(!live(b)||b.el.closest('[disabled]')||inert(b.el)||!b.el.getClientRects().length)return null;
    for(let el=b.el;el;el=el.parentElement) {
      const cs=getComputedStyle(el),translate=cs.translate==='none'?[0,0]:cs.translate.split(/\s+/).map(parseFloat);
      if(cs.visibility!=='visible'||cs.transform!=='none'||cs.perspective!=='none'||!['none','0deg'].includes(cs.rotate)
        ||!['none','1'].includes(cs.scale)||!['1','normal',''].includes(cs.zoom)||el!==b.row&&translate.some(v=>v!==0))return null;
      for(const a of el.getAnimations())if(a.effect?.target===el&&!['idle','finished'].includes(a.playState)) {
        const forbidden=['scale','rotate','transform','perspective','zoom',...(el===b.row?[]:['translate'])];
        if(forbidden.includes(a.transitionProperty)||a.effect.getKeyframes().some(f=>forbidden.some(p=>p in f)))return null;
      }
    }
    return collections.reorderMapping(b.list,y);
  }
  const facts=(d,op,extra={})=>({...d.binding,...d.map,op,token:d.token??0,x:d.value?.[0]??0,y:d.value?.[1]??0,now:now(),...extra});
  const call=(d,op,extra)=>request(facts(d,op,extra));
  function adoptReply(d,r,captured=[]) {
    if(r.accepted!==true)return false;
    d.token=r.token??d.token;d.frame=r.frame??d.frame;d.terminal=r.terminal??d.terminal;
    motion.adoptReorder(d.frame??[],captured,d.binding.runtime);
    if(r.batch)applyBatch(r.batch);return current===d;
  }
  function stopEdge(){if(edge!==null)cancelAnimationFrame(edge);edge=null;edgeTime=null;}
  function clearContact(d) {
    stopEdge();if(d.binding.el.hasPointerCapture(d.pointer))d.binding.el.releasePointerCapture(d.pointer);
    for(const [el,name,fn] of d.listeners??[])el.removeEventListener(name,fn);
    d.listeners=[];
  }
  function finish(d) {
    if(current!==d)return;
    const r=call(d,'reorder-finish');if(r.accepted!==true)return;
    // Remove identity before applying a receipt or promise can reenter.
    current=null;clearContact(d);motion.raiseReorder(d.binding.wrapper,false);
    collections.releaseRetainedInteraction(d.lease);if(r.batch)applyBatch(r.batch);
  }
  function terminal(d,cancel=false) {
    if(current!==d||d.phase!=='active')return;
    d.phase='terminalizing';clearContact(d);busy=true;
    try {
      const captured=motion.captureReorder(d.frame??[]);
      const last=d.samples.at(-1),first=d.samples[Math.max(0,d.samples.length-2)],dt=last[0]-first[0];
      const velocity=cancel||dt<=0?[0,0]:[(last[1]-first[1])*1000/dt,(last[2]-first[2])*1000/dt];
      let r=call(d,cancel?'reorder-cancel':'reorder-terminal',{rows:captured,vx:velocity[0],vy:velocity[1]});
      if(r.accepted!==true&&!cancel)r=call(d,'reorder-cancel',{rows:captured});
      if(!adoptReply(d,r,captured)){d.phase='active';return;}
      const old=new Map(captured.map(s=>[s.key,s]));
      const survivors=(d.frame??[]).filter(row=>row.hold!=='0'&&old.has(row.key)).map(row=>({...old.get(row.key),...row}));
      const rebased=motion.rebaseReorder(survivors);
      r=call(d,'reorder-rebase',{rows:rebased});
      if(r.accepted!==true){d.phase='terminalizing';return;}
      motion.releaseReorder(rebased);d.phase='settling';
      if(r.batch)applyBatch(r.batch);
      motion.settleReorder(d.binding.wrapper,()=>finish(d),()=>current===d&&d.phase==='settling');
    } finally {busy=false;}
  }
  function sampleMove(d,x,y) {
    if(current!==d||d.phase!=='active')return false;
    d.x=x;d.y=y;const m=mapping(d.binding,y);if(current!==d||d.phase!=='active'||m===undefined)return false;if(!m){terminal(d,true);return false;}
    d.map=m;d.x=x;d.y=y;d.value=[d.base[0],d.base[1]+y-d.originY+m.raw-d.originRaw];
    const t=now();d.samples.push([t,...d.value]);while(d.samples.length>2&&(d.samples.length>8||t-d.samples[0][0]>80))d.samples.shift();
    const source=d.frame?.find(r=>r.key===d.binding.wrapperKey);
    const captured=source?[{...source,value:d.value,el:d.binding.row}]:[];
    // The existing hold is already adopted: use the fixed packet to update Rust,
    // then update the same DOM overlay without an extra clock/lowering call.
    const r=call(d,'reorder-preview');if(!adoptReply(d,r,captured)){terminal(d,true);return false;}
    motion.presentReorder(d.binding.wrapper,source?.hold,d.value);return true;
  }
  function edgeRange(d) {
    const m=d.map,offset=d.y-m.portTop,direction=offset<32?-1:offset>m.portHeight-32?1:0;
    // Translate contributes to browser scroll overflow. Never chase the held
    // source beyond the collection's certified untransformed content extent.
    return {m,direction,available:Math.max(0,direction<0?m.raw:m.totalExtent-m.portHeight-m.raw)};
  }
  function edges(d) {
    if(current!==d||d.phase!=='active')return;
    const range=edgeRange(d);if(!range.direction||!range.available){stopEdge();return;}
    if(edge!==null)return;
    edge=requestAnimationFrame(at=>{edge=null;
      if(current!==d||d.phase!=='active'||!ready()||!Number.isFinite(at)){stopEdge();return;}
      // 720 CSS px/s, with at most 32ms of catch-up after a delayed frame.
      // The first frame after idle establishes a fresh rAF clock origin.
      const dt=edgeTime===null?0:Math.min(32,Math.max(0,at-edgeTime));edgeTime=at;
      const {m,direction,available}=edgeRange(d);
      if(!direction||!available){stopEdge();return;}
      if(!dt){edges(d);return;}
      const before=m.port.scrollTop;m.port.scrollTop+=direction*Math.min(available,720*dt/1000);
      if(m.port.scrollTop!==before&&sampleMove(d,d.x,d.y))edges(d);else stopEdge();
    });
  }
  function down(b,e) {
    if(!ready()||!e.isPrimary||e.button!==0||e.target.closest('input,textarea,select,[contenteditable]'))return;
    let reservation=null,returning=current?.phase==='settling'&&current.binding.wrapper===b.wrapper?current:null;
    // A tap or horizontal refusal is not a takeover. Keep the old Terminal
    // and its return/pin owner until replacement recognition actually succeeds.
    if(current&&!returning){
      if(current.phase==='active')terminal(current,true);
      if(current?.phase==='settling'){
        if(current.binding.wrapper===b.wrapper){
          reservation=collections.transferRetainedInteraction(current.lease,b.row,e.pointerId);
          if(!reservation)return; // Keep the still-visible old source on refusal.
        }
        finish(current);
      }
      if(current)return;
    }
    pending?.();b=bindings.get(b.id);if(!b){collections.releaseRetainedInteraction(reservation);return;}
    if(!reservation&&!returning)collections.reorderContact(b.el,e.pointerId);
    let contact={x:e.clientX,y:e.clientY},drag=null;
    const events=[],on=(el,name,fn)=>{el.addEventListener(name,fn);events.push([el,name,fn]);};
    const cleanup=()=>{for(const [el,name,fn]of events)el.removeEventListener(name,fn);if(pending===cleanup)pending=null;if(!drag){if(reservation)collections.releaseRetainedInteraction(reservation);else collections.releaseInteraction(e.pointerId);}};
    cleanup.handle=b.id;
    pending=cleanup;
    on(b.el.ownerDocument,'pointermove',v=>{
      if(v.pointerId!==e.pointerId)return;
      if(!drag){
        const dx=v.clientX-contact.x,dy=v.clientY-contact.y;
        if(Math.abs(dx)>8&&Math.abs(dx)>Math.abs(dy)){cleanup();return;}
        if(Math.abs(dy)<8)return;
        if(returning){
          if(current!==returning||!mapping(b,v.clientY)){cleanup();return;}
          reservation=collections.transferRetainedInteraction(returning.lease,b.row,e.pointerId);
          if(!reservation)return;
          finish(returning);returning=null;
          if(current){cleanup();return;}
          b=bindings.get(b.id);if(!b){cleanup();return;}
        }
        const m=mapping(b,v.clientY);
        const lease=m&&(reservation?collections.transferRetainedInteraction(reservation,b.el,e.pointerId):collections.retainInteraction(b.el,e.pointerId));
        if(reservation&&m!==null&&!lease)return; // Await another sample/feedback; pointerup releases this one reservation.
        if(!m||!lease){cleanup();return;}
        if(reservation)reservation=lease;
        const captured=motion.captureReorder([{view:b.wrapper,key:b.wrapperKey,hold:'0'}]);
        if(captured.length!==1){collections.releaseRetainedInteraction(lease);cleanup();return;}
        const d={binding:b,map:m,lease,pointer:e.pointerId,phase:'active',value:captured[0].value,base:captured[0].value,
          originY:v.clientY,originRaw:m.raw,x:v.clientX,y:v.clientY,samples:[[now(),...captured[0].value]],listeners:events};
        current=d;busy=true;let ok;
        try{ok=adoptReply(d,call(d,'reorder-begin'),captured);}finally{busy=false;}
        if(!ok){current=null;collections.releaseRetainedInteraction(lease);cleanup();return;}
        drag=d;pending=null;motion.raiseReorder(b.wrapper,true);b.el.setPointerCapture(v.pointerId);
        on(m.port,'scroll',()=>{if(!busy&&sampleMove(d,d.x,d.y))edges(d);});
      }
      if(sampleMove(drag,v.clientX,v.clientY)){v.preventDefault();v.stopPropagation();edges(drag);}
    });
    const up=v=>{if(v.pointerId!==e.pointerId)return;cleanup();if(!drag)return;
      const cancel=v.type!=='pointerup';if(!cancel&&!sampleMove(drag,v.clientX,v.clientY)){terminal(drag,true);return;}
      terminal(drag,cancel);};
    for(const name of ['pointerup','pointercancel','lostpointercapture'])on(b.el.ownerDocument,name,up);
  }
  function destroy(id) {
    const b=bindings.get(id);if(!b)return;
    b.el.removeEventListener('pointerdown',b.down);b.el.style.touchAction=b.touch;bindings.delete(id);
    if(pending?.handle===id)pending();
    // The completed batch supplies the terminal frame and surviving source
    // wrapper. Preserve that hold/pin until commit can capture and rebase it.
  }
  return {
    binding(op) {
      const old=bindings.get(op.id);
      if(old&&['runtime','handleKey','listKey','wrapperKey','rootKey','rowEpoch'].every(k=>old[k]===op[k])&&live(old))return;
      destroy(op.id);
      if(op.list===null||!views.has(op.id)||!views.has(op.wrapper))return;
      const b={...op,el:views.get(op.id),row:views.get(op.wrapper),generation:generation()};
      b.touch=b.el.style.touchAction;b.el.style.touchAction='none';b.down=e=>down(b,e);
      bindings.set(op.id,b);b.el.addEventListener('pointerdown',b.down);
    },
    destroy,
    state(op) {if(current&&op.runtime===current.binding.runtime&&op.token===current.token){current.frame=op.frame;current.terminal=op.terminal;}},
    commit() {
      if(busy||collections.reporting()||!current)return;const d=current;busy=true;let invalid=false;
      try{if(d.phase==='active'){
        const m=d.terminal?null:mapping(d.binding,d.y);invalid=m===null;
        if(m&&['raw','portWidth','portHeight','rowWidth','scrollSequence'].some(k=>m[k]!==d.map[k]))sampleMove(d,d.x,d.y);
      }}finally{busy=false;}
      if(invalid)terminal(d,true);if(current!==d)return;
      if(d.phase==='active'){motion.raiseReorder(d.binding.wrapper,true);edges(d);}
      if(d.phase==='settling'&&motion.reorderSettled(d.binding.wrapper))finish(d);
    },
    reset() {
      if(current){if(current.phase==='active')terminal(current,true);if(current?.phase==='settling')finish(current);}
      if(current){clearContact(current);collections.releaseRetainedInteraction(current.lease);motion.raiseReorder(current.binding.wrapper,false);current=null;}
      for(const b of bindings.values()){b.el.removeEventListener('pointerdown',b.down);b.el.style.touchAction=b.touch;}
      pending?.();pending=null;bindings.clear();stopEdge();
    },
  };
}

// A `markup="markdown"` text node's pieces, as the wasm emitted them
// (`[text, scale, weight, flags, href]`; flags italic 1, mono 2, strike 4, link 8,
// quiet 16), built into spans with textContent — never HTML. Lives here because it
// must run at boot and glue.js is at its line cap. LLP 1045 D3/D4.
//
// One scheme allowlist for every URL the page can navigate to: a link's
// `href` (an authored `link`, an inline run bound to data, a Markdown link),
// an iframe's `src`, and `openURL`. A `javascript:` URL in any of them runs
// in this page's origin. The browser's own parser reads the scheme, with the
// whitespace and control characters `java\tscript:` hides behind.
export function navigableURL(href, base = document.baseURI) {
  try {
    const url = new URL(href, base);
    return ["http:", "https:", "mailto:", "tel:"].includes(url.protocol) ? url.href : null;
  } catch { return null; }
}
export const navigates = (el, name) => name === "href" || (name === "src" && el.localName === "iframe");
/** A refused URL is never written: a link loses its `href`, an iframe shows about:blank. */
export function refuseURL(el, name, value) {
  console.warn(`exact: refused ${name} ${JSON.stringify(String(value).slice(0, 80))}: only http, https, mailto and tel navigate`);
  if (name === "src") el.setAttribute(name, "about:blank"); else el.removeAttribute(name);
}
export function renderMarkup(el, json) {
  let pieces;
  try { pieces = JSON.parse(json); } catch { pieces = []; }
  el.replaceChildren();
  for (const [text, scale, weight, flags, href] of pieces) {
    const destination = flags & 8 && href ? navigableURL(href) : null;
    const span = document.createElement(destination ? "a" : "span");
    // Newlines are `<br>`s: the node's own white-space row still applies to the rest.
    text.split("\n").forEach((line, i) => { if (i) span.appendChild(document.createElement("br")); if (line) span.appendChild(document.createTextNode(line)); });
    if (scale !== 1) span.style.fontSize = `${scale}em`;
    if (weight) span.style.fontWeight = weight;
    if (flags & 1) span.style.fontStyle = "italic";
    if (flags & 2) span.style.fontFamily = "ui-monospace, monospace";
    if (flags & 4) span.style.textDecoration = "line-through";
    if (flags & 16) span.style.opacity = "0.62";
    if (destination) span.href = destination;
    el.appendChild(span);
  }
}

export function focusController({ready, elements, inert}) {
  const processed = new WeakSet();
  let pointerTarget = null;
  const autofocus = () => {
    if (!ready()) return;
    for (const el of elements()) {
      if (processed.has(el) || !el.exactAutofocus || !el.getClientRects().length || inert(el) || el.matches(':disabled') || getComputedStyle(el).visibility !== 'visible') continue;
      processed.add(el); // Once per mount, including a refused autofocus.
      const active = document.activeElement;
      if (active && active !== document.body && active !== pointerTarget && !(active.matches('[data-gpu-input]') && active.contains(el))) return;
      el.setAttribute('autofocus', ''); el.focus(); return;
    }
  };
  return {autofocus, press(event, el, dispatch) {
    event.stopPropagation();
    const canvas = el.closest('[data-gpu-input]');
    const previous = pointerTarget;
    pointerTarget = event.detail > 0 ? el : null;
    try {
      dispatch(); // Blur handlers must not retire the press target before dispatch.
      const removed = !el.isConnected && document.activeElement === document.body;
      if (el instanceof HTMLButtonElement && ((pointerTarget && document.activeElement === el) || removed) && el.getAttribute('role') !== 'slider' && !el.hasAttribute('data-action')) {
        if (canvas?.isConnected) {event.preventDefault();canvas.focus({preventScroll:true});pointerTarget=canvas;}
      }
      autofocus();
    } finally {pointerTarget = previous;}
  }};
}

