// A node marked `hook="word"` on the JS target (@ref LLP 1075.003.000 §3.2).
// The app's page module (`modules/web/index.js`) may export
//
//   export function element(e)       // after the commit that mounts the node,
//                                    // again when its data-* words change
//   export function elementEnded(e)  // before the node leaves
//
// with `e` = { hook, data, element, isNew, isLive, click(), focus(), blur() }:
// `element` is the node's own element and `data` its `dataset`, the web's own
// (so the host's words, `data-testid` and the rest, are there too). On the web
// a hooked node gives up nothing but the call. The page module loads after
// first paint, as a module view's does (rt.js `painted`), and a node mounted before
// then is told once it has. rt.js re-exports `hk`: only a plan that marks a
// node bundles this.
import { onEnd, journal, clock, viewId, inflight, painted } from "./rt.js";

let Page = null;
const said = new Set(), warned = new Set();
const say = line => journal.push(`t=${clock.now} hook ${line}`);
const page = () => Page ??= painted().then(() => import("./native.js")).then(m => m.pageTable());
// `state.hooks` (the agent), as Apple's: per word, its live nodes, and its
// calls by moment. A row's calls after the first of each moment are counted,
// not journaled, so a fling does not flood the journal.
const stats = {};
const counted = word => stats[word] ??= { live: 0, reusable: 0, lost: [], calls: {} };

/** The page module's `element` hook for `e`, and its end. */
export function hk(e) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  if (globalThis.exact) globalThis.exact.hookStats ??= stats;
  const word = e.getAttribute("data-hook"), id = viewId(e);
  const h = {
    hook: word, element: e, data: e.dataset, isNew: true, isLive: true,
    click() { if (h.isLive) e.click(); }, focus() { if (h.isLive) e.focus(); }, blur() { if (h.isLive) e.blur(); },
  };
  let module = null, inRow = false;
  const call = (name, moment) => {
    if (typeof module?.[name] !== "function") return;
    const n = counted(word).calls[moment] = (counted(word).calls[moment] ?? 0) + 1;
    if (!inRow || n === 1) say(`element ${word} #${id}: ${moment}`);
    try { module[name](h); } catch (error) { say(`element ${word} #${id}: ${name} threw ${error?.message ?? error}`); }
  };
  inflight.n++;
  page().then(m => {
    if (!h.isLive) return;
    module = m;
    counted(word).live++;
    if (!said.has(word)) { said.add(word); say(`element ${word}: nothing beyond the call on this host (LLP 1075.003.000)`); }
    const row = e.closest("[data-listitemkey]");
    inRow = !!row;
    if (row && !warned.has(word)) {
      warned.add(word);
      let list = row.parentElement;
      while (list && !list.$list) list = list.parentElement;
      say(`element ${word} is in a row of list ${list?.dataset.testid ?? "#" + (list ? viewId(list) : "?")}: each row's mount calls its hook on the main thread`);
    }
    // A change of its words (dataset.js) is heard after the effect that made
    // it, once however many words changed: what the hook does in answer (a
    // click) is then an ordinary update, not one inside the running effect.
    // Installed before `built`, so what `built` itself causes is heard too.
    let due = false;
    e.$hk = () => {
      if (due) return;
      due = true;
      queueMicrotask(() => { due = false; if (h.isLive) { h.isNew = false; call("element", "changed"); } });
    };
    call("element", "built");
  }, error => say(`element ${word} #${id}: no page module: ${error?.message ?? error}`))
    .finally(() => setTimeout(() => inflight.n--));
  onEnd(() => {
    delete e.$hk;
    const told = module && h.isLive;
    // As on Apple: the handle is no longer live in `elementEnded` (its
    // `click()` does nothing), and keeps no element after it, since a list
    // may give the element to another row.
    h.isLive = false;
    if (told) { counted(word).live--; call("elementEnded", "ended"); }
    h.element = null;
  });
}

// The page module's container hooks (LLP 1075.003.000 §3.7), loaded after
// first paint only for a page module that exports one (web-js/build.mjs):
//
//   export function navigation(e)  // a navigation root's element, once
//   export function route(r)       // a route's element, when it mounts:
//                                  // { key, data, element, navigation, isNew, isLive }
//   export function routeEnded(r)  // the route's element is leaving
//   export function tabs(e)        // the root's tablist's element, once
//
// Each runs as `navigation.js` projects a router change.
const roots = new WeakSet(), lists = new WeakSet(), routes = new Map();
export function containers() {
  return page().then(m => {
    const project = root => {
      if (!root) return;
      const live = new Set();
      const call = (name, what, arg) => {
        if (typeof m[name] !== "function") return;
        say(`${what}`);
        try { m[name](arg); } catch (error) { say(`${name} threw ${error?.message ?? error}`); }
      };
      for (const nav of root.querySelectorAll("[navigationBack]")) {
        if (!roots.has(nav)) { roots.add(nav); call("navigation", `navigation #${viewId(nav)}: built`, nav); }
        for (const list of nav.querySelectorAll('[role="tablist"]')) {
          if (lists.has(list) || list.parentElement?.closest("[navigationKey]") !== nav) continue;
          lists.add(list);
          call("tabs", `tabs: built`, list);
        }
        for (const element of nav.querySelectorAll("[navigationKey]")) {
          // A route is its nearest root's; a nested root is not a route; one
          // leaving (its exit animation playing) has ended.
          if (element.hasAttribute("navigationBack") || element.parentElement?.closest("[navigationBack]") !== nav
            || element.closest("[data-exiting]")) continue;
          live.add(element);
          if (routes.has(element)) continue;
          const r = { key: element.getAttribute("navigationKey"), data: element.dataset, element, navigation: nav, isNew: true, isLive: true };
          routes.set(element, r);
          call("route", `route ${r.key}: built`, r);
        }
      }
      for (const [element, r] of routes) {
        if (live.has(element) && element.isConnected) continue;
        routes.delete(element);
        call("routeEnded", `route ${r.key}: ended`, r);
        r.isLive = false;
      }
    };
    globalThis.exact.onProject = project;
    project(document.getElementById("exact-root"));
  }, error => say(`no page module: ${error?.message ?? error}`));
}
