// The browser's geometry events, loaded after first paint only where used.
// @ref LLP 1005 intersection visibility; the resize event's contentRect.
import { onEnd, inflight, say, geoLoading } from "./rt.js";

let Resizes = null;
// The element's first size is in flight until it is delivered (its action run on it), not only until the observer
// is attached: a settle (the agent's, conform's boot step) reads the state the first report made, however late a
// loaded machine delivers it. One with no box (gone, or `display: none`), which a browser may not report, holds
// nothing; any other, 2 s at most.
const FIRST_MS = 2000;
export function onResize(e, f) {
  if (typeof ResizeObserver !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  let first = true, bound = null;
  const landed = () => { if (first) { first = false; clearTimeout(bound); inflight.n--; } };
  // (A view destroyed meanwhile reports nothing: let go with it.)
  onEnd(landed);
  // (After the page's geometry, where it reads any: an action a first size runs reads frames, which answer
  // unavailable until it has loaded. Both load after first paint; neither is waited on before it.)
  (Resizes ??= new Promise(r => requestAnimationFrame(() => r())).then(() => Promise.all([import("./resize-glue.js"), geoLoading()?.catch(() => {})])).then(([m]) => m))
    .then(m => {
      m.observeResize(e, r => { try { if (e.isConnected) f(r.width, r.height, [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]); } finally { landed(); } });
      if (!e.isConnected || !e.getClientRects().length) landed();
      // (`__exactFirstSizeBound`: a test's, to see a release that is no bound's.)
      else if (first) bound = setTimeout(landed, globalThis.__exactFirstSizeBound ?? FIRST_MS);
    })
    .catch(err => { say(`resize: ${err.message}`); landed(); });
}
let Intersects = null;
// As a first size: the observer's first entry (one always comes, intersecting or not) is in flight until it is
// delivered, so a settle (conform's boot step) reads the state it made however late a loaded machine delivers it;
// one gone first holds nothing, any other 2 s at most (the wasm host's twin, host/web/glue.js).
export function onIntersect(e, f) {
  if (globalThis.__exactRender) return;
  let gone = false, stop, first = true, bound = null;
  const landed = () => { if (first) { first = false; clearTimeout(bound); inflight.n--; } };
  onEnd(() => { gone = true; stop?.(); landed(); });
  inflight.n++;
  (Intersects ??= new Promise(r => requestAnimationFrame(r)).then(() => import("./intersect-glue.js")))
    .then(m => {
      if (gone || !e.isConnected) { landed(); return; }
      stop = m.observeIntersect(e, r => { try { if (!gone) f([r.isIntersecting, r.intersectionRatio]); } finally { landed(); } });
      if (first) bound = setTimeout(landed, globalThis.__exactFirstSizeBound ?? FIRST_MS);
    })
    .catch(err => { say(`intersect: ${err.message}`); landed(); });
}
