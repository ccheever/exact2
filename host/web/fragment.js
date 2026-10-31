// @ref LLP 1153 D5 — the browser's "scroll to the fragment" (HTML: find a potential indicated element, scroll it into
// view, move the sequential focus navigation starting point to it), made by the runtime once a route's data has
// arrived: an Exact page's content comes after load, so the browser's own step finds nothing. The element is looked
// for in the visit's own route, since a route retained beneath may hold the same `id`; it is held at the start while
// the route's pictures load, as a browser holds its fragment in view while its document loads. Both web runtimes load
// this module at a fragment's first scroll: the JS target's router.js, which decides when, and the wasm host's glue.js,
// which runs the runner's `scrollToFragment`. `arm` comes with the router change that asks for a scroll, `scroll` once
// the data has settled, and `cancel` with the app's own `scrollIntoView`, which wins.

let armed = null, held = null;
// Where each history entry (navigation.js's `exact`) left the scrollers it moved: what Back or Forward between two
// fragments of one visit puts back, as the browser restores a traversed entry's scroll position.
const kept = new Map(), entry = () => globalThis.history?.state?.exact;
addEventListener("scroll", e => {
  const at = entry(), el = e.target === document ? document.scrollingElement : e.target;
  if (at == null || !(el instanceof Element)) return;
  if (!kept.has(at)) kept.set(at, new Map());
  kept.get(at).set(el, [el.scrollTop, el.scrollLeft]);
}, { capture: true, passive: true });

/** The route of `visit`: the element keyed by it that is not a navigation root, whose key is its top's. */
function route(root, visit) {
  for (const el of root?.querySelectorAll("[navigationKey]") ?? []) {
    if (el.getAttribute("navigationKey") === String(visit) && !el.hasAttribute("navigationBack") && !el.hasAttribute("data-exiting")) return el;
  }
  return null;
}
const scrolls = el => el && (el.scrollTop || el.scrollLeft || el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth);
/** Every element in the route that scrolls or is scrolled. */
const inside = r => [r, ...r.querySelectorAll("*")].filter(scrolls);
/** What a scroll to an element of the route can move: those, the route's ancestors that scroll, and the document's. */
function scrollers(r) {
  const out = [document.scrollingElement];
  for (let el = r.parentElement; el; el = el.parentElement) if (el !== document.scrollingElement && scrolls(el)) out.push(el);
  return [...out, ...inside(r)];
}
/** The fragment of a location: everything after its first `#`. */
const fragmentOf = url => { const at = url?.indexOf("#") ?? -1; return at < 0 ? "" : url.slice(at + 1); };
/** Whether the history entry shown is still `visit` at `fragment` (navigation.js writes `id` and `url`). */
export const shows = (visit, fragment, s = globalThis.history?.state) => !s || (s.id === visit && fragmentOf(s.url) === fragment);
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

/** A router change asked for a scroll to its top's fragment (`op`: its `top`, `fragment` and `activation`; `traversal`
 * when the browser's Back or Forward made it, `pushed` when it pushed a history entry of the visit, navigation.js;
 * `since`, the person's scrolls when it was made). A pushed entry keeps where the one it left had the route; a scroll
 * by the person since (`person.personScrolls`, counted by navigation.js from the page's start) cancels it, and the
 * hold of an earlier one ends. */
export function arm(root, op, person) {
  const r = route(root, op.top), at = entry();
  if (op.pushed && r && at != null) {
    for (const k of kept.keys()) if (k >= at) kept.delete(k);
    kept.set(at - 1, new Map(scrollers(r).map(el => [el, [el.scrollTop, el.scrollLeft]])));
  }
  held?.abort();
  armed = { visit: op.top, fragment: op.fragment, activation: op.activation, traversal: !!op.traversal, since: op.since ?? person.personScrolls, person };
}
/** The app's own scroll: a fragment's scroll held gives way, and with `pending` (a `scrollIntoView`) one not yet made. */
export function cancel(pending = true) { held?.abort(); if (armed && pending) armed.authored = true; }

