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

// A colour record's CSS text, kept for the colours a list repeats (the
// context parses it again on assignment either way).
const colors = new Map();
function colorText(r, g, b, a) {
  if ((r & 255) !== r || (g & 255) !== g || (b & 255) !== b) return `rgba(${r}, ${g}, ${b}, ${a})`;
  const key = r * 16777216 + g * 65536 + b * 256;
  let byAlpha = colors.get(key);
  if (!byAlpha) { if (colors.size > 4096) colors.clear(); colors.set(key, byAlpha = new Map()); }
  let text = byAlpha.get(a);
  if (text === undefined) { text = `rgba(${r}, ${g}, ${b}, ${a})`; if (byAlpha.size < 64) byAlpha.set(a, text); }
  return text;
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
    // Its getContext settings (LLP 1100 D12a), which an element keeps from
    // its first context: other settings take a new element.
    const settings = { colorSpace: op.p3 ? "display-p3" : "srgb", colorType: op.float16 ? "float16" : "unorm8" }, key = `${settings.colorSpace} ${settings.colorType}`;
    if (el.exactSettings !== undefined && el.exactSettings !== key) { const fresh = el.cloneNode(false); el.replaceWith(fresh); el = fresh; }
    el.exactSettings = key;
    this.el = el; this.lifetime = op.lifetime; this.generation = op.generation; this.p3 = op.p3 === true;
    // Assigning the bitmap's size clears it and resets the context (HTML).
    el.width = op.w; el.height = op.h;
    this.ctx = el.getContext("2d", settings);
    this.base = op.scale; this.author = [1, 0, 0, 1, 0, 0]; this.plain = true; this.stack = []; this.objects = new Map(); this.srcs = new Map();
    this.scratch = new Path2D();
    this.ctx.setTransform(this.base, 0, 0, this.base, 0, 0);
  }
  /** The author matrix, and whether it is the identity (a paint then needs
   *  no transform of its own). */
  setAuthor(m) { this.author = m; this.plain = m[0] === 1 && m[1] === 0 && m[2] === 0 && m[3] === 1 && m[4] === 0 && m[5] === 0; }
  paint(f, extra) {
    if (this.plain && !extra) { f(this.ctx); return; }
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
    // A `display-p3` canvas's colours are Display P3 bytes.
    const rgba = this.p3 ? (i) => `color(display-p3 ${n[i] / 255} ${n[i + 1] / 255} ${n[i + 2] / 255} / ${n[i + 3]})` : (i) => colorText(n[i], n[i + 1], n[i + 2], n[i + 3]);
    // A list in wasm memory is 8-aligned, as its records are: read it
    // through typed arrays over the memory itself (little-endian, as wasm
    // is); a DataView otherwise.
    const aligned = view.byteOffset % 8 === 0 && view.byteLength % 8 === 0;
    const words = aligned ? new Uint32Array(view.buffer, view.byteOffset, view.byteLength >> 2) : null;
    const doubles = aligned ? new Float64Array(view.buffer, view.byteOffset, view.byteLength >> 3) : null;
    let at = 8;
    while (at + 8 <= view.byteLength) {
      const code = aligned ? words[at >> 2] : view.getUint32(at, true), count = aligned ? words[(at >> 2) + 1] : view.getUint32(at + 4, true);
      at += 8;
      if (at + count * 8 > view.byteLength || !OPS[code]) return false;
      if (count > n.length) n = new Float64Array(count);
      const k = n;
      if (aligned) { const from = at >> 3; for (let i = 0; i < count; i++) k[i] = doubles[from + i]; }
      else for (let i = 0; i < count; i++) k[i] = view.getFloat64(at + i * 8, true);
      at += count * 8;
      switch (code) { // numbers: a jump table, where names compare one by one
        case 1: /* save */ this.stack.push(this.author); ctx.save(); break;
        case 2: /* restore */ if (this.stack.length) { this.setAuthor(this.stack.pop()); ctx.restore(); } break;
        case 3: /* reset */
          this.stack.length = 0; this.setAuthor([1, 0, 0, 1, 0, 0]); this.scratch = new Path2D();
          if (ctx.reset) ctx.reset(); else this.el.width = this.el.width;
          ctx.setTransform(this.base, 0, 0, this.base, 0, 0);
          break;
        case 4: /* setTransform */ this.setAuthor([k[0], k[1], k[2], k[3], k[4], k[5]]); break;
        case 10: /* fillColor */ ctx.fillStyle = rgba(0); break;
        case 12: /* strokeColor */ ctx.strokeStyle = rgba(0); break;
        case 11: /* fillGradient */ case 36: /* fillPattern */ ctx.fillStyle = this.objects.get(k[0]) ?? "transparent"; break;
        case 13: /* strokeGradient */ case 37: /* strokePattern */ ctx.strokeStyle = this.objects.get(k[0]) ?? "transparent"; break;
        case 14: /* lineWidth */ ctx.lineWidth = k[0]; break;
        case 15: /* lineCap */ ctx.lineCap = CAPS[k[0]]; break;
        case 16: /* lineJoin */ ctx.lineJoin = JOINS[k[0]]; break;
        case 17: /* miterLimit */ ctx.miterLimit = k[0]; break;
        case 18: /* lineDash */ ctx.setLineDash(Array.from(k.subarray(0, count))); break;
        case 19: /* lineDashOffset */ ctx.lineDashOffset = k[0]; break;
        case 20: /* globalAlpha */ ctx.globalAlpha = k[0]; break;
        case 21: /* composite */ ctx.globalCompositeOperation = COMPOSITE[k[0]]; break;
        // Shadows are in bitmap pixels whatever the transform (HTML): canvas
        // units times the base scale.
        case 22: /* shadowColor */ ctx.shadowColor = rgba(0); break;
        case 23: /* shadowBlur */ ctx.shadowBlur = k[0] * this.base; break;
        case 24: /* shadowOffset */ ctx.shadowOffsetX = k[0] * this.base; ctx.shadowOffsetY = k[1] * this.base; break;
        case 25: /* smoothing */ ctx.imageSmoothingEnabled = k[0] === 1; ctx.imageSmoothingQuality = ["low", "medium", "high"][k[1]]; break;
        case 30: /* linearGradient */ this.objects.set(k[0], ctx.createLinearGradient(k[1], k[2], k[3], k[4])); break;
        case 31: /* radialGradient */ this.objects.set(k[0], ctx.createRadialGradient(k[1], k[2], k[3], k[4], k[5], k[6])); break;
        case 33: /* conicGradient */ this.objects.set(k[0], ctx.createConicGradient(k[1], k[2], k[3])); break;
        case 32: /* colorStop */ this.objects.get(k[0])?.addColorStop(k[1], rgba(2)); break;
        case 70: /* image */ this.srcs.set(k[0], textOf(k, 1, count)); break;
        case 34: /* pattern */ { const el = this.image(k[1]); this.objects.set(k[0], el ? ctx.createPattern(el, REPEAT[k[2]]) : null); break; }
        case 35: /* patternTransform */ this.objects.get(k[0])?.setTransform({ a: k[1], b: k[2], c: k[3], d: k[4], e: k[5], f: k[6] }); break;
        case 40: /* beginPath */ ctx.beginPath(); break;
        case 41: /* moveTo */ ctx.moveTo(k[0], k[1]); break;
        case 42: /* lineTo */ ctx.lineTo(k[0], k[1]); break;
        case 43: /* quadTo */ ctx.quadraticCurveTo(k[0], k[1], k[2], k[3]); break;
        case 44: /* cubicTo */ ctx.bezierCurveTo(k[0], k[1], k[2], k[3], k[4], k[5]); break;
        case 45: /* closePath */ ctx.closePath(); break;
        case 50: /* fill */ { const rule = k[0] === 1 ? "evenodd" : "nonzero"; if (this.plain) ctx.fill(rule); else this.paint(c => c.fill(rule)); break; }
        case 51: /* stroke */ if (this.plain) ctx.stroke(); else this.paint(c => c.stroke()); break;
        case 52: /* clip */ ctx.clip(k[0] === 1 ? "evenodd" : "nonzero"); break;
        case 53: /* fillRect */ { const [x, y, w, h] = k; this.paint(c => c.fillRect(x, y, w, h)); break; }
        case 54: /* strokeRect */ { const [x, y, w, h] = k; this.paint(c => c.strokeRect(x, y, w, h)); break; }
        case 55: /* clearRect */ { const [x, y, w, h] = k; this.paint(c => c.clearRect(x, y, w, h)); break; }
        case 60: /* font */ {
          ctx.font = fontCss(k, count);
          ctx.fontStretch = STRETCH[k[3]] ?? "normal"; ctx.fontVariantCaps = VARIANT_CAPS[k[4]] ?? "normal";
          ctx.fontKerning = KERNING[k[5]] ?? "auto"; ctx.textRendering = RENDERING[k[6]] ?? "auto";
          ctx.letterSpacing = `${k[7]}px`; ctx.wordSpacing = `${k[8]}px`;
          break;
        }
        case 61: /* fillText */ case 62: /* strokeText */ {
          // The run's left end on its alphabetic baseline, squeezed by maxWidth.
          const text = textOf(k, 4, count), fill = code === 61;
          ctx.textAlign = "left"; ctx.textBaseline = "alphabetic"; ctx.direction = k[3] === 1 ? "rtl" : "ltr";
          this.paint(c => (fill ? c.fillText(text, 0, 0) : c.strokeText(text, 0, 0)), [k[2], 0, 0, 1, k[0], k[1]]);
          break;
        }
        case 71: /* drawImage */ {
          const el = this.image(k[0]);
          if (el) { const a = Array.from(k.subarray(1, 9)); this.paint(c => c.drawImage(el, ...a)); }
          break;
        }
        case 72: /* putImageData */ {
          const [x, y, w, h] = k, px = new Uint8ClampedArray(w * h * 4);
          for (let i = 0; i < w * h; i++) { const v = k[4 + i]; px[i * 4] = Math.floor(v / 16777216); px[i * 4 + 1] = (v >>> 16) & 255; px[i * 4 + 2] = (v >>> 8) & 255; px[i * 4 + 3] = v & 255; }
          ctx.putImageData(new ImageData(px, w, h, { colorSpace: this.p3 ? "display-p3" : "srgb" }), x, y);
          break;
        }
        case 80: /* pMoveTo */ this.scratch.moveTo(k[0], k[1]); break;
        case 81: /* pLineTo */ this.scratch.lineTo(k[0], k[1]); break;
        case 82: /* pQuadTo */ this.scratch.quadraticCurveTo(k[0], k[1], k[2], k[3]); break;
        case 83: /* pCubicTo */ this.scratch.bezierCurveTo(k[0], k[1], k[2], k[3], k[4], k[5]); break;
        case 84: /* pClose */ this.scratch.closePath(); break;
        case 85: /* fillPath */ {
          // Canvas coordinates under the base, but styles in user space: the
          // path taken back through the author matrix, painted under it.
          const p = this.userPath(this.scratch), rule = k[0] === 1 ? "evenodd" : "nonzero";
          if (p) this.paint(c => c.fill(p, rule));
          this.scratch = new Path2D(); break;
        }
        case 86: /* strokePath */ { const p = this.userPath(this.scratch); if (p) this.paint(c => c.stroke(p)); this.scratch = new Path2D(); break; }
        case 87: /* clipPath */ ctx.clip(this.scratch, k[0] === 1 ? "evenodd" : "nonzero"); this.scratch = new Path2D(); break;
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
      // Each list is `[address, length]` in the wasm's memory, read in place.
      for (const [at, len] of op.lists ?? []) if (!r.apply(new DataView(exact.wasm.memory.buffer, at, len))) console.error(`exact: canvas ${op.id}: unreadable list`);
    },
    /** Resolves once every watched canvas has reported its first geometry. */
    settled() {
      if (!waiting.size) return Promise.resolve();
      if (!settle) { let resolve; const promise = new Promise(r => { resolve = r; }); settle = { promise, resolve }; }
      return settle.promise;
    },
  };
};
