// @ref LLP 1038 D6/D7/D11 — host projection and the last committed URL.
const AGENT_ADMITTED = true; // false in a production bake: host/web/build.mjs rewrites this line (LLP 1069.007 D2)
let last = null;
let refused = new WeakMap();
// The tabpanels each root last hid or showed: one it stops naming shows again.
let managed = new WeakMap();
let written = [], gone = new Set(), cursor = 0, first = null, originIndex = null;
let echo = null, pop = null, draining = false;
const queue = [];
const waiters = new Set();
let root, navigate, log;
const routesIn = node => [...node.children].filter(r => r.hasAttribute("navigationKey"));
// @ref LLP 1075.003 §3.7 — a navigation root's tabs: the tabpanels its own
// tablist's tabs name with aria-controls, in tab order (a tablist inside a
// route is that route's). Each tab's stack is its panel's route rows, kept
// mounted; without tabs the stack is the root's own rows.
const panelsOf = nav => {
  if (!nav) return [];
  for (const list of nav.querySelectorAll('[role="tablist"]')) {
    if (list.parentElement?.closest("[navigationKey]") !== nav) continue;
    const panels = [...list.children].filter(tab => tab.getAttribute("role") === "tab" && tab.hasAttribute("aria-controls"))
      .map(tab => nav.querySelector(`#${CSS.escape(tab.getAttribute("aria-controls"))}`))
      .filter(panel => panel?.getAttribute("role") === "tabpanel" && panel.parentElement?.closest("[navigationKey]") === nav);
    if (panels.length) return panels;
  }
  return [];
};
const stacksOf = nav => { const panels = panelsOf(nav); return panels.length ? panels.map(routesIn) : [nav ? routesIn(nav) : []]; };
/** What a navigation root leaves unselected, by `project`'s rule read from attributes: every tabpanel but the one
 * holding the selected route, and that stack's other routes (the runner's `unselected`; the agent's `inactive`). */
export function unselected(nav) {
  const key = nav.getAttribute("navigationKey"), panels = panelsOf(nav), stacks = panels.length ? panels.map(routesIn) : [routesIn(nav)];
  const at = stacks.findIndex(routes => routes.some(r => r.getAttribute("navigationKey") === key));
  return at < 0 ? [] : [...panels.filter((_, i) => i !== at), ...stacks[at].filter(r => r.getAttribute("navigationKey") !== key)];
}
const routesOf = nav => stacksOf(nav).find(routes => routes.some(r => r.getAttribute("navigationKey") === nav.getAttribute("navigationKey"))) ?? [];
const selectedRoute = nav => routesOf(nav).find(r => r.getAttribute("navigationKey") === nav.getAttribute("navigationKey"));
const browserIndex = () => globalThis.navigation?.currentEntry?.index ?? null;
// Agent launch facts belong to the carrier, not the router's typed URL. Keep
// them on every History entry so a browser reload retains its agent adapter.
const agentParameters = ['agent', 'seed', 'locale', 'timeZone', 'epoch', 'storage', 'failFetch'];
function historyURL(path) {
  if (!AGENT_ADMITTED) return location.origin + path;
  const facts = launched();
  if (!facts.has('agent')) return location.origin + path;
  const url = new URL(location.origin + path);
  for (const key of agentParameters) {
    if (facts.has(key)) url.searchParams.set(key, facts.get(key));
  }
  return url.href;
}
const stamp = (index, op) => ({ exact: index, id: op.top, url: op.url });

/** The selected route's Back control (1035.001 D1: the `id` the root's `navigationBack` names), pressable or not. */
const backControl = nav => { const route = selectedRoute(nav); return route && [...route.querySelectorAll("[id]")].find(node => node.id === nav.getAttribute("navigationBack")); };
/** Press the Back control; else why it was not pressed. */
function pressBack(nav) {
  const route = selectedRoute(nav), control = backControl(nav);
  if (!route) return "no route is selected";
  if (["modal", "fullscreen"].includes(route.getAttribute("navigationPresentation")) && route.getAttribute("closedby") === "none") return `route ${route.getAttribute("navigationKey")} is closedby="none"`;
  if (!control) return `route ${route.getAttribute("navigationKey")} has no id="${nav.getAttribute("navigationBack")}" control`;
  if (control.matches(":disabled") || control.closest("[inert]") || !control.getClientRects().length || getComputedStyle(control).visibility !== "visible")
    return `its id="${control.id}" control is disabled, inert or not shown`;
  control.click();
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
    history.replaceState(written[0], "", historyURL(op.url));
  } else if (written[cursor]?.id === op.top) {
    if (written[cursor].url !== op.url) {
      written[cursor] = stamp(cursor, op);
      history.replaceState(written[cursor], "", historyURL(op.url));
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
    history.pushState(written[cursor], "", historyURL(op.url));
  }
}

