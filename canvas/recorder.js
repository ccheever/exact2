// The TypeScript recorder (LLP 1056 D3): the context a data module's `draw`
// calls, in the executor. It is the Rust recorder (`canvas/src/context.rs`)
// in JavaScript — the same rules at the call, the same state, the same list
// bytes (`canvas/src/list.rs`) — and `canvas/tests/cases.txt` holds both to
// Chrome's answers. The bake bundles it only into a module that exports
// `draw`. Plain ES2020: it runs under Hermes natively and in the page's
// module realm on the web.

const MAGIC = 0x44324345, VERSION = 1, SEAL = 1 << 20;
const OP = {
  Save: 1, Restore: 2, Reset: 3, SetTransform: 4,
  FillColor: 10, FillGradient: 11, StrokeColor: 12, StrokeGradient: 13,
  LineWidth: 14, LineCap: 15, LineJoin: 16, MiterLimit: 17, LineDash: 18, LineDashOffset: 19,
  GlobalAlpha: 20, Composite: 21,
  LinearGradient: 30, RadialGradient: 31, ColorStop: 32,
  BeginPath: 40, MoveTo: 41, LineTo: 42, QuadTo: 43, CubicTo: 44, ClosePath: 45,
  Fill: 50, Stroke: 51, Clip: 52, FillRect: 53, StrokeRect: 54, ClearRect: 55,
};
export const COMPOSITE = ["source-over", "source-in", "source-out", "source-atop", "destination-over", "destination-in",
  "destination-out", "destination-atop", "lighter", "copy", "xor", "multiply", "screen", "overlay", "darken", "lighten",
  "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"];
const STAGE2_COMPOSITE = new Set([1, 2, 5, 7, 9]);
const CAPS = ["butt", "round", "square"], JOINS = ["miter", "round", "bevel"];
const TAU = Math.PI * 2, HALF_PI = Math.PI / 2;

class DOMExceptionLike extends Error {
  constructor(message, name) { super(message); this.name = name; }
}
function throwDom(name, message) {
  if (name === "TypeError") throw new TypeError(message);
  if (name === "RangeError") throw new RangeError(message);
  const Native = globalThis.DOMException;
  throw Native ? new Native(message, name) : new DOMExceptionLike(message, name);
}
const finite = (...v) => v.every(Number.isFinite);

// --- colours (canvas/src/color.rs) ------------------------------------------

const NAMED = ("aliceblue f0f8ff antiquewhite faebd7 aqua 00ffff aquamarine 7fffd4 azure f0ffff beige f5f5dc bisque ffe4c4 " +
  "black 000000 blanchedalmond ffebcd blue 0000ff blueviolet 8a2be2 brown a52a2a burlywood deb887 cadetblue 5f9ea0 " +
  "chartreuse 7fff00 chocolate d2691e coral ff7f50 cornflowerblue 6495ed cornsilk fff8dc crimson dc143c cyan 00ffff " +
  "darkblue 00008b darkcyan 008b8b darkgoldenrod b8860b darkgray a9a9a9 darkgreen 006400 darkgrey a9a9a9 darkkhaki bdb76b " +
  "darkmagenta 8b008b darkolivegreen 556b2f darkorange ff8c00 darkorchid 9932cc darkred 8b0000 darksalmon e9967a " +
  "darkseagreen 8fbc8f darkslateblue 483d8b darkslategray 2f4f4f darkslategrey 2f4f4f darkturquoise 00ced1 " +
  "darkviolet 9400d3 deeppink ff1493 deepskyblue 00bfff dimgray 696969 dimgrey 696969 dodgerblue 1e90ff " +
  "firebrick b22222 floralwhite fffaf0 forestgreen 228b22 fuchsia ff00ff gainsboro dcdcdc ghostwhite f8f8ff gold ffd700 " +
  "goldenrod daa520 gray 808080 green 008000 greenyellow adff2f grey 808080 honeydew f0fff0 hotpink ff69b4 " +
  "indianred cd5c5c indigo 4b0082 ivory fffff0 khaki f0e68c lavender e6e6fa lavenderblush fff0f5 lawngreen 7cfc00 " +
  "lemonchiffon fffacd lightblue add8e6 lightcoral f08080 lightcyan e0ffff lightgoldenrodyellow fafad2 lightgray d3d3d3 " +
  "lightgreen 90ee90 lightgrey d3d3d3 lightpink ffb6c1 lightsalmon ffa07a lightseagreen 20b2aa lightskyblue 87cefa " +
  "lightslategray 778899 lightslategrey 778899 lightsteelblue b0c4de lightyellow ffffe0 lime 00ff00 limegreen 32cd32 " +
  "linen faf0e6 magenta ff00ff maroon 800000 mediumaquamarine 66cdaa mediumblue 0000cd mediumorchid ba55d3 " +
  "mediumpurple 9370db mediumseagreen 3cb371 mediumslateblue 7b68ee mediumspringgreen 00fa9a mediumturquoise 48d1cc " +
  "mediumvioletred c71585 midnightblue 191970 mintcream f5fffa mistyrose ffe4e1 moccasin ffe4b5 navajowhite ffdead " +
  "navy 000080 oldlace fdf5e6 olive 808000 olivedrab 6b8e23 orange ffa500 orangered ff4500 orchid da70d6 " +
  "palegoldenrod eee8aa palegreen 98fb98 paleturquoise afeeee palevioletred db7093 papayawhip ffefd5 peachpuff ffdab9 " +
  "peru cd853f pink ffc0cb plum dda0dd powderblue b0e0e6 purple 800080 rebeccapurple 663399 red ff0000 " +
  "rosybrown bc8f8f royalblue 4169e1 saddlebrown 8b4513 salmon fa8072 sandybrown f4a460 seagreen 2e8b57 " +
  "seashell fff5ee sienna a0522d silver c0c0c0 skyblue 87ceeb slateblue 6a5acd slategray 708090 slategrey 708090 " +
  "snow fffafa springgreen 00ff7f steelblue 4682b4 tan d2b48c teal 008080 thistle d8bfd8 tomato ff6347 " +
  "turquoise 40e0d0 violet ee82ee wheat f5deb3 white ffffff whitesmoke f5f5f5 yellow ffff00 yellowgreen 9acd32").split(" ");

