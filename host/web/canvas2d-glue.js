// Canvas 2D on the web (LLP 1056 D4, D7): each 2D canvas's content box is
// watched with a ResizeObserver and reported to the runner
// (`exact_canvas_geometry`); the runner's stamped lists are replayed, in
// order, into the canvas element's real CanvasRenderingContext2D, so the
// browser is the rasterizer and the oracle. The list's geometry is resolved
// (canvas/src/list.rs): path points arrive in canvas coordinates, so they are
// added under the base transform (the device scale), and every paint runs
// under base ∘ author, where the browser strokes and shades in user space as
// it always does. Injected after first paint with the first 2D canvas; an
// app without one never fetches it (LLP 1047).
const OPS = {
  1: "save", 2: "restore", 3: "reset", 4: "setTransform",
  10: "fillColor", 11: "fillGradient", 12: "strokeColor", 13: "strokeGradient",
  14: "lineWidth", 15: "lineCap", 16: "lineJoin", 17: "miterLimit", 18: "lineDash", 19: "lineDashOffset",
  20: "globalAlpha", 21: "composite",
  30: "linearGradient", 31: "radialGradient", 32: "colorStop",
  40: "beginPath", 41: "moveTo", 42: "lineTo", 43: "quadTo", 44: "cubicTo", 45: "closePath",
  50: "fill", 51: "stroke", 52: "clip", 53: "fillRect", 54: "strokeRect", 55: "clearRect",
};
const COMPOSITE = ["source-over", "source-in", "source-out", "source-atop", "destination-over", "destination-in",
  "destination-out", "destination-atop", "lighter", "copy", "xor", "multiply", "screen", "overlay", "darken", "lighten",
  "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"];
const CAPS = ["butt", "round", "square"], JOINS = ["miter", "round", "bevel"];
const MAGIC = 0x44324345;

function decode(text) {
  const bin = atob(text), bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new DataView(bytes.buffer);
}

/** One canvas: the element, its context, and what the replayer keeps. */
class Replayer {
  constructor(el, op) {
    this.el = el; this.lifetime = op.lifetime; this.generation = op.generation;
    // Assigning the bitmap's size clears it and resets the context (HTML).
    el.width = op.w; el.height = op.h;
    this.ctx = el.getContext("2d");
    this.base = op.scale; this.author = [1, 0, 0, 1, 0, 0]; this.stack = []; this.gradients = new Map();
    this.ctx.setTransform(this.base, 0, 0, this.base, 0, 0);
  }
  paint(f) {
    const [a, b, c, d, e, g] = this.author, s = this.base;
    this.ctx.setTransform(s * a, s * b, s * c, s * d, s * e, s * g);
    f(this.ctx);
    this.ctx.setTransform(s, 0, 0, s, 0, 0);
  }
  apply(view) {
    if (view.byteLength < 8 || view.getUint32(0, true) !== MAGIC || view.getUint32(4, true) !== 1) return false;
    const ctx = this.ctx, n = new Float64Array(64);
    const rgba = (i) => `rgba(${n[i]}, ${n[i + 1]}, ${n[i + 2]}, ${n[i + 3]})`;
    let at = 8;
    while (at + 8 <= view.byteLength) {
      const code = view.getUint32(at, true), count = view.getUint32(at + 4, true);
      at += 8;
      if (at + count * 8 > view.byteLength || !OPS[code]) return false;
      const k = count > n.length ? new Float64Array(count) : n;
      for (let i = 0; i < count; i++) k[i] = view.getFloat64(at + i * 8, true);
      at += count * 8;
      switch (OPS[code]) {
        case "save": this.stack.push(this.author); ctx.save(); break;
        case "restore": if (this.stack.length) { this.author = this.stack.pop(); ctx.restore(); } break;
        case "reset":
          this.stack.length = 0; this.author = [1, 0, 0, 1, 0, 0];
          if (ctx.reset) ctx.reset(); else this.el.width = this.el.width;
          ctx.setTransform(this.base, 0, 0, this.base, 0, 0);
          break;
        case "setTransform": this.author = [k[0], k[1], k[2], k[3], k[4], k[5]]; break;
        case "fillColor": ctx.fillStyle = rgba(0); break;
        case "strokeColor": ctx.strokeStyle = rgba(0); break;
        case "fillGradient": ctx.fillStyle = this.gradients.get(k[0]) ?? "transparent"; break;
        case "strokeGradient": ctx.strokeStyle = this.gradients.get(k[0]) ?? "transparent"; break;
        case "lineWidth": ctx.lineWidth = k[0]; break;
        case "lineCap": ctx.lineCap = CAPS[k[0]]; break;
        case "lineJoin": ctx.lineJoin = JOINS[k[0]]; break;
        case "miterLimit": ctx.miterLimit = k[0]; break;
        case "lineDash": ctx.setLineDash(Array.from(k.subarray(0, count))); break;
        case "lineDashOffset": ctx.lineDashOffset = k[0]; break;
        case "globalAlpha": ctx.globalAlpha = k[0]; break;
        case "composite": ctx.globalCompositeOperation = COMPOSITE[k[0]]; break;
        case "linearGradient": this.gradients.set(k[0], ctx.createLinearGradient(k[1], k[2], k[3], k[4])); break;
        case "radialGradient": this.gradients.set(k[0], ctx.createRadialGradient(k[1], k[2], k[3], k[4], k[5], k[6])); break;
        case "colorStop": this.gradients.get(k[0])?.addColorStop(k[1], rgba(2)); break;
        case "beginPath": ctx.beginPath(); break;
        case "moveTo": ctx.moveTo(k[0], k[1]); break;
        case "lineTo": ctx.lineTo(k[0], k[1]); break;
        case "quadTo": ctx.quadraticCurveTo(k[0], k[1], k[2], k[3]); break;
        case "cubicTo": ctx.bezierCurveTo(k[0], k[1], k[2], k[3], k[4], k[5]); break;
        case "closePath": ctx.closePath(); break;
        case "fill": { const rule = k[0] === 1 ? "evenodd" : "nonzero"; this.paint(c => c.fill(rule)); break; }
        case "stroke": this.paint(c => c.stroke()); break;
        case "clip": ctx.clip(k[0] === 1 ? "evenodd" : "nonzero"); break;
        case "fillRect": { const [x, y, w, h] = k; this.paint(c => c.fillRect(x, y, w, h)); break; }
        case "strokeRect": { const [x, y, w, h] = k; this.paint(c => c.strokeRect(x, y, w, h)); break; }
        case "clearRect": { const [x, y, w, h] = k; this.paint(c => c.clearRect(x, y, w, h)); break; }
      }
    }
    return at === view.byteLength;
  }
}

