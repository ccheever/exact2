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
  let first = true;
  const landed = () => { if (first) { first = false; inflight.n--; } };
  // (After the page's geometry, where it reads any: an action a first size runs reads frames, which answer
  // unavailable until it has loaded. Both load after first paint; neither is waited on before it.)
  (Resizes ??= new Promise(r => requestAnimationFrame(() => r())).then(() => Promise.all([import("./resize-glue.js"), geoLoading()?.catch(() => {})])).then(([m]) => m))
    .then(m => {
      m.observeResize(e, r => { if (e.isConnected) f(r.width, r.height, [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]); landed(); });
      if (!e.isConnected || !e.getClientRects().length) landed();
      else setTimeout(landed, FIRST_MS);
    })
    .catch(err => { say(`resize: ${err.message}`); landed(); });
}
let Intersects = null;
export function onIntersect(e, f) {
  if (globalThis.__exactRender) return;
  let gone = false, stop;
  onEnd(() => { gone = true; stop?.(); });
  inflight.n++;
  (Intersects ??= new Promise(r => requestAnimationFrame(r)).then(() => import("./intersect-glue.js")))
    .then(m => { if (!gone && e.isConnected) stop = m.observeIntersect(e, r => { if (!gone) f([r.isIntersecting, r.intersectionRatio]); }); })
    .catch(err => say(`intersect: ${err.message}`)).finally(() => inflight.n--);
}