function hexColor(hex) {
  if (!/^[0-9a-f]+$/.test(hex)) return null;
  const d = (i) => parseInt(hex[i], 16) * 17, p = (i) => parseInt(hex.slice(i, i + 2), 16);
  if (hex.length === 3 || hex.length === 4) return [d(0), d(1), d(2), (hex.length === 4 ? d(3) : 255) / 255];
  if (hex.length === 6 || hex.length === 8) return [p(0), p(2), p(4), (hex.length === 8 ? p(6) : 255) / 255];
  return null;
}
function component(t) {
  if (t === "none") return { num: 0 };
  if (t.endsWith("%")) { const v = number(t.slice(0, -1)); return v === null ? null : { pct: v }; }
  for (const [unit, per] of [["deg", 1], ["grad", 0.9], ["rad", 180 / Math.PI], ["turn", 360]]) {
    if (t.endsWith(unit)) { const v = number(t.slice(0, -unit.length)); return v === null ? null : { num: v * per }; }
  }
  const v = number(t);
  return v === null ? null : { num: v };
}
function number(t) {
  if (!t || !/^[0-9.eE+-]+$/.test(t)) return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
}
const channel = (v) => Math.round(Math.min(255, Math.max(0, v)));
function alphaOf(t) {
  let a = 1;
  if (t !== undefined) { const c = component(t); if (!c) return null; a = c.pct !== undefined ? c.pct / 100 : c.num; }
  return Math.round(Math.min(1, Math.max(0, a)) * 255) / 255;
}
function hsl(h, s, l) {
  h = ((h % 360) + 360) % 360;
  const f = (n) => { const k = (n + h / 30) % 12, a = s * Math.min(l, 1 - l); return l - a * Math.min(Math.max(Math.min(k - 3, 9 - k), -1), 1); };
  return [f(0), f(8), f(4)];
}
function functional(name, body) {
  body = body.trim();
  const legacy = body.includes(",");
  let parts, alpha;
  if (legacy) {
    parts = body.split(",").map((p) => p.trim());
    if (parts.some((p) => !p || p.includes(" ") || p.includes("/"))) return null;
    if (parts.length === 4) alpha = parts.pop(); else if (parts.length !== 3) return null;
  } else {
    const [main, a] = body.includes("/") ? body.split("/") : [body, undefined];
    if (a !== undefined && !a.trim()) return null;
    alpha = a === undefined ? undefined : a.trim();
    parts = main.trim().split(/\s+/);
    if (parts.length !== 3) return null;
  }
  const c = parts.map(component);
  if (c.some((x) => !x)) return null;
  const a = alphaOf(alpha);
  if (a === null) return null;
  if (name === "rgb" || name === "rgba") {
    const pct = c.filter((x) => x.pct !== undefined).length;
    if (legacy && pct !== 0 && pct !== 3) return null;
    const v = (x) => (x.pct !== undefined ? (x.pct * 255) / 100 : x.num);
    return [channel(v(c[0])), channel(v(c[1])), channel(v(c[2])), a];
  }
  if (name === "hsl" || name === "hsla" || name === "hwb") {
    if (c[0].pct !== undefined) return null;
    const frac = (x) => (x.pct !== undefined ? Math.min(1, Math.max(0, x.pct / 100)) : !legacy ? Math.min(1, Math.max(0, x.num / 100)) : null);
    const s = frac(c[1]), l = frac(c[2]);
    if (s === null || l === null) return null;
    let rgb;
    if (name === "hwb") {
      if (legacy) return null;
      if (s + l >= 1) { const g = s / (s + l); rgb = [g, g, g]; }
      else rgb = hsl(c[0].num, 1, 0.5).map((x) => x * (1 - s - l) + s);
    } else rgb = hsl(c[0].num, s, l);
    return [channel(rgb[0] * 255), channel(rgb[1] * 255), channel(rgb[2] * 255), a];
  }
  return null;
}
/** A CSS colour as `[r, g, b, a]`, `"current"`, or null. */
export function parseColor(input) {
  const s = String(input).trim().toLowerCase();
  if (s === "currentcolor") return "current";
  if (s === "transparent") return [0, 0, 0, 0];
  if (s.startsWith("#")) return hexColor(s.slice(1));
  const open = s.indexOf("(");
  if (open >= 0) {
    if (!s.endsWith(")")) return null;
    return functional(s.slice(0, open).trim(), s.slice(open + 1, -1));
  }
  const i = NAMED.indexOf(s);
  return i >= 0 && i % 2 === 0 ? hexColor(NAMED[i + 1]) : null;
}
function alphaText(a) {
  const byte = Math.round(a * 255);
  if (byte <= 0) return "0";
  for (let places = 1; places <= 3; places++) {
    const scale = 10 ** places, v = Math.round((byte / 255) * scale) / scale;
    if (Math.round(v * 255) === byte) return String(Number(v.toFixed(places)));
  }
  return (byte / 255).toFixed(3);
}
/** The canvas serialisation of a colour. */
export function serializeColor(c) {
  const hex = (v) => v.toString(16).padStart(2, "0");
  return Math.round(c[3] * 255) >= 255 ? `#${hex(c[0])}${hex(c[1])}${hex(c[2])}` : `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${alphaText(c[3])})`;
}

