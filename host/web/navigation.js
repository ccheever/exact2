// @ref LLP 1038 D6/D7/D11 — host projection and the last committed URL.
const AGENT_ADMITTED = true; // false in a production bake: host/web/build.mjs rewrites this line (LLP 1069.007 D2)
let last = null, refused = new WeakMap();
// The tabpanels each root last hid or showed (one it stops naming shows again); the keys of the stack it last showed.
let managed = new WeakMap(), shown = new WeakMap();
let written = [], gone = new Set(), cursor = 0, first = null, originIndex = null;
let echo = null, pop = null, draining = false;
const queue = [], waiters = new Set();
let root, navigate, log, hostBack, hostGo, routeMatches = () => false;
const routesIn = node => [...node.children].filter(r => r.hasAttribute("navigationKey"));
// @ref LLP 1075.003 §3.7 — tabs name their kept panels with aria-controls; each panel holds a route stack.
// Attribute and parent walks also work in the server renderer's DOM, which has no selector engine.
const tabsOf = nav => {
  const all = [...(nav?.getElementsByTagName("*") ?? [])];
  const owns = el => { for (let p = el.parentElement; p; p = p.parentElement) if (p.hasAttribute("navigationKey")) return p === nav; return false; };
  for (const list of all.filter(el => el.getAttribute("role") === "tablist" && owns(el))) {
    const panels = [...list.children].filter(tab => tab.getAttribute("role") === "tab" && tab.hasAttribute("aria-controls"))
      .map(tab => all.find(panel => panel.getAttribute("id") === tab.getAttribute("aria-controls")))
      .filter(panel => panel?.getAttribute("role") === "tabpanel" && owns(panel));
    if (panels.length) return [list, panels];
  }
  return [null, []];
};
const panelsOf = nav => tabsOf(nav)[1];
const stacksOf = nav => { const panels = panelsOf(nav); return panels.length ? panels.map(routesIn) : [nav ? routesIn(nav) : []]; };
// @ref LLP 1115.002 D4, LLP 1116.004 D1.2 — the shared list/outer-press rule;
// WIDE is the web's presentation threshold, not a different navigation rule.
const WIDE = "(min-width: 900px)", wide = () => !!globalThis.matchMedia?.(WIDE).matches;
let splitLists = new WeakMap(), rowOf = new WeakMap(), actionRows = new WeakMap(), visits = new Map();
const activationRequired = new WeakSet(), activated = new WeakSet();
const sheet = route => ["modal", "fullscreen"].includes(route?.getAttribute("navigationPresentation"));
let pressedRow = null, pendingRow = null;
const excluded = el => el.hasAttribute("popover") || el.hasAttribute("data-exiting") || getComputedStyle(el).display === "none";
const rowPress = el => el.getAttribute("data-exact-router-press");
const rowHref = el => el.getAttribute("href") || null;
const linked = href => href?.startsWith("/") && !href.startsWith("//") && routeMatches(href);
/** Outermost press owners; unlike candidate discovery, this walk crosses a navigationKey. */
function listRows(list) {
  const rows = [], walk = parent => {
    for (const el of parent.children) {
      if (excluded(el)) continue;
      const press = rowPress(el), href = rowHref(el);
      if (press !== null || href) rows.push({ el, press, href });
      else walk(el);
    }
  };
  walk(list); return rows;
}
function pushingList(route) {
  const retained = splitLists.get(route), stack = [...route.children].reverse();
  let empty = null;
  while (stack.length) {
    const el = stack.pop();
    if (el.hasAttribute("navigationKey") || excluded(el)) continue;
    const style = getComputedStyle(el), sideways = /flex/.test(style.display) && style.flexDirection.startsWith("row")
      || Number(el.getAttribute("data-exact-estimated-item-width")) > 0;
    if (el.hasAttribute("data-exact-list") && !sideways) {
      const rows = listRows(el), found = { list: el, rows };
      if (rows.some(r => r.press === "true" || r.press === null && linked(r.href))) return found;
      if (el === retained && !rows.length) empty = found;
      continue; // An admitted list owns its row scan, including nested lists.
    }
    stack.push(...[...el.children].reverse());
  }
  return empty;
}
/** Recheck authored admission even while narrow. Projection's visibility/inert never opt a list out. */
function splitOf([route]) {
  // A rendered document has route and handler facts but no browser layout.
  if (typeof globalThis.getComputedStyle !== "function" || typeof globalThis.matchMedia !== "function") {
    if (route) splitLists.delete(route);
    return { width: 0, list: null, rows: [] };
  }
  const said = route?.getAttribute("navigationSplit"), explicit = Number(said);
  const found = route && said !== "none" && !sheet(route) ? pushingList(route) : null;
  const width = route && said !== "none" && !sheet(route) && wide() && /^(absolute|fixed)$/.test(getComputedStyle(route).position)
    ? Number.isFinite(explicit) && explicit > 0 ? explicit : found ? 320 : 0 : 0;
  if (route) { if (found && (width || splitLists.has(route))) splitLists.set(route, found.list); else splitLists.delete(route); }
  return { width, list: found?.list, rows: found?.rows ?? [] };
}
function captureRow(event) {
  pressedRow = null;
  if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const route = event.target.closest?.('[data-exact-split="list"]'), list = route && splitLists.get(route);
  if (!list || !root.contains(route)) return;
  let owner = event.target;
  while (owner && owner !== list && rowPress(owner) === null && !rowHref(owner)) owner = owner.parentElement;
  if (owner?.matches("a[href]") && (owner.target && owner.target !== "_self" || owner.hasAttribute("download"))) return;
  if (listRows(list).some(r => r.el === owner && r.press === "true")) {
    const candidate = pressedRow = { row: owner, list, route, from: last?.url, event };
    // Qualification below reads the dispatch itself. This timer only releases the
    // retained Event after the dispatch; it does not extend row ownership.
    setTimeout(() => { if (pressedRow === candidate) pressedRow = null; }, 0);
  }
}
/** Href rows restore from retained visits. Action-only rows remember a router change made inside a normal click dispatch. */
function detailRows(routes, found) {
  for (const route of routes.slice(1)) {
    const id = route.getAttribute("navigationKey"), url = visits.get(id);
    let action = actionRows.get(route);
    if (pendingRow?.id === id && pendingRow.url === url) action = pendingRow;
    const valid = action && action.route === routes[0] && action.list === found.list && action.url === url
      && found.rows.some(r => r.el === action.row && r.press === "true");
    if (valid) actionRows.set(route, action); else actionRows.delete(route);
    const link = found.rows.find(r => r.press === null && linked(r.href) && r.href === url)?.el;
    const row = link ?? (valid ? action.row : null);
    if (row) rowOf.set(route, row); else rowOf.delete(route);
  }
}
// @ref LLP 1116.005 D3 — a route `replace` gives another path (a split's detail, another row's) keeps its id, so its node: it opens at its top, as a pushed one does; a new query keeps its place.
const paths = new WeakMap(), freshPath = route => { const path = last?.url?.split(/[?#]/)[0], was = paths.get(route); paths.set(route, path); if (was !== undefined && was !== path) for (const el of [route, ...route.querySelectorAll("*")]) if (el.scrollTop || el.scrollLeft) el.scrollTo(0, 0); };
/** Whether a root's tablist is a bar whose placement the app left unsaid, read while the root is not yet a sidebar. */
const unsaidBar = (nav, list) => list?.parentElement === nav && getComputedStyle(list).appearance !== "none" && list.getAttribute("aria-orientation") !== "vertical"
  && (([n, l]) => /flex/.test(n.display) && n.flexDirection.startsWith("column") && /^(static|relative)$/.test(l.position) && l.flexDirection.startsWith("row"))([getComputedStyle(nav), getComputedStyle(list)]);
/** Routes and panels excluded from input or document fields. Only input keeps a visible split list live. */
function unselectedWithin(nav, keepList) {
  const key = nav.getAttribute("navigationKey"), panels = panelsOf(nav), stacks = panels.length ? panels.map(routesIn) : [routesIn(nav)];
  const at = stacks.findIndex(routes => routes.some(r => r.getAttribute("navigationKey") === key)), routes = stacks[at] ?? [], top = routes.find(r => r.getAttribute("navigationKey") === key);
  const beside = keepList && routes[0]?.getAttribute("data-exact-split") === "list" && !sheet(top) ? routes[0] : top;
  return at < 0 ? [] : [...panels.filter((_, i) => i !== at), ...routes.filter(r => r !== top && r !== beside)];
}
export function unselected(nav) { return unselectedWithin(nav, true); } export function unselectedForHead(nav) { return unselectedWithin(nav, false); }
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
/** A route's Back control, the selected route's unless named (1035.001 D1: the `id` the root's `navigationBack` names), pressable or not. */
const backControl = (nav, route = selectedRoute(nav), name = nav.getAttribute("navigationBack")) => route && name !== null ? route.querySelector(`[id="${CSS.escape(name)}"]`) : undefined;
// @ref LLP 1116.000 D4 — a pushed route's app bar draws Back, as iOS draws UIKit's system Back button
// (NavigationBarIOS.swift `prepareRoutes`): a route above its stack's root (a tab's root has none), not
// itself a sheet (whose Cancel is its Back), whose `header` does not already hold the root's
// `navigationBack` control, and none when that control is disabled. The button is the header's shadow
// root's, before its slot, so the header's children stay the renderer's. Its words are the control's when
// one is named elsewhere in the route (a symbol alone: the chevron alone), else the route beneath's title
// while it is short, else "Back" where the page is in English (`<html lang>`, LLP 1115.000 D4.14): the bar has that word in no other language, so there it is the chevron alone, named for assistive technology by the title beneath, and an app gives it words by naming a Back control.
const BACK = `<style>button{all:unset;display:inline-flex;align-items:center;gap:2px;flex:none;vertical-align:middle;margin-inline:-6px 6px;padding:4px 8px 4px 4px;border-radius:8px;color:var(--exact-accent,AccentColor);cursor:pointer;white-space:nowrap;-webkit-user-select:none;user-select:none}button[hidden],span[hidden]{display:none}button:focus-visible{outline:2px solid var(--exact-accent,AccentColor);outline-offset:1px}button:active>*{opacity:var(--exact-back-press,.5)}@media (hover:hover){button:hover{background-color:light-dark(#7676801f,#7676803d)}}svg{width:.75em;height:1.25em;fill:none;stroke:currentColor;stroke-width:2.2;stroke-linecap:round;stroke-linejoin:round}button:dir(rtl) svg{scale:-1 1}</style><button part="back" type="button"><svg viewBox="6 3 11 18" aria-hidden="true"><path d="M15 5 8 12l7 7"/></svg><span></span></button><slot></slot>`;
const headerOf = route => [...(route?.children ?? [])].find(child => child.localName === "header");
// @ref LLP 1115.000 D4.1 — a route is a page, so the title follows it: `document.title` is the authored `head`'s title
// (`said`: each target's head writer passes it, null for none), else the selected route's header's heading (what iOS
// puts in the bar), else the app's name. Read again after every projection.
const titleOf = route => headerOf(route)?.querySelector("h1, h2, h3, h4, h5, h6, [role=heading]")?.textContent.trim() ?? "";
let saidTitle = null, appName = null;
export function pageTitle(said = saidTitle) {
  saidTitle = said; appName ??= document.querySelector('meta[property="og:site_name"]')?.content ?? document.title;
  const nav = root?.querySelector("[navigationBack]"), title = said ?? ((nav && titleOf(selectedRoute(nav))) || appName);
  if (document.title !== title) document.title = title;
}
function appBarBack(nav, routes, index, split = false) {
  const route = routes[index], header = headerOf(route);
  if (!header) return;
  let button = header.shadowRoot?.querySelector('button[part="back"]');
  // In a split, a detail its list's row pushed has none: the list beside it is its way back (LLP 1116.004 D1).
  const pushed = index > 0 && !sheet(route) && !(split && (index === 1 || rowOf.has(route))), control = pushed ? backControl(nav, route) : null;
  // Until the page is interactive the glue disables every control itself (`aria-busy`): not the author's.
  if (!pushed || header.contains(control) || (control?.matches(":disabled") && !nav.closest('[aria-busy="true"]'))) {
    if (button) button.hidden = true;
    return;
  }
  if (!button) {
    if (header.shadowRoot || !header.attachShadow) return; // a shadow root of someone else's
    header.attachShadow({ mode: "open" }).innerHTML = BACK;
    button = header.shadowRoot.querySelector("button");
    button.addEventListener("click", event => { event.stopPropagation(); appBack(header.parentElement); });
  }
  const title = control ? control.textContent.trim() : titleOf(routes[index - 1]), english = /^en\b/i.test(document.documentElement.lang || "en");
  const label = control || (title && title.length <= 14) ? title : english ? "Back" : "", name = control ? control.getAttribute("aria-label") || label || "Back" : !english ? title || "Back" : label === "Back" ? label : `Back to ${label}`;
  button.hidden = false; button.lastChild.hidden = !label;
  if (button.lastChild.textContent !== label) button.lastChild.textContent = label;
  if (button.getAttribute("aria-label") !== name) button.setAttribute("aria-label", name);
}
/** The app bar's Back on `route`, pressed: the browser's Back from its route (`popped`), applied at once: the route's control
 * pressed, else the root's `navigate` with the location beneath, else the runner's own `back` (LLP 1115 D5). The
 * history follows the commit as it follows any Back the app takes (`commit`). */
function appBack(route) {
  const nav = route?.parentElement?.closest("[navigationBack]");
  if (!nav || route !== selectedRoute(nav)) return;
  const routes = routesOf(nav), below = routes[routes.indexOf(route) - 1], id = Number(route.getAttribute("navigationKey"));
  if (!below) return;
  if (backControl(nav)) { const why = pressBack(nav); if (why) log(`app bar: Back refused: ${why}`); return; }
  const entry = written[cursor - 1], known = cursor > first && written[cursor]?.id === id && entry?.id === Number(below.getAttribute("navigationKey"));
  if (!known || navigate(entry.url) === false) hostBack?.(id);
}
const closedNone = route => ["modal", "fullscreen"].includes(route?.getAttribute("navigationPresentation")) && route.getAttribute("closedby") === "none";
/** Press the Back control; else why it was not pressed. */
function pressBack(nav) {
  const route = selectedRoute(nav), control = backControl(nav);
  if (!route) return "no route is selected";
  if (closedNone(route)) return `route ${route.getAttribute("navigationKey")} is closedby="none"`;
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
  // A completed pop presses the selected route's Back control; a route with none still goes back by the root's `navigate`
  // with the entry's URL. Unheard, a Back one entry down is the platform's own, as iOS's (NavigationRules.back): the route's
  // control pressed (disabled, or closedby="none", refuses), else the runner's `back` from the visit the entry left (LLP 1115 D5), however routes are keyed. Any other entry unheard (Forward, or more than one away) is the runner's own `go` there (LLP 1115.000 D4.3).
  const step = owned && j === cursor - 1, beneath = step && selected > 0 && routes[selected - 1].getAttribute("navigationKey") === String(entry.id);
  let back = beneath && !!backControl(nav), own = false, why = null;
  pop = {};
  try {
    if (back) why = pressBack(nav);
    else if (navigate(target) === false) {
      own = true; back = step && (!!backControl(nav) || closedNone(selectedRoute(nav)));
      if (back) why = pressBack(nav); else if (step) hostBack?.(written[cursor].id); else hostGo?.(target);
    }
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
      else log(`history: ${own ? `${step ? "Back to" : "navigate"} ${JSON.stringify(target)} refused: the navigation root has no navigate handler (navigate=…), and the runner's own ${step ? `back from visit ${written[cursor].id}` : "go there"} committed no router change` : `${beneath ? `Back to ${JSON.stringify(target)} refused: route ${nav.getAttribute("navigationKey")} has no id="${nav.getAttribute("navigationBack")}" control, and` : `navigate ${JSON.stringify(target)} refused:`} the navigation root's navigate handler (navigate=…) committed no router change, or the root has none`}; restoring the entry`);
      if (j !== null && j !== cursor) go(cursor, j);
      else history.replaceState(written[cursor], "", historyURL(written[cursor].url));
    }
  } finally { pop = null; }
}
function drain() {
  if (draining || echo !== null) return;
  draining = true;
  try { for (let item; queue.length && echo === null;) { item = queue.shift(); if (item.op) commit(item.op); else popped(item); } } finally { draining = false; }
}
const settled = () => { if (echo === null && !queue.length) for (const finish of [...waiters]) finish(); };
export const navigation = {
  /** `dispatch(location)` is false when no `navigate` handler heard it; `back(id)` is the runner's own `back` from visit `id`, `goTo(location)` its own `go`. */
  connect(hostRoot, dispatch, journal, back = null, goTo = null, matchesRoute = () => false, waitForActivation = false) {
    root = hostRoot; navigate = dispatch; log = journal; hostBack = back; hostGo = goTo; routeMatches = matchesRoute;
    if (waitForActivation) { activationRequired.add(hostRoot); activated.delete(hostRoot); }
    else { activationRequired.delete(hostRoot); activated.add(hostRoot); }
    // Capture before either target's link listener commits. Early replay before connect has no action-row association.
    document.addEventListener("click", captureRow, true);
    document.addEventListener("dragstart", event => { if (root.contains(event.target) && event.target.matches?.('img[data-symbol-source], :is(button, a[href], [data-exact-on~="press"]) img')) event.preventDefault(); }); // @ref LLP 1115.000 D4.14 — symbols and images inside pressables do not start picture drags.
    globalThis.matchMedia?.(WIDE).addEventListener?.("change", () => navigation.project(root, log));
    addEventListener("resize", () => navigation.project(root, log)); // Bare plans have no exactViewport callback.
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
  /** Enable projection after a target has adopted its served DOM, then make the first projection. */
  activate(hostRoot = root, journal = log) {
    activated.add(hostRoot);
    navigation.project(hostRoot, journal);
  },
  reset(op = null) {
    refused = new WeakMap(); splitLists = new WeakMap(); rowOf = new WeakMap(); actionRows = new WeakMap();
    pressedRow = pendingRow = null; visits = new Map((op?.visits ?? []).map(([id, url]) => [String(id), url]));
    if (op && written[cursor]?.id === op.top) return;
    last = null;
    written = []; gone = new Set(); cursor = 0; first = null; originIndex = null;
    echo = null; pop = null; queue.length = 0;
    for (const finish of [...waiters]) finish();
  },
  apply(op) {
    visits = new Map((op.visits ?? []).map(([id, url]) => [String(id), url]));
    pendingRow = pressedRow && pressedRow.event.eventPhase !== 0 && op.url !== pressedRow.from
      ? { row: pressedRow.row, list: pressedRow.list, route: pressedRow.route, id: String(op.top), url: op.url } : null;
    pressedRow = null;
    last = op;
    if (pop) { pop.op = op; for (const id of op.removed) gone.add(id); }
    else { queue.push({ op }); drain(); }
  },
  // @ref LLP 1116.006 D4 — the agent's `back` from a route: the browser's Back where the entry beneath is the route beneath, else (a reload left none) the app bar's.
  back(hostRoot) { const nav = hostRoot?.querySelector("[navigationBack]"), routes = nav ? routesOf(nav) : [], route = nav && selectedRoute(nav), at = routes.indexOf(route), way = sheet(route) ? "sheet" : "route", key = nav?.getAttribute("navigationKey"), kept = () => nav.getAttribute("navigationKey") === key ? { refused: "the app kept the route: its Back control is disabled, its closedby is \"none\", or its navigate changed nothing (logs say which)" } : {}; if (at < 1) return { error: "back: nothing to go back from: no dialog is open, and no route is beneath the selected one" }; if (cursor > first && written[cursor]?.id === Number(route.getAttribute("navigationKey")) && written[cursor - 1]?.id === Number(routes[at - 1].getAttribute("navigationKey"))) return navigation.travel(nav, -1).then(r => ({ ...r, back: way, ...kept() })); appBack(route); return { back: way, delivery: "app-bar", ...kept() }; },
  // @ref LLP 1038 D11 — the agent uses the real browser traversal.
  travel(nav, delta) {
    if (nav !== root.querySelector("[navigationBack]")) return { error: "history target is not the navigation root" };
    if (!Number.isInteger(delta) || delta === 0) return { malformed: true, error: "history must be a nonzero integer" };
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
    if (activationRequired.has(root) && !activated.has(root)) return;
    for (const nav of root.querySelectorAll("[navigationBack]")) {
      let key = nav.getAttribute("navigationKey"), [list, panels] = tabsOf(nav);
      nav.removeAttribute("data-exact-sidebar"); // Read authored placement, including changes on retained nodes.
      nav.toggleAttribute("data-exact-sidebar", wide() && unsaidBar(nav, list));
      for (const panel of managed.get(nav) ?? []) {
        if (!panels.includes(panel)) { panel.style.visibility = ""; panel.inert = !!panel.authoredInert; }
      }
      managed.set(nav, panels);
      const stacks = panels.length ? panels.map(routesIn) : [routesIn(nav)];
      const declared = k => stacks.some(routes => routes.some(route => route.getAttribute("navigationKey") === k));
      if (!declared(key)) {
        if (refused.get(nav) !== key) { refused.set(nav, key); log(`navigationKey "${key}" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged`); }
        // @ref LLP 1035.001 D1 — left alone, less the routes that left the tree: the deepest of the stack last shown
        // still declared is selected, so a closed sheet or a popped screen never leaves the route beneath it inert.
        key = (shown.get(nav) ?? []).findLast(declared);
        if (key === undefined) continue;
      } else refused.delete(nav);
      const at = stacks.findIndex(routes => routes.some(route => route.getAttribute("navigationKey") === key));
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
        if (stack === at) shown.set(nav, routes.slice(0, selected + 1).map(route => route.getAttribute("navigationKey")));
        const found = splitOf(routes), split = found.width, live = split && !sheet(routes[selected]);
        const modal = routes[selected]?.getAttribute("navigationPresentation") === "modal";
        detailRows(routes, found);
        if (stack === at && list) list.inert = sheet(routes[selected]) || !!list.authoredInert; // a sheet is modal: the root's tab bar is inert beneath it, as the route it covers is (LLP 1115.000 D4.5)
        // A split's detail shown is the top, or the route a sheet covers; its row is the one whose press pushed it, or pushed a route beneath it in its column.
        const detail = split ? routes.findLastIndex((r, i) => i > 0 && i <= selected && !sheet(r)) : -1;
        for (const [index, route] of routes.entries()) {
          const active = index === selected || (live && index === 0);
          if (!active && route.contains(document.activeElement)) document.activeElement.blur();
          const covered = modal && (index === selected - 1 || (split > 0 && index === 0));
          route.style.visibility = active || covered ? "" : "hidden";
          route.toggleAttribute("data-exact-covered", covered);
          route.inert = !active || !!route.authoredInert;
          const column = !split ? null : index === 0 ? "list" : sheet(route) ? null : "detail";
          if (route.getAttribute("data-exact-split") !== column) column ? route.setAttribute("data-exact-split", column) : route.removeAttribute("data-exact-split");
          route.style.setProperty("--exact-split", column ? `${split}px` : "");
          appBarBack(nav, routes, index, split > 0); if (stack === at && index === selected) freshPath(route);
        }
        let row = null; for (let i = detail; i > 0 && !row; i--) row = rowOf.get(routes[i])?.isConnected ? rowOf.get(routes[i]) : null;
        for (const el of routes[0]?.querySelectorAll("[data-exact-split-row]") ?? []) if (el !== row) el.removeAttribute("data-exact-split-row");
        row?.setAttribute("data-exact-split-row", "");
        // @ref LLP 1116.006 D2 — a split showing no detail fills the detail pane, as an iPad's split view does: the platform's placeholder (the shell draws `data-exact-split-empty`), or the author's, a box of the root route marked `navigationSplit="placeholder"` (which the lowerer hides everywhere else), at the pane's width.
        const empty = split > 0 && detail < 0, own = routes[0]?.querySelector('[navigationSplit="placeholder"]'), box = routes[0]?.parentElement;
        own?.toggleAttribute("data-exact-split-placeholder", empty); box?.toggleAttribute("data-exact-split-empty", empty && !own);
        if (empty) { box.style.setProperty("--exact-split", `${split}px`); own && routes[0].style.setProperty("--exact-split-pane", `${(routes[0].offsetParent ?? box).clientWidth - split}px`); }
      }
    }
    pendingRow = null;
    // The page module's container hatches, when it has them (LLP 1075.003.000 §3.7); the wake lock, which follows what shows (LLP 1125 D11).
    globalThis.exact?.onProject?.(root); globalThis.exact?.wakeLockGlue?.projected(root); pageTitle();
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
      popover: popoverObservation(root),
    };
  },
};
// @ref LLP 1047 D5 — springs, holds and drags (`motion-glue.js`) and virtualized collections
// (`collection-glue.js`) are after-paint pieces, fetched when a batch first needs one. Until they arrive
// their calls wait in order and replay then; before any use, a call with nothing to reconcile is dropped,
// and a style is set at once, as the controller sets one with nothing held. `o.wasm(name, bytes)` is one
// wasm call's reply, or null before the wasm; `o.replayed()` follows a replay, as the end of a batch
// follows its ops.
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
  collections.restyled = () => live?.collections.restyled(); // nothing to restyle before the piece
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
// One scheme allowlist for every URL the page can navigate to: a link's `href` (an authored `link`, an inline run bound
// to data, a Markdown link), an iframe's `src`, and `openURL`. A `javascript:` URL in any of them runs in this page's
// origin. The browser's own parser reads the scheme, with the whitespace and control characters `java\tscript:` hides
// behind.
export function navigableURL(href, base = document.baseURI) {
  try {
    const url = new URL(href, base);
    return ["http:", "https:", "mailto:", "tel:"].includes(url.protocol) ? url.href : null;
  } catch { return null; }
}
export const navigates = (el, name) => name === "href" || (name === "src" && el.localName === "iframe");
// A present sandbox without `allow-same-origin` gives an opaque origin; tokens use ASCII whitespace and ignore case.
export const opaqueSandbox = sandbox => sandbox != null && !sandbox.toLowerCase().split(/[\t\n\f\r ]+/).includes("allow-same-origin");
export function dataURL(src, base = document.baseURI) { try { return src != null && new URL(src, base).protocol === "data:"; } catch { return false; } }
/** Whether an iframe in `sandbox` shows `src`: a `navigableURL`, or a `data:`
 * document in an opaque sandbox, whose script cannot reach this page (LLP 1020 §10). */
export const frameURL = (src, sandbox, base = document.baseURI) => navigableURL(src, base) !== null || (opaqueSandbox(sandbox) && dataURL(src, base));
/** A refused URL is never written: a link loses its `href`, an iframe shows about:blank. */
export function refuseURL(el, name, value) {
  console.warn(`exact: refused ${name} ${JSON.stringify(String(value).slice(0, 80))}: only http, https, mailto and tel navigate${name === "src" ? ", and data: in a sandbox without allow-same-origin" : ""}`);
  if (name === "src") el.setAttribute(name, "about:blank"); else el.removeAttribute(name);
}
// A list item's paragraph (indent > 0, LLP 1045 D4) is a block with the item's indent as `padding-left`; its marker
// (flags 32) is a 40 px box pulled into the gutter by the block's negative `text-indent`, the marker's end at the
// indent (`flex-end`, overflowing leftwards as the browser's outside marker does): `<ul>`/`<ol>`'s layout, the one
// native hosts copy with a head indent. 40 is exact-markdown's `LIST_INDENT`.
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
// @ref LLP 1007 §6 — a page the dev server serves names its current generation, and its first boot is that one, not
// app.wasm's older baked plan (a deep link booted stale on 2026-09-22). dev.js supplies it once asked (glue.js asks
// when the wasm is up); without it, or with none in 5 s, the page boots the baked plan.
export function devFirst() {
  if (!document.querySelector('meta[name="exact-dev-generation"]')) return null;
  const slot = globalThis.exactDevFirst ??= {};
  return new Promise(resolve => {
    const timer = setTimeout(() => resolve(null), 5000);
    const ask = () => { clearTimeout(timer); slot.provide().then(resolve, () => resolve(null)); };
    if (slot.provide) ask(); else slot.ready = ask;
  });
}
// Refusals and malformed request bodies are known while their enclosing batch is still applying. Deliver them on the
// next microtask so their commits cannot re-enter `apply` halfway through that batch; in flight until then, so a wait
// sees what their parse asks for next.
export function deferredFulfill(fulfill, inflight) {
  return (...args) => { const p = Promise.resolve().then(() => fulfill(...args)); inflight.add(p); p.finally(() => inflight.delete(p)); };
}
// An admission refusal answers its ticket Refused (kind 2) with the reason, in its incarnation.
export const refusal = (op, incarnation) => [incarnation, op.ticket, 2, 0, "", new TextEncoder().encode(op.message)];
export function focusController({ready, elements, inert, removed = el => !el.isConnected}) {
  const processed = new WeakSet(), outgoing = new WeakMap();
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
  return {autofocus, keep, restart,
    detached(el, event) { const pending = outgoing.get(event.target); if (!pending || pending.event && pending.event !== event) return false; pending.event = event; const held = pending.path.delete(el); if (!pending.path.size) outgoing.delete(event.target); return held; },
    blur(el, dispatch) {
      const path = []; for (let at = el; at; at = at.parentElement) path.push(at);
      dispatch();
      // Only the subtree detached by this blur hears the following DOM focusout.
      const pending = new Set(); for (const at of path) { if (!removed(at)) break; if (at.exactHandlers?.includes("focusout")) pending.add(at); }
      if (pending.size) outgoing.set(el, { path: pending, event: null });
    }, press(event, el, dispatch) {
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
// Field and focus commands run after the batch's nodes and values commit; their handlers may dispatch an action.
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
    if (name === "removeAllRanges") { getSelection()?.removeAllRanges(); continue; }
    if (name === "setSelectionRange") { setFieldSelection([...root.querySelectorAll("[id]")].find(node => node.id === args?.[0]), args, log); continue; }
    if (name === "setRangeText") { setFieldRangeText([...root.querySelectorAll("[id]")].find(node => node.id === args?.[0]), args, log); continue; }
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
// @ref LLP 1078 D6, D7 — the fold as the browser reports it: the Device Posture API's `navigator.devicePosture.type`
// and the viewport segments `window.viewport.segments` (two or more means a divider splits the viewport; the columns
// are the distinct lefts, the rows the distinct tops). Where the browser lacks the APIs: `continuous`, 1 × 1, which is
// also what Chromium reports on a flat display. Under the agent, `prefer posture` and `prefer segments` go through
// CDP's display-feature and posture overrides (the driver), so the browser's own readings change; where CDP offers
// none, the driver's substitute lands here (`preferFold`) and stands in for them — the facts and `layout.env`, not
// CSS's own `env()` resolution. The substitute lives on `globalThis.exact`: the JS target's agent reads its own copy of
// this module (`agent-navigation.js`), and both copies must agree.
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
/** The fold as `exact_resize`'s facts word carries it beside the preference bits (LLP 1078 D6): bit 8 `folded`, bits 9–16 the columns, 17–24 the rows. Bit 25 is reserved. */
export function foldBits() { const f = fold(); return (f.posture === "folded" ? 256 : 0) | ((f.cols & 255) << 9) | ((f.rows & 255) << 17); }
export function onFold(changed) {
  globalThis.navigator?.devicePosture?.addEventListener?.("change", changed);
  addEventListener("resize", changed);
} /** A context menu's popover, opened for the `contextmenu` event `e` (LLP 1021 §5.1): `find` answers `[popover, source]` when there is still one to open. It opens once the press that asked is over: a Mac's `contextmenu` comes with the button still down, and Firefox and WebKit take the release that follows, outside a popover just opened, for its light dismiss, so the menu never showed there. And it opens at the pointer (LLP 1115.000 D4.7): where the press was is left on the popover (`--exact-context-x`, `-y`) until it closes, for the page's sheet to place it by (neutral.css; the shell's own reads neither). A menu key has no point, and its menu opens at its row. */ export function contextAt(e, find) { const open = () => { const [p, source] = find() ?? []; if (!p) return; if (e.clientX || e.clientY) { p.style.setProperty("--exact-context-x", `${e.clientX}px`); p.style.setProperty("--exact-context-y", `${e.clientY}px`); p.exactAt ??= (p.addEventListener("toggle", t => { if (t.newState === "closed") for (const axis of "xy") p.style.removeProperty(`--exact-context-${axis}`); }), true); } try { p.showPopover({ source }); } catch {} }, ends = ["pointerup", "pointercancel", "mouseup"], end = () => { for (const type of ends) removeEventListener(type, end, true); setTimeout(open); }; if (!e.buttons) return void setTimeout(open); for (const type of ends) addEventListener(type, end, true); }
/** The grid a host without a fold makes for `prefer segments <cols>x<rows> [gap <points>]`: the viewport split evenly, the gap centred on each divider; refused by name. */
export function evenSegments(width, height, cols, rows, gap = 0) {
  if (!(cols >= 1 && rows >= 1)) throw Object.assign(new Error(`segments ${cols}x${rows}: each count is at least 1`), { malformed: true });
  if (!(Number.isFinite(gap) && gap >= 0)) throw Object.assign(new Error(`segments: gap ${gap} is not a non-negative length`), { malformed: true });
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
      case "posture": if (raw !== "folded" && raw !== "continuous") throw Object.assign(new Error(`prefer: posture: ${raw} is folded or continuous`), { malformed: true }); next.posture = raw; break;
      case "cols": case "rows": if (!(Number.isInteger(n) && n >= 1)) throw Object.assign(new Error(`prefer: segments: ${raw} ${name === "cols" ? "columns" : "rows"} is not a count`), { malformed: true }); next[name] = n; grid = true; break;
      case "gap": if (!(Number.isFinite(n) && n >= 0)) throw Object.assign(new Error(`prefer: segments: gap ${raw} is not a length`), { malformed: true }); gap = n; grid = true; break;
      default: throw Object.assign(new Error(`prefer: ${name} is not a fold fact this host sets`), { malformed: true });
    }
  }
  if (grid) { try { next.rects = evenSegments(innerWidth, innerHeight, next.cols, next.rows, gap); } catch (e) { throw Object.assign(new Error(`prefer: ${e.message}`), { malformed: e.malformed }); } }
  (globalThis.exact ??= {}).foldSubstitute = next;
  return foldEnv();
}
// @ref LLP 1061 D4, LLP 1069.000 D1: media facts ride each boot and resize.
// Bits 0–4: motion, transparency, contrast (more|less), dark; 5–7: coarse|none pointer, no hover.
const DISPLAY_MODES = ["standalone", "browser", "fullscreen", "minimal-ui", "window-controls-overlay", "picture-in-picture", "tabbed"];
let preferenceQueries;
// Bits 8–25 belong to foldBits(); 26–27 gamut, 28 HDR (LLP 1100 D9), 29–31 display-mode (LLP 1039 D7).
const queries = () => (preferenceQueries ??= [["(prefers-reduced-motion: reduce)", 1], ["(prefers-reduced-transparency: reduce)", 2], ["(prefers-contrast: more)", 4], ["(prefers-contrast: less)", 8], ["(prefers-contrast: custom)", 12], ["(prefers-color-scheme: dark)", 16], ["(pointer: coarse)", 32], ["(pointer: none)", 64], ["(hover: none)", 128], ["(color-gamut: p3)", 1 << 26], ["(color-gamut: rec2020)", 1 << 27], ["(dynamic-range: high)", 1 << 28], ...DISPLAY_MODES.map((v, i) => [`(display-mode: ${v})`, i << 29])].map(([q, bits]) => [matchMedia(q), bits]));
export const preferences = () => { const bits = queries().reduce((bits, [q, bit]) => bits | (q.matches ? bit : 0), 0), pinned = globalThis.exact?.systemScheme; return pinned ? (bits & ~16) | (pinned === "dark" ? 16 : 0) : bits; }; // a dev pin's scheme is the system's (LLP 1122 D4; dev.js)
export const displayMode = () => DISPLAY_MODES[(preferences() >>> 29) & 7];
export const onPreferences = (changed) => queries().forEach(([q]) => q.addEventListener("change", changed));
// @ref LLP 1069.000 D2 — the page's facts as `exact_set_page` takes them: bit 0 `document.visibilityState == "hidden"`, bit 1
// `!navigator.onLine`, bit 2 `typeof navigator.share === "function"` (LLP 1069.003 D5), bit 3 `typeof showOpenFilePicker ===
// "function"` (LLP 1069.010 D2; studio diary R31), bit 4 `!document.hasFocus()` (#114), told again at the window's `focus` and `blur`,
// bits 5–6 Navigation Timing's `type` (runner/src/page.rs `NAVIGATION_TYPES`; a prerender's is `navigate`). Under the agent the drive's values stand in (visible, online, a share sheet, the pickers, focus, a `navigate`: LLP 1069.000 D6), set by `prefer`'s `page` group; the machine is never read, but for the display's scale (below).
export function pageReporter(agent, platform = globalThis) {
  const facts = { "visibility-state": "visible", online: true, "can-share": true, "can-open-files": true, "has-focus": true, "root-font-size": 16 };
  // @ref LLP 1069.000 D3 — the root font size: the document element's computed `font-size`, the browser's setting
  // unless a page sets it; under the agent the drive sets it on the element (`prefer root-font-size`). The app's own
  // size (`appRootFontSize`) is set over it and read past.
  const rootFontSize = () => agent ? facts["root-font-size"] : beneathApp(platform, () => parseFloat(platform.getComputedStyle(platform.document.documentElement).fontSize) || 16), displayScale = () => platform.devicePixelRatio > 0 ? platform.devicePixelRatio : 1; // @ref LLP 1115.000 D10.3 — the display's scale is the page's own `devicePixelRatio`, read under the agent too: the drive sets it through the browser's emulation (`prefer display-scale`), never by a substitute, and a zoom fires `resize`, which tells it again
  const loaded = agent ? "" : platform.performance?.getEntriesByType?.("navigation")?.[0]?.type;
  const navigationType = ["reload", "back_forward"].includes(loaded) ? loaded : "navigate";
  const read = () => agent ? { ...facts, "navigation-type": navigationType } : { "visibility-state": platform.document.visibilityState === "hidden" ? "hidden" : "visible", online: platform.navigator.onLine !== false, "can-share": typeof platform.navigator.share === "function", "can-open-files": typeof platform.showOpenFilePicker === "function", "has-focus": typeof platform.document.hasFocus !== "function" || platform.document.hasFocus(), "navigation-type": navigationType };
  const bits = () => { const f = read(); return (f["visibility-state"] === "hidden" ? 1 : 0) | (f.online ? 0 : 2) | (f["can-share"] ? 4 : 0) | (f["can-open-files"] ? 8 : 0) | (f["has-focus"] ? 0 : 16) | (["navigate", "reload", "back_forward"].indexOf(f["navigation-type"]) + 1) << 5; };
  const prefer = (page) => {
    const next = { ...facts };
    for (const [name, raw] of Object.entries(page ?? {})) {
      const value = String(raw);
      if (name === "visibility-state" && (value === "visible" || value === "hidden")) next[name] = value;
      else if ((name === "online" || name === "can-share" || name === "can-open-files" || name === "has-focus") && (value === "true" || value === "false")) next[name] = value === "true";
      else if (name === "root-font-size" && Number(value) > 0 && Number.isFinite(Number(value))) next[name] = Number(value); else if (name === "display-scale" && Number(value) > 0 && Number.isFinite(Number(value))) { if (Math.abs(displayScale() - Number(value)) > 0.01) throw new Error(`prefer: display-scale: ${value}: the page's is ${displayScale()}, which the browser sets (its own emulation, or the device)`); }
      else throw Object.assign(new Error(`prefer: ${name}: ${value} is not a page fact this host sets`), name === "keyboard-navigation" ? {} : { malformed: true }); // keyboard-navigation is a real page fact this host does not set: a refusal
    }
    Object.assign(facts, next);
    if (page?.["root-font-size"] !== undefined) { const { style } = platform.document.documentElement; style.fontSize = `${facts["root-font-size"]}px`; style.setProperty("--exact-host-font-size", style.fontSize); } // the size an app's percentage is of
  };
  const onChange = (changed) => { if (agent) return; platform.document.addEventListener("visibilitychange", changed); for (const name of ["online", "offline", "focus", "blur"]) platform.addEventListener(name, changed); };
  return { bits, read: () => ({ ...read(), "root-font-size": rootFontSize(), "display-scale": displayScale() }), prefer, onChange, rootFontSize, displayScale };
}
// @ref LLP 1069.000 D3, LLP 1121 D6 — the app's `setRootFontSize(px)`: `:root { font-size: <px> !important }`, an author rule over the root element's own size (the browser's setting, or the agent's
// `prefer root-font-size`), which can change beneath it; a percentage is a share of that size which follows it (CSS's `125%` of `medium`, or of the agent's substitute); `"medium"` removes the rule. The wasm runner checked the value; the JS target checks it here, by the runner's grammar (exact_num's `css_percent`) and its share, `host * ratio` as an f32 above 0.
const APP_ROOT = "exact-root-font-size", PERCENT = /^[+-]?(\d+|\d*\.\d+)(e[+-]?\d+)?%$/i;
export function appRootFontSize(value, say, platform = globalThis) {
  const doc = platform.document, host = platform.getComputedStyle ? beneathApp(platform, () => parseFloat(platform.getComputedStyle(doc.documentElement).fontSize) || 16) : 16;
  let rule = doc.getElementById(APP_ROOT);
  if (value === "medium") return void rule?.remove();
  const ratio = typeof value === "string" && PERCENT.test(value) ? parseFloat(value) / 100 : NaN, share = Math.fround(host * ratio), px = typeof value === "number" && Math.fround(value) > 0 && Number.isFinite(Math.fround(value));
  if (!px && !(share > 0 && Number.isFinite(share))) return say?.(`setRootFontSize(${JSON.stringify(value) ?? ""}) refused: the root font size is a number of px above 0, a percentage of the host's above 0 ("125%"), or "medium"`);
  if (!rule) { rule = doc.createElement("style"); rule.id = APP_ROOT; doc.head.append(rule); }
  rule.textContent = px ? `:root{font-size:${value}px!important}` : `:root{font-size:calc(var(--exact-host-font-size, 100%) * ${ratio})!important}`;
}
// @ref LLP 1116.001 D1.4 — `-exact-fit: shrink`. The browser lays the page out, so it picks the scale the kernel would,
// by the kernel's rule (kernel/src/ layout/fit.rs): the text's min-content width against its room, down to half its
// size (display text bigger than twice the root's: down to the root size). It draws at it as CSS `zoom` on the element,
// which scales the text, its runs, line height and letter spacing together and leaves the authored rows alone; a style
// write clears it and the next pass sets it again.
export function fitText(el) {
  el.style.zoom = "";
  const cs = getComputedStyle(el), px = (v) => parseFloat(v) || 0;
  if (cs.getPropertyValue("--exact-fit").trim() !== "shrink" || cs.display === "none" || !el.offsetWidth) return;
  // Layout pixels, through any transform above it (a route sliding in).
  const k = el.getBoundingClientRect().width / el.offsetWidth || 1;
  const inset = px(cs.paddingLeft) + px(cs.paddingRight) + px(cs.borderLeftWidth) + px(cs.borderRightWidth);
  const content = () => el.getBoundingClientRect().width / k - inset, box = content(), width = el.style.width;
  el.style.width = "min-content"; const natural = content(); el.style.width = width;
  // Sized by its text, it has its container's room.
  const up = el.parentElement, ps = up && getComputedStyle(up);
  const room = box + 0.5 < natural || !up ? box : up.clientWidth - px(ps.paddingLeft) - px(ps.paddingRight) - Math.max(0, px(cs.marginLeft)) - Math.max(0, px(cs.marginRight));
  if (!(natural > room + 0.01)) return;
  const size = px(cs.fontSize), root = px(getComputedStyle(document.documentElement).fontSize);
  el.style.zoom = String(Math.max(size > 0 && root > 0 ? Math.min(0.5, root / size) : 0.5, Math.max(0, room) / natural));
  zoomed.add(el);
}
// What `fitText` scaled: one whose row went (a bound `-exact-fit`) is let go.
let fitSeen = false, zoomed = new Set();
/** Whether a style the page writes fits a text: then every pass refits. */
export const noteFit = (css) => { if (!fitSeen && css?.includes("--exact-fit")) fitSeen = true; };
/** Every fitted text again: after a tree update, a resize, the fonts' arrival. */
export function refitTexts() {
  if (!fitSeen) return;
  const fitted = new Set(document.querySelectorAll('[style*="--exact-fit"]'));
  for (const el of zoomed) if (!fitted.has(el)) { el.style.zoom = ""; zoomed.delete(el); }
  for (const el of fitted) fitText(el);
}
/** The JS target's: after each tree update (rt.js `After`), on resize and once the fonts are in. */
export function installFits(after) {
  if (fitSeen || typeof document === "undefined" || typeof addEventListener !== "function" || globalThis.__exactRender) return;
  fitSeen = true; after.push(refitTexts); addEventListener("resize", refitTexts); document.fonts?.ready.then(refitTexts);
}
// What `read` gives with the app's rule set aside: the host's own reading.
function beneathApp(platform, read) {
  const rule = platform.document.getElementById?.(APP_ROOT);
  if (!rule) return read();
  rule.disabled = true;
  try { return read(); } finally { rule.disabled = false; }
}
// The page launch owns its seed, a whole number below 2^32 on every host (LLP 1116.005 D5); a new runner during
// development reuses it. Agent facts are supplied by the drive, never by the browser's environment.
export function placeReporter(params, platform = globalThis) {
  let seed;
  return () => {
    if (AGENT_ADMITTED && params.has('agent')) {
      const value = Number(params.get('seed') ?? 1);
      if (!Number.isSafeInteger(value) || value < 0 || value >= 2 ** 32) throw new Error('seed: a whole number from 0 through 2^32 - 1 (4294967295)');
      return [params.get('locale') ?? 'en-US', params.get('timeZone') ?? 'UTC', value].join('\0');
    }
    if (seed === undefined) seed = platform.crypto.getRandomValues(new Uint32Array(1))[0];
    return [platform.navigator.language, platform.Intl.DateTimeFormat().resolvedOptions().timeZone, seed].join('\0');
  };
}
// @ref LLP 1027.000.000 D3 — under the agent the date at the clock's zero is the drive's epoch (default
// 2026-01-01T00:00:00Z), and the offset its zone's at the virtual instant `elapsed` names; `clock +N` moves the date
// because `now()` moves, and the glue tells the offset again after each clock move, so crossing a DST change re-answers
// it (LLP 1069.007 D2).
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
// Where the page launches (LLP 1038 D5): its path and query, less a drive's own parameters, which are the host's facts
// and not a route's — a driven page numbers its visits as a native host does (feed F16). The JS target launches here
// too (rt.js re-exports it).
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
export const reportPlace = () => (pagePlace ??= placeReporter(launched()))();
// A store write in the page (LLP 1018 D6, LLP 1116 D5, LLP 1027 D4): a secret or state the quota refuses sets the kept answers aside, a cache the runner bounds together, and is tried once more, the kept answers put back if it fails again; a kept answer it refuses leaves the older one, which the next launch may still match, and is said.
export function storeInPage(storage, name, value, warn = console.warn) { const key = name.startsWith("exact.state.") ? name : "exact.secret." + name, write = () => value == null ? storage.removeItem(key) : storage.setItem(key, value);
  try { try { write(); } catch (e) { if (name.startsWith("exact.kept.") || !/quota/i.test(e?.name ?? "")) throw e; const aside = Array.from({ length: storage.length }, (_, i) => storage.key(i)).filter(k => k?.startsWith("exact.secret.exact.kept.")).map(k => [k, storage.getItem(k)]); for (const [k] of aside) storage.removeItem(k); try { write(); } catch (again) { for (const [k, v] of aside) try { storage.setItem(k, v); } catch {} throw again; } } } catch (e) { warn("exact: store", name, String(e)); } }

// An iframe guest's origin as authored when its `src` or `sandbox` was
// committed (an opaque sandbox posts as "null"), which a `message` from it must match.
const iframeOrigins = new WeakMap(); // iframe -> authored/committed guest origin
export function commitGuestOrigin(el) {
  const opaque = opaqueSandbox(el.getAttribute("sandbox"));
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

// @ref LLP 1069.001 D4 (amended 2026-10-04) — a select, range or date is controlled as a text field is: the committed
// `value` is written when it changes, and shown again after the options change or a refused `type`; after the action
// that heard the person (`acted`) it keeps their choice, as the web build's does (x2apps kanban2 #5: a date cleared
// while a `send` was in flight).
const VALUED = new Set(["range", "date", "time", "datetime-local"]);
export const valuedControl = (el) => el instanceof HTMLSelectElement || (el instanceof HTMLInputElement && VALUED.has(el.type));
export function settleValue(el, acted = false) {
  const c = el instanceof HTMLOptionElement ? el.parentElement : el;
  if (acted) return;
  if (c && valuedControl(c) && c.exactValue !== undefined && c.value !== c.exactValue) c.value = c.exactValue;
}
// What `type <id> <value>` sets rather than types into (D9): the valued controls, a checkbox (or `switch`), which a
// value turns on or off, and a radio, which `true` checks (x2apps survey #2).
export const typedControl = (el) => valuedControl(el) || (el instanceof HTMLInputElement && (el.type === "checkbox" || el.type === "radio"));
// A text field (x2apps codeedit #2): an input that is no control, or a textarea that is not the Markdown editor; its `input` and `change` carry its selection, and its `select` is HTML's.
export const textField = (el) => el instanceof HTMLTextAreaElement ? el.getAttribute("markup") !== "markdown" : el instanceof HTMLInputElement && !typedControl(el) && el.type !== "file";
export const editMenu = (target) => { const f = target.closest("input,textarea,[contenteditable]"); return !!f && (!(f instanceof HTMLInputElement) || textField(f)); }; // a `contextmenu` from a text field, a textarea or an editable region opens the browser's edit menu and runs no handler (review b5-b 3); a control's input (a checkbox, switch, radio, range, date, time or file) has no such menu, so its node's handler runs and its popover opens, as Chrome sends the event and macOS runs it (#487)
// Host kinds 40–42 carry UTF-16 start,end,direction before the field’s text.
export const fieldSelection = (el) => { const end = el.value.length; return `${el.selectionStart ?? end},${el.selectionEnd ?? end},${el.selectionDirection ?? "none"},`; };
// A radio's group as committed again (x2apps survey #2): the browser checked the radio and unchecked its group at once;
// the action decides, and one that refuses snaps the group back. `bound` reads a radio's committed `checked` (undefined
// when it has none); the unchecks go first, so the radio left checked is the committed one.
export function settleRadios(el, bound) {
  const group = el.name ? [...document.querySelectorAll('input[type="radio"]')].filter((r) => r.name === el.name) : [el];
  for (const r of group) if (bound(r) === false && r.checked) r.checked = false;
  for (const r of group) if (bound(r) === true && !r.checked) r.checked = true;
}
export function setFieldSelection(el, args, log) {
  const [id, start, end, direction] = args ?? [];
  if (!el) return log(`setSelectionRange "${id}" refused: no live node with that id`);
  if (el.matches("[data-exact-text], [data-exact-text] *")) {
    const size = textSource(el).length, clamp = n => Math.min(size, Number(n) >>> 0), b = clamp(end), a = Math.min(clamp(start), b);
    const [an, ao] = textPosition(el, a), [bn, bo] = textPosition(el, b);
    getSelection().setBaseAndExtent(...(direction === "backward" ? [bn, bo, an, ao] : [an, ao, bn, bo])); return;
  }
  if (typeof el.setSelectionRange !== "function") return log(`setSelectionRange "${id}" refused: not a text field or text`);
  try { el.setSelectionRange(start, end, direction); } catch { log(`setSelectionRange "${id}" refused: an input type=${el.type} has no text selection`); }
}
// @ref LLP 1069.001, 2026-10-12 — HTML's range edit, plus one input to synchronize the app's value; no undo step.
export function setFieldRangeText(el, args, log) {
  const [id, text, ...range] = args;
  if (!el) return log(`setRangeText "${id}" refused: no live node with that id`);
  if (typeof el.setRangeText !== "function" || el.exactMarkup || el.getAttribute("markup") === "markdown") return log(`setRangeText "${id}" refused: not a plain text field`);
  try { el.setRangeText(text, ...range); }
  catch (error) { return log(`setRangeText "${id}" refused: ${error.name}`); }
  el.dispatchEvent(new InputEvent("input", { inputType: "insertReplacementText", data: text, bubbles: true }));
}
// Edit data is length-prefixed in UTF-8; commas and newlines are valid text.
function inputPayload(el, e) { return `${e.inputType ?? ""},${+e.cancelable},${+!!e.isComposing},${e.data == null ? -1 : new TextEncoder().encode(e.data).length},${e.data ?? ""}${fieldSelection(el)}${el.value}`; }
export function controlEvent(el, kind, on, dispatch) {
  if (kind === "select" || kind === "selectionchange") return void (textField(el) && on(kind, () => dispatch(kind === "select" ? 42 : 52, fieldSelection(el) + el.value)));
  if (kind === "beforeinput") return on(kind, e => dispatch(53, inputPayload(el, e), e));
  if (el.type === "checkbox") return on(kind, () => { dispatch(kind === "change" ? 24 : 25, String(el.checked)); if (el.exactChecked !== undefined && el.checked !== el.exactChecked) el.checked = el.exactChecked; });
  if (el.type === "radio") return on(kind, () => { dispatch(kind === "change" ? 1 : 23, el.value); settleRadios(el, (r) => r.exactChecked); });
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
    dispatch(textField(el) ? 54 : 23, textField(el) ? inputPayload(el, e) : value); settleValue(el, true);
  });
}
// D9: `type <id> <value>` sets a control's value as the platform would on a choice or a release: HTML's `input`, then
// `change`. A select takes one of its enabled options by value, else by its one label (Playwright's `selectOption`, the
// diaries' kanban F17 and shop F10); a checkbox takes `true` or `false` and is clicked when that differs, as a person
// would.
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
const exactOnly = source => /^(#|surface\.read |surface\.write |device\.|runtime\.|auth\.)/.test(source);
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
// The Rust parser is the only producer. Admission validates an incoming JSON value in full before making its authority
// immutable; a prior validation is never a reason to trust a mutable or branded object.
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
// Module metadata carries the author's spelling while the bake carries the Rust parser's normalized set. Blank lines
// and indentation have no bearing on activation; the normalized declarations do.
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
// Text selections use source UTF-16 offsets; a Markdown <br> represents one newline.
const selectedTexts = new Map();
function textSource(e) { return e.nodeType === Node.TEXT_NODE ? e.data : e.nodeName === 'BR' ? "\n" : [...e.childNodes].map(textSource).join(''); }
function textPosition(e, offset) {
  const walker = document.createTreeWalker(e, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) { if (offset <= n.length) return [n, offset]; offset -= n.length; }
    else if (n.nodeName === 'BR') { const i = [...n.parentNode.childNodes].indexOf(n); if (offset <= 1) return [n.parentNode, i + offset]; offset--; }
  }
  return [e, e.childNodes.length];
}
function selectionRects(e, a, b) {
  if (a === b) return [];
  const range = document.createRange(); range.setStart(...textPosition(e, a)); range.setEnd(...textPosition(e, b));
  return [...range.getClientRects()].map(r => [r.x, r.y, r.width, r.height, false, false]);
}
export function onSelection(e, report) {
  if (!selectedTexts.size) document.addEventListener("selectionchange", () => {
    const s = getSelection(), r = s.rangeCount && !s.isCollapsed ? s.getRangeAt(0) : null;
    for (const [e, h] of selectedTexts) {
      if (!e.isConnected) { selectedTexts.delete(e); continue; }
      const at = (n, o) => { const p = document.createRange(); p.selectNodeContents(e); const c = p.comparePoint(n, o); if (!c) p.setEnd(n, o); return c < 0 ? 0 : textSource(p.cloneContents()).length; };
      let a = 0, b = 0;
      if (r?.intersectsNode(e)) { a = at(r.startContainer, r.startOffset); b = at(r.endContainer, r.endOffset); }
      if (a === b) a = b = 0;
      if (h.a !== a || h.b !== b) { h.a = a; h.b = b; h.report(textSource(e).slice(a, b), a, b, selectionRects(e, a, b)); }
    }
  });
  selectedTexts.set(e, { report, a: 0, b: 0 });
}
// HTML's methods and source, shared by both web targets (LLP 1021 §4).
export function popoverCommand(method, id, source, say) {
  const pop = document.getElementById(id), anchor = source == null ? null : document.getElementById(source);
  if (!pop?.hasAttribute("popover")) return say(`${method} "${id}" refused: no popover has that id`);
  if (source != null && !anchor) return say(`${method} "${id}" refused: no source has id "${source}"`);
  try { pop[method](method === "hidePopover" || !anchor ? undefined : { source: anchor }); }
  catch (e) { say(`${method} "${id}" refused: ${e.name}`); }
}
const popoverSources = new WeakMap();
globalThis.document?.addEventListener?.("beforetoggle", e => {
  if (e.target.hasAttribute("popover") && e.newState === "open") popoverSources.set(e.target, e.source ?? null);
}, true);
function popoverObservation(root) {
  const pop = [...root.querySelectorAll("[popover]:popover-open")].at(-1), source = pop && popoverSources.get(pop);
  const id = el => globalThis.exact?.viewId?.(el) ?? Number(el.dataset.view);
  return pop ? { phase: "open", popover: id(pop), source: source ? id(source) : null } : null;
}
