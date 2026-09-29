// Canvas 2D surfaces drawn by a TypeScript source on the JS runtime (LLP
// 1056; LLP 1071 §7): the app's own `draw`, with the TypeScript recorder the
// bake bundles into a drawing module (`canvas/recorder.js`, the Rust
// recorder's rules and list bytes), one recorder per canvas generation as
// its `canvasSeam` keeps them. The lists stay bytes: nothing crosses a seam,
// so no base64. Text is measured and images sized on the page by the web
// host's own glue (`exact.canvas2dHost`, LLP 1056 D8, D9), as the wasm
// host's module realm asks it. In `canvas2d.js`'s chunk: an app without a 2D
// canvas never loads it.
// The recorder installs its Path2D as the global, as a draw expects; in the
// page the glue replays with the browser's, so the global is the recorder's
// only during a draw. `path2d.js` (the build's) keeps the browser's,
// evaluated before the recorder.
import { browserPath2D } from './path2d.js';
import { draw } from '__APP_TS__';
import { Recorder, Path2D } from '__RECORDER__';
import { named } from './ts-data.js';
globalThis.Path2D = browserPath2D;

const CAUSES = ['mount', 'args', 'size', 'frame', 'image', 'font'];
const recorders = new Map(); // canvas lifetime -> { generation, rec }

export const drawer = {
  draw(q) {
    let entry = recorders.get(q.canvas);
    if (!entry || entry.generation !== q.generation) recorders.set(q.canvas, entry = { generation: q.generation, rec: new Recorder() });
    const inner = entry.rec._;
    inner.env = { canvas: q.canvas, currentColor: q.color ?? null, rtl: q.rtl === true, host: globalThis.exact?.canvas2dHost ?? null };
    const [time, mounted, bits, width, height, pixelWidth, pixelHeight, scale] = q.frame;
    const causes = CAUSES.filter((_, i) => bits & (1 << i));
    // The arguments as a GPU surface's `bind` receives them: by name when
    // the surface names them, records as objects by their declared types.
    const value = (v, i) => named(v, q.types[i] || null);
    const args = q.names.length ? Object.fromEntries(q.names.map((n, i) => [n, value(q.args[i], i)])) : q.args.map(value);
    let wants = false, error = null;
    globalThis.Path2D = Path2D;
    try {
      const r = draw(q.surface, args, entry.rec, Object.freeze({ time, mounted, cause: causes[0] ?? 'frame', causes, width, height, pixelWidth, pixelHeight, scale }));
      if (r && typeof r.then === 'function') throw new TypeError('draw returned a promise; a draw must finish in its turn');
      wants = r === true;
    } catch (e) {
      error = e && typeof e === 'object' && e.message !== undefined ? `${e.name ?? 'Error'}: ${e.message}` : String(e);
    } finally { globalThis.Path2D = browserPath2D; }
    inner.env = { ...inner.env, host: null };
    return { lists: inner.take(), frame: wants && !error, error, notes: inner.notes.splice(0) };
  },
  retire(canvas) { recorders.delete(canvas); },
};
