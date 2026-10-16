// What a view says about its page's document (LLP 1048.003), for rt.js: its
// head (D1) and the element that scrolls it (D4); and which routes of its
// navigation roots it shows (LLP 1038 D6).
//
// The head is the runner's (runner/src/head.rs): the innermost active `head`
// wins field by field, a deeper head over a shallower one and, at one depth,
// the later in document order; a head inside a route its navigation root has
// not selected is inactive. A root is an element with `navigationBack` and
// `navigationKey`; its selected route is in its direct stack or one of its
// tabpanels (`unselectedForHead`, navigation.js). Selection comes from those
// attributes, never from what the projection hides (`projectRoots`).
// After each commit's tree update, once the page has had a head, the active
// head is found again, so one that changes, leaves or is covered gives its
// fields to the next; the page's title and description follow it as the web
// host's `documentHead` writes them (document-glue.js). Where no head sets a
// title it is the selected route's header's heading, else the app's name
// (navigation.js `pageTitle`, both web targets': LLP 1115.000 D4.1).
import { unselected, unselectedForHead, pageTitle, panelsOf } from "./navigation.js";
import { scope, owner, onEnd, adopting } from "./rt.js";

/** The active head's fields by prop name (`headTitle` …): what a renderer
 * reads (render.mjs), and the agent's `state.head`. */
export const Head = {};
const Heads = new Set();

/** A head: its node `t` (a `<template>` rt.js placed) and its `fields`, each
 * text or a getter `effect` follows. Its fields are the page's while it is
 * the innermost active head, found after each commit (`after`, rt.js
 * `After`); the function returned forgets it, as its scope ends. */
export function head(t, fields, effect, after) {
  const h = { t, f: {} };
  for (const k in fields) { const v = fields[k]; if (typeof v === "function") effect(() => { h.f[k] = v(); }); else h.f[k] = v; }
  if (!after.includes(publish)) after.push(publish);
  Heads.add(h);
  return () => Heads.delete(h);
}
/** Whether `e` lies under a route or a tabpanel its navigation root leaves unselected (runner/src/head.rs `inactive`). */
export function inactive(e) { for (let p = e.parentNode; p; p = p.parentNode) if (p.hasAttribute?.("navigationBack") && unselected(p).some(off => off.contains(e))) return true; return false; }
function publish() {
  const root = document.getElementById("exact-root"), live = [], next = {}, depth = {};
  // Each active head with its place: its own and each ancestor's position
  // among its siblings, from the root's children down (the order without
  // `compareDocumentPosition`, which a render's DOM, dom.js, lacks).
  for (const h of Heads) {
    const at = [], path = new Set([h.t]);
    let c = h.t;
    for (let p = c.parentNode; p && c !== root; c = p, p = p.parentNode) {
      if (p.hasAttribute?.("navigationBack") && unselectedForHead(p).some(off => path.has(off))) break;
      at.unshift([].indexOf.call(p.childNodes, c));
      path.add(p);
    }
    if (c === root) live.push([at, h.f]);
  }
  live.sort(([a], [b]) => { let i = 0; while (i < a.length && a[i] === b[i]) i++; return (a[i] ?? -1) - (b[i] ?? -1); });
  for (const [at, f] of live) for (const k in f) {
    const v = f[k];
    if (v == null || at.length < depth[k]) continue;
    // A status that is not a whole 0–65535 sets none, and any head may set one after it (head.rs).
    if (k === "headStatus" && v !== (v & 65535)) { delete depth[k]; delete next[k]; continue; }
    depth[k] = at.length; next[k] = k === "headStatus" ? v : String(v);
  }
  if (JSON.stringify(next) === JSON.stringify(Head)) return;
  for (const k in Head) delete Head[k];
  Object.assign(Head, next);
  pageTitle(Head.headTitle ?? null);
  let m = document.querySelector('meta[name="description"]');
  if (Head.headDescription == null) m?.remove();
  else { if (!m) { m = document.createElement("meta"); m.setAttribute("name", "description"); document.head.append(m); } m.content = Head.headDescription; }
}