globalThis.exact.canvas2dGlue = function canvas2dGlue(o) {
  const exact = globalThis.exact;
  const canvases = new Map(); // view id -> Replayer
  const reported = new Map(); // view id -> "w h scale"
  const waiting = new Set(); // watched views whose first geometry is not in yet
  let settle = null, frames = false, framing = false;
  const agentMode = new URL(location.href).searchParams.has("agent");
  const surfaceOf = (el) => el && [...el.children].find(c => c.dataset && c.dataset.surface !== undefined);
  function report(id, el, width, height) {
    const scale = devicePixelRatio || 1, key = `${width} ${height} ${scale}`;
    waiting.delete(id);
    if (reported.get(id) === key) return;
    reported.set(id, key);
    // The bitmap sits in the content box, clipped to the box's curve.
    const surface = surfaceOf(el), cs = getComputedStyle(el);
    if (surface) {
      // A replaced element takes its bitmap's size unless told the box's:
      // an explicit bitmap is stretched to it (LLP 1056 D6).
      surface.style.inset = `${cs.paddingTop} auto auto ${cs.paddingLeft}`;
      surface.style.width = `${width}px`; surface.style.height = `${height}px`;
      surface.style.borderRadius = "inherit";
    }
    exact.send(exact.wasm.exact_canvas_geometry(id, width, height, scale));
  }
  const observer = new ResizeObserver(entries => {
    for (const e of entries) {
      const id = Number(e.target.dataset.view), box = e.contentBoxSize?.[0];
      report(id, e.target, box ? box.inlineSize : e.contentRect.width, box ? box.blockSize : e.contentRect.height);
    }
    if (!waiting.size && settle) { settle.resolve(); settle = null; }
  });
  // A display-scale change is a new generation (D4): every canvas reports again.
  const rescale = () => matchMedia(`(resolution: ${devicePixelRatio}dppx)`).addEventListener("change", () => {
    for (const id of reported.keys()) { const el = o.views.get(id); if (el) { observer.unobserve(el); observer.observe(el); } }
    rescale();
  }, { once: true });
  rescale();
  function frame() {
    framing = false;
    if (!frames || agentMode) return;
    exact.send(exact.wasm.exact_advance(o.now(), 0));
    if (frames && !framing) { framing = true; requestAnimationFrame(frame); }
  }
  return {
    op(op) {
      if (op.frames !== undefined && op.id === undefined) {
        frames = op.frames;
        if (frames && !framing && !agentMode) { framing = true; requestAnimationFrame(frame); }
        return;
      }
      const el = o.views.get(op.id);
      if (!el) return;
      if (op.watch) { waiting.add(op.id); observer.observe(el); return; }
      const surface = surfaceOf(el);
      if (!surface) return;
      if (op.fresh) canvases.set(op.id, new Replayer(surface, op));
      const r = canvases.get(op.id);
      if (!r || r.lifetime !== op.lifetime || r.generation !== op.generation) return;
      for (const text of op.lists ?? []) if (!r.apply(decode(text))) console.error(`exact: canvas ${op.id}: unreadable list`);
    },
    /** Resolves once every watched canvas has reported its first geometry. */
    settled() {
      if (!waiting.size) return Promise.resolve();
      if (!settle) { let resolve; const promise = new Promise(r => { resolve = r; }); settle = { promise, resolve }; }
      return settle.promise;
    },
  };
};