// --- geometry (canvas/src/geom.rs) ------------------------------------------

const M = {
  identity: () => [1, 0, 0, 1, 0, 0],
  then: (s, m) => [s[0] * m[0] + s[2] * m[1], s[1] * m[0] + s[3] * m[1], s[0] * m[2] + s[2] * m[3], s[1] * m[2] + s[3] * m[3],
    s[0] * m[4] + s[2] * m[5] + s[4], s[1] * m[4] + s[3] * m[5] + s[5]],
  apply: (m, x, y) => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]],
  det: (m) => m[0] * m[3] - m[1] * m[2],
  invertible: (m) => m.every(Number.isFinite) && M.det(m) !== 0 && Number.isFinite(M.det(m)),
  invert(m) {
    if (!M.invertible(m)) return null;
    const k = 1 / M.det(m);
    return [m[3] * k, -m[1] * k, -m[2] * k, m[0] * k, (m[2] * m[5] - m[3] * m[4]) * k, (m[1] * m[4] - m[0] * m[5]) * k];
  },
};
function angles(start, end, anticlockwise) {
  let s = start % TAU;
  if (s < 0) s += TAU;
  let e = end + (s - start);
  if (!anticlockwise && e - s >= TAU) e = s + TAU;
  else if (anticlockwise && s - e >= TAU) e = s - TAU;
  else if (!anticlockwise && s > e) e = s + (TAU - ((s - e) % TAU));
  else if (anticlockwise && s < e) e = s - (TAU - ((e - s) % TAU));
  return [s, e];
}
/** `[first, segs]`: segs are ["L", x, y] or ["C", a, b, c, d, x, y] in user space. */
function ellipseSegs(cx, cy, rx, ry, rotation, start, end, anticlockwise) {
  const [s, e] = angles(start, end, anticlockwise);
  const sinR = Math.sin(rotation), cosR = Math.cos(rotation);
  const at = (t) => { const x = rx * Math.cos(t), y = ry * Math.sin(t); return [cx + x * cosR - y * sinR, cy + x * sinR + y * cosR]; };
  const tangent = (t) => { const x = -rx * Math.sin(t), y = ry * Math.cos(t); return [x * cosR - y * sinR, x * sinR + y * cosR]; };
  const first = at(s), out = [];
  if (rx === 0 || ry === 0) { const last = at(e); out.push(["L", last[0], last[1]]); return [first, out]; }
  const sweep = e - s;
  if (sweep === 0) return [first, out];
  const n = Math.max(1, Math.ceil(Math.abs(sweep) / HALF_PI - 1e-9));
  const step = sweep / n, k = (4 / 3) * Math.tan(step / 4);
  for (let i = 0; i < n; i++) {
    const t0 = s + step * i, t1 = s + step * (i + 1);
    const p0 = at(t0), p1 = i + 1 === n ? at(e) : at(t1), d0 = tangent(t0), d1 = tangent(t1);
    out.push(["C", p0[0] + k * d0[0], p0[1] + k * d0[1], p1[0] - k * d1[0], p1[1] - k * d1[1], p1[0], p1[1]]);
  }
  return [first, out];
}
function arcToSegs(p0, p1, p2, r) {
  if ((p0[0] === p1[0] && p0[1] === p1[1]) || (p1[0] === p2[0] && p1[1] === p2[1]) || r === 0) return null;
  const v1 = [p0[0] - p1[0], p0[1] - p1[1]], v2 = [p2[0] - p1[0], p2[1] - p1[1]];
  const cross = v1[0] * v2[1] - v1[1] * v2[0], l1 = Math.hypot(v1[0], v1[1]), l2 = Math.hypot(v2[0], v2[1]);
  if (Math.abs(cross) <= 1e-12 * l1 * l2) return null;
  const u1 = [v1[0] / l1, v1[1] / l1], u2 = [v2[0] / l2, v2[1] / l2];
  const cos = Math.min(1, Math.max(-1, u1[0] * u2[0] + u1[1] * u2[1]));
  const half = Math.acos(cos) / 2, dist = r / Math.tan(half);
  const t1 = [p1[0] + u1[0] * dist, p1[1] + u1[1] * dist], t2 = [p1[0] + u2[0] * dist, p1[1] + u2[1] * dist];
  const b = [u1[0] + u2[0], u1[1] + u2[1]], bl = Math.hypot(b[0], b[1]), h = r / Math.sin(half);
  const c = [p1[0] + (b[0] / bl) * h, p1[1] + (b[1] / bl) * h];
  const a0 = Math.atan2(t1[1] - c[1], t1[0] - c[0]), a1 = Math.atan2(t2[1] - c[1], t2[0] - c[0]);
  return [t1, ellipseSegs(c[0], c[1], r, r, 0, a0, a1, cross > 0)[1]];
}
function roundRectSegs(x, y, w, h, radii) {
  const r = (i) => ({ ...radii[i] });
  let [ul, ur, lr, ll] = radii.length === 1 ? [r(0), r(0), r(0), r(0)] : radii.length === 2 ? [r(0), r(1), r(0), r(1)]
    : radii.length === 3 ? [r(0), r(1), r(2), r(1)] : [r(0), r(1), r(2), r(3)];
  let x0 = x, y0 = y, ww = w, hh = h;
  if (w < 0) { x0 += w; ww = -w; [ul, ur] = [ur, ul]; [ll, lr] = [lr, ll]; }
  if (h < 0) { y0 += h; hh = -h; [ul, ll] = [ll, ul]; [ur, lr] = [lr, ur]; }
  let scale = 1;
  for (const [side, sum] of [[ww, ul.x + ur.x], [hh, ur.y + lr.y], [ww, lr.x + ll.x], [hh, ul.y + ll.y]]) if (sum > 0) scale = Math.min(scale, side / sum);
  if (scale < 1) for (const c of [ul, ur, lr, ll]) { c.x *= scale; c.y *= scale; }
  const k = 0.5522847498307934, x1 = x0 + ww, y1 = y0 + hh, pts = [];
  const corner = (from, to, c, horizontalFirst) => {
    if (c.x === 0 || c.y === 0) { pts.push(["L", to[0], to[1]]); return; }
    const [c1, c2] = horizontalFirst
      ? [[from[0] + (to[0] - from[0]) * k, from[1]], [to[0], to[1] - (to[1] - from[1]) * k]]
      : [[from[0], from[1] + (to[1] - from[1]) * k], [to[0] - (to[0] - from[0]) * k, to[1]]];
    pts.push(["C", c1[0], c1[1], c2[0], c2[1], to[0], to[1]]);
  };
  const start = [x0 + ul.x, y0];
  pts.push(["L", x1 - ur.x, y0]); corner([x1 - ur.x, y0], [x1, y0 + ur.y], ur, true);
  pts.push(["L", x1, y1 - lr.y]); corner([x1, y1 - lr.y], [x1 - lr.x, y1], lr, false);
  pts.push(["L", x0 + ll.x, y1]); corner([x0 + ll.x, y1], [x0, y1 - ll.y], ll, true);
  pts.push(["L", x0, y0 + ul.y]); corner([x0, y0 + ul.y], start, ul, false);
  if ((w < 0) !== (h < 0)) {
    const ends = [start, ...pts.map((p) => [p[p.length - 2], p[p.length - 1]])], out = [];
    for (let i = pts.length - 1; i >= 0; i--) {
      const from = ends[i], p = pts[i];
      out.push(p[0] === "L" ? ["L", from[0], from[1]] : ["C", p[3], p[4], p[1], p[2], from[0], from[1]]);
    }
    return [ends[ends.length - 1], out];
  }
  return [start, pts];
}

