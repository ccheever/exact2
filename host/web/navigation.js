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
      const collections = c.collectionController({ root: o.root, views: o.views, settled: () => arrange.commit(), report(bytes) {
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
  // `use`: the call needs its piece; otherwise it only reconciles what uses made.
  const call = (piece, name, use = true) => (...args) => {
    if (live) return live[piece][name](...args);
    if (!use && !loading) return;
    queue.push([piece, name, args]); start();
  };
  const motion = { style(id, text) { if (live) return live.motion.style(id, text); const el = o.views.get(id); if (el) el.style.cssText = text; } };
  for (const name of ['animate', 'retire', 'heightBinding', 'transformBinding', 'attachSwipe', 'attachHeightDrag', 'attachTransformDrag']) motion[name] = call('motion', name);
  const arrange = { binding: call('arrange', 'binding'), state: call('arrange', 'state') };
  for (const piece of [motion, arrange]) for (const name of ['commit', 'reset', 'destroy']) piece[name] = call(piece === motion ? 'motion' : 'arrange', name, false);
  // Every first batch commits the (empty) collection set: a use only with items.
  const commit = call('collections', 'commit'), reconcile = call('collections', 'commit', false);
  const collections = { commit: items => (items.length ? commit : reconcile)(items) };
  for (const name of ['reset', 'dataReady', 'releaseInteraction']) collections[name] = call('collections', name, false);
  collections.jump = call('collections', 'jump');
  // `preload`: a plan that uses motion (its wasm exports `exact_motion`) needs
  // them before its first spring. `pending`: the load in flight, else null.
  return { collections, motion, arrange, preload: start, pending: () => live || !loading ? null : loading };
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
// `@keyframes` named by content, each inserted once, kept across a restart (LLP 1057 D5).
// The sheet is made at the first rule, not at import: this module is also imported where there is
// no document (tests, workers).
let keyframesSheet = null; const keyframeNames = new Set();
export function keyframes(op) {
  if (keyframeNames.has(op.name)) return;
  keyframeNames.add(op.name);
  keyframesSheet ??= document.head.appendChild(document.createElement("style"));
  keyframesSheet.sheet.insertRule(op.css, keyframesSheet.sheet.cssRules.length);
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
  };
}

// @ref LLP 1061 D4 — the user's display preferences as the page's media
// queries report them: bit 0 `prefers-reduced-motion: reduce`, bit 1
// `prefers-reduced-transparency: reduce` (a browser that does not know the
// feature answers no preference, as CSS does). Told with each boot and resize.
let preferenceQueries;
const queries = () => (preferenceQueries ??= ["(prefers-reduced-motion: reduce)", "(prefers-reduced-transparency: reduce)"].map((q) => matchMedia(q)));
export const preferences = () => queries().reduce((bits, q, i) => bits | (q.matches ? 1 << i : 0), 0);
export const onPreferences = (changed) => queries().forEach((q) => q.addEventListener("change", changed));
