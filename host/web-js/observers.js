// The browser's geometry events, loaded after first paint only where used.
// @ref LLP 1005 intersection visibility; the resize event's contentRect.
import { onEnd, inflight, say } from "./rt.js";

let Resizes = null;
export function onResize(e, f) {
  if (typeof ResizeObserver !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  (Resizes ??= new Promise(r => requestAnimationFrame(() => r())).then(() => import("./resize-glue.js")))
    .then(m => m.observeResize(e, r => { if (e.isConnected) f(r.width, r.height, [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]); }))
    .catch(err => say(`resize: ${err.message}`)).finally(() => inflight.n--);
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
