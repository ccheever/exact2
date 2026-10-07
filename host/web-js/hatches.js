// A node marked `hatch="word"` on the JS target (@ref LLP 1075.003.000 §3.2).
// The app's page module (`modules/web/index.js`) may export
//
//   export function element(e)       // after the commit that mounts the node,
//                                    // again when its data-* words change
//   export function elementEnded(e)  // before the node leaves
//
// with `e` = { hatch, data, element, isNew, isLive, click(), focus(), blur() }:
// `element` is the node's own element and `data` its `dataset`, the web's own
// (so the host's words, `data-testid` and the rest, are there too). On the web
// a hatched node gives up nothing but the call. The page module loads after
// first paint, as a module view's does (rt.js `painted`), and a node mounted before
// then is told once it has. rt.js re-exports `ht`: only a plan that marks a
// node bundles this.
import { onEnd, journal, clock, viewId, inflight, painted } from "./rt.js";

let Page = null;
const said = new Set(), warned = new Set();
const say = line => journal.push(`t=${clock.now} hatch ${line}`);
const page = () => Page ??= painted().then(() => import("./native.js")).then(m => m.pageTable());
// `state.hatches` (the agent), as Apple's: per word, its live nodes, and its
// calls by moment. A row's calls after the first of each moment are counted,
// not journaled, so a fling does not flood the journal.
const stats = Object.create(null);
const counted = word => stats[word] ??= { live: 0, reusable: 0, lost: [], calls: Object.create(null) };
const publish = () => { if (globalThis.exact) globalThis.exact.hatchStats ??= stats; };

/** The page module's `element` hatch for `e`, and its end. */
export function ht(e) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  publish();
  const word = e.getAttribute("data-hatch"), id = viewId(e);
  // What a hatch asks of an element runs after the effect or commit that
  // called the hatch (a microtask), never inside it, as on Apple.
  const later = act => queueMicrotask(() => { if (h.isLive) h.element?.[act](); });
  const h = {
    hatch: word, element: e, data: e.dataset, isNew: true, isLive: true,
    click() { if (h.isLive) later("click"); }, focus() { if (h.isLive) later("focus"); }, blur() { if (h.isLive) later("blur"); },
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
    publish();
    counted(word).live++;
    if (!said.has(word)) { said.add(word); say(`element ${word}: nothing beyond the call on this host (LLP 1075.003.000)`); }
    const row = e.closest("[data-listitemkey]");
    inRow = !!row;
    if (row && !warned.has(word)) {
      warned.add(word);
      let list = row.parentElement;
      while (list && !list.$list) list = list.parentElement;
      say(`element ${word} is in a row of list ${list?.dataset.testid ?? "#" + (list ? viewId(list) : "?")}: each row's mount calls its hatch on the main thread`);
    }
    // A change of its words (dataset.js) is heard after the effect that made
    // it, once however many words changed: what the hatch does in answer (a
    // click) is then an ordinary update, not one inside the running effect.
    // Installed before `built`, so what `built` itself causes is heard too.
    let due = false;
    e.$ht = () => {
      if (due) return;
      due = true;
      queueMicrotask(() => { due = false; if (h.isLive) { h.isNew = false; call("element", "changed"); } });
    };
    call("element", "built");
  }, error => say(`element ${word} #${id}: no page module: ${error?.message ?? error}`))
    .finally(() => setTimeout(() => inflight.n--));
  onEnd(() => {
    delete e.$ht;
    const told = module && h.isLive;
    // As on Apple: the handle is no longer live in `elementEnded` (its
    // `click()` does nothing), and keeps no element after it, since a list
    // may give the element to another row.
    h.isLive = false;
    if (told) { counted(word).live--; call("elementEnded", "ended"); }
    h.data = { ...e.dataset };
    h.element = null;
  });
}

// The page module's container hatches (LLP 1075.003.000 §3.7), loaded after
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
