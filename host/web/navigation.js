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

// A completed pop and Escape use the same live, selected control (1035.001 D1/D2).
function pressBack(nav) {
  const route = selectedRoute(nav);
  if (!route || ["modal", "fullscreen"].includes(route.getAttribute("navigationPresentation")) && route.getAttribute("closedby") === "none") return;
  const control = [...route.querySelectorAll("[id]")].find(node => node.id === nav.getAttribute("navigationBack"));
  if (control && !control.matches(":disabled") && !control.closest("[inert]")
      && control.getClientRects().length && getComputedStyle(control).visibility === "visible") control.click();
}

function go(to, from, finish = () => {}) {
  // A second popstate can already have arrived while an echo was pending.
  // Restore from the browser's *current* position, not the queued event's.
  const index = browserIndex();
  const current = index !== null && originIndex !== null ? index - originIndex : from;
  if (to === current) { finish(); return; }
  echo = { index: to, finish };
  history.go(to - current);
}

// D7's commit table. Intervening *removed ids*, never URL equality or DOM
// differences, distinguish a pop from a tab switch or a newly opened chain.
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
    // Accepted entries from before this boot can have negative indices.
    // Array.length/splice do not account for those indexed properties.
    for (const index of Object.keys(written)) if (Number(index) > cursor) delete written[index];
    written.length = Math.max(0, cursor + 1);
    written[++cursor] = stamp(cursor, op);
    history.pushState(written[cursor], "", location.origin + op.url);
  }
}