// --- the list writer (canvas/src/list.rs) ------------------------------------

class Writer {
  constructor() { this.buf = new ArrayBuffer(256); this.view = new DataView(this.buf); this.len = 8; this.ops = 0; this.view.setUint32(0, MAGIC, true); this.view.setUint32(4, VERSION, true); }
  op(code, operands) {
    const need = this.len + 8 + operands.length * 8;
    if (need > this.buf.byteLength) {
      const grown = new ArrayBuffer(Math.max(need, this.buf.byteLength * 2));
      new Uint8Array(grown).set(new Uint8Array(this.buf, 0, this.len));
      this.buf = grown; this.view = new DataView(grown);
    }
    this.view.setUint32(this.len, code, true); this.view.setUint32(this.len + 4, operands.length, true);
    for (let i = 0; i < operands.length; i++) this.view.setFloat64(this.len + 8 + i * 8, operands[i], true);
    this.len = need; this.ops++;
  }
  finish() { return new Uint8Array(this.buf, 0, this.len); }
}

// --- the recorder (canvas/src/context.rs) ------------------------------------

const BLACK = [0, 0, 0, 1];
const defaults = () => ({ transform: M.identity(), fill: { color: BLACK }, stroke: { color: BLACK }, lineWidth: 1, cap: 0, join: 0,
  miter: 10, dash: [], dashOffset: 0, alpha: 1, composite: 0 });
