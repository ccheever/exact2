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
const AGENT_ADMITTED = true; // false in a production bake: host/web/build.mjs rewrites this line (LLP 1069.007 D2)
const OPS = {
  1: "save", 2: "restore", 3: "reset", 4: "setTransform",
  10: "fillColor", 11: "fillGradient", 12: "strokeColor", 13: "strokeGradient",
  14: "lineWidth", 15: "lineCap", 16: "lineJoin", 17: "miterLimit", 18: "lineDash", 19: "lineDashOffset",
  20: "globalAlpha", 21: "composite", 22: "shadowColor", 23: "shadowBlur", 24: "shadowOffset", 25: "smoothing",
  30: "linearGradient", 31: "radialGradient", 32: "colorStop", 33: "conicGradient",
  34: "pattern", 35: "patternTransform", 36: "fillPattern", 37: "strokePattern",
  40: "beginPath", 41: "moveTo", 42: "lineTo", 43: "quadTo", 44: "cubicTo", 45: "closePath",
  50: "fill", 51: "stroke", 52: "clip", 53: "fillRect", 54: "strokeRect", 55: "clearRect",
  60: "font", 61: "fillText", 62: "strokeText", 70: "image", 71: "drawImage", 72: "putImageData",
  80: "pMoveTo", 81: "pLineTo", 82: "pQuadTo", 83: "pCubicTo", 84: "pClose", 85: "fillPath", 86: "strokePath", 87: "clipPath",
};
const COMPOSITE = ["source-over", "source-in", "source-out", "source-atop", "destination-over", "destination-in",
  "destination-out", "destination-atop", "lighter", "copy", "xor", "multiply", "screen", "overlay", "darken", "lighten",
  "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"];
const CAPS = ["butt", "round", "square"], JOINS = ["miter", "round", "bevel"];
const VARIANT_CAPS = ["normal", "small-caps", "all-small-caps", "petite-caps", "all-petite-caps", "unicase", "titling-caps"];
const KERNING = ["auto", "normal", "none"], RENDERING = ["auto", "optimizeSpeed", "optimizeLegibility", "geometricPrecision"];
const STRETCH = { 50: "ultra-condensed", 62.5: "extra-condensed", 75: "condensed", 87.5: "semi-condensed", 100: "normal",
  112.5: "semi-expanded", 125: "expanded", 150: "extra-expanded", 200: "ultra-expanded" };
const GENERIC = new Set(["serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded", "math", "emoji", "fangsong"]);
const REPEAT = ["repeat", "repeat-x", "repeat-y", "no-repeat"];
const MAGIC = 0x44324345;

function decode(text) {
  const bin = atob(text), bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new DataView(bytes.buffer);
}
const textOf = (k, from, count) => { let s = ""; for (let i = from; i < count; i++) s += String.fromCodePoint(k[i]); return s; };
// A declared family is registered under its stack's alias (host.rs `font_faces`).
const families = (list) => list.split(",").map((f) => (GENERIC.has(f) ? f : `"${(globalThis.exact.fontAliases?.[f] ?? f).replace(/"/g, '\\"')}"`)).join(", ");

/** The CSS font for a list's `Font` record (canvas/src/list.rs). */
function fontCss(k, count) {
  const [size, weight, style] = k;
  return `${style === 1 ? "italic " : style === 2 ? "oblique " : ""}${weight} ${size}px ${families(textOf(k, 9, count))}`;
}

// The page's decoded image handles (LLP 1056 D9): src -> { el, w, h, ok, subscribers }.
const images = new Map();

