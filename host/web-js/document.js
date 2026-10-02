// What a view says about its page's document (LLP 1048.003), for rt.js: its
// head (D1) and the element that scrolls it (D4).
//
// The head is the runner's (runner/src/head.rs): the innermost active `head`
// wins field by field, a deeper head over a shallower one and, at one depth,
// the later in document order; a head inside a route its navigation root has
// not selected is inactive. A root is an element with `navigationBack` and
// `navigationKey`, its routes its children with a `navigationKey`, and the
// selected one the route whose key is the root's, when one is: read from
// those attributes, never from what the projection hides (navigation.js
// `project`, a microtask later). After each commit's tree update, once the
// page has had a head, the active head is found again, so one that changes,
// leaves or is covered gives its fields to the next; the page's title and
// description follow it as the web host's `documentHead` writes them
// (document-glue.js), the page's own title where no head sets one.

/** The active head's fields by prop name (`headTitle` …): what a renderer
 * reads (render.mjs), and the agent's `state.head`. */
export const Head = {};
const Heads = new Set();
let Own = null; // the page's title before a head's

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
/** Whether `c`, a child of `p`, is a route `p` has not selected. */
function covered(p, c) {
  const key = p.getAttribute?.("navigationKey"), own = c.getAttribute?.("navigationKey");
  return key != null && own != null && own !== key && p.hasAttribute("navigationBack")
    && [].some.call(p.childNodes, r => r.getAttribute?.("navigationKey") === key);
}
function publish() {
  const root = document.getElementById("exact-root"), live = [], next = {}, depth = {};
  // Each active head with its place: its own and each ancestor's position
  // among its siblings, from the root's children down (the order without
  // `compareDocumentPosition`, which a render's DOM, dom.js, lacks).
  for (const h of Heads) {
    const at = [];
    let c = h.t;
    for (let p = c.parentNode; p && c !== root && !covered(p, c); c = p, p = p.parentNode) at.unshift([].indexOf.call(p.childNodes, c));
    if (c === root) live.push([at, h.f]);
  }
  live.sort(([a], [b]) => { let i = 0; while (i < a.length && a[i] === b[i]) i++; return (a[i] ?? -1) - (b[i] ?? -1); });
  for (const [at, f] of live) for (const k in f) {
    const v = f[k];
    if (v == null || at.length < depth[k]) continue;
    // A status no response can carry (a whole 0–65535) sets none, and any head may set one after it (head.rs).
    if (k === "headStatus" && v !== (v & 65535)) { delete depth[k]; delete next[k]; continue; }
    depth[k] = at.length; next[k] = k === "headStatus" ? v : String(v);
  }
  if (JSON.stringify(next) === JSON.stringify(Head)) return;
  for (const k in Head) delete Head[k];
  Object.assign(Head, next);
  Own ??=document.querySelector('meta[property="og:site_name"]')?.content ?? document.title;
  const title = Head.headTitle ?? Own;
  if (document.title !== title) document.title = title;
  let m = document.querySelector('meta[name="description"]');
  if (Head.headDescription == null) m?.remove();
  else { if (!m) { m = document.createElement("meta"); m.setAttribute("name", "description"); document.head.append(m); } m.content = Head.headDescription; }
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