const clone = (s) => ({ ...s, transform: s.transform.slice(), dash: s.dash.slice() });
const samePaint = (a, b) => (a.gradient !== undefined ? a.gradient === b.gradient : b.color !== undefined && a.color.every((v, i) => v === b.color[i]));

/** `CanvasGradient`: an id in its context's list. */
export class CanvasGradient {
  constructor(inner, id) { Object.defineProperty(this, "_inner", { value: inner }); Object.defineProperty(this, "_id", { value: id }); }
  addColorStop(offset, color) {
    offset = Number(offset);
    if (!Number.isFinite(offset)) throwDom("TypeError", "The provided double value is non-finite.");
    if (offset < 0 || offset > 1) throwDom("IndexSizeError", `The provided value (${offset}) is outside the range (0.0, 1.0).`);
    let c = parseColor(String(color));
    if (c === "current") c = BLACK;
    if (!c) throwDom("SyntaxError", `The value provided ('${color}') could not be parsed as a color.`);
    this._inner.op(OP.ColorStop, [this._id, offset, ...c]);
  }
  get [Symbol.toStringTag]() { return "CanvasGradient"; }
}

class Inner {
  constructor() { this.state = defaults(); this.stack = []; this.writer = new Writer(); this.sealed = []; this.subpath = null; this.nextGradient = 0; this.currentColor = null; this.notes = []; }
  op(code, operands) {
    if (this.writer.len + 8 + operands.length * 8 > SEAL && this.writer.ops) { this.sealed.push(this.writer.finish()); this.writer = new Writer(); }
    this.writer.op(code, operands);
  }
  paintOp(fill, p) {
    if (p.gradient !== undefined) this.op(fill ? OP.FillGradient : OP.StrokeGradient, [p.gradient]);
    else this.op(fill ? OP.FillColor : OP.StrokeColor, p.color);
  }
  setTransform(m) {
    if (!m.every((v, i) => v === this.state.transform[i])) { this.state.transform = m; this.op(OP.SetTransform, m); }
  }
  toCanvas(x, y) { return M.invertible(this.state.transform) ? M.apply(this.state.transform, x, y) : null; }
  moveTo(x, y) { const p = this.toCanvas(x, y); if (p) { this.op(OP.MoveTo, p); this.subpath = [p, p]; } }
  lineTo(x, y) {
    const p = this.toCanvas(x, y);
    if (!p) return;
    if (this.subpath) { this.subpath[1] = p; this.op(OP.LineTo, p); } else { this.op(OP.MoveTo, p); this.subpath = [p, p]; }
  }
  ensure(x, y) { if (!this.subpath) this.moveTo(x, y); }
  segments(segs) {
    const m = this.state.transform;
    for (const s of segs) {
      if (s[0] === "L") this.lineTo(s[1], s[2]);
      else {
        const a = M.apply(m, s[1], s[2]), b = M.apply(m, s[3], s[4]), p = M.apply(m, s[5], s[6]);
        this.op(OP.CubicTo, [a[0], a[1], b[0], b[1], p[0], p[1]]);
        if (this.subpath) this.subpath[1] = p;
      }
    }
  }
  paints() { return M.invertible(this.state.transform); }
  take() { const out = this.sealed.splice(0); if (this.writer.ops) { out.push(this.writer.finish()); this.writer = new Writer(); } return out; }
}

