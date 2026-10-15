// The web host's own modules the JS target's runtime loads, copied beside it,
// and the rewrites that point the runtime's `../web/…` imports at those copies.
// One list for every runtime staged from host/web-js: a page (build.mjs) and
// the differential driver's headless runtime (semantics/difftest/js/drive.mjs),
// which lost the bundle when it kept a list of its own (intersect-glue.js,
// 2026-10-12). The async lane's `difftest-js` inputs read it too.
import { cpSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

export const HOST_WEB_GLUE = [
  // Loaded after first paint: motion.js, a pan, `select`, text flow, rt.js `pr`, native.js, rt.js `geo`, media.js, notify.js, sounds.js.
  'frames.js', 'motion-glue.js', 'group-glue.js', 'input-glue.js', 'markup-editor.js', 'textflow-glue.js', 'timer-glue.js',
  'presence-glue.js', 'native-glue.js', 'geometry-glue.js', 'resize-glue.js', 'intersect-glue.js', 'media-glue.js',
  'notify-glue.js', 'sound-glue.js',
  // Virtualized lists' browser half; animated images on the agent's clock (agent.js only); Canvas 2D's replayer.
  'collection-glue.js', 'image-glue.js', 'canvas2d-glue.js',
  // Sign-in, storage, request bodies and injected faults (ts-data.js, auth.js, admission.js, agent.js).
  'auth-glue.js', 'storage-environment.js', 'http-body.js', 'faults.js',
];

/** Copy the host/web glue from the repository at `root` into `dir`. */
export function stageHostGlue(root, dir) {
  for (const f of HOST_WEB_GLUE) cpSync(resolve(root, 'host/web', f), resolve(dir, f));
}

/**
 * What the differential driver's headless runtime copies from the repository
 * at `root`, as paths relative to it: every host/web-js module, the glue, and
 * navigation.js and its grant admission whole (build.mjs splits them for
 * chunking). The async lane's `difftest-js` inputs are these files.
 */
export function driverRuntime(root) {
  const own = readdirSync(resolve(root, 'host/web-js')).filter(f => f.endsWith('.js')).sort().map(f => `host/web-js/${f}`);
  return [...own, ...[...HOST_WEB_GLUE, 'navigation.js', 'grant-admission.js'].map(f => `host/web/${f}`)];
}

/** A runtime module's `../web/…` imports, pointed at the copies beside it. */
export const localImports = text => text
  .replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'")
  .replaceAll("'../web/faults.js'", "'./faults.js'");
