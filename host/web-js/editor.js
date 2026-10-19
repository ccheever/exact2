// A plain textarea's decorations (LLP 1114.007), loaded after first paint by
// the first field that has them, outside the signal runtime.
let Decorations;
export function startDecoration(e, {own, onEnd, inflight, say}) {
  if (typeof requestAnimationFrame !== 'function' || globalThis.__exactRender) return;
  let handle; onEnd(() => handle?.destroy()); inflight.n++;
  (Decorations ??= new Promise(r => requestAnimationFrame(r)).then(() => import('./decorations.js'))).then(module => {
    if (e.isConnected && !own?.gone) handle = module.install(e, say);
  }).catch(error => say(`decorations: ${error.message}`)).finally(() => inflight.n--);
}