/** The recording `CanvasRenderingContext2D` for one canvas generation. */
export class Recorder {
  constructor() { Object.defineProperty(this, "_", { value: new Inner() }); }
  get [Symbol.toStringTag]() { return "CanvasRenderingContext2D"; }
  // State
  save() { const g = this._; g.stack.push(clone(g.state)); g.op(OP.Save, []); }
  restore() { const g = this._; if (g.stack.length) { g.state = g.stack.pop(); g.op(OP.Restore, []); } }
  reset() { const g = this._; g.state = defaults(); g.stack.length = 0; g.subpath = null; g.op(OP.Reset, []); }
  // Transforms
  translate(x, y) { if (finite(x, y)) this._.setTransform(M.then(this._.state.transform, [1, 0, 0, 1, x, y])); }
  rotate(a) { if (finite(a)) { const s = Math.sin(a), c = Math.cos(a); this._.setTransform(M.then(this._.state.transform, [c, s, -s, c, 0, 0])); } }
  scale(x, y) { if (finite(x, y)) this._.setTransform(M.then(this._.state.transform, [x, 0, 0, y, 0, 0])); }
  transform(a, b, c, d, e, f) { if (finite(a, b, c, d, e, f)) this._.setTransform(M.then(this._.state.transform, [a, b, c, d, e, f])); }
  setTransform(a, b, c, d, e, f) {
    if (typeof a === "object" && a !== null) {
      const m = [a.a ?? a.m11 ?? 1, a.b ?? a.m12 ?? 0, a.c ?? a.m21 ?? 0, a.d ?? a.m22 ?? 1, a.e ?? a.m41 ?? 0, a.f ?? a.m42 ?? 0];
      if (!finite(...m)) throwDom("TypeError", "The matrix has non-finite entries.");
      this._.setTransform(m);
      return;
    }
    if (a === undefined) { this._.setTransform(M.identity()); return; }
    if (finite(a, b, c, d, e, f)) this._.setTransform([a, b, c, d, e, f]);
  }
  resetTransform() { this._.setTransform(M.identity()); }
  getTransform() {
    const [a, b, c, d, e, f] = this._.state.transform;
    return { a, b, c, d, e, f, m11: a, m12: b, m21: c, m22: d, m41: e, m42: f, is2D: true,
      get isIdentity() { return a === 1 && b === 0 && c === 0 && d === 1 && e === 0 && f === 0; },
      inverse() { const i = M.invert([a, b, c, d, e, f]) ?? [NaN, NaN, NaN, NaN, NaN, NaN]; return { a: i[0], b: i[1], c: i[2], d: i[3], e: i[4], f: i[5], is2D: true }; } };
  }
  // Paths
  beginPath() { const g = this._; g.subpath = null; g.op(OP.BeginPath, []); }
  moveTo(x, y) { if (finite(x, y)) this._.moveTo(x, y); }
  lineTo(x, y) { if (finite(x, y)) this._.lineTo(x, y); }
  quadraticCurveTo(cpx, cpy, x, y) {
    const g = this._;
    if (!finite(cpx, cpy, x, y) || !M.invertible(g.state.transform)) return;
    g.ensure(cpx, cpy);
    const c = M.apply(g.state.transform, cpx, cpy), p = M.apply(g.state.transform, x, y);
    g.op(OP.QuadTo, [c[0], c[1], p[0], p[1]]);
    if (g.subpath) g.subpath[1] = p;
  }
  bezierCurveTo(a, b, c, d, x, y) {
    const g = this._;
    if (!finite(a, b, c, d, x, y) || !M.invertible(g.state.transform)) return;
    g.ensure(a, b);
    g.segments([["C", a, b, c, d, x, y]]);
  }
  closePath() { const g = this._; if (g.subpath) { g.subpath = [g.subpath[0], g.subpath[0]]; g.op(OP.ClosePath, []); } }
  rect(x, y, w, h) {
    const g = this._;
    if (!finite(x, y, w, h) || !M.invertible(g.state.transform)) return;
    g.moveTo(x, y); g.lineTo(x + w, y); g.lineTo(x + w, y + h); g.lineTo(x, y + h);
    this.closePath(); g.moveTo(x, y);
  }
  arc(x, y, radius, start, end, anticlockwise = false) {
    if (!finite(x, y, radius, start, end)) return;
    if (radius < 0) throwDom("IndexSizeError", `The radius provided (${radius}) is negative.`);
    this.ellipse(x, y, radius, radius, 0, start, end, anticlockwise);
  }
  ellipse(x, y, rx, ry, rotation, start, end, anticlockwise = false) {
    const g = this._;
    if (!finite(x, y, rx, ry, rotation, start, end)) return;
    if (rx < 0 || ry < 0) throwDom("IndexSizeError", `The radius provided (${rx < 0 ? rx : ry}) is negative.`);
    if (!M.invertible(g.state.transform)) return;
    const [first, segs] = ellipseSegs(x, y, rx, ry, rotation, start, end, !!anticlockwise);
    g.lineTo(first[0], first[1]);
    g.segments(segs);
  }
  arcTo(x1, y1, x2, y2, radius) {
    const g = this._;
    if (!finite(x1, y1, x2, y2, radius) || !M.invertible(g.state.transform)) return;
    g.ensure(x1, y1);
    if (radius < 0) throwDom("IndexSizeError", `The radius provided (${radius}) is negative.`);
    const last = g.subpath ? g.subpath[1] : [0, 0], inv = M.invert(g.state.transform) ?? M.identity();
    const r = arcToSegs(M.apply(inv, last[0], last[1]), [x1, y1], [x2, y2], radius);
    if (!r) g.lineTo(x1, y1); else { g.lineTo(r[0][0], r[0][1]); g.segments(r[1]); }
  }
  roundRect(x, y, w, h, radii = 0) {
    const g = this._;
    if (!finite(x, y, w, h)) return;
    const list = Array.isArray(radii) ? radii : [radii];
    if (list.length < 1 || list.length > 4) throwDom("RangeError", `${list.length} radii provided. Between one and four radii are necessary.`);
    const norm = [];
    for (const r of list) {
      const p = typeof r === "object" && r !== null ? { x: Number(r.x ?? 0), y: Number(r.y ?? 0) } : { x: Number(r), y: Number(r) };
      if (!finite(p.x, p.y)) return;
      if (p.x < 0 || p.y < 0) throwDom("RangeError", `Radius value ${p.x < 0 ? p.x : p.y} is negative.`);
      norm.push(p);
    }
    if (!M.invertible(g.state.transform)) return;
    const [start, segs] = roundRectSegs(x, y, w, h, norm);
    g.moveTo(start[0], start[1]); g.segments(segs); this.closePath(); g.moveTo(x, y);
  }
  // Painting
  fill(rule = "nonzero") { const g = this._; if (g.paints()) g.op(OP.Fill, [rule === "evenodd" ? 1 : 0]); }
  stroke() { const g = this._; if (g.paints()) g.op(OP.Stroke, []); }
  clip(rule = "nonzero") { this._.op(OP.Clip, [rule === "evenodd" ? 1 : 0]); }
  fillRect(x, y, w, h) { const g = this._; if (finite(x, y, w, h) && g.paints()) g.op(OP.FillRect, [x, y, w, h]); }
  strokeRect(x, y, w, h) { const g = this._; if (finite(x, y, w, h) && g.paints()) g.op(OP.StrokeRect, [x, y, w, h]); }
  clearRect(x, y, w, h) { const g = this._; if (finite(x, y, w, h) && g.paints()) g.op(OP.ClearRect, [x, y, w, h]); }
  // Line styles
  get lineWidth() { return this._.state.lineWidth; }
  set lineWidth(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && v > 0 && g.state.lineWidth !== v) { g.state.lineWidth = v; g.op(OP.LineWidth, [v]); } }
  get lineCap() { return CAPS[this._.state.cap]; }
  set lineCap(v) { const k = CAPS.indexOf(v), g = this._; if (k >= 0 && g.state.cap !== k) { g.state.cap = k; g.op(OP.LineCap, [k]); } }
  get lineJoin() { return JOINS[this._.state.join]; }
  set lineJoin(v) { const k = JOINS.indexOf(v), g = this._; if (k >= 0 && g.state.join !== k) { g.state.join = k; g.op(OP.LineJoin, [k]); } }
  get miterLimit() { return this._.state.miter; }
  set miterLimit(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && v > 0 && g.state.miter !== v) { g.state.miter = v; g.op(OP.MiterLimit, [v]); } }
  setLineDash(segments) {
    const list = Array.from(segments, Number);
    if (list.some((v) => !Number.isFinite(v) || v < 0)) return;
    const dash = list.length % 2 ? list.concat(list) : list;
    this._.op(OP.LineDash, dash); this._.state.dash = dash;
  }
  getLineDash() { return this._.state.dash.slice(); }
  get lineDashOffset() { return this._.state.dashOffset; }
  set lineDashOffset(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && g.state.dashOffset !== v) { g.state.dashOffset = v; g.op(OP.LineDashOffset, [v]); } }
  // Fill and stroke styles
  _setStyle(fill, v) {
    const g = this._;
    let p;
    if (v instanceof CanvasGradient) {
      if (v._inner !== g) { g.notes.push("a gradient from another canvas was ignored"); return; }
      p = { gradient: v._id };
      if (fill) g.state.fill = p; else g.state.stroke = p;
      g.paintOp(fill, p);
      return;
    }
    let c = parseColor(String(v));
    if (c === "current") c = g.currentColor ?? BLACK;
    if (!c) return;
    p = { color: c };
    const slot = fill ? "fill" : "stroke";
    if (!samePaint(g.state[slot], p)) { g.state[slot] = p; g.paintOp(fill, p); }
  }
  _style(fill) { const p = fill ? this._.state.fill : this._.state.stroke; return p.gradient !== undefined ? new CanvasGradient(this._, p.gradient) : serializeColor(p.color); }
  get fillStyle() { return this._style(true); }
  set fillStyle(v) { this._setStyle(true, v); }
  get strokeStyle() { return this._style(false); }
  set strokeStyle(v) { this._setStyle(false, v); }
  _gradient(code, operands) { const g = this._, id = g.nextGradient++; g.op(code, [id, ...operands]); return new CanvasGradient(g, id); }
  createLinearGradient(x0, y0, x1, y1) {
    const v = [x0, y0, x1, y1].map(Number);
    if (!finite(...v)) throwDom("TypeError", "The provided double value is non-finite.");
    return this._gradient(OP.LinearGradient, v);
  }
  createRadialGradient(x0, y0, r0, x1, y1, r1) {
    const v = [x0, y0, r0, x1, y1, r1].map(Number);
    if (!finite(...v)) throwDom("TypeError", "The provided double value is non-finite.");
    if (v[2] < 0 || v[5] < 0) throwDom("IndexSizeError", `The ${v[2] < 0 ? "r0" : "r1"} provided is less than 0.`);
    return this._gradient(OP.RadialGradient, v);
  }
  // Compositing
  get globalAlpha() { return this._.state.alpha; }
  set globalAlpha(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && v >= 0 && v <= 1 && g.state.alpha !== v) { g.state.alpha = v; g.op(OP.GlobalAlpha, [v]); } }
  get globalCompositeOperation() { return COMPOSITE[this._.state.composite]; }
  set globalCompositeOperation(v) {
    const k = COMPOSITE.indexOf(v), g = this._;
    if (k < 0) return;
    if (STAGE2_COMPOSITE.has(k)) { g.notes.push(`globalCompositeOperation \`${v}\` is stage 2; ignored`); return; }
    if (g.state.composite !== k) { g.state.composite = k; g.op(OP.Composite, [k]); }
  }
}