// D7's popstate table. Hold a synchronous router op until we distinguish
// acceptance of this traversal from a separate change the app chose.
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
      // The action changed the router elsewhere. Restore before mirroring
      // its commit so the browser cursor and the value share a base again.
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
  connect(hostRoot, dispatch, journal) {
    root = hostRoot; navigate = dispatch; log = journal;
    addEventListener("popstate", event => {
      if (!last) return;
      const index = browserIndex();
      const j = index !== null && originIndex !== null ? index - originIndex
        : Number.isInteger(event.state?.exact) ? event.state.exact : null;
      if (echo !== null && j === echo.index) {
        const finish = echo.finish; echo = null; finish(); drain(); settled(); return;
      }
      queue.push({ j, state: event.state, url: location.pathname + location.search });
      // A newer browser traversal can cancel an outstanding go. Reissue
      // the absolute destination from this new position; do not wait for
      // an echo the browser will no longer send.
      if (echo !== null) {
        const pending = echo; echo = null;
        go(pending.index, j ?? cursor, pending.finish);
      }
      drain(); settled();
    });
    document.addEventListener("keydown", event => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (!root.contains(event.target) && event.target !== document.body && event.target !== document.documentElement) return;
      // The browser's top layer gets first refusal of Escape.
      if (document.querySelector("dialog:modal") || [...document.querySelectorAll(":popover-open")].some(p => p.popover === "auto" || p.popover === "hint")) return;
      for (const nav of root.querySelectorAll("[navigationBack]")) {
        if (!["modal", "fullscreen"].includes(selectedRoute(nav)?.getAttribute("navigationPresentation"))) continue;
        event.preventDefault(); pressBack(nav); return;
      }
    });
  },
  reset(op = null) {
    refused = new WeakMap();
    // An in-document reboot carries the router, not the rebuilt DOM ids.
    // Its first op proves whether our browser cursor still names its top.
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
      // Out-of-range history.go is a browser no-op, with no popstate.
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
// Keep this in the existing host controller module: the boot graph stays two files.
// The runner alone selects/mounts rows; the host reports only live geometry.
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

// Apply committed geometry before interpreting the collection call's outcome.
export function applyCollectionFeedback(batch, applyBatch) {
  applyBatch(batch);
  return batch.accepted === true;
}

export function collectionController({ root, views, report,
  requestFrame = fn => requestAnimationFrame(fn), cancelFrame = id => cancelAnimationFrame(id) }) {
  const states = new Map(), dirty = new Set(), waiting = new Set(), rowOwners = new WeakMap(), doc = root.ownerDocument;
  let frame = null, delivering = false, interaction = null, reportsLeft = 4;
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
    // Only authored descendants of a live wrapper can pin. No logical lookup
    // or manufactured offscreen selection/search result exists in the DOM host.
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
    return [liveView(s, doc.activeElement), liveView(s, interaction?.element)];
  }
  function retiring(s) {
    const next = desired(s), old = s.lastFacts;
    return old && ((old.focus_view != null && old.focus_view !== next[0])
      || (old.interaction_view != null && old.interaction_view !== next[1]));
  }
  function flush(beforePaint = false) {
    if (!beforePaint) { frame = null; reportsLeft = 4; }
    // One queued frame, at most four reports/frame and two dependent passes
    // per external stimulus or new measurement epoch. Retire pins before replacements,
    // including when focus/interaction swap owners in the same turn.
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
      const visible = g && g.height > 0 && g.rowWidth > 0
        && s.rows.every(row => row.el.isConnected && row.el.getClientRects().length);
      if (visible) {
        measurements = s.rows.map(row => ({ view: row.view, epoch: row.epoch,
          height: row.el.getBoundingClientRect().height }));
      } else if (releases.includes(s)) {
        // A hidden/partially attached former owner can release with its last
        // real geometry and no measurements; this never admits a new pin.
        g = { raw: old.scroll_top, width: old.port_width, height: old.port_height, rowWidth: old.row_width };
      } else continue;
      scrollChanged(s);
      const dimensions = `${g.width},${g.height},${g.rowWidth}`;
      if (s.dimensions !== null && dimensions !== s.dimensions) s.sequence++;
      s.dimensions = dimensions;
      const facts = { view: s.snapshot.view, revision: s.snapshot.revision, scroll_sequence: s.sequence,
        scroll_top: Math.max(0, g.raw), port_width: g.width, port_height: g.height, row_width: g.rowWidth,
        focus_view: pins[0], interaction_view: pins[1], measurements };
      // Revision alone is not a stimulus: stale feedback and repeated snapshots
      // cannot cause a loop. Changed wrapper epochs, pins or geometry can.
      const signature = [facts.scroll_top, dimensions, ...pins,
        ...measurements.flatMap(r => [r.view, r.epoch, r.height])].join('|');
      if (s.signature === signature) continue;
      let bytes;
      try { bytes = collectionBytes(facts); } catch { continue; }
      s.budget--; reportsLeft--;
      for (const el of s.observed.keys()) s.observed.set(el, size(el));
      delivering = true;
      let accepted;
      try { accepted = report(bytes) !== false; } finally { delivering = false; }
      // The synchronous Rust call consumes the current revision/epochs. A
      // rejected report retains the reservation; an edge-action refusal after
      // accepted geometry must release it, despite the surfaced action error.
      if (accepted) {
        s.signature = signature; s.lastFacts = facts;
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
      // Own commits are already queued with their remaining pass budget. Their
      // ResizeObserver notifications must not replenish that budget indefinitely.
      s.observed.set(el, size(el));
    }
  }
  function focusChanged() { for (const s of states.values()) enqueue(s, true); }
  function pointerDown(event) {
    interaction = { element: event.target, pointer: event.pointerId };
    focusChanged();
  }
  function pointerUp(event) {
    if (event.pointerId !== interaction?.pointer) return;
    interaction = null; focusChanged();
  }
  root.addEventListener('focusin', focusChanged, true);
  root.addEventListener('focusout', focusChanged, true);
  root.addEventListener('pointerdown', pointerDown, true);
  for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) doc.addEventListener(name, pointerUp);
  return {
    releaseInteraction(pointer) { pointerUp({pointerId:pointer}); },
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
            let changed = false, resizedPort = false;
            for (const { target } of entries) {
              const next = size(target);
              if (s.observed.has(target) && s.observed.get(target) !== next) {
                changed = true;
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
            }
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
        const g = geometry(s), correction = snapshot.correction;
        if (g && correction && s.corrected !== snapshot.revision
            && BigInt(correction.scrollSequence) === s.sequence
            && (s.dimensions === null || s.dimensions === `${g.width},${g.height},${g.rowWidth}`)
            && Number.isFinite(correction.scrollTop) && correction.scrollTop >= 0) {
          s.corrected = snapshot.revision;
          // Relative conversion also handles a list below siblings in its port.
          port.scrollTop += correction.scrollTop - g.raw;
          s.scrollTop = port.scrollTop; // consume the programmatic scroll echo
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
    if(!active.length) return;
    const transition=el.style.transition;
    el.style.transition=[...(transition && transition!=='none'?[transition]:[]), ...active.map(h=>`${h.property} 0s linear 0s`)].join(',');
    for(const h of active) { el.style.setProperty(h.property,css(h.property,h.value)); cancelProperty(id,h.property,el); }
  }
  function restore(id) {
    const el=views.get(id); if(!el) return;
    if(authored.has(id)) el.style.cssText=authored.get(id);
    overlay(id);
    if(!properties.some(p=>held.has(key(id,p)))) authored.delete(id);
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
      for(const property of properties) { cancelProperty(id,property); held.delete(key(id,property)); }
      authored.delete(id);
    },
    reset() {
      for(const stop of drags.values()) stop(); drags.clear();
      heightBindings.clear();
      for(const b of [...transformBindings.values()])detachTransform(b);
      geometryDirty.clear();if(geometryFrame!==null)cancelAnimationFrame(geometryFrame);geometryFrame=null;
      for(const animation of animations.values()) animation.cancel(); animations.clear();
      const ids=[...authored.keys()]; held.clear(); for(const id of ids) restore(id); authored.clear();
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
