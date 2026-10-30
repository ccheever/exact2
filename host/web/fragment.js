// @ref LLP 1153 D5 — the browser's "scroll to the fragment" (HTML: find a potential indicated element, scroll it into
// view, move the sequential focus navigation starting point to it), made by the runtime once a route's content has
// arrived: an Exact page's content comes after load, so the browser's own step finds nothing. The element is looked
// for in the visit's own route, since a route retained beneath may hold the same `id`. Both web runtimes load this
// module at a fragment's first scroll: the JS target's router.js, which decides when, and the wasm host's glue.js, which
// runs the runner's `scrollToFragment`. `arm` comes with the router change that asks for a scroll, `scroll` once the
// content has settled.

let armed = null, stop = null;
// Where a visit's route was scrolled while it showed a fragment, by `${visit}#${fragment}`: what Back or Forward between
// two fragments of one visit puts back, as the browser restores a traversed entry's position.
const places = new Map(), showing = new Map();

/** The route of `visit`: the element keyed by it that is not a navigation root, whose key is its top's. */
function route(root, visit) {
  for (const el of root?.querySelectorAll("[navigationKey]") ?? []) {
    if (el.getAttribute("navigationKey") === String(visit) && !el.hasAttribute("navigationBack") && !el.hasAttribute("data-exiting")) return el;
  }
  return null;
}
/** The document's scroller and every element in the route that scrolls or is scrolled. */
const scrollers = r => [document.scrollingElement, r, ...r.querySelectorAll("*")]
  .filter(el => el && (el.scrollTop || el.scrollLeft || el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth));
const place = r => scrollers(r).map(el => [el, el.scrollTop, el.scrollLeft]);
/** The fragment percent-decoded, then UTF-8 decoded without BOM or fail (HTML, "find a potential indicated element"). */
function decoded(fragment) {
  const bytes = [], utf8 = new TextEncoder();
  for (let i = 0; i < fragment.length; i++) {
    if (fragment[i] === "%" && /^[0-9a-f]{2}$/i.test(fragment.slice(i + 1, i + 3))) { bytes.push(parseInt(fragment.slice(i + 1, i + 3), 16)); i += 2; }
    else bytes.push(...utf8.encode(fragment[i]));
  }
  try { return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(new Uint8Array(bytes)); } catch { return null; }
}
const byId = (r, id) => id ? [...r.querySelectorAll("[id]")].find(el => el.id === id && !el.closest("[data-exiting]")) ?? null : null;

/** A router change asked for a scroll to its top's fragment (`op`: its `top`, `fragment` and, when the browser's Back
 * or Forward made it, `traversal`). Where the visit's route is now is kept under the fragment it showed, and a scroll
 * by the person before `scroll` runs cancels it: a wheel, a touch's move, or a key that scrolls. */
export function arm(root, op) {
  // What is kept for a visit whose route left the page goes with it.
  for (const visit of showing.keys()) if (!route(root, visit)) { showing.delete(visit); for (const key of places.keys()) if (key.startsWith(`${visit}#`)) places.delete(key); }
  const r = route(root, op.top);
  if (r) places.set(`${op.top}#${showing.get(op.top) ?? ""}`, place(r));
  showing.set(op.top, op.fragment);
  armed = { visit: op.top, fragment: op.fragment, traversal: !!op.traversal, cancelled: false };
  stop?.abort(); stop = new AbortController();
  const cancel = e => {
    if (e.type === "keydown" && (!/^(Arrow(Up|Down|Left|Right)|Page(Up|Down)|Home|End| )$/.test(e.key) || e.target.closest?.("input, textarea, select, [contenteditable]"))) return;
    if (armed) armed.cancelled = true;
  };
  for (const type of ["wheel", "touchmove", "keydown"]) addEventListener(type, cancel, { capture: true, passive: true, signal: stop.signal });
}

/** Scroll `visit`'s route to `fragment`: the element with that id, else its decoded id, at the start of every scroller
 * above it and the starting point of the next Tab; `top`, or no such element, the route's top. A traversal between two
 * fragments of the visit puts back where the route was instead. */
export function scroll(root, visit, fragment, log) {
  const was = armed?.visit === visit && armed.fragment === fragment ? armed : null;
  if (was) { armed = null; stop?.abort(); }
  if (was?.cancelled) return log(`fragment #${fragment}: the person scrolled first; the route stays where it is`);
  const r = route(root, visit);
  if (!r) return log(`fragment #${fragment}: no route of visit ${visit} is on the page`);
  const back = was?.traversal && places.get(`${visit}#${fragment}`);
  if (back) { for (const [el, top, left] of back) if (el.isConnected) el.scrollTo({ top, left, behavior: "instant" }); return; }
  const id = fragment && decoded(fragment), target = fragment ? byId(r, fragment) ?? byId(r, id) : null;
  if (target) {
    target.scrollIntoView({ block: "start", inline: "nearest" });
    // The starting point: the target takes the focus, as the next Tab and a screen reader start from it. A field the
    // person is typing in keeps it.
    if (document.activeElement?.matches?.("input, textarea, select, [contenteditable]")) return;
    if (!target.matches("a[href], button, input, select, textarea, [tabindex], [contenteditable]")) {
      target.setAttribute("tabindex", "-1");
      target.addEventListener("blur", () => target.removeAttribute("tabindex"), { once: true });
    }
    target.focus({ preventScroll: true });
    return;
  }
  if (fragment && id?.toLowerCase() !== "top") log(`fragment #${fragment}: no element in the route of visit ${visit} has that id; the route goes to its top`);
  for (const el of scrollers(r)) if (el !== document.scrollingElement) el.scrollTo({ top: 0, left: 0, behavior: "instant" });
  r.scrollIntoView({ block: "start", inline: "nearest", behavior: "instant" });
}

// The wasm host loads this module by a script tag (glue.js `loadAfterPaint`), which finds it here.
(globalThis.exact ??= {}).fragment = { arm, scroll };