// --- the seam: one recorder per canvas generation (LLP 1056 D4) --------------

const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
function base64(bytes) {
  let out = "", i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out += B64[n >> 18] + B64[(n >> 12) & 63] + B64[(n >> 6) & 63] + B64[n & 63];
  }
  if (i < bytes.length) {
    const n = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8);
    out += B64[n >> 18] + B64[(n >> 12) & 63] + (i + 1 < bytes.length ? B64[(n >> 6) & 63] : "=") + "=";
  }
  return out;
}

/** The executor's draw seam over the module's `draw`: requests in, JSON
 * `{lists: [base64…], frame, error}` out; a throw keeps what it recorded. */
export function canvasSeam(draw) {
  const recorders = new Map(); // canvas -> { generation, rec }
  return {
    draw(request) {
      const { canvas, generation, surface, args, frame } = request;
      let entry = recorders.get(canvas);
      if (!entry || entry.generation !== generation) { entry = { generation, rec: new Recorder() }; recorders.set(canvas, entry); }
      let wants = false, error = null;
      try {
        const result = draw(surface, args, entry.rec, Object.freeze(frame));
        if (result && typeof result.then === "function") throw new TypeError("draw returned a promise; a draw must finish in its turn");
        wants = result === true;
      } catch (e) {
        error = e && typeof e === "object" && e.message !== undefined ? `${e.name ?? "Error"}: ${e.message}` : String(e);
      }
      const inner = entry.rec._, notes = inner.notes.splice(0);
      return JSON.stringify({ lists: inner.take().map(base64), frame: wants && !error, error, notes });
    },
    retire(retired) { for (const [canvas, generation] of retired) if (recorders.get(canvas)?.generation === generation) recorders.delete(canvas); },
  };
}