function popped({ j, state, url }) {
  const entry = written[j];
  const owned = entry && state?.exact === j && state.id === entry.id && state.url === entry.url;
  const target = owned ? entry.url : url;
  const nav = root.querySelector("[navigationBack]");
  const routes = routesOf(nav), selected = routes.indexOf(selectedRoute(nav));
  // A completed pop presses the selected route's Back control. A route with
  // none (a screen with no Back button) still goes back, as the web's Back
  // does: the root's `navigate` with the entry's URL, as any other traversal.
  const beneath = owned && j === cursor - 1 && selected > 0
    && routes[selected - 1].getAttribute("navigationKey") === String(entry.id);
  const back = beneath && !!backControl(nav);
  let why = null;
  pop = {};
  try {
    if (back) why = pressBack(nav);
    else navigate(target);
    const accepted = back ? last?.top === entry.id : pop.op?.url === target;
    if (accepted) {
      cursor = j ?? cursor;
      first = Math.min(first, cursor);
      written[cursor] = stamp(cursor, last);
      go(cursor, j ?? cursor, () => history.replaceState(written[cursor], "", historyURL(written[cursor].url)));
    } else if (pop.op) {
      const op = pop.op;
      if (j !== null && j !== cursor) go(cursor, j, () => commit(op));
      else {
        history.replaceState(written[cursor], "", historyURL(written[cursor].url));
        commit(op);
      }
    } else {
      if (back) log(`history: Back refused: ${why ?? `pressing the Back control did not select entry ${entry.id}`}; restoring the entry`);
      else log(`history: ${beneath ? `Back to ${JSON.stringify(target)} refused: route ${nav.getAttribute("navigationKey")} has no id="${nav.getAttribute("navigationBack")}" control, and` : `navigate ${JSON.stringify(target)} refused:`} the navigation root's navigate handler (navigate=…) committed no router change, or the root has none; restoring the entry`);
      if (j !== null && j !== cursor) go(cursor, j);
      else history.replaceState(written[cursor], "", historyURL(written[cursor].url));
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
      queue.push({ j, state: event.state, url: launchLocation() });
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
      const key = nav.getAttribute("navigationKey");
      const panels = panelsOf(nav);
      for (const panel of managed.get(nav) ?? []) {
        if (!panels.includes(panel)) { panel.style.visibility = ""; panel.inert = !!panel.authoredInert; }
      }
      managed.set(nav, panels);
      const stacks = panels.length ? panels.map(routesIn) : [routesIn(nav)];
      const at = stacks.findIndex(routes => routes.some(route => route.getAttribute("navigationKey") === key));
      if (at < 0) {
        if (refused.get(nav) !== key) {
          refused.set(nav, key);
          log(`navigationKey "${key}" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged`);
        }
        continue;
      }
      refused.delete(nav);
      // Every tab but the selected one stays mounted, hidden and inert.
      for (const [index, panel] of panels.entries()) {
        const active = index === at;
        if (!active && panel.contains(document.activeElement)) document.activeElement.blur();
        panel.style.visibility = active ? "" : "hidden";
        panel.inert = !active || !!panel.authoredInert;
      }
      for (const [stack, routes] of stacks.entries()) {
        // The selected stack shows the route the root names; another keeps its top laid out.
        const selected = stack === at ? routes.findIndex(route => route.getAttribute("navigationKey") === key) : routes.length - 1;
        const modal = routes[selected]?.getAttribute("navigationPresentation") === "modal";
        for (const [index, route] of routes.entries()) {
          const active = index === selected;
          if (!active && route.contains(document.activeElement)) document.activeElement.blur();
          const covered = modal && index === selected - 1;
          route.style.visibility = active || covered ? "" : "hidden";
          route.toggleAttribute("data-exact-covered", covered);
          route.inert = !active || !!route.authoredInert;
        }
      }
    }
    // The page module's container hatches, when it has them (LLP 1075.003.000 §3.7).
    globalThis.exact?.onProject?.(root);
  },
  observation(root) {
    const nav = root.querySelector("[navigationBack]");
    const routes = routesOf(nav);
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

// @ref LLP 1047 D5 — springs, holds and drags (`motion-glue.js`) and
// virtualized collections (`collection-glue.js`) are after-paint pieces,
// fetched when a batch first needs one. Until they arrive their calls wait in
// order and replay then; before any use, a call with nothing to reconcile is
// dropped, and a style is set at once, as the controller sets one with nothing
// held. `o.wasm(name, bytes)` is one wasm call's reply, or null before the wasm;
// `o.replayed()` follows a replay, as the end of a batch follows its ops.
export function afterPaintPieces(load, o) {
  let live = null, loading = null;
  const queue = [];
  const start = () => loading ??= Promise.all([load('./collection-glue.js', 'collectionGlue'), load('./motion-glue.js', 'motionGlue'), load('./group-glue.js', 'groupGlue')])
    .then(([c, m, g]) => {
      const common = { views: o.views, now: o.now, generation: o.generation, inert: o.inert, applyBatch: o.applyBatch, ready: o.ready, log: o.log };
      const request = facts => o.wasm('exact_motion', m.motionBytes(facts)) ?? { accepted: false };
      const collections = c.collectionController({ root: o.root, views: o.views, agent: !!o.agent?.(), settled: () => arrange.commit(), report(bytes) {
        const batch = o.wasm('exact_collection_feedback', bytes);
        return batch ? c.applyCollectionFeedback(batch, o.applyBatch) : false;
      } });
      const motion = m.motionController({ ...common, releaseInteraction: pointer => collections.releaseInteraction(pointer), request });
      const arrange = m.arrangeController({ ...common, collections, motion, request, grouped: g.groupController, root: o.root });
      live = { collections, motion, arrange };
      for (const [piece, name, args] of queue.splice(0)) {
        try { live[piece][name](...args); } catch (error) { console.error(`exact: ${piece}.${name} failed`, error); }
      }
      o.replayed?.();
    })
    .catch(error => { loading = null; queue.length = 0; console.error('exact: after-paint pieces:', error); });
  // @ref LLP 1055 D7: CSS animations need no piece — the browser runs them
  // from one `@keyframes` rule per name in a stylesheet the page owns.
  const keyframes = (name, body) => {
    const sheet = (document.getElementById('exact-keyframes') ?? document.head.appendChild(Object.assign(document.createElement('style'), { id: 'exact-keyframes' }))).sheet;
    sheet.insertRule(`@keyframes ${CSS.escape(name)}{${body}}`, sheet.cssRules.length);
  };
  // `use`: the call needs its piece; otherwise it only reconciles what uses made.
  const call = (piece, name, use = true) => (...args) => {
    if (live) return live[piece][name](...args);
    if (!use && !loading) return;
    queue.push([piece, name, args]); start();
  };
  const motion = { style(id, text) { if (live) return live.motion.style(id, text); const el = o.views.get(id); if (el) el.style.cssText = text; }, keyframes };
  for (const name of ['animate', 'retire', 'heightBinding', 'transformBinding', 'attachSwipe', 'attachHeightDrag', 'attachTransformDrag']) motion[name] = call('motion', name);
  // A pan's release velocity (LLP 1057 §10.6): only once motion is here.
  motion.pan = { sample: (...a) => live?.motion.panSample(...a), velocity: (...a) => live?.motion.panVelocity(...a) };
  const arrange = { binding: call('arrange', 'binding'), state: call('arrange', 'state'), group: call('arrange', 'group') };
  for (const piece of [motion, arrange]) for (const name of ['commit', 'reset', 'destroy']) piece[name] = call(piece === motion ? 'motion' : 'arrange', name, false);
  motion.followTimelines = call('motion', 'followTimelines', false);
  // Every first batch commits the (empty) collection set: a use only with items.
  const commit = call('collections', 'commit'), reconcile = call('collections', 'commit', false);
  const collections = { commit: items => (items.length ? commit : reconcile)(items) };
  for (const name of ['reset', 'dataReady', 'releaseInteraction', 'settle']) collections[name] = call('collections', name, false);
  collections.jump = call('collections', 'jump');
  // @ref LLP 1056 D7 — Canvas 2D's replayer and ResizeObserver: its own
  // piece, injected two animation frames after the first 2D canvas's op.
  let c2d = null, c2dLoading = null; const c2dQueue = [];
  const canvas2d = (op) => {
    if (c2d) return c2d.op(op);
    c2dQueue.push(op);
    c2dLoading ??= new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))
      .then(() => load('./canvas2d-glue.js', 'canvas2dGlue'))
      .then(make => { c2d = make({ views: o.views, now: o.now, applyBatch: o.applyBatch, generation: o.generation }); for (const q of c2dQueue.splice(0)) c2d.op(q); })
      .catch(error => { c2dLoading = null; c2dQueue.length = 0; console.error('exact: canvas2d:', error); });
  };
  const c2dPending = () => (c2d ? c2d.settled() : c2dLoading?.then(() => c2d?.settled()));
  // `preload`: a plan that uses motion (its wasm exports `exact_motion`) needs
  // them before its first spring. `pending`: the load in flight, else null.
  return { collections, motion, arrange, canvas2d, preload: start, pending: () => {
    const p = live || !loading ? null : loading, c = c2dPending();
    return p && c ? Promise.all([p, c]) : (p ?? c ?? null);
  } };
}

// @ref LLP 1063 — -exact-exit-animation and -exact-layout-transition play in
// `presence-glue.js`, fetched when a batch first carries either row. A batch
// with an exit that arrives before the module does waits for it, and every
// batch after it waits behind it, so no exit is lost and order holds; the
// caller hands them back through `apply`. `live` is the module, once loaded.
export function presenceLoader(load, root, apply, log) {
  let live = null, loading = null, unavailable = false;
  const held = [];
  const release = () => { for (const batch of held.splice(0)) apply(batch); };
  const start = () => loading ??= load('./presence-glue.js', 'presence')
    .then(create => { live = create(root); release(); })
    .catch(error => { unavailable = true; loading = null; log(`presence module: unavailable; motion skipped: ${error}`); release(); });
  return {
    get live() { return live; },
    resize(batch) { return { ...batch, presenceSnap: true }; },
    hold(batch) {
      if (live || unavailable) return false;
      const ops = batch.ops ?? [];
      if (ops.some(op => op.op === 'exit' || op.css?.includes('--exact-'))) start();
      if (!held.length && !(loading && ops.some(op => op.op === 'exit'))) return false;
      held.push(batch);
      return true;
    },
  };
}

// The agent's browser clock (LLP 1012): author-paused animations keep their
// own time (LLP 1055 D10); every other animation follows the runner's clock.
export function animationClock(now, settled, synced) {
  const starts = new WeakMap(), held = new WeakSet(), clocks = animationClocks(document);
  // A scroll-driven animation follows its scroll, not a clock.
  const timed = () => document.getAnimations().filter(a => !a.timeline || a.timeline instanceof DocumentTimeline);
  return {
    register(t) {
      clocks.commit();
      for (const a of timed()) if (!starts.has(a)) { starts.set(a, clocks.start(a, t) ?? t); if (a.playState === 'paused') held.add(a); }
    },
    seek(to) {
      for (const a of timed()) {
        const timing = a.effect?.getComputedTiming();
        if (!timing) continue;
        const t = to - (starts.get(a) ?? now());
        if (held.has(a)) continue; else if (t >= timing.endTime && timing.endTime !== Infinity) a.finish();
        else { a.pause(); a.currentTime = t; }
      }
      synced();
    },
    // The springs' engine knows its end; browser animations report theirs.
    settle() {
      let to = now();
      const s = settled();
      if (s != null) to = Math.max(to, s);
      for (const a of timed()) {
        const timing = a.effect?.getComputedTiming();
        if (timing && timing.endTime !== Infinity && !held.has(a)) to = Math.max(to, (starts.get(a) ?? now()) + timing.endTime);
      }
      return to;
    },
  };
}

