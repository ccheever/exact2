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
const routesOf = nav => stacksOf(nav).find(routes => routes.some(r => r.getAttribute("navigationKey") === nav.getAttribute("navigationKey"))) ?? [];
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
          log(`navigationKey "${key}" matches no route; the stack is unchanged`);
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
          route.style.visibility = active || (modal && index === selected - 1) ? "" : "hidden";
          route.inert = !active || !!route.authoredInert;
        }
      }
    }
    // The page module's container hooks, when it has them (LLP 1075.003.000 §3.7).
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
  const start = () => loading ??= Promise.all([load('./collection-glue.js', 'collectionGlue'), load('./motion-glue.js', 'motionGlue')])
    .then(([c, m]) => {
      const common = { views: o.views, now: o.now, generation: o.generation, inert: o.inert, applyBatch: o.applyBatch, ready: o.ready };
      const request = facts => o.wasm('exact_motion', m.motionBytes(facts)) ?? { accepted: false };
      const collections = c.collectionController({ root: o.root, views: o.views, agent: !!o.agent?.(), settled: () => arrange.commit(), report(bytes) {
        const batch = o.wasm('exact_collection_feedback', bytes);
        return batch ? c.applyCollectionFeedback(batch, o.applyBatch) : false;
      } });
      const motion = m.motionController({ ...common, releaseInteraction: pointer => collections.releaseInteraction(pointer), request });
      const arrange = m.arrangeController({ ...common, collections, motion, request });
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
  const arrange = { binding: call('arrange', 'binding'), state: call('arrange', 'state') };
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

// @ref LLP 1063 — exit-animation and layout-transition play in
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
  const starts = new WeakMap(), held = new WeakSet();
  return {
    register(t) {
      for (const a of document.getAnimations()) if (!starts.has(a)) { starts.set(a, t); if (a.playState === 'paused') held.add(a); }
    },
    seek(to) {
      for (const a of document.getAnimations()) {
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
      for (const a of document.getAnimations()) {
        const timing = a.effect?.getComputedTiming();
        if (timing && timing.endTime !== Infinity && !held.has(a)) to = Math.max(to, (starts.get(a) ?? now()) + timing.endTime);
      }
      return to;
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


// The focus, blur and selectText commands a batch carried, run once every
// node and value in it is committed (a focus handler may dispatch an action).
export function runFocusCommands(commands, { root, ready, inertAncestor, log }) {
  for (const { name, args } of commands) {
    if (name === "blur") { // `blur()` drops whatever holds focus; `blur(id)` only when that node holds it.
      const active = document.activeElement;
      if (ready && active && active !== document.body && (!args?.length || active.id === args[0])) active.blur();
      continue;
    }
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
// a feature answers no preference, as CSS does). Told with each boot and resize.
let preferenceQueries;
const queries = () => (preferenceQueries ??= [["(prefers-reduced-motion: reduce)", 1], ["(prefers-reduced-transparency: reduce)", 2], ["(prefers-contrast: more)", 4], ["(prefers-contrast: less)", 8], ["(prefers-contrast: custom)", 12], ["(prefers-color-scheme: dark)", 16]].map(([q, bits]) => [matchMedia(q), bits]));
export const preferences = () => queries().reduce((bits, [q, bit]) => bits | (q.matches ? bit : 0), 0);
export const onPreferences = (changed) => queries().forEach(([q]) => q.addEventListener("change", changed));

// @ref LLP 1069.000 D2 — the page's facts as `exact_set_page` takes them:
// bit 0 `document.visibilityState == "hidden"`, bit 1 `!navigator.onLine`,
// bit 2 `typeof navigator.share === "function"` (LLP 1069.003 D5). Under the
// agent the drive's values stand in (visible, online, a share sheet: LLP
// 1069.000 D6), set by `prefer`'s `page` group; the machine is never read.
export function pageReporter(agent, platform = globalThis) {
  const facts = { "visibility-state": "visible", online: true, "can-share": true, "root-font-size": 16 };
  // @ref LLP 1069.000 D3 — the root font size: the document element's
  // computed `font-size`, the browser's setting unless a page sets it; under
  // the agent the drive sets it on the element (`prefer root-font-size`).
  const rootFontSize = () => agent ? facts["root-font-size"] : parseFloat(platform.getComputedStyle(platform.document.documentElement).fontSize) || 16;
  const read = () => agent ? { ...facts } : { "visibility-state": platform.document.visibilityState === "hidden" ? "hidden" : "visible", online: platform.navigator.onLine !== false, "can-share": typeof platform.navigator.share === "function" };
  const bits = () => { const f = read(); return (f["visibility-state"] === "hidden" ? 1 : 0) | (f.online ? 0 : 2) | (f["can-share"] ? 4 : 0); };
  const prefer = (page) => {
    const next = { ...facts };
    for (const [name, raw] of Object.entries(page ?? {})) {
      const value = String(raw);
      if (name === "visibility-state" && (value === "visible" || value === "hidden")) next[name] = value;
      else if ((name === "online" || name === "can-share") && (value === "true" || value === "false")) next[name] = value === "true";
      else if (name === "root-font-size" && Number(value) > 0 && Number.isFinite(Number(value))) next[name] = Number(value);
      else throw new Error(`prefer: ${name}: ${value} is not a page fact this host sets`);
    }
    Object.assign(facts, next);
    if (page?.["root-font-size"] !== undefined) platform.document.documentElement.style.fontSize = `${facts["root-font-size"]}px`;
  };
  const onChange = (changed) => { if (agent) return; platform.document.addEventListener("visibilitychange", changed); platform.addEventListener("online", changed); platform.addEventListener("offline", changed); };
  return { bits, read: () => ({ ...read(), "root-font-size": rootFontSize() }), prefer, onChange, rootFontSize };
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
let pageTime;
export const reportTime = (elapsed) => (pageTime ??= timeReporter(new URL(location.href).searchParams))(elapsed);

let pagePlace;
export function reportPlace() {
  pagePlace ??= placeReporter(new URL(location.href).searchParams);
  return pagePlace();
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
    return { typed: request.id, guest: true, key, value: "value" in target ? target.value : target.textContent };
  }
  const text = String(request.text ?? "");
  if ("value" in target) target.value = text; else target.textContent = text;
  target.dispatchEvent(new guest.InputEvent("input", { data: text, inputType: "insertText", bubbles: true, composed: true }));
  target.dispatchEvent(new guest.Event("change", { bubbles: true, composed: true }));
  return { typed: request.id, guest: true, value: "value" in target ? target.value : target.textContent };
}

// @ref LLP 1069.001 D4 — a select (and the range and date inputs) is
// controlled: the committed `value` is authoritative, so the element shows
// it again after the options change, and after an action that refused.
const VALUED = new Set(["range", "date", "time", "datetime-local"]);
export const valuedControl = (el) => el instanceof HTMLSelectElement || (el instanceof HTMLInputElement && VALUED.has(el.type));
export function settleValue(el) {
  const c = el instanceof HTMLOptionElement ? el.parentElement : el;
  if (c && valuedControl(c) && c.exactValue !== undefined && c.value !== c.exactValue) c.value = c.exactValue;
}
// D9: `type <id> <value>` sets a control's value as the platform would on a
// choice or a release: HTML's `input`, then `change`. A select takes one of
// its enabled options' values, and nothing else.
export function typeControl(el, request) {
  const text = String(request.text ?? ""), id = request.id;
  if (el.disabled || inertAncestor(el)) return { handled: true, error: `view ${id} is disabled or inert` };
  if (el instanceof HTMLSelectElement && ![...el.options].some((o) => o.value === text && !o.disabled))
    return { handled: true, error: `select ${id} has no enabled option ${JSON.stringify(text)} (options: ${[...el.options].map((o) => JSON.stringify(o.value)).join(", ")})` };
  if (el.type === "range" && !(text.trim() !== "" && Number.isFinite(Number(text)))) return { handled: true, error: `${JSON.stringify(text)} is not a number` };
  el.value = text;
  if (el.value !== text && el instanceof HTMLInputElement && el.type !== "range") return settleValue(el), { handled: true, error: `${JSON.stringify(text)} is not a value an input type=${el.type} takes; it sanitized to ${JSON.stringify(el.value)}` };
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
  return { typed: id, value: el.value, delivery: "recognized", handled: true };
}

// A view's box as the agent reports it. A text folded into its box's content
// (LLP 1007.001, `display: contents`) makes no box of its own: its box is the
// anonymous block its text is, as a style-less block child's was — its line
// boxes along the main axis, and its box's content box across when the box
// stretches its items (a block, or a flex or grid box that stretches).
export function viewBox(el) {
  if (getComputedStyle(el).display !== "contents") return el.getBoundingClientRect();
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
    && ['', 'app:', 'doc:'].includes(grant[1])
    && grant.slice(2).every(component => typeof component === 'string' && component && component !== '.' && component !== '..' && !component.includes('/'));
  if (!nameGrants.has(grant[0]) || grant.length !== 2 || typeof grant[1] !== 'string') return false;
  return grant[0] === 'env-read' || validGrantName(grant[1]);
};

const pathTuple = (kind, target) => {
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
  if (words.length !== 2) return null;
  const [capability, target] = words;
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