/** Hold `target` at the start of its scrollers while `r`'s pictures load, until the person scrolls, it leaves, or the
 * entry shown is no longer `visit` at `fragment`. */
function hold(r, target, visit, fragment, person) {
  held?.abort();
  const loading = () => [...r.querySelectorAll("img")].some(img => !img.complete);
  if (!loading()) return;
  const h = held = new AbortController(), since = person?.personScrolls, again = () => {
    if (!target.isConnected || person?.personScrolls !== since || !shows(visit, fragment)) return h.abort();
    target.scrollIntoView({ block: "start", inline: "nearest", behavior: "instant" });
    if (!loading()) h.abort();
  };
  for (const type of ["load", "error"]) r.addEventListener(type, again, { capture: true, signal: h.signal });
}

/** Scroll `visit`'s route to `fragment` for the router change armed with `activation`: the element with that id, else
 * its decoded id, at the start of every scroller above it and the starting point of the next Tab; `top`, or no such
 * element, the route's top. A traversal puts back where its history entry left the route, or leaves it where it is. */
export function scroll(root, visit, fragment, log, activation) {
  const was = armed && (activation != null ? armed.activation === activation : armed.visit === visit && armed.fragment === fragment) ? armed : null;
  if (was) armed = null;
  if (was?.authored) return log(`fragment #${fragment}: the app's own scrollIntoView took its place`);
  if (was && was.person.personScrolls !== was.since) return log(`fragment #${fragment}: the person scrolled first; the route stays where it is`);
  const r = route(root, visit);
  if (!r) return log(`fragment #${fragment}: no route of visit ${visit} is on the page`);
  if (was?.traversal) {
    const place = new Map(kept.get(entry())), restore = () => { for (const [el, [top, left]] of place) if (el.isConnected && (el === document.scrollingElement || r.contains(el) || el.contains(r))) el.scrollTo({ top, left, behavior: "instant" }); };
    // The browser's own traversal to an entry with a fragment then scrolls to the document's first element of that
    // id, which knows no route, at the `hashchange` it sends or, its fragment unchanged, at its next frame's layout:
    // the place, as it was before that scroll, is put back once more after each, unless the person scrolled since.
    restore();
    const since = was.person?.personScrolls, again = () => { if (was.person?.personScrolls === since) restore(); };
    if (place.size) { addEventListener("hashchange", again, { once: true }); setTimeout(() => removeEventListener("hashchange", again), 1000); requestAnimationFrame(() => requestAnimationFrame(again)); }
    return;
  }
  const id = fragment && decoded(fragment), target = fragment ? byId(r, fragment) ?? byId(r, id) : null;
  if (target) {
    target.scrollIntoView({ block: "start", inline: "nearest" });
    hold(r, target, visit, fragment, was?.person);
    // The starting point: the target takes the focus, as the next Tab and a screen reader start from it, with no focus
    // ring, as a browser's own scroll to a fragment draws none. A field the person is typing in keeps it.
    if (document.activeElement?.matches?.("input, textarea, select, [contenteditable]")) return;
    if (!target.matches("a[href], button, input, select, textarea, [tabindex], [contenteditable]")) {
      target.setAttribute("tabindex", "-1");
      target.addEventListener("blur", () => target.removeAttribute("tabindex"), { once: true });
    }
    target.focus({ preventScroll: true, focusVisible: false });
    return;
  }
  if (fragment && id?.toLowerCase() !== "top") log(`fragment #${fragment}: no element in the route of visit ${visit} has that id; the route goes to its top`);
  for (const el of inside(r)) el.scrollTo({ top: 0, left: 0, behavior: "instant" });
  r.scrollIntoView({ block: "start", inline: "nearest", behavior: "instant" });
}

// The wasm host loads this module by a script tag (glue.js `loadAfterPaint`), which finds it here.
(globalThis.exact ??= {}).fragment = { arm, scroll, cancel, shows };