/** One canvas: the element, its context, and what the replayer keeps. */
class Replayer {
  constructor(el, op) {
    this.el = el; this.lifetime = op.lifetime; this.generation = op.generation;
    // Assigning the bitmap's size clears it and resets the context (HTML).
    el.width = op.w; el.height = op.h;
    this.ctx = el.getContext("2d");
    this.base = op.scale; this.author = [1, 0, 0, 1, 0, 0]; this.stack = []; this.objects = new Map(); this.srcs = new Map();
    this.scratch = new Path2D();
    this.ctx.setTransform(this.base, 0, 0, this.base, 0, 0);
  }
  paint(f, extra) {
    const [a, b, c, d, e, g] = this.author, s = this.base;
    this.ctx.setTransform(s * a, s * b, s * c, s * d, s * e, s * g);
    if (extra) this.ctx.transform(...extra);
    f(this.ctx);
    this.ctx.setTransform(s, 0, 0, s, 0, 0);
  }
  /** The scratch path in user space, for a stroke under the author matrix. */
  userPath(path) {
    const [a, b, c, d, e, f] = this.author, det = a * d - b * c;
    if (!det) return null;
    const p = new Path2D();
    p.addPath(path, { a: d / det, b: -b / det, c: -c / det, d: a / det, e: (c * f - d * e) / det, f: (b * e - a * f) / det });
    return p;
  }
  image(id) { const src = this.srcs.get(id); const entry = src === undefined ? null : images.get(src); return entry?.ok ? entry.el : null; }
  apply(view) {
    if (view.byteLength < 8 || view.getUint32(0, true) !== MAGIC || view.getUint32(4, true) !== 1) return false;
    const ctx = this.ctx;
    let n = new Float64Array(64);
    const rgba = (i) => `rgba(${n[i]}, ${n[i + 1]}, ${n[i + 2]}, ${n[i + 3]})`;
    let at = 8;
    while (at + 8 <= view.byteLength) {
      const code = view.getUint32(at, true), count = view.getUint32(at + 4, true);
      at += 8;
      if (at + count * 8 > view.byteLength || !OPS[code]) return false;
      if (count > n.length) n = new Float64Array(count);
      const k = n;
      for (let i = 0; i < count; i++) k[i] = view.getFloat64(at + i * 8, true);
      at += count * 8;
      switch (OPS[code]) {
        case "save": this.stack.push(this.author); ctx.save(); break;
        case "restore": if (this.stack.length) { this.author = this.stack.pop(); ctx.restore(); } break;
        case "reset":
          this.stack.length = 0; this.author = [1, 0, 0, 1, 0, 0]; this.scratch = new Path2D();
          if (ctx.reset) ctx.reset(); else this.el.width = this.el.width;
          ctx.setTransform(this.base, 0, 0, this.base, 0, 0);
          break;
        case "setTransform": this.author = [k[0], k[1], k[2], k[3], k[4], k[5]]; break;
        case "fillColor": ctx.fillStyle = rgba(0); break;
        case "strokeColor": ctx.strokeStyle = rgba(0); break;
        case "fillGradient": case "fillPattern": ctx.fillStyle = this.objects.get(k[0]) ?? "transparent"; break;
        case "strokeGradient": case "strokePattern": ctx.strokeStyle = this.objects.get(k[0]) ?? "transparent"; break;
        case "lineWidth": ctx.lineWidth = k[0]; break;
        case "lineCap": ctx.lineCap = CAPS[k[0]]; break;
        case "lineJoin": ctx.lineJoin = JOINS[k[0]]; break;
        case "miterLimit": ctx.miterLimit = k[0]; break;
        case "lineDash": ctx.setLineDash(Array.from(k.subarray(0, count))); break;
        case "lineDashOffset": ctx.lineDashOffset = k[0]; break;
        case "globalAlpha": ctx.globalAlpha = k[0]; break;
        case "composite": ctx.globalCompositeOperation = COMPOSITE[k[0]]; break;
        // Shadows are in bitmap pixels whatever the transform (HTML): canvas
        // units times the base scale.
        case "shadowColor": ctx.shadowColor = rgba(0); break;
        case "shadowBlur": ctx.shadowBlur = k[0] * this.base; break;
        case "shadowOffset": ctx.shadowOffsetX = k[0] * this.base; ctx.shadowOffsetY = k[1] * this.base; break;
        case "smoothing": ctx.imageSmoothingEnabled = k[0] === 1; ctx.imageSmoothingQuality = ["low", "medium", "high"][k[1]]; break;
        case "linearGradient": this.objects.set(k[0], ctx.createLinearGradient(k[1], k[2], k[3], k[4])); break;
        case "radialGradient": this.objects.set(k[0], ctx.createRadialGradient(k[1], k[2], k[3], k[4], k[5], k[6])); break;
        case "conicGradient": this.objects.set(k[0], ctx.createConicGradient(k[1], k[2], k[3])); break;
        case "colorStop": this.objects.get(k[0])?.addColorStop(k[1], rgba(2)); break;
        case "image": this.srcs.set(k[0], textOf(k, 1, count)); break;
        case "pattern": { const el = this.image(k[1]); this.objects.set(k[0], el ? ctx.createPattern(el, REPEAT[k[2]]) : null); break; }
        case "patternTransform": this.objects.get(k[0])?.setTransform({ a: k[1], b: k[2], c: k[3], d: k[4], e: k[5], f: k[6] }); break;
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
        case "font": {
          ctx.font = fontCss(k, count);
          ctx.fontStretch = STRETCH[k[3]] ?? "normal"; ctx.fontVariantCaps = VARIANT_CAPS[k[4]] ?? "normal";
          ctx.fontKerning = KERNING[k[5]] ?? "auto"; ctx.textRendering = RENDERING[k[6]] ?? "auto";
          ctx.letterSpacing = `${k[7]}px`; ctx.wordSpacing = `${k[8]}px`;
          break;
        }
        case "fillText": case "strokeText": {
          // The run's left end on its alphabetic baseline, squeezed by maxWidth.
          const text = textOf(k, 4, count), fill = OPS[code] === "fillText";
          ctx.textAlign = "left"; ctx.textBaseline = "alphabetic"; ctx.direction = k[3] === 1 ? "rtl" : "ltr";
          this.paint(c => (fill ? c.fillText(text, 0, 0) : c.strokeText(text, 0, 0)), [k[2], 0, 0, 1, k[0], k[1]]);
          break;
        }
        case "drawImage": {
          const el = this.image(k[0]);
          if (el) { const a = Array.from(k.subarray(1, 9)); this.paint(c => c.drawImage(el, ...a)); }
          break;
        }
        case "putImageData": {
          const [x, y, w, h] = k, px = new Uint8ClampedArray(w * h * 4);
          for (let i = 0; i < w * h; i++) { const v = k[4 + i]; px[i * 4] = Math.floor(v / 16777216); px[i * 4 + 1] = (v >>> 16) & 255; px[i * 4 + 2] = (v >>> 8) & 255; px[i * 4 + 3] = v & 255; }
          ctx.putImageData(new ImageData(px, w, h), x, y);
          break;
        }
        case "pMoveTo": this.scratch.moveTo(k[0], k[1]); break;
        case "pLineTo": this.scratch.lineTo(k[0], k[1]); break;
        case "pQuadTo": this.scratch.quadraticCurveTo(k[0], k[1], k[2], k[3]); break;
        case "pCubicTo": this.scratch.bezierCurveTo(k[0], k[1], k[2], k[3], k[4], k[5]); break;
        case "pClose": this.scratch.closePath(); break;
        case "fillPath": {
          // Canvas coordinates under the base, but styles in user space: the
          // path taken back through the author matrix, painted under it.
          const p = this.userPath(this.scratch), rule = k[0] === 1 ? "evenodd" : "nonzero";
          if (p) this.paint(c => c.fill(p, rule));
          this.scratch = new Path2D(); break;
        }
        case "strokePath": { const p = this.userPath(this.scratch); if (p) this.paint(c => c.stroke(p)); this.scratch = new Path2D(); break; }
        case "clipPath": ctx.clip(this.scratch, k[0] === 1 ? "evenodd" : "nonzero"); this.scratch = new Path2D(); break;
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
  const agentMode = AGENT_ADMITTED && new URL(location.href).searchParams.has("agent");
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
  // Image handles (LLP 1056 D9): loaded as an `image` node's src resolves,
  // then the runner is told, and the canvases that asked draw again.
  function load(src, subscriber) {
    let entry = images.get(src);
    if (entry) { if (subscriber !== undefined && !entry.done) entry.subscribers.add(subscriber); return entry; }
    entry = { el: new Image(), ok: false, done: false, subscribers: new Set(subscriber === undefined ? [] : [subscriber]) };
    images.set(src, entry);
    const finish = (ok) => {
      entry.done = true; entry.ok = ok && entry.el.naturalWidth > 0 && entry.el.naturalHeight > 0;
      const payload = [src, entry.el.naturalWidth, entry.el.naturalHeight, entry.ok ? 1 : 0, JSON.stringify([...entry.subscribers])].join("\0");
      exact.send(exact.wasm.exact_canvas_image(exact.writeIn(payload)));
    };
    entry.el.onload = () => (entry.el.decode ? entry.el.decode().then(() => finish(true), () => finish(false)) : finish(true));
    entry.el.onerror = () => finish(false);
    entry.el.src = exact.assetURL ? exact.assetURL(src) : src;
    return entry;
  }
  // Text is measured on the page, whose document has the app's faces (LLP
  // 1056 D8): the run at left/alphabetic, and the em box from "top" and
  // "bottom" (Chrome has no emHeight* yet).
  const scratch = document.createElement("canvas").getContext("2d");
  function measure(json) {
    const r = JSON.parse(json), f = r.font, c = scratch;
    c.font = `${f[2] === 1 ? "italic " : f[2] === 2 ? "oblique " : ""}${f[1]} ${f[0]}px ${families(r.families)}`;
    c.fontStretch = STRETCH[f[3]] ?? "normal"; c.fontVariantCaps = VARIANT_CAPS[f[4]] ?? "normal";
    c.fontKerning = KERNING[r.kerning] ?? "auto"; c.letterSpacing = `${r.ls}px`; c.wordSpacing = `${r.ws}px`;
    c.textAlign = "left"; c.direction = r.rtl ? "rtl" : "ltr";
    c.textBaseline = "alphabetic";
    const m = c.measureText(r.text);
    c.textBaseline = "top"; const top = c.measureText(r.text).alphabeticBaseline;
    c.textBaseline = "bottom"; const bottom = c.measureText(r.text).alphabeticBaseline;
    return JSON.stringify([m.width, m.actualBoundingBoxLeft, m.actualBoundingBoxRight, m.actualBoundingBoxAscent, m.actualBoundingBoxDescent,
      m.fontBoundingBoxAscent, m.fontBoundingBoxDescent, -top, bottom, m.hangingBaseline, m.ideographicBaseline]);
  }
  function image(json) {
    const r = JSON.parse(json), entry = load(r.src, r.canvas);
    return entry.ok ? JSON.stringify([entry.el.naturalWidth, entry.el.naturalHeight]) : "";
  }
  exact.canvas2dHost = { measure, image };
  document.fonts?.addEventListener("loadingdone", () => exact.send(exact.wasm.exact_canvas_fonts()));
  return {
    op(op) {
      if (op.images) { for (const src of op.images) load(src); return; }
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