/** The page's navigation roots, with a router or without (`history` is
 * navigation.js's, the web host's own): each root's routes are projected,
 * the covered hidden and inert, now and after every commit's tree (`after`,
 * rt.js `After`), as the web host projects after every batch; Escape on a
 * modal route presses its back, and `navigate` takes a popstate's location
 * (unheard, `hostBack` and `hostGo` are the router's own). */
export function projectRoots(history, navigate, say, after, hostBack, hostGo, routeMatches) {
  const root = document.getElementById("exact-root"), project = () => history.project(root, say);
  history.connect(root, navigate, say, hostBack, hostGo, routeMatches, true);
  after.push(project); history.activate(root, say);
}

/** The elements that may be the page's scroller: `<html data-scrolldocument>`
 * while one is (D4), which the shell's rule reads, as the web host's glue. */
export const Docs = new Set();
let Marked = false;
export function markDocument() {
  if (!Docs.size && !Marked) return;
  Marked = false;
  for (const e of Docs) if (!e.isConnected) Docs.delete(e); else Marked ||= e.getAttribute("data-scrolldocument") === "true";
  document.documentElement?.toggleAttribute?.("data-scrolldocument", Marked);
}

// @ref LLP 1075.003 §3.7 — a navigation tab is built the first time it is
// selected, as the runner builds it (runner/src/instance/tabs.rs): a panel the
// compiler names (emit.rs, `panel_sites`) is made empty, and its contents are
// made after the tree update of the commit whose tree first selects it (rt.js
// `commit` runs `openTabs`), before its paint. The selection is the projection's: the panel whose
// routes hold the root's `navigationKey`; while no built panel holds it, the
// panel whose tab is `aria-selected` is built, else the first waiting one.
// A page rendered ahead holds every panel, as its renderer builds them all
// (`Runner::build_every_tab`; a render here likewise): the boot that adopts
// it builds every one, and none waits for the page's life.
const Waiting = new Map();
let Every = false;
/** Panel `e` waits; `build` makes its contents, in the scope in force here, when it is first selected. */
export function tp(e, build) {
  if ((Every ||= !!globalThis.__exactRender || adopting())) return scope(build, owner());
  const up = owner(); Waiting.set(e, () => scope(build, up)); onEnd(() => Waiting.delete(e));
}
/** Whether `e` is a panel whose tab has never been selected: the agent's `unopened`. */
export const unopened = e => Waiting.has(e);
/** Build each waiting panel its root now selects, and any no root's tablist names. */
export function openTabs() {
  for (let open = toOpen(); open.length; open = toOpen()) for (const e of open) { const build = Waiting.get(e); Waiting.delete(e); build(); }
}
function toOpen() {
  const open = [], roots = new Set();
  for (const e of Waiting.keys()) {
    let root = e.parentElement;
    while (root && !root.hasAttribute("navigationKey")) root = root.parentElement;
    if (!root?.hasAttribute("navigationBack")) root = null;
    const panels = root ? panelsOf(root) : [];
    if (!panels.includes(e)) { open.push(e); continue; }
    if (roots.has(root)) continue;
    roots.add(root);
    const key = root.getAttribute("navigationKey");
    // A panel holds the key once this runtime built it: an adopted page's
    // waiting panel already shows its rendered routes, which only `build` wires.
    if (panels.some(p => !Waiting.has(p) && [...p.children].some(r => r.getAttribute("navigationKey") === key))) continue;
    const named = [...root.getElementsByTagName("*")].filter(t => t.getAttribute("aria-selected") === "true").map(t => t.getAttribute("aria-controls"));
    const waiting = panels.filter(p => Waiting.has(p)), chosen = waiting.find(p => named.includes(p.getAttribute("id"))) ?? waiting[0];
    if (chosen) open.push(chosen);
  }
  return open;
}
