// Optional decorated-field mounting, outside the signal runtime.
let DecoratedEditor;
export function startDecoration(e, {own, onEnd, inflight, viewId, Ids, Views, say}) {
  if (typeof requestAnimationFrame !== 'function' || globalThis.__exactRender) return;
  let cleanup; onEnd(() => cleanup?.()); inflight.n++;
  (DecoratedEditor ??= import('./decoration-editor.js')).then(async module => {
    const install = await globalThis.exact.installDecorationEditor;
    if (e.isConnected && !own?.gone) cleanup = module.bindRuntime(e, {install, id:viewId(e), Ids, Views, log:say});
  }).catch(error => say(`decorations: ${error.message}`)).finally(() => inflight.n--);
}