// Synced animations (LLP 1055.002): a node whose `animation-timeline` is
// `clock(Name)` carries `--exact-animation-clock:Name` (css.rs), and each CSS
// animation on it joins that clock. A clock is one origin, set when an
// animation joins it idle (no other member unfinished) and kept while it is
// busy; a joiner starts on the latest cycle boundary at or before it joins
// (a cycle is two iterations under `alternate`), so it ends where it would.
// `start` is the synced start at `now` (the agent's clock seeks from it);
// `sync`, after a commit, sets each joined or resumed animation's
// `startTime` once on the page's timeline. Nothing runs per frame.
export function animationClocks(root) {
  const origins = new Map(), members = new Map(), paused = new WeakMap(), clocked = new WeakMap();
  const clockOf = a => a.animationName === undefined ? '' : a.effect?.target?.style?.getPropertyValue('--exact-animation-clock').trim() ?? '';
  const live = a => a.effect?.target?.isConnected && a.playState !== 'idle' && a.playState !== 'finished';
  // Each commit (`sync`, or the agent's `register`) first lets go of every
  // member whose node left, whose play ended, or that moved to another
  // clock: it holds no clock busy, and a removed screen's targets are not
  // kept for the page's lifetime.
  const commit = () => {
    for (const [c, m] of members) { for (const b of m) if (!live(b) || clockOf(b) !== c) m.delete(b); if (!m.size) members.delete(c); }
  };
  function start(a, now) {
    const c = clockOf(a);
    if (!c) return null;
    let m = members.get(c);
    if (!m) members.set(c, m = new Set());
    // Busy while any member is live, `a` included: a paused or resumed
    // member keeps the origin, so a resume rejoins its phase (D6).
    if (!m.size || !origins.has(c)) origins.set(c, now);
    m.add(a);
    const { duration, direction } = a.effect.getComputedTiming(), period = duration * (/alternate/.test(direction) ? 2 : 1);
    if (!(period > 0 && Number.isFinite(period))) return now;
    // On a boundary in float can read a hair before it: that is on it.
    const into = ((now - origins.get(c)) % period + period) % period;
    return now - (period - into < 1e-6 ? 0 : into);
  }
  return {
    start,
    commit,
    sync(now = document.timeline.currentTime) {
      commit();
      if (!root.querySelector('[style*="--exact-animation-clock"]')) return;
      for (const a of document.getAnimations()) {
        const is = a.playState === 'paused', was = paused.get(a), c = clockOf(a), had = clocked.get(a) ?? '';
        // On a clock and not its member: new, moved onto it, or let go
        // while it was off one (its name taken away and given back).
        const member = !c || members.get(c)?.has(a);
        if (was === is && c === had && member) continue;
        paused.set(a, is); clocked.set(a, c);
        // An ended play stays ended: a clock does not restart it.
        if (a.playState === 'finished' || a.playState === 'idle') continue;
        // A pause keeps its membership: paused, it still holds the clock
        // busy. A new one joins (paused, without a start, which would
        // unpause it); a resume rejoins.
        if (is && was !== undefined && c === had && member) continue;
        const s = start(a, now);
        if (s !== null && !is) a.startTime = s;
      }
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

// A `markup="markdown"` text node's pieces, as the wasm emitted them
// (`[text, scale, weight, flags, href, indent]`; flags italic 1, mono 2, strike 4,
// link 8, quiet 16, hanging marker 32), built into spans with textContent — never HTML. Lives here because it
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
// A list item's paragraph (indent > 0, LLP 1045 D4) is a block with the
// item's indent as `padding-left`; its marker (flags 32) is a 40 px box
// pulled into the gutter by the block's negative `text-indent`, the marker's
// end at the indent (`flex-end`, overflowing leftwards as the browser's
// outside marker does): `<ul>`/`<ol>`'s layout, the one native hosts copy
// with a head indent. 40 is exact-markdown's `LIST_INDENT`.
export function renderMarkup(el, json) {
  let pieces;
  try { pieces = JSON.parse(json); } catch { pieces = []; }
  el.replaceChildren();
  let box = el, start = true;
  const paragraph = indent => {
    if (start) { start = false; box = el; if (indent > 0) { box = document.createElement("span"); box.style.display = "block"; box.style.paddingLeft = `${indent}px`; el.appendChild(box); } }
    return box;
  };
  for (const [text, scale, weight, flags, href, indent = 0] of pieces) {
    const destination = flags & 8 && href ? navigableURL(href) : null;
    const make = () => {
      const span = document.createElement(destination ? "a" : "span");
      if (scale !== 1) span.style.fontSize = `${scale}em`;
      if (weight) span.style.fontWeight = weight;
      if (flags & 1) span.style.fontStyle = "italic";
      if (flags & 2) span.style.fontFamily = "ui-monospace, monospace";
      if (flags & 4) span.style.textDecoration = "line-through";
      if (flags & 16) span.style.opacity = "0.62";
      if (destination) span.href = destination;
      if (destination && /^(https?:)?\/\//i.test(href.trim())) { span.target = "_blank"; span.rel = "external noopener"; } // it leaves the app, as natively (element.rs `leaves_app`)
      return paragraph(indent).appendChild(span);
    };
    if (flags & 32) {
      const marker = make();
      marker.style.cssText += "display:inline-flex;justify-content:flex-end;width:40px;white-space:pre;text-indent:0";
      marker.textContent = text; box.style.textIndent = "-40px";
      continue;
    }
    // Newlines are `<br>`s: the node's own white-space row still applies to the rest.
    let span = null;
    text.split("\n").forEach((line, i) => {
      if (i) { (span ??= make()).appendChild(document.createElement("br")); start = true; span = null; }
      if (line) (span ??= make()).appendChild(document.createTextNode(line));
    });
  }
}

// @ref LLP 1007 §6 — a page the dev server serves names its current
// generation, and its first boot is that one, not app.wasm's older baked
// plan (a deep link booted stale on 2026-09-22). dev.js supplies it once asked
// (glue.js asks when the wasm is up); without it, or with none in 5 s, the
// page boots the baked plan.
export function devFirst() {
  if (!document.querySelector('meta[name="exact-dev-generation"]')) return null;
  const slot = globalThis.exactDevFirst ??= {};
  return new Promise(resolve => {
    const timer = setTimeout(() => resolve(null), 5000);
    const ask = () => { clearTimeout(timer); slot.provide().then(resolve, () => resolve(null)); };
    if (slot.provide) ask(); else slot.ready = ask;
  });
}

// Refusals and malformed request bodies are known while their enclosing
// batch is still applying. Deliver them on the next microtask so their
// commits cannot re-enter `apply` halfway through that batch; in flight
// until then, so a wait sees what their parse asks for next.
export function deferredFulfill(fulfill, inflight) {
  return (...args) => { const p = Promise.resolve().then(() => fulfill(...args)); inflight.add(p); p.finally(() => inflight.delete(p)); };
}
// An admission refusal answers its ticket Refused (kind 2) with the reason, in its incarnation.
export const refusal = (op, incarnation) => [incarnation, op.ticket, 2, 0, "", new TextEncoder().encode(op.message)];
export function focusController({ready, elements, inert}) {
  const processed = new WeakSet();
  let pointerTarget = null, restarting = false;
  const autofocus = () => {
    if (restarting || !ready()) return;
    for (const el of elements()) {
      if (processed.has(el) || !el.exactAutofocus || !el.getClientRects().length || inert(el) || el.matches(':disabled') || getComputedStyle(el).visibility !== 'visible') continue;
      processed.add(el); // Once per mount, including a refused autofocus.
      const active = document.activeElement;
      if (active && active !== document.body && active !== pointerTarget && !(active.matches('[data-gpu-input]') && active.contains(el))) return;
      el.setAttribute('autofocus', ''); el.focus(); return;
    }
  };
  // A carried restart (dev reload, delivered update) keeps focus at its place in the runner's tree — index among siblings at each level, and type — and autofocuses nothing it rebuilt.
  const keep = (tree, id) => { const nodes = new Map(tree.nodes?.map(n => [n.id, n])), path = [];
    for (let n = nodes.get(id); n; n = nodes.get(n.parent)) path.unshift((n.parent == null ? tree.roots : nodes.get(n.parent)?.children ?? []).indexOf(n.id));
    return nodes.has(id) && !path.includes(-1) ? {path, type: nodes.get(id).type} : null; };
  const restart = (kept, apply, tree, view) => {
    if (kept === undefined) return apply(); // a fresh boot autofocuses (LLP 1035.000 D9)
    try { restarting = true; apply(); } finally { restarting = false; for (const el of elements()) if (el.exactAutofocus) processed.add(el); }
    const t = kept && tree(), nodes = new Map(t?.nodes?.map(n => [n.id, n]));
    const [, id] = kept?.path.reduce(([ids], i) => [nodes.get(ids?.[i])?.children, ids?.[i]], [t.roots]) ?? [];
    const el = id != null && nodes.get(id)?.type === kept.type ? view(id) : null;
    if (el?.isConnected && el.getClientRects().length && !inert(el) && !el.matches(':disabled')) el.focus({preventScroll: true});
  };
  return {autofocus, keep, restart, press(event, el, dispatch) {
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


// The focus, blur, selectText and setSelectionRange commands a batch carried, run once every
// node and value in it is committed (a focus handler may dispatch an action).
export function runFocusCommands(commands, { root, ready, inertAncestor, log }) {
  for (const { name, args } of commands) {
    if (name === "scrollIntoView") { // `Element.scrollIntoView()` by the element's id, after the batch's layout (minesweeper F3)
      const el = [...root.querySelectorAll("[id]")].find(node => node.id === args?.[0]);
      if (el) el.scrollIntoView({ block: args[1] ?? "start", inline: args[2] ?? "nearest", behavior: args[3] ?? "auto" }); else log(`scrollIntoView "${args?.[0]}" refused: no live node with that id`);
      continue;
    }
    if (name === "blur") { // `blur()` drops whatever holds focus; `blur(id)` only when that node holds it.
      const active = document.activeElement;
      if (ready && active && active !== document.body && (!args?.length || active.id === args[0])) active.blur();
      continue;
    }
    if (name === "setSelectionRange") { setFieldSelection([...root.querySelectorAll("[id]")].find(node => node.id === args?.[0]), args, log); continue; }
    const selectText = name === "selectText";
    if (args?.length !== 1 || typeof args[0] !== "string" || !ready) continue;
    const el = [...root.querySelectorAll("[id]")].find(node => node.id === args[0]);
    const reason = !el ? "no live node with that id" : !el.isConnected ? "not mounted" : el.matches(":disabled,[disabled]") ? "disabled"
      : inertAncestor(el) ? "inert ancestor" : !el.getClientRects().length ? "zero size"
      : getComputedStyle(el).visibility !== "visible" ? "hidden ancestor" : null;
    if (reason) { log(`focus "${args[0]}" refused: ${reason}`); continue; }
    if (selectText && typeof el.select !== "function") { log(`selectText "${args[0]}" refused: not a text editor`); continue; }
    el.focus();
    if (selectText && document.activeElement === el) el.select();
  }
}

// The nearest inert ancestor (a modal dialog ends the search: its subtree is live).
export function inertAncestor(el) {
  for (let node = el; node; node = node.parentElement) {
    if (node.hasAttribute("inert")) return node;
    if (node.localName === "dialog" && node.matches(":modal")) return null;
  }
  return null;
}

// The page's environment: the safe-area insets from a hidden probe's padding, and the
// keyboard's height as the visual viewport reports it.
let probe;
export function environment() {
  if (!probe) {
    probe = document.createElement("div");
    probe.style.cssText = "position:fixed;inset:0;visibility:hidden;pointer-events:none;padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)";
    document.body.append(probe);
  }
  const r2 = (x) => Math.round(x * 100) / 100;
  const cs = getComputedStyle(probe);
  return {
    "safe-area-inset-top": r2(parseFloat(cs.paddingTop) || 0),
    "safe-area-inset-right": r2(parseFloat(cs.paddingRight) || 0),
    "safe-area-inset-bottom": r2(parseFloat(cs.paddingBottom) || 0),
    "safe-area-inset-left": r2(parseFloat(cs.paddingLeft) || 0),
    "keyboard-inset-height": r2(Math.max(0, innerHeight - (visualViewport?.height ?? innerHeight))),
    ...foldEnv(),
  };
}

// @ref LLP 1078 D6, D7 — the fold as the browser reports it: the Device
// Posture API's `navigator.devicePosture.type` and the viewport segments
// `window.viewport.segments` (two or more means a divider splits the
// viewport; the columns are the distinct lefts, the rows the distinct tops).
// Where the browser lacks the APIs: `continuous`, 1 × 1, which is also what
// Chromium reports on a flat display. Under the agent, `prefer posture` and
// `prefer segments` go through CDP's display-feature and posture overrides
// (the driver), so the browser's own readings change; where CDP offers
// none, the driver's substitute lands here (`preferFold`) and stands in for
// them — the facts and `layout.env`, not CSS's own `env()` resolution. The
// substitute lives on `globalThis.exact`: the JS target's agent reads its own
// copy of this module (`agent-navigation.js`), and both copies must agree.
const foldSubstitute = () => globalThis.exact?.foldSubstitute ?? null;
const r2 = (x) => Math.round(x * 100) / 100;
function readFold() {
  const posture = globalThis.navigator?.devicePosture?.type === "folded" ? "folded" : "continuous";
  const segments = globalThis.viewport?.segments;
  const rects = Array.isArray(segments) && segments.length >= 2 ? segments.map((s) => [s.x, s.y, s.width, s.height]) : [];
  const cols = rects.length ? new Set(rects.map((r) => r[0])).size : 1, rows = rects.length ? new Set(rects.map((r) => r[1])).size : 1;
  return { posture, cols, rows, rects };
}
export const fold = () => foldSubstitute() ?? readFold();
/** `layout.env`'s four names (LLP 1012 §1). */
export function foldEnv() {
  const f = fold();
  return { "device-posture": f.posture, "horizontal-viewport-segments": f.cols, "vertical-viewport-segments": f.rows, "viewport-segments": f.rects.map((r) => r.map(r2)) };
}
/** The fold as `exact_resize`'s facts word carries it beside the preference bits (LLP 1078 D6): bit 8 `folded`, bits 9–16 the columns, 17–24 the rows. */
export function foldBits() { const f = fold(); return (f.posture === "folded" ? 256 : 0) | ((f.cols & 255) << 9) | ((f.rows & 255) << 17); }
export function onFold(changed) {
  globalThis.navigator?.devicePosture?.addEventListener?.("change", changed);
  addEventListener("resize", changed);
}
/** The grid a host without a fold makes for `prefer segments <cols>x<rows> [gap <points>]`: the viewport split evenly, the gap centred on each divider; refused by name. */
export function evenSegments(width, height, cols, rows, gap = 0) {
  if (!(cols >= 1 && rows >= 1)) throw new Error(`segments ${cols}x${rows}: each count is at least 1`);
  if (!(Number.isFinite(gap) && gap >= 0)) throw new Error(`segments: gap ${gap} is not a non-negative length`);
  if (cols * rows === 1) return [];
  const span = (total, n) => { const bands = (n - 1) * gap; if (bands >= total) throw new Error(`segments ${cols}x${rows} gap ${gap}: the gap is wider than the viewport (${width} × ${height})`); return (total - bands) / n; };
  const w = span(width, cols), h = span(height, rows), out = [];
  for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) out.push([x * (w + gap), y * (h + gap), w, h]);
  return out;
}
/** The agent's substitute (`prefer`'s `fold` group): `posture`, `cols`, `rows`, `gap`; `null` drops it and the browser's own readings return. */
export function preferFold(request) {
  if (request == null) { (globalThis.exact ??= {}).foldSubstitute = null; return foldEnv(); }
  const next = { ...fold() };
  let gap = 0, grid = false;
  for (const [name, raw] of Object.entries(request)) {
    const n = Number(raw);
    switch (name) {
      case "posture": if (raw !== "folded" && raw !== "continuous") throw new Error(`prefer: posture: ${raw} is folded or continuous`); next.posture = raw; break;
      case "cols": case "rows": if (!(Number.isInteger(n) && n >= 1)) throw new Error(`prefer: segments: ${raw} ${name === "cols" ? "columns" : "rows"} is not a count`); next[name] = n; grid = true; break;
      case "gap": if (!(Number.isFinite(n) && n >= 0)) throw new Error(`prefer: segments: gap ${raw} is not a length`); gap = n; grid = true; break;
      default: throw new Error(`prefer: ${name} is not a fold fact this host sets`);
    }
  }
  if (grid) { try { next.rects = evenSegments(innerWidth, innerHeight, next.cols, next.rows, gap); } catch (e) { throw new Error(`prefer: ${e.message}`); } }
  (globalThis.exact ??= {}).foldSubstitute = next;
  return foldEnv();
}

// @ref LLP 1061 D4, LLP 1069.000 D1 — the user's display preferences as the
// page's media queries report them: bit 0 `prefers-reduced-motion: reduce`,
// bit 1 `prefers-reduced-transparency: reduce`, bit 2 `prefers-contrast: more`,
// bit 3 `less` (both: `custom`), bit 4 `prefers-color-scheme: dark` — the
// system's, whatever the page's `color-scheme` (a browser that does not know
// a feature answers no preference, as CSS does) — and the primary input's
// `pointer: coarse` (bit 5), `pointer: none` (bit 6) and `hover: none` (bit 7),
// zero being a mouse. Told with each boot and resize.
let preferenceQueries;
// Bits 8–9 `color-gamut` (256 p3; 512 with it, rec2020) and bit 10 `dynamic-range: high` (LLP 1100 D9).
const queries = () => (preferenceQueries ??= [["(prefers-reduced-motion: reduce)", 1], ["(prefers-reduced-transparency: reduce)", 2], ["(prefers-contrast: more)", 4], ["(prefers-contrast: less)", 8], ["(prefers-contrast: custom)", 12], ["(prefers-color-scheme: dark)", 16], ["(pointer: coarse)", 32], ["(pointer: none)", 64], ["(hover: none)", 128], ["(color-gamut: p3)", 256], ["(color-gamut: rec2020)", 512], ["(dynamic-range: high)", 1024]].map(([q, bits]) => [matchMedia(q), bits]));
export const preferences = () => queries().reduce((bits, [q, bit]) => bits | (q.matches ? bit : 0), 0);
export const onPreferences = (changed) => queries().forEach(([q]) => q.addEventListener("change", changed));

// @ref LLP 1069.000 D2 — the page's facts as `exact_set_page` takes them:
// bit 0 `document.visibilityState == "hidden"`, bit 1 `!navigator.onLine`,
// bit 2 `typeof navigator.share === "function"` (LLP 1069.003 D5), bit 3
// `typeof showOpenFilePicker === "function"` (LLP 1069.010 D2; studio diary
// R31), bit 4 `!document.hasFocus()` (#114), told again at the window's
// `focus` and `blur`. Under the agent the drive's values stand in (visible,
// online, a share sheet, the pickers, focus: LLP 1069.000 D6), set by
// `prefer`'s `page` group; the machine is never read.
export function pageReporter(agent, platform = globalThis) {
  const facts = { "visibility-state": "visible", online: true, "can-share": true, "can-open-files": true, "has-focus": true, "root-font-size": 16 };
  // @ref LLP 1069.000 D3 — the root font size: the document element's
  // computed `font-size`, the browser's setting unless a page sets it; under
  // the agent the drive sets it on the element (`prefer root-font-size`).
  // The app's own size (`appRootFontSize`) is set over it and read past.
  const rootFontSize = () => agent ? facts["root-font-size"] : beneathApp(platform, () => parseFloat(platform.getComputedStyle(platform.document.documentElement).fontSize) || 16);
  const read = () => agent ? { ...facts } : { "visibility-state": platform.document.visibilityState === "hidden" ? "hidden" : "visible", online: platform.navigator.onLine !== false, "can-share": typeof platform.navigator.share === "function", "can-open-files": typeof platform.showOpenFilePicker === "function", "has-focus": typeof platform.document.hasFocus !== "function" || platform.document.hasFocus() };
  const bits = () => { const f = read(); return (f["visibility-state"] === "hidden" ? 1 : 0) | (f.online ? 0 : 2) | (f["can-share"] ? 4 : 0) | (f["can-open-files"] ? 8 : 0) | (f["has-focus"] ? 0 : 16); };
  const prefer = (page) => {
    const next = { ...facts };
    for (const [name, raw] of Object.entries(page ?? {})) {
      const value = String(raw);
      if (name === "visibility-state" && (value === "visible" || value === "hidden")) next[name] = value;
      else if ((name === "online" || name === "can-share" || name === "can-open-files" || name === "has-focus") && (value === "true" || value === "false")) next[name] = value === "true";
      else if (name === "root-font-size" && Number(value) > 0 && Number.isFinite(Number(value))) next[name] = Number(value);
      else throw new Error(`prefer: ${name}: ${value} is not a page fact this host sets`);
    }
    Object.assign(facts, next);
    if (page?.["root-font-size"] !== undefined) platform.document.documentElement.style.fontSize = `${facts["root-font-size"]}px`;
  };
  const onChange = (changed) => { if (agent) return; platform.document.addEventListener("visibilitychange", changed); for (const name of ["online", "offline", "focus", "blur"]) platform.addEventListener(name, changed); };
  return { bits, read: () => ({ ...read(), "root-font-size": rootFontSize() }), prefer, onChange, rootFontSize };
}

// @ref LLP 1069.000 D3 — the app's `setRootFontSize(px)`: `:root {
// font-size: <px> !important }`, an author rule over the root element's own
// size (the browser's setting, or the agent's `prefer root-font-size`), which
// can change beneath it; `"medium"` removes the rule. The wasm runner checked
// the value; the JS target checks it here.
const APP_ROOT = "exact-root-font-size";
export function appRootFontSize(value, say, platform = globalThis) {
  const doc = platform.document;
  let rule = doc.getElementById(APP_ROOT);
  if (value === "medium") return void rule?.remove();
  if (typeof value !== "number" || !(Math.fround(value) > 0) || !Number.isFinite(Math.fround(value))) return say?.(`setRootFontSize(${JSON.stringify(value) ?? ""}) refused: the root font size is a number of px above 0, or "medium"`);
  if (!rule) { rule = doc.createElement("style"); rule.id = APP_ROOT; doc.head.append(rule); }
  rule.textContent = `:root{font-size:${value}px!important}`;
}
// What `read` gives with the app's rule set aside: the host's own reading.
function beneathApp(platform, read) {
  const rule = platform.document.getElementById?.(APP_ROOT);
  if (!rule) return read();
  rule.disabled = true;
  try { return read(); } finally { rule.disabled = false; }
}

// The page launch owns its seed; a new runner during development reuses it.
// Agent facts are supplied by the drive, never by the browser's environment.
export function placeReporter(params, platform = globalThis) {
  let seed;
  return () => {
    if (AGENT_ADMITTED && params.has('agent')) {
      const value = Number(params.get('seed') ?? 1);
      if (!Number.isSafeInteger(value) || value < 0) throw new Error('seed: an integer from 0 through 2^53 - 1');
      return [params.get('locale') ?? 'en-US', params.get('timeZone') ?? 'UTC', value].join('\0');
    }
    if (seed === undefined) {
      const words = platform.crypto.getRandomValues(new Uint32Array(2));
      seed = (words[0] & 0x1fffff) * 4294967296 + words[1];
    }
    return [platform.navigator.language, platform.Intl.DateTimeFormat().resolvedOptions().timeZone, seed].join('\0');
  };
}

// @ref LLP 1027.000.000 D3 — under the agent the date at the clock's zero is
// the drive's epoch (default 2026-01-01T00:00:00Z), and the offset its zone's
// at the virtual instant `elapsed` names; `clock +N` moves the date because
// `now()` moves, and the glue tells the offset again after each clock move, so
// crossing a DST change re-answers it (LLP 1069.007 D2).
export function timeReporter(params, platform = globalThis) {
  return (elapsed) => {
    if (!(AGENT_ADMITTED && params.has('agent'))) return [platform.Date.now() - elapsed, -new platform.Date().getTimezoneOffset()];
    const epoch = Number(params.get('epoch') ?? Date.UTC(2026, 0, 1));
    if (!Number.isSafeInteger(epoch) || epoch < 0) throw new Error('epoch: Unix milliseconds at or after 1970');
    const at = {};
    for (const {type, value} of new Intl.DateTimeFormat('en-US', {timeZone: params.get('timeZone') ?? 'UTC', hourCycle: 'h23', year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric'}).formatToParts(epoch + elapsed)) at[type] = Number(value);
    return [epoch, (Date.UTC(at.year, at.month - 1, at.day, at.hour, at.minute, at.second) - Math.floor((epoch + elapsed) / 1000) * 1000) / 60000];
  };
}
// Where the page launches (LLP 1038 D5): its path and query, less a drive's
// own parameters, which are the host's facts and not a route's — a driven
// page numbers its visits as a native host does (feed F16). The JS target
// launches here too (rt.js re-exports it).
export function launchLocation(platform = globalThis) {
  const { pathname, search } = platform.location, q = new URLSearchParams(search);
  if (!(AGENT_ADMITTED && q.has('agent'))) return pathname + search;
  for (const k of agentParameters) q.delete(k);
  const rest = q.toString();
  return pathname + (rest ? '?' + rest : '');
}
// The drive's facts are the launch URL's, even after a route changes the
// address bar (storage-environment.js `launchHref`).
const launched = () => new URL(globalThis.performance?.getEntriesByType?.('navigation')[0]?.name ?? location.href).searchParams;
let pageTime;
export const reportTime = (elapsed) => (pageTime ??= timeReporter(launched()))(elapsed);

let pagePlace;
export function reportPlace() {
  pagePlace ??= placeReporter(launched());
  return pagePlace();
}

// An iframe guest's origin as authored when its `src` or `sandbox` was
// committed (an opaque sandbox posts as "null"), which a `message` from it must match.
const iframeOrigins = new WeakMap(); // iframe -> authored/committed guest origin
export function commitGuestOrigin(el) {
  const sandbox = new Set((el.getAttribute("sandbox") ?? "").split(/\s+/).filter(Boolean));
  const opaque = el.hasAttribute("sandbox") && !sandbox.has("allow-same-origin");
  let origin = null;
  if (!opaque) {
    const src = el.getAttribute("src");
    try { origin = !src || src === "about:blank" ? location.origin : new URL(src, document.baseURI).origin; }
    catch { origin = null; }
    if (origin === "null") origin = null;
  }
  iframeOrigins.set(el, { origin, opaque });
}
export function guestMessageAuthorized(el, eventOrigin) {
  const committed = iframeOrigins.get(el);
  if (!committed) return false;
  return committed.opaque ? eventOrigin === "null" : eventOrigin === committed.origin;
}
// A same-origin guest joins `tree` as a compact, bounded outline. Access to
// a sandboxed or cross-origin document is simply absent (@ref LLP 1020 D4).
export function guestOutline(frame) {
  let doc;
  try { doc = frame.contentDocument; } catch { return null; }
  if (!doc) return null;
  const outline = [];
  const visit = (el, depth) => {
    if (depth > 4 || outline.length >= 32) return;
    const id = el.id || undefined;
    const testId = el.getAttribute("data-testid") ?? el.getAttribute("testId") ?? undefined;
    const text = [...el.childNodes]
      .filter((n) => n.nodeType === Node.TEXT_NODE)
      .map((n) => n.textContent.trim())
      .filter(Boolean)
      .join(" ")
      .replace(/\s+/g, " ")
      .slice(0, 160) || undefined;
    if (id || testId || text) {
      outline.push({ guest: true, depth, tag: el.localName, ...(id ? { id } : {}), ...(testId ? { testId } : {}), ...(text ? { text } : {}) });
    }
    for (const child of el.children) visit(child, depth + 1);
  };
  for (const child of doc.body?.children ?? []) visit(child, 0);
  return outline;
}
function guestDocument(frame) {
  try {
    const document = frame.contentDocument;
    return document ? { document } : { error: "guest is cross-origin" };
  } catch {
    return { error: "guest is cross-origin" };
  }
}
export function guestTap(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const x = Number.isFinite(request.x) ? request.x : guest.innerWidth / 2;
  const y = Number.isFinite(request.y) ? request.y : guest.innerHeight / 2;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, error: "guest tap has an invalid selector" }; }
  target ||= document.elementFromPoint(x, y) || document.body;
  if (!target) return { guest: true, error: "guest tap found no target" };
  // Script input is intentionally untrusted (@ref LLP 1020 D4;
  // exact1 20260806-webview-frame-guest-click-delivery).
  target.dispatchEvent(new guest.PointerEvent("pointerdown", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 1 }));
  target.dispatchEvent(new guest.PointerEvent("pointerup", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 0 }));
  target.dispatchEvent(new guest.MouseEvent("click", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0 }));
  return { tapped: request.id, guest: true };
}
export function guestType(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const active = document.activeElement;
  const editable = active?.matches?.("input,textarea,[contenteditable]") ? active : null;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, error: "guest type has an invalid selector" }; }
  target ||= editable || document.querySelector("input,textarea,[contenteditable]");
  if (!target) return { guest: true, error: "guest type found no target" };
  // These are the Apple guest script's event shapes, including focus and
  // isTrusted:false (@ref LLP 1020 D4).
  target.focus();
  if (request.key != null) {
    const key = String(request.key);
    target.dispatchEvent(new guest.KeyboardEvent("keydown", { key, bubbles: true, composed: true }));
    target.dispatchEvent(new guest.KeyboardEvent("keyup", { key, bubbles: true, composed: true }));
    return { typed: request.id, guest: true, key, value: guestValue(target) };
  }
  const text = String(request.text ?? "");
  if ("value" in target) target.value = text; else target.textContent = text;
  target.dispatchEvent(new guest.InputEvent("input", { data: text, inputType: "insertText", bubbles: true, composed: true }));
  target.dispatchEvent(new guest.Event("change", { bubbles: true, composed: true }));
  return { typed: request.id, guest: true, value: guestValue(target) };
}
// What a guest field holds, as the reply shows it: a password's is a fixed mark, whatever its length (#134).
const guestValue = (target) => "value" in target ? (target.type === "password" && target.value ? "•••" : target.value) : target.textContent;

// @ref LLP 1069.001 D4 (amended 2026-10-04) — a select, range or date is
// controlled as a text field is: the committed `value` is written when it
// changes, and shown again after the options change or a refused `type`;
// after the action that heard the person (`acted`) it keeps their choice,
// as the web build's does (x2apps kanban2 #5: a date cleared while a `send`
// was in flight).
const VALUED = new Set(["range", "date", "time", "datetime-local"]);
export const valuedControl = (el) => el instanceof HTMLSelectElement || (el instanceof HTMLInputElement && VALUED.has(el.type));
export function settleValue(el, acted = false) {
  const c = el instanceof HTMLOptionElement ? el.parentElement : el;
  if (acted) return;
  if (c && valuedControl(c) && c.exactValue !== undefined && c.value !== c.exactValue) c.value = c.exactValue;
}
// What `type <id> <value>` sets rather than types into (D9): the valued
// controls, a checkbox (or `switch`), which a value turns on or off, and a
// radio, which `true` checks (x2apps survey #2).
export const typedControl = (el) => valuedControl(el) || (el instanceof HTMLInputElement && (el.type === "checkbox" || el.type === "radio"));
// A text field (x2apps codeedit #2): an input that is no control, or a
// textarea that is not the Markdown editor; its `input` and `change` carry
// its selection, and its `select` is HTML's.
export const textField = (el) => el instanceof HTMLTextAreaElement ? el.getAttribute("markup") !== "markdown" : el instanceof HTMLInputElement && !typedControl(el) && el.type !== "file";
// Its selection as host kinds 40 to 42 carry it, before its text:
// `start,end,direction,` in UTF-16 units, the DOM's; a type with none
// (email, number) has its caret after its text, as the runner assumes.
export const fieldSelection = (el) => { const end = el.value.length; return `${el.selectionStart ?? end},${el.selectionEnd ?? end},${el.selectionDirection ?? "none"},`; };
// A radio's group as committed again (x2apps survey #2): the browser checked
// the radio and unchecked its group at once; the action decides, and one
// that refuses snaps the group back. `bound` reads a radio's committed
// `checked` (undefined when it has none); the unchecks go first, so the
// radio left checked is the committed one.
export function settleRadios(el, bound) {
  const group = el.name ? [...document.querySelectorAll('input[type="radio"]')].filter((r) => r.name === el.name) : [el];
  for (const r of group) if (bound(r) === false && r.checked) r.checked = false;
  for (const r of group) if (bound(r) === true && !r.checked) r.checked = true;
}
// `setSelectionRange("id", start, end[, direction])` (x2apps codeedit #2):
// the field's own method, by its id, after the batch; it does not focus.
export function setFieldSelection(el, args, log) {
  const [id, start, end, direction] = args ?? [];
  if (!el) return log(`setSelectionRange "${id}" refused: no live node with that id`);
  if (typeof el.setSelectionRange !== "function") return log(`setSelectionRange "${id}" refused: not a text field`);
  try { el.setSelectionRange(start, end, direction); } catch { log(`setSelectionRange "${id}" refused: an input type=${el.type} has no text selection`); }
}
// The wasm host's `input`, `change` and a text field's `select` (LLP
// 1069.001 D4; x2apps survey #2, codeedit #2), `dispatch(kind, payload)`:
// a checkbox's checked state (25, 24), a radio's value (23, 1), a text
// field's selection then its text (40, 41, 42), another control's value
// (23, 1). The platform moves a control at once; the action decides, and
// one that refuses snaps a checkbox or a radio group back.
export function controlEvent(el, kind, on, dispatch) {
  if (kind === "select") return void (textField(el) && on("select", () => dispatch(42, fieldSelection(el) + el.value)));
  if (el.type === "checkbox") return on(kind, () => { dispatch(kind === "change" ? 24 : 25, String(el.checked)); if (el.exactChecked !== undefined && el.checked !== el.exactChecked) el.checked = el.exactChecked; });
  if (el.type === "radio") return on(kind, () => { dispatch(kind === "change" ? 1 : 23, el.value); settleRadios(el, (r) => r.exactChecked); });
  // HTML's `change`: a text field's value committed, on blur or Enter.
  if (kind === "change") return on("change", () => { dispatch(textField(el) ? 41 : 1, (textField(el) ? fieldSelection(el) : "") + el.value); settleValue(el, true); });
  on("input", (e) => {
    const value = el.value;
    if (el.getAttribute("emojiPicker") === "true") {
      if (e.isComposing) return;
      el.value = "";
      const clusters = [...new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(value)];
      if (clusters.length !== 1 || !(/\p{Emoji_Presentation}/u.test(value) || (/[\uFE0F\u20E3]/u.test(value) && /\p{Emoji}/u.test(value)))) return;
      return void dispatch(23, value);
    }
    dispatch(textField(el) ? 40 : 23, (textField(el) ? fieldSelection(el) : "") + value); settleValue(el, true);
  });
}
// D9: `type <id> <value>` sets a control's value as the platform would on a
// choice or a release: HTML's `input`, then `change`. A select takes one of
// its enabled options by value, else by its one label (Playwright's
// `selectOption`, the diaries' kanban F17 and shop F10); a checkbox takes
// `true` or `false` and is clicked when that differs, as a person would.
export function typeControl(el, request) {
  let text = String(request.text ?? "");
  const id = request.id;
  if (el.disabled || inertAncestor(el)) return { handled: true, error: `view ${id} is disabled or inert` };
  // A read-only field takes no value a person could enter. HTML's `readonly` applies to a textarea and the
  // text-like and date/time inputs only; a checkbox, range, color or select ignores it.
  if (el.readOnly && (el instanceof HTMLTextAreaElement || /^(text|search|url|tel|email|password|date|month|week|time|datetime-local|number)$/.test(el.type))) return { handled: true, error: `view ${id} is read-only` };
  if (el.type === "checkbox") {
    if (text !== "true" && text !== "false") return { handled: true, error: `checkbox ${id} takes true or false, not ${JSON.stringify(text)}` };
    if (el.checked !== (text === "true")) el.click();
    return { typed: id, checked: el.checked, delivery: "recognized", handled: true };
  }
  // A radio is checked by choosing it, and unchecked only by checking another (x2apps survey #2).
  if (el.type === "radio") {
    if (text !== "true") return { handled: true, error: text === "false" ? `radio ${id} is unchecked by checking another of its group` : `radio ${id} takes true, not ${JSON.stringify(text)}` };
    if (!el.checked) el.click();
    return { typed: id, checked: el.checked, delivery: "recognized", handled: true };
  }
  if (el instanceof HTMLSelectElement) {
    const enabled = [...el.options].filter((o) => !o.disabled), labelled = enabled.filter((o) => o.label.trim() === text.trim());
    if (!enabled.some((o) => o.value === text) && labelled.length === 1) text = labelled[0].value;
    else if (!enabled.some((o) => o.value === text))
      return { handled: true, error: `select ${id} has no enabled option ${JSON.stringify(text)}${labelled.length > 1 ? " (that label is on more than one option: choose by value)" : ""} (options: ${enabled.map((o) => `${JSON.stringify(o.value)} ${JSON.stringify(o.label)}`).join(", ")})` };
  }
  if (el.type === "range" && !(text.trim() !== "" && Number.isFinite(Number(text)))) return { handled: true, error: `${JSON.stringify(text)} is not a number` };
  el.value = text;
  if (el.value !== text && el instanceof HTMLInputElement && el.type !== "range") return settleValue(el), { handled: true, error: `${JSON.stringify(text)} is not a value an input type=${el.type} takes; it sanitized to ${JSON.stringify(el.value)}` };
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
  return { typed: id, value: el.value, delivery: "recognized", handled: true };
}

// The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a
// target whose middle is out of view is scrolled to the middle of its
// nearest scroll containers, then of the page (across, only as far as it
// takes) — Playwright's actionability scroll, the web's own `scrollIntoView`. A scroll event reaches the app as a
// person's scroll would. `scrolled` is where the middle moved from and to;
// a middle already in view moves nothing.
export function reveal(el, id) {
  if (!el?.isConnected) return { error: `no view ${id}` };
  const middle = () => { const b = viewBox(el); return [b.left + b.width / 2, b.top + b.height / 2]; };
  const [x, y] = middle();
  let seen = x >= 0 && y >= 0 && x < innerWidth && y < innerHeight;
  for (let a = el.parentElement; seen && a && a.id !== "exact-root"; a = a.parentElement) {
    const cs = getComputedStyle(a), b = a.getBoundingClientRect();
    if ((cs.overflowX !== "visible" || cs.overflowY !== "visible") && !(x >= b.left && x < b.right && y >= b.top && y < b.bottom)) seen = false;
  }
  if (seen) return { revealed: id, scrolled: false };
  (folded(el) ? el.parentElement : el).scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" });
  return { revealed: id, scrolled: true, from: [x, y], to: middle() };
}

// A text folded into its box's content (LLP 1007.001): `display: contents`,
// or an inline box under a box that restricts touch. A paragraph's inline
// runs are not `data-exact-text`; only the paragraph is.
const folded = el => { const d = getComputedStyle(el).display; return d === "contents" || d === "inline" && el.hasAttribute("data-exact-text"); };

// A view's box as the agent reports it. A folded text has no block box of its
// own: its box is the anonymous block its text is, as a style-less block
// child's was — its line boxes along the main axis, and its box's content box
// across when the box stretches its items (a block, or a flex or grid box
// that stretches).
export function viewBox(el) {
  if (!folded(el)) return el.getBoundingClientRect();
  const range = document.createRange();
  range.selectNodeContents(el);
  const t = range.getBoundingClientRect(), p = el.parentElement;
  if (!p) return t;
  const cs = getComputedStyle(p), b = p.getBoundingClientRect(), n = s => parseFloat(cs[s]) || 0;
  const left = b.left + n("borderLeftWidth") + n("paddingLeft"), right = b.right - n("borderRightWidth") - n("paddingRight");
  const top = b.top + n("borderTopWidth") + n("paddingTop"), bottom = b.bottom - n("borderBottomWidth") - n("paddingBottom");
  const flex = /flex|grid/.test(cs.display), row = flex && cs.display.includes("flex") && !cs.flexDirection.startsWith("column");
  const stretch = !flex || /normal|stretch/.test(cs.alignItems);
  if (row) return stretch ? new DOMRect(t.x, top, t.width, bottom - top) : t;
  return stretch ? new DOMRect(left, t.y, right - left, t.height) : t;
}

// WHATWG URL normalization for the wasm grant parser, using the browser's
// existing tables. This is pure parsing: it grants no host I/O to the app.
export function grantOrigins(memory) {
  return { origin(ptr, len, wildcard, out, capacity) {
    try {
      const target = new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(memory().buffer, ptr >>> 0, len >>> 0));
      const u = new URL(target), port = u.port || ({ 'http:': 80, 'https:': 443, 'ws:': 80, 'wss:': 443, 'ftp:': 21 })[u.protocol];
      if (!u.hostname || port == null) return -1;
      const address = u.hostname.startsWith('[') || /^(?:https?|wss?|ftp):$/.test(u.protocol) && /^[\d.]+$/.test(u.hostname);
      if (wildcard && (address || !['', '/'].includes(u.pathname) || target.includes('?') || target.includes('#')
        || u.hostname.endsWith('.') || u.hostname.split('.').filter(Boolean).length < 2)) return -1;
      const bytes = new TextEncoder().encode([u.protocol.slice(0, -1), u.hostname.toLowerCase(), port].join('\0'));
      if (capacity) { if (capacity < bytes.length) return -1; new Uint8Array(memory().buffer, out >>> 0, bytes.length).set(bytes); }
      return bytes.length;
    } catch { return -1; }
  } };
}

// grant admission: begin — self-contained; the JS target's build (host/web-js/build.mjs) moves these lines into a module of
// their own, so a page that admits nothing before a lazy chunk does not carry them; the wasm host keeps them here (boot.mjs).
// Match only the sealed, typed output of exact-runner's Rust grant parser.
// App code is the page, so this is parity admission rather than a sandbox.
const INVALID_GRANTS = 'the grant set was not validated';

const grantFNV = text => {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
};
// Rust's JSON writer and JavaScript's serializer use this one spelling for the
// two ECMAScript line separators before the seal is calculated.
const grantBody = set => JSON.stringify({ version: 1, entries: set.entries, error: set.error })
  .replaceAll('\u2028', '\\u2028').replaceAll('\u2029', '\\u2029');
const networkGrants = new Set(['fetch', 'fetch-subdomains', 'websocket']);
const pathGrants = new Set(['fs-read', 'fs-write', 'sqlite-open']);
const nameGrants = new Set(['env-read', 'secret-keep', 'storage-kv']);
const validGrantName = name => typeof name === 'string' && name.length >= 1 && name.length <= 64
  && /^[a-z0-9._-]+$/.test(name) && !/^\.+$/.test(name);
// Rust `str::trim`/`split_whitespace` use Unicode White_Space, which differs
// from JavaScript's `\s` at U+0085 and U+FEFF.
const rustSpace = '[\\u0009-\\u000d\\u0020\\u0085\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000]';
const rustTrim = value => String(value).replace(new RegExp(`^${rustSpace}+|${rustSpace}+$`, 'g'), '');
const rustWords = value => rustTrim(value).split(new RegExp(`${rustSpace}+`)).filter(Boolean);
const nativeNamespace = value => typeof value === 'string' && /^win:[A-Z]$/.test(value);
const nativeLeaf = value => {
  if (typeof value !== 'string' || !value || value.length > 255 || value === '.' || value === '..'
      || /[\\/:*?"<>|\u0000-\u001f\u007f-\u009f]/.test(value) || /[. ]$/.test(value)
      || /[\ud800-\udfff]/u.test(value)) return false;
  const base = value.split('.')[0].replace(/[a-z]/g, char => char.toUpperCase());
  return !/^(?:CON|PRN|AUX|NUL|CLOCK\$|(?:COM|LPT)[1-9¹²³])$/.test(base);
};
const grantTupleValid = grant => {
  if (!Array.isArray(grant) || typeof grant[0] !== 'string') return false;
  if (networkGrants.has(grant[0])) {
    const address = grant[2]?.startsWith('[')
      || /^(?:https?|wss?|ftp)$/.test(grant[1]) && /^[\d.]+$/.test(grant[2]);
    return grant.length === 4
    && /^[a-z][a-z0-9+.-]*$/.test(grant[1])
    && typeof grant[2] === 'string' && grant[2] === grant[2].toLowerCase() && grant[2].length > 0
    && Number.isInteger(grant[3]) && grant[3] >= 0 && grant[3] <= 65535
    && (grant[0] !== 'fetch-subdomains' || !address && !grant[2].endsWith('.') && grant[2].split('.').filter(Boolean).length >= 2);
  }
  if (pathGrants.has(grant[0])) return grant.length >= 2
    && (['', 'app:', 'doc:'].includes(grant[1]) || nativeNamespace(grant[1]))
    && grant.slice(2).every(component => nativeNamespace(grant[1]) ? nativeLeaf(component)
      : typeof component === 'string' && component && component !== '.' && component !== '..' && !component.includes('/'));
  if (!nameGrants.has(grant[0]) || grant.length !== 2 || typeof grant[1] !== 'string') return false;
  return grant[0] === 'env-read' || validGrantName(grant[1]);
};

const pathTuple = (kind, target) => {
  const native = target.startsWith('\\\\?\\') ? target.slice(4) : target;
  if (/^[a-z]:[/\\]/i.test(native)) {
    const parts = native.slice(3).split(/[/\\]/).filter(Boolean);
    return parts.every(nativeLeaf) ? [kind, `win:${native[0].toUpperCase()}`, ...parts] : null;
  }
  const at = target.indexOf(':/');
  const namespace = at < 0 ? target.startsWith('/') ? '' : null : target.slice(0, at) + ':';
  if (namespace == null || !['', 'app:', 'doc:'].includes(namespace)) return null;
  const rest = at < 0 ? target.slice(1) : target.slice(at + 2);
  const parts = rest.split('/').filter(Boolean);
  return parts.some(part => part === '.' || part === '..') ? null : [kind, namespace, ...parts];
};
const networkTuple = (kind, target) => {
  const wildcard = kind === 'fetch-subdomains';
  if (wildcard && !target.includes('://*.')) return null;
  // Userinfo is refused, as in Rust: `https://a.example@evil.com` is evil.com.
  if (target.includes('@')) return null;
  try {
    const url = new URL(wildcard ? target.replace('://*.', '://') : target);
    const port = Number(url.port || ({ 'http:': 80, 'https:': 443, 'ws:': 80, 'wss:': 443, 'ftp:': 21 })[url.protocol]);
    if (!url.hostname || !Number.isInteger(port)) return null;
    if (wildcard && (!['', '/'].includes(url.pathname) || url.search || url.hash)) return null;
    return [kind, url.protocol.slice(0, -1).toLowerCase(), url.hostname.toLowerCase(), port];
  } catch { return null; }
};
const sourceTuple = source => {
  const words = rustWords(source);
  const capability = words[0];
  if (capability === 'fs.read' || capability === 'fs.write') {
    const rest = rustTrim(source.slice(capability.length));
    if (rest.startsWith('"')) {
      try {
        const target = JSON.parse(rest);
        // serde_json refuses lone UTF-16 surrogates; JSON.parse does not.
        if (typeof target !== 'string' || /[\u0000-\u001f\u007f-\u009f]/.test(target)
            || /[\ud800-\udfff]/u.test(target)) return null;
        return pathTuple(capability === 'fs.read' ? 'fs-read' : 'fs-write', target);
      } catch { return null; }
    }
  }
  if (words.length !== 2) return null;
  const target = words[1];
  if (capability === 'net.fetch') return networkTuple(target.includes('://*.') ? 'fetch-subdomains' : 'fetch', target);
  if (capability === 'net.websocket') return target.includes('*') ? null : networkTuple('websocket', target);
  if (capability === 'fs.read') return pathTuple('fs-read', target);
  if (capability === 'fs.write') return pathTuple('fs-write', target);
  if (capability === 'sqlite.open') return pathTuple('sqlite-open', target);
  if (capability === 'env.read') return ['env-read', target];
  if (capability === 'secret.keep') return ['secret-keep', target];
  if (capability === 'storage.kv') return ['storage-kv', target];
  return null;
};
const exactOnly = source => source.startsWith('#') || source.startsWith('surface.read ')
  || source.startsWith('surface.write ') || source.startsWith('device.') || source.startsWith('auth.');
const entryMatchesSource = ([, source, grant, error]) => {
  if (grant === null) return error !== null || exactOnly(source);
  return error === null && JSON.stringify(sourceTuple(source)) === JSON.stringify(grant);
};

function validateGrantSet(set) {
  if (!set || set.version !== 1 || !Array.isArray(set.entries)
      || (set.error !== null && typeof set.error !== 'string') || !/^[0-9a-f]{16}$/.test(set.seal ?? '')) return false;
  let last = 0, hasError = false;
  for (const entry of set.entries) {
    if (!Array.isArray(entry) || entry.length !== 4 || !Number.isInteger(entry[0]) || entry[0] <= last
        || typeof entry[1] !== 'string' || !entry[1] || rustTrim(entry[1]) !== entry[1]
        || entry[2] !== null && !grantTupleValid(entry[2])
        || entry[3] !== null && typeof entry[3] !== 'string' || !entryMatchesSource(entry)) return false;
    last = entry[0];
    hasError ||= entry[3] !== null;
  }
  return (!hasError || set.error !== null) && grantFNV(grantBody(set)) === set.seal;
}

const freezeGrantSet = set => {
  for (const entry of set.entries) {
    if (entry[2]) Object.freeze(entry[2]);
    Object.freeze(entry);
  }
  Object.freeze(set.entries);
  return Object.freeze(set);
};

// The Rust parser is the only producer. Admission validates an incoming JSON
// value in full before making its authority immutable; a prior validation is
// never a reason to trust a mutable or branded object.
export function createGrantSet(value) {
  if (!validateGrantSet(value)) return freezeGrantSet(makeGrantSet([], INVALID_GRANTS));
  return freezeGrantSet(value);
}

function makeGrantSet(entries, error) {
  const set = { version: 1, entries, error, seal: '' };
  set.seal = grantFNV(grantBody(set));
  return freezeGrantSet(set);
}

export function grantError(set) {
  return validateGrantSet(set) ? set.error : INVALID_GRANTS;
}

export const rawGrantText = set => validateGrantSet(set) ? set.entries.map(entry => entry[1]).join('\n') : '';
export const hasGrant = (set, kind) => validateGrantSet(set) && !set.error && set.entries.some(entry => entry[2]?.[0] === kind);

const normalizedDeclaration = set => validateGrantSet(set)
  ? set.entries.map(([, source, grant, error]) => JSON.stringify([source, grant, error])) : [];

// Module metadata carries the author's spelling while the bake carries the
// Rust parser's normalized set. Blank lines and indentation have no bearing
// on activation; the normalized declarations do.
export function sameGrantDeclaration(set, source) {
  if (typeof source !== 'string' || !validateGrantSet(set)) return false;
  const child = scopedGrantSet(set, source);
  const parentLines = normalizedDeclaration(set), childLines = normalizedDeclaration(child);
  return parentLines.length === childLines.length && parentLines.every((line, index) => line === childLines[index]);
}

export function scopedGrantSet(parent, source) {
  if (source == null) return parent;
  if (!validateGrantSet(parent) || typeof source !== 'string') return makeGrantSet([], 'source scope exceeds the app\'s admitted grants');
  const available = new Map(parent.entries.map(entry => [entry[1], entry]));
  const entries = [], errors = [];
  for (const [index, raw] of source.split('\n').entries()) {
    const line = rustTrim(raw);
    if (!line) continue;
    const found = available.get(line);
    if (!found) return makeGrantSet([], 'source scope exceeds the app\'s admitted grants');
    entries.push([index + 1, line, found[2], found[3]]);
    if (found[3]) errors.push(`line ${index + 1}: ${found[3]}`);
  }
  return makeGrantSet(entries, errors.length ? `the app's grants did not parse: ${errors.join('; ')}` : null);
}

export function unionGrantSets(...sets) {
  if (sets.some(set => !validateGrantSet(set))) return makeGrantSet([], INVALID_GRANTS);
  const entries = [], seen = new Set();
  for (const set of sets) for (const entry of set.entries) if (!seen.has(entry[1])) {
    seen.add(entry[1]);
    entries.push([entries.length + 1, entry[1], entry[2], entry[3]]);
  }
  const error = sets.map(set => set.error).find(Boolean) ?? null;
  return makeGrantSet(entries, error);
}

const grantPort = url => Number(url.port || ({ 'http:': 80, 'https:': 443, 'ws:': 80, 'wss:': 443, 'ftp:': 21 })[url.protocol]);
export function admitsNetwork(set, value, operation = 'fetch') {
  if (grantError(set)) return false;
  let target;
  try { target = new URL(value); } catch { return false; }
  const kind = operation === 'websocket' ? 'websocket' : 'fetch';
  const scheme = target.protocol.slice(0, -1).toLowerCase(), host = target.hostname.toLowerCase(), port = grantPort(target);
  return set.entries.some(([, , grant]) => grant && grant[1] === scheme && grant[3] === port && (
    grant[0] === kind && grant[2] === host
    || kind === 'fetch' && grant[0] === 'fetch-subdomains' && host.length > grant[2].length + 1 && host.endsWith('.' + grant[2])
  ));
}

export function admitsSecret(set, name) {
  return !String(name).startsWith('exact.kept.') && !grantError(set)
    && set.entries.some(([, , grant]) => grant?.[0] === 'secret-keep' && grant[1] === String(name));
}

function grantPathParts(path) {
  if (typeof path !== 'string') return null;
  const at = path.indexOf(':/');
  const namespace = at < 0 ? path.startsWith('/') ? '' : null : path.slice(0, at) + ':';
  if (namespace == null || !['', 'app:', 'doc:'].includes(namespace)) return null;
  const rest = at < 0 ? path.slice(1) : path.slice(at + 2);
  const parts = rest.split('/').filter(Boolean);
  return parts.some(part => part === '.' || part === '..' || part.includes('\0')) ? null : [namespace, ...parts];
}

export function coversPath(set, capability, path) {
  if (grantError(set)) return false;
  const target = grantPathParts(path), kind = ({ 'fs.read': 'fs-read', 'fs.write': 'fs-write', 'sqlite.open': 'sqlite-open' })[capability];
  return !!target && set.entries.some(([, , grant]) => grant?.[0] === kind && grant.slice(1).every((part, index) => target[index] === part));
}
// grant admission: end

// `selectionchange` on a `text` (the reader diary), on both web targets: its
// part of the page's selection, reported as the text and its UTF-16 start and
// end in the element's own text when that part changes; nothing selected
// there is "" at 0, 0. One document listener serves every such element.
const selectedTexts = new Map();
export function onSelection(e, report) {
  if (!selectedTexts.size) document.addEventListener("selectionchange", () => {
    const s = getSelection(), r = s.rangeCount && !s.isCollapsed ? s.getRangeAt(0) : null;
    for (const [e, h] of selectedTexts) {
      if (!e.isConnected) { selectedTexts.delete(e); continue; }
      const at = (n, o) => { const p = document.createRange(); p.selectNodeContents(e); const c = p.comparePoint(n, o); if (!c) p.setEnd(n, o); return c < 0 ? 0 : p.toString().length; };
      let a = 0, b = 0;
      if (r?.intersectsNode(e)) { a = at(r.startContainer, r.startOffset); b = at(r.endContainer, r.endOffset); }
      if (a === b) a = b = 0;
      if (h.a !== a || h.b !== b) { h.a = a; h.b = b; h.report(e.textContent.slice(a, b), a, b); }
    }
  });
  selectedTexts.set(e, { report, a: 0, b: 0 });
}
