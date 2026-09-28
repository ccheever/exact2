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
  GlobalAlpha: 20, Composite: 21, ShadowColor: 22, ShadowBlur: 23, ShadowOffset: 24, ImageSmoothing: 25,
  LinearGradient: 30, RadialGradient: 31, ColorStop: 32, ConicGradient: 33,
  Pattern: 34, PatternTransform: 35, FillPattern: 36, StrokePattern: 37,
  BeginPath: 40, MoveTo: 41, LineTo: 42, QuadTo: 43, CubicTo: 44, ClosePath: 45,
  Fill: 50, Stroke: 51, Clip: 52, FillRect: 53, StrokeRect: 54, ClearRect: 55,
  Font: 60, FillText: 61, StrokeText: 62, Image: 70, DrawImage: 71, PutImageData: 72,
  PathMoveTo: 80, PathLineTo: 81, PathQuadTo: 82, PathCubicTo: 83, PathClose: 84, FillPath: 85, StrokePath: 86, ClipPath: 87,
};
export const COMPOSITE = ["source-over", "source-in", "source-out", "source-atop", "destination-over", "destination-in",
  "destination-out", "destination-atop", "lighter", "copy", "xor", "multiply", "screen", "overlay", "darken", "lighten",
  "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"];
const CAPS_LINE = ["butt", "round", "square"], JOINS = ["miter", "round", "bevel"];
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
// The wide forms (canvas/src/color.rs `wide`): sRGB pixels, clipped, and
// their own serialisation, as Chrome's getters return it.
const mul3 = (m, v) => [0, 1, 2].map((i) => m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]);
const XYZ_TO_SRGB = [[3.2409699419045226, -1.537383177570094, -0.4986107602930034], [-0.9692436362808796, 1.8759675015077202, 0.04155505740717559], [0.05563007969699366, -0.20397695888897652, 1.0569715142428786]];
const D50_TO_D65 = [[0.955473421488075, -0.02309845494876471, 0.06325924320057072], [-0.0283697093338637, 1.0099953980813041, 0.021041441191917323], [0.012314014864481998, -0.020507649298898964, 1.330365926242124]];
const P3_TO_XYZ = [[0.4865709486482162, 0.26566769316909306, 0.1982172852343625], [0.2289745640697488, 0.6917385218365064, 0.079286914093745], [0, 0.04511338185890264, 1.043944368900976]];
const toLinear = (c) => { const a = Math.abs(c); return Math.sign(c) * (a <= 0.04045 ? a / 12.92 : ((a + 0.055) / 1.055) ** 2.4); };
const fromLinear = (c) => { const a = Math.abs(c); return Math.min(1, Math.max(0, Math.sign(c) * (a <= 0.0031308 ? 12.92 * a : 1.055 * a ** (1 / 2.4) - 0.055))); };
function labToLinear(l, a, b) {
  const K = 24389 / 27, E = 216 / 24389, f1 = (l + 16) / 116, f0 = a / 500 + f1, f2 = f1 - b / 200;
  const x = f0 ** 3 > E ? f0 ** 3 : (116 * f0 - 16) / K, y = l > K * E ? f1 ** 3 : l / K, z = f2 ** 3 > E ? f2 ** 3 : (116 * f2 - 16) / K;
  return mul3(XYZ_TO_SRGB, mul3(D50_TO_D65, [x * 0.3457 / 0.3585, y, z * (1 - 0.3457 - 0.3585) / 0.3585]));
}
function oklabToLinear(l, a, b) {
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3, m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3, s_ = (l - 0.0894841775 * a - 1.2914855480 * b) ** 3;
  return [4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_, -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_, -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_];
}
const WIDE = { lab: 0, lch: 1, oklab: 2, oklch: 3 };
const WIDE_SPACES = { srgb: 4, "srgb-linear": 5, "display-p3": 6, xyz: 7, "xyz-d65": 7, "xyz-d50": 8 };
function wide(name, body) {
  let space = null, kind;
  if (name === "color") {
    const t = body.trimStart(), end = t.search(/\s/);
    if (end < 0) return null;
    space = t.slice(0, end); body = t.slice(end); kind = WIDE_SPACES[space];
  } else kind = WIDE[name];
  if (kind === undefined) return null;
  if (body.includes(",")) return undefined;
  const [main, a] = body.includes("/") ? body.split("/") : [body, undefined];
  if (a !== undefined && !a.trim()) return undefined;
  const parts = main.trim().split(/\s+/);
  if (parts.length !== 3) return undefined;
  const c = parts.map(component);
  if (c.some((x) => !x)) return undefined;
  const pct = [[100, 125, 125], [100, 150, 0], [1, 0.4, 0.4], [1, 0.4, 0], [1, 1, 1], [1, 1, 1], [1, 1, 1], [1, 1, 1], [1, 1, 1]][kind];
  const v = [];
  for (let i = 0; i < 3; i++) {
    if (c[i].pct === undefined) v.push(c[i].num);
    else if ((kind === 1 || kind === 3) && i === 2) return undefined;
    else v.push(c[i].pct / 100 * pct[i]);
  }
  if (kind <= 1) v[0] = Math.min(100, Math.max(0, v[0]));
  if (kind === 2 || kind === 3) v[0] = Math.min(1, Math.max(0, v[0]));
  if (kind === 1 || kind === 3) { v[1] = Math.max(0, v[1]); v[2] = ((v[2] % 360) + 360) % 360; }
  let alpha = 1;
  if (a !== undefined) { const x = component(a.trim()); if (!x) return undefined; alpha = x.pct !== undefined ? x.pct / 100 : x.num; }
  alpha = Math.min(1, Math.max(0, alpha));
  const rad = (d) => d * Math.PI / 180;
  const lin = kind === 0 ? labToLinear(v[0], v[1], v[2]) : kind === 1 ? labToLinear(v[0], v[1] * Math.cos(rad(v[2])), v[1] * Math.sin(rad(v[2])))
    : kind === 2 ? oklabToLinear(v[0], v[1], v[2]) : kind === 3 ? oklabToLinear(v[0], v[1] * Math.cos(rad(v[2])), v[1] * Math.sin(rad(v[2])))
    : kind === 4 ? v.map(toLinear) : kind === 5 ? v : kind === 6 ? mul3(XYZ_TO_SRGB, mul3(P3_TO_XYZ, v.map(toLinear)))
    : kind === 7 ? mul3(XYZ_TO_SRGB, v) : mul3(XYZ_TO_SRGB, mul3(D50_TO_D65, v));
  const out = [...lin.map((x) => channel(fromLinear(x) * 255)), Math.round(alpha * 255) / 255];
  out.text = `${space ? `color(${space} ` : `${name}(`}${v.map(jsNumber).join(" ")}${alpha < 1 ? ` / ${jsNumber(alpha)}` : ""})`;
  return out;
}
/** A CSS colour as `[r, g, b, a]` (a wide one with its `.text`), `"current"`, or null. */
export function parseColor(input) {
  const s = String(input).trim().toLowerCase();
  if (s === "currentcolor") return "current";
  if (s === "transparent") return [0, 0, 0, 0];
  if (s.startsWith("#")) return hexColor(s.slice(1));
  const open = s.indexOf("(");
  if (open >= 0) {
    if (!s.endsWith(")")) return null;
    const w = wide(s.slice(0, open).trim(), s.slice(open + 1, -1));
    if (w !== null) return w ?? null;
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

// --- fonts (canvas/src/font.rs) ---------------------------------------------

const jsNumber = (v) => String(v);
export const CAPS = ["normal", "small-caps", "all-small-caps", "petite-caps", "all-petite-caps", "unicase", "titling-caps"];
const KERNING = ["auto", "normal", "none"], RENDERING = ["auto", "optimizeSpeed", "optimizeLegibility", "geometricPrecision"];
const STRETCH = [["ultra-condensed", 50], ["extra-condensed", 62.5], ["condensed", 75], ["semi-condensed", 87.5], ["normal", 100],
  ["semi-expanded", 112.5], ["expanded", 125], ["extra-expanded", 150], ["ultra-expanded", 200]];
const ALIGN = ["start", "end", "left", "right", "center"], BASELINE = ["top", "hanging", "middle", "alphabetic", "ideographic", "bottom"];
const DIRECTION = ["ltr", "rtl", "inherit"];
const GENERIC = ["serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded", "math", "emoji", "fangsong"];
const BASE_SIZE = 10;
const defaultFont = () => ({ style: 0, weight: 400, stretch: 100, caps: 0, size: 10, families: ["sans-serif"] });
function fnum(t) { return /^[0-9.eE+-]+$/.test(t) && /[0-9]/.test(t) && Number.isFinite(Number(t)) ? Number(t) : null; }
function lengthPx(t, pct) {
  t = t.toLowerCase();
  if (pct && t.endsWith("%")) { const v = fnum(t.slice(0, -1)); return v === null ? null : v * BASE_SIZE / 100; }
  for (const [u, per] of [["px", 1], ["pt", 4 / 3], ["pc", 16], ["in", 96], ["cm", 96 / 2.54], ["mm", 96 / 25.4], ["q", 96 / 101.6], ["rem", BASE_SIZE], ["em", BASE_SIZE], ["ex", BASE_SIZE / 2], ["ch", BASE_SIZE / 2]]) {
    if (t.endsWith(u)) { const v = fnum(t.slice(0, -u.length)); return v === null ? null : v * per; }
  }
  return null;
}
function fontTokens(s) {
  const out = []; let cur = "";
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '"' || c === "'") { const end = s.indexOf(c, i + 1); if (end < 0) return null; cur += s.slice(i, end + 1); i = end; }
    else if (/\s/.test(c)) { if (cur) { out.push(cur); cur = ""; } }
    else if (c === "/" || c === ",") { if (cur) { out.push(cur); cur = ""; } out.push(c); }
    else cur += c;
  }
  if (cur) out.push(cur);
  return out;
}
function fontFamilies(toks) {
  const out = []; let cur = [], quoted = null;
  const finish = () => {
    if (quoted !== null) { if (cur.length) return false; out.push(quoted); quoted = null; return true; }
    if (!cur.length) return false;
    for (const w of cur) if (/^[0-9]/.test(w) || ["inherit", "initial", "unset", "default", "revert"].includes(w.toLowerCase())) return false;
    const name = cur.join(" "), lower = name.toLowerCase();
    out.push(GENERIC.includes(lower) ? lower : name); cur = []; return true;
  };
  for (const t of toks) {
    if (t === ",") { if (!finish()) return null; continue; }
    if (t[0] === '"' || t[0] === "'") { if (cur.length || quoted !== null || t.length < 2) return null; quoted = t.slice(1, -1); continue; }
    if (quoted !== null || t === "/") return null;
    cur.push(t);
  }
  return finish() ? out : null;
}
/** The CSS `font` shorthand as a canvas assignment parses it, or null. */
export function parseFont(input) {
  const toks = fontTokens(String(input).trim());
  if (!toks) return null;
  const f = defaultFont(); f.families = [];
  let style = false, variant = false, weight = false, stretch = false, i = 0;
  while (i < toks.length && i < 4) {
    const t = toks[i].toLowerCase();
    if (t === "normal") { i++; continue; }
    if (!style && (t === "italic" || t === "oblique")) {
      style = true; f.style = t === "italic" ? 1 : 2; i++;
      if (t === "oblique" && toks[i] && ["deg", "grad", "rad", "turn"].some((u) => toks[i].toLowerCase().endsWith(u) && fnum(toks[i].slice(0, -u.length)) !== null)) i++;
      continue;
    }
    if (!variant && t === "small-caps") { variant = true; f.caps = 1; i++; continue; }
    if (!weight) {
      let w = t === "bold" || t === "bolder" ? 700 : t === "lighter" ? 100 : null;
      if (w === null) { const v = fnum(t); if (v !== null && v >= 1 && v <= 1000 && !/[eE]/.test(t)) w = Math.round(v); }
      if (w !== null) { weight = true; f.weight = w; i++; continue; }
    }
    if (!stretch) { const k = STRETCH.find(([n]) => n === t); if (k) { stretch = true; f.stretch = k[1]; i++; continue; } }
    break;
  }
  const st = toks[i]; if (st === undefined) return null;
  const KW = { "xx-small": 9, "x-small": 10, small: 13, medium: 16, large: 18, "x-large": 24, "xx-large": 32, "xxx-large": 48, larger: BASE_SIZE * 1.2, smaller: BASE_SIZE / 1.2 };
  const size = KW[st.toLowerCase()] ?? lengthPx(st, true);
  if (size === null || size < 0) return null;
  f.size = size; i++;
  if (toks[i] === "/") {
    const lh = toks[i + 1]; if (lh === undefined) return null;
    const l = lh.toLowerCase();
    if (l !== "normal" && fnum(l) === null && lengthPx(l, true) === null) return null;
    i += 2;
  }
  const fam = fontFamilies(toks.slice(i));
  if (!fam) return null;
  f.families = fam;
  return f;
}
const quoteFamily = (f) => (f && !/^[0-9]/.test(f) && [...f].every((c) => /[A-Za-z0-9_-]/.test(c) || c.charCodeAt(0) > 127) ? f : `"${f.replace(/"/g, '\\"')}"`);
/** Chrome's serialisation of a canvas font. */
export function serializeFont(f) {
  let s = "";
  if (f.style === 1) s += "italic ";
  if (f.weight === 700) s += "bold "; else if (f.weight !== 400) s += `${f.weight} `;
  if (f.caps === 1) s += "small-caps ";
  return `${s}${jsNumber(f.size)}px ${f.families.map(quoteFamily).join(", ")}`;
}
const prepareText = (t) => String(t).replace(/[\t\n\f\r]/g, " ");
/** A run's metrics at left/alphabetic when no host engine is given (canvas/src/font.rs `Estimate`). */
function estimate(run) {
  const size = run.font.size, n = [...run.text].length, spaces = [...run.text].filter((c) => c === " ").length;
  const width = n * size * 0.5 + n * run.ls + spaces * run.ws;
  return [width, 0, width, n > 0 ? size * 0.7 : 0, 0, size, size * 0.2, size * 0.8, size * 0.2, size * 0.8, -size * 0.2];
}
function baselineShift(raw, b) {
  return b === 0 ? raw[7] : b === 1 ? raw[9] : b === 2 ? (raw[7] - raw[8]) / 2 : b === 4 ? raw[10] : b === 5 ? -raw[8] : 0;
}
const alignFraction = (a, rtl) => (a === 4 ? 0.5 : a === 3 || (a === 0 && rtl) || (a === 1 && !rtl) ? 1 : 0);
/** `TextMetrics` (canvas/src/font.rs `metrics`). */
export class TextMetrics {
  constructor(raw, align, baseline, rtl) {
    const dx = raw[0] * alignFraction(align, rtl), b = baselineShift(raw, baseline);
    this.width = raw[0];
    this.actualBoundingBoxLeft = raw[1] + dx; this.actualBoundingBoxRight = raw[2] - dx;
    this.fontBoundingBoxAscent = raw[5] - b; this.fontBoundingBoxDescent = raw[6] + b;
    this.actualBoundingBoxAscent = raw[3] - b; this.actualBoundingBoxDescent = raw[4] + b;
    this.emHeightAscent = raw[7] - b; this.emHeightDescent = raw[8] + b;
    this.hangingBaseline = raw[9] - b; this.alphabeticBaseline = -b; this.ideographicBaseline = raw[10] - b;
  }
  get [Symbol.toStringTag]() { return "TextMetrics"; }
}
function spacing(v, fontSize) {
  const t = String(v).trim().toLowerCase(), i = t.search(/[a-z]/);
  if (i <= 0) return null;
  const n = t.slice(0, i), unit = t.slice(i), value = fnum(n);
  if (value === null) return null;
  const px = unit === "em" ? value * fontSize : lengthPx(t, false);
  return px === null || !Number.isFinite(px) ? null : [`${jsNumber(value)}${unit}`, px];
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
const defaultText = () => ({ font: defaultFont(), ls: ["0px", 0], ws: ["0px", 0], kerning: 0, rendering: 0, align: 0, baseline: 3, direction: 2 });
const defaults = () => ({ transform: M.identity(), fill: { color: BLACK }, stroke: { color: BLACK }, lineWidth: 1, cap: 0, join: 0,
  miter: 10, dash: [], dashOffset: 0, alpha: 1, composite: 0, shadow: { color: [0, 0, 0, 0] }, shadowBlur: 0, shadowOffset: [0, 0],
  smoothing: [true, 0], text: defaultText(), fontSent: null });
const clone = (s) => ({ ...s, transform: s.transform.slice(), dash: s.dash.slice(), shadowOffset: s.shadowOffset.slice(), smoothing: s.smoothing.slice(),
  text: { ...s.text, font: { ...s.text.font, families: s.text.font.families.slice() } } });
const sameColor = (a, b) => a.every((v, i) => v === b[i]);
const samePaint = (a, b) => (a.gradient !== undefined ? a.gradient === b.gradient : a.pattern !== undefined ? a.pattern === b.pattern
  : b.color !== undefined && sameColor(a.color, b.color) && a.text === b.text);
const textOperands = (s, out) => { for (const c of String(s)) out.push(c.codePointAt(0)); return out; };

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
    this._inner.op(OP.ColorStop, [this._id, offset, c[0], c[1], c[2], c[3]]);
  }
  get [Symbol.toStringTag]() { return "CanvasGradient"; }
}

/** `CanvasPattern`: an id in its context's list. */
export class CanvasPattern {
  constructor(inner, id) { Object.defineProperty(this, "_inner", { value: inner }); Object.defineProperty(this, "_id", { value: id }); }
  setTransform(m = {}) {
    const v = [m.a ?? m.m11 ?? 1, m.b ?? m.m12 ?? 0, m.c ?? m.m21 ?? 0, m.d ?? m.m22 ?? 1, m.e ?? m.m41 ?? 0, m.f ?? m.m42 ?? 0].map(Number);
    if (!finite(...v)) throwDom("TypeError", "The matrix has non-finite entries.");
    this._inner.op(OP.PatternTransform, [this._id, ...v]);
  }
  get [Symbol.toStringTag]() { return "CanvasPattern"; }
}

/** `ImageData`: raw RGBA pixels, non-premultiplied. The platform's own
 * when there is one (the web), else this. */
class ImageDataShim {
  constructor(a, b, c) {
    let data, w, h;
    if (typeof a === "number") {
      w = Math.trunc(a); h = Math.trunc(b);
      if (!w || !h) throwDom("IndexSizeError", "The source width or height is zero.");
      data = new Uint8ClampedArray(Math.abs(w) * Math.abs(h) * 4); w = Math.abs(w); h = Math.abs(h);
    } else {
      data = a; w = b >>> 0;
      if (!data || !data.length || data.length % 4) throwDom("InvalidStateError", "The input data length is not a non-zero multiple of 4.");
      if (!w || (data.length / 4) % w) throwDom("IndexSizeError", "The input data length is not a multiple of (4 * width).");
      h = data.length / 4 / w;
      if (c !== undefined && c >>> 0 !== h) throwDom("IndexSizeError", "The input data length is not equal to (4 * width * height).");
    }
    Object.defineProperty(this, "width", { value: w, enumerable: true });
    Object.defineProperty(this, "height", { value: h, enumerable: true });
    Object.defineProperty(this, "data", { value: data, enumerable: true });
    Object.defineProperty(this, "colorSpace", { value: "srgb", enumerable: true });
  }
  get [Symbol.toStringTag]() { return "ImageData"; }
}
const ImageDataClass = globalThis.ImageData ?? ImageDataShim;
if (!globalThis.ImageData) globalThis.ImageData = ImageDataShim;
function dimension(v, which) {
  v = Number(v);
  if (!Number.isFinite(v)) throwDom("TypeError", "The provided double value is non-finite.");
  const n = Math.abs(Math.trunc(v));
  if (!n) throwDom("IndexSizeError", `The source ${which} is zero or not a number.`);
  return n;
}

class Inner {
  constructor() { this.state = defaults(); this.stack = []; this.writer = new Writer(); this.sealed = []; this.subpath = null; this.nextId = 0; this.imageIds = new Map(); this.env = { canvas: 0, currentColor: null, rtl: false, host: null }; this.notes = []; }
  op(code, operands) {
    if (this.writer.len + 8 + operands.length * 8 > SEAL && this.writer.ops) { this.sealed.push(this.writer.finish()); this.writer = new Writer(); }
    this.writer.op(code, operands);
  }
  nextObject() { return this.nextId++; }
  paintOp(fill, p) {
    if (p.gradient !== undefined) this.op(fill ? OP.FillGradient : OP.StrokeGradient, [p.gradient]);
    else if (p.pattern !== undefined) this.op(fill ? OP.FillPattern : OP.StrokePattern, [p.pattern]);
    else this.op(fill ? OP.FillColor : OP.StrokeColor, p.color.slice(0, 4));
  }
  color(v) {
    const c = parseColor(String(v));
    if (c === "current") return this.env.currentColor ?? BLACK;
    return c;
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
  // Text (canvas/src/context/text.rs)
  rtl() { const d = this.state.text.direction; return d === 2 ? this.env.rtl : d === 1; }
  measureRaw(text) {
    const t = this.state.text, run = { font: t.font, text, rtl: this.rtl(), ls: t.ls[1], ws: t.ws[1], kerning: t.kerning };
    const host = this.env.host;
    if (!host || !host.measure) return estimate(run);
    const f = t.font;
    const reply = host.measure(JSON.stringify({ font: [f.size, f.weight, f.style, f.stretch, f.caps], families: f.families.join(","), css: serializeFont(f),
      text, rtl: run.rtl, ls: run.ls, ws: run.ws, kerning: t.kerning }));
    const raw = typeof reply === "string" ? JSON.parse(reply) : reply;
    return Array.from({ length: 11 }, (_, i) => (Number.isFinite(raw?.[i]) ? raw[i] : 0));
  }
  sendFont() {
    const t = this.state.text, f = t.font;
    const rec = textOperands(f.families.join(","), [f.size, f.weight, f.style, f.stretch, f.caps, t.kerning, t.rendering, t.ls[1], t.ws[1]]);
    const sent = this.state.fontSent;
    if (!sent || sent.length !== rec.length || !rec.every((v, i) => v === sent[i])) { this.op(OP.Font, rec); this.state.fontSent = rec; }
  }
  drawText(code, text, x, y, maxWidth) {
    x = Number(x); y = Number(y);
    if (!finite(x, y) || !this.paints()) return;
    if (maxWidth !== undefined) { maxWidth = Number(maxWidth); if (Number.isNaN(maxWidth) || maxWidth <= 0) return; }
    text = prepareText(text);
    const raw = this.measureRaw(text);
    const squeeze = maxWidth !== undefined && maxWidth < raw[0];
    const width = squeeze ? maxWidth : raw[0], scale = squeeze ? (raw[0] > 0 ? maxWidth / raw[0] : 0) : 1;
    const rtl = this.rtl(), t = this.state.text;
    const ox = x - width * alignFraction(t.align, rtl), oy = y + baselineShift(raw, t.baseline);
    this.sendFont();
    this.op(code, textOperands(text, [ox, oy, scale, rtl ? 1 : 0]));
  }
  // Images (canvas/src/context/image.rs)
  imageSize(src) {
    const host = this.env.host;
    if (!host || !host.image) return null;
    const reply = host.image(JSON.stringify({ canvas: this.env.canvas, src: String(src) }));
    const size = typeof reply === "string" ? (reply ? JSON.parse(reply) : null) : reply;
    return size && size[0] > 0 && size[1] > 0 ? size : null;
  }
  imageId(src) {
    src = String(src);
    let id = this.imageIds.get(src);
    if (id === undefined) { id = this.nextObject(); this.imageIds.set(src, id); this.op(OP.Image, textOperands(src, [id])); }
    return id;
  }
}

// Path2D (canvas/src/path2d.rs): segments in the path's own coordinates.
const PATH = Symbol("path");
class PathData {
  constructor() { this.segs = []; this.subpath = null; }
  moveTo(x, y) { this.segs.push(["M", x, y]); this.subpath = [[x, y], [x, y]]; }
  lineTo(x, y) { if (this.subpath) { this.subpath[1] = [x, y]; this.segs.push(["L", x, y]); } else this.moveTo(x, y); }
  ensure(x, y) { if (!this.subpath) this.moveTo(x, y); }
  last(p) { if (this.subpath) this.subpath[1] = p; }
  segments(segs) { for (const s of segs) { if (s[0] === "L") this.lineTo(s[1], s[2]); else { this.segs.push(["C", ...s.slice(1)]); this.last([s[5], s[6]]); } } }
  close() { if (this.subpath) { this.subpath = [this.subpath[0], this.subpath[0]]; this.segs.push(["Z"]); } }
  quad(cx, cy, x, y) { this.ensure(cx, cy); this.segs.push(["Q", cx, cy, x, y]); this.last([x, y]); }
  cubic(a, b, c, d, x, y) { this.ensure(a, b); this.segs.push(["C", a, b, c, d, x, y]); this.last([x, y]); }
  ellipse(x, y, rx, ry, rot, s, e, ac) { const [first, segs] = ellipseSegs(x, y, rx, ry, rot, s, e, ac); this.lineTo(first[0], first[1]); this.segments(segs); }
}
const transformSeg = (s, m) => {
  const out = [s[0]];
  for (let i = 1; i < s.length; i += 2) { const p = M.apply(m, s[i], s[i + 1]); out.push(p[0], p[1]); }
  return out;
};
function svgArc(d, p0, rx, ry, phiDeg, large, sweep, p) {
  if (p0[0] === p[0] && p0[1] === p[1]) return;
  rx = Math.abs(rx); ry = Math.abs(ry);
  if (rx === 0 || ry === 0) { d.lineTo(p[0], p[1]); return; }
  const phi = phiDeg * Math.PI / 180, sin = Math.sin(phi), cos = Math.cos(phi);
  const hx = (p0[0] - p[0]) / 2, hy = (p0[1] - p[1]) / 2, x1 = cos * hx + sin * hy, y1 = -sin * hx + cos * hy;
  const lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if (lambda > 1) { const k = Math.sqrt(lambda); rx *= k; ry *= k; }
  const num = Math.max(0, rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1), den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  let co = den === 0 ? 0 : Math.sqrt(num / den);
  if (large === sweep) co = -co;
  const cx1 = (co * rx * y1) / ry, cy1 = (-co * ry * x1) / rx;
  const cx = cos * cx1 - sin * cy1 + (p0[0] + p[0]) / 2, cy = sin * cx1 + cos * cy1 + (p0[1] + p[1]) / 2;
  const angle = (ux, uy, vx, vy) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
  const t1 = angle(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry);
  let dt = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
  if (!sweep && dt > 0) dt -= TAU; else if (sweep && dt < 0) dt += TAU;
  d.segments(ellipseSegs(cx, cy, rx, ry, phi, t1, t1 + dt, dt < 0)[1]);
  const last = d.segs[d.segs.length - 1];
  if (last && last[0] === "C") { last[5] = p[0]; last[6] = p[1]; }
  d.last(p);
}
function svgPath(text, d) {
  const s = String(text); let at = 0, cmd = "", cur = [0, 0], start = [0, 0], lastC = null, lastQ = null, first = true;
  const skip = (comma) => { let seen = false; while (at < s.length && (/\s/.test(s[at]) || (comma && s[at] === "," && !seen))) { if (s[at] === ",") seen = true; at++; } };
  const number = () => { skip(true); const m = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?/.exec(s.slice(at)); if (!m) return null; at += m[0].length; const v = Number(m[0]); return Number.isFinite(v) ? v : null; };
  const flag = () => { skip(true); const c = s[at++]; return c === "0" ? false : c === "1" ? true : null; };
  const more = () => { skip(true); return at < s.length && /[0-9.+-]/.test(s[at]); };
  for (;;) {
    skip(false);
    if (at >= s.length) return;
    if (/[A-Za-z]/.test(s[at])) cmd = s[at++]; else if (!cmd || !more()) return;
    if (first && cmd !== "M" && cmd !== "m") return;
    first = false;
    const rel = cmd === cmd.toLowerCase(), base = rel ? cur : [0, 0];
    const pt = () => { const x = number(); if (x === null) return null; const y = number(); return y === null ? null : [base[0] + x, base[1] + y]; };
    let c2 = null, q = null;
    switch (cmd.toUpperCase()) {
      case "M": { const p = pt(); if (!p) return; d.moveTo(p[0], p[1]); cur = start = p; cmd = rel ? "l" : "L"; break; }
      case "L": { const p = pt(); if (!p) return; d.lineTo(p[0], p[1]); cur = p; break; }
      case "H": { const x = number(); if (x === null) return; cur = [rel ? cur[0] + x : x, cur[1]]; d.lineTo(cur[0], cur[1]); break; }
      case "V": { const y = number(); if (y === null) return; cur = [cur[0], rel ? cur[1] + y : y]; d.lineTo(cur[0], cur[1]); break; }
      case "C": { const a = pt(), b = a && pt(), p = b && pt(); if (!p) return; d.cubic(a[0], a[1], b[0], b[1], p[0], p[1]); c2 = b; cur = p; break; }
      case "S": { const b = pt(), p = b && pt(); if (!p) return; const a = lastC ? [2 * cur[0] - lastC[0], 2 * cur[1] - lastC[1]] : cur; d.cubic(a[0], a[1], b[0], b[1], p[0], p[1]); c2 = b; cur = p; break; }
      case "Q": { const c = pt(), p = c && pt(); if (!p) return; d.quad(c[0], c[1], p[0], p[1]); q = c; cur = p; break; }
      case "T": { const p = pt(); if (!p) return; const c = lastQ ? [2 * cur[0] - lastQ[0], 2 * cur[1] - lastQ[1]] : cur; d.quad(c[0], c[1], p[0], p[1]); q = c; cur = p; break; }
      case "A": {
        const rx = number(), ry = rx === null ? null : number(), rot = ry === null ? null : number();
        const large = rot === null ? null : flag(), sweep = large === null ? null : flag();
        if (sweep === null) return;
        const p = pt(); if (!p) return;
        d.ensure(cur[0], cur[1]); svgArc(d, cur, rx, ry, rot, large, sweep, p); cur = p; break;
      }
      case "Z": d.close(); cur = start; d.subpath = [start, start]; break;
      default: return;
    }
    lastC = c2; lastQ = q;
  }
}

/** `Path2D`. */
export class Path2D {
  constructor(init) {
    const d = new PathData();
    Object.defineProperty(this, PATH, { value: d });
    if (init instanceof Path2D) { for (const s of init[PATH].segs) d.segs.push(s.slice()); d.subpath = init[PATH].subpath && init[PATH].subpath.map((p) => p.slice()); }
    else if (init !== undefined) svgPath(init, d);
  }
  get [Symbol.toStringTag]() { return "Path2D"; }
  addPath(path, m = {}) {
    if (!(path instanceof Path2D)) throw new TypeError("Failed to execute 'addPath' on 'Path2D': parameter 1 is not of type 'Path2D'.");
    const t = [m.a ?? m.m11 ?? 1, m.b ?? m.m12 ?? 0, m.c ?? m.m21 ?? 0, m.d ?? m.m22 ?? 1, m.e ?? m.m41 ?? 0, m.f ?? m.m42 ?? 0].map(Number);
    if (!finite(...t)) throwDom("TypeError", "The matrix has non-finite entries.");
    const d = this[PATH];
    for (const s of path[PATH].segs.map((x) => transformSeg(x, t))) {
      if (s[0] === "M") d.moveTo(s[1], s[2]); else if (s[0] === "Z") d.close(); else { d.segs.push(s); d.last([s[s.length - 2], s[s.length - 1]]); }
    }
  }
  moveTo(x, y) { if (finite(x, y)) this[PATH].moveTo(x, y); }
  lineTo(x, y) { if (finite(x, y)) this[PATH].lineTo(x, y); }
  quadraticCurveTo(a, b, x, y) { if (finite(a, b, x, y)) this[PATH].quad(a, b, x, y); }
  bezierCurveTo(a, b, c, d, x, y) { if (finite(a, b, c, d, x, y)) this[PATH].cubic(a, b, c, d, x, y); }
  closePath() { this[PATH].close(); }
  rect(x, y, w, h) { if (!finite(x, y, w, h)) return; const d = this[PATH]; d.moveTo(x, y); d.lineTo(x + w, y); d.lineTo(x + w, y + h); d.lineTo(x, y + h); d.close(); d.moveTo(x, y); }
  roundRect(x, y, w, h, radii = 0) {
    if (!finite(x, y, w, h)) return;
    const norm = radiiList(radii);
    if (!norm) return;
    const [start, segs] = roundRectSegs(x, y, w, h, norm), d = this[PATH];
    d.moveTo(start[0], start[1]); d.segments(segs); d.close(); d.moveTo(x, y);
  }
  arc(x, y, r, s, e, ac = false) {
    if (!finite(x, y, r, s, e)) return;
    if (r < 0) throwDom("IndexSizeError", `The radius provided (${r}) is negative.`);
    this[PATH].ellipse(x, y, r, r, 0, s, e, !!ac);
  }
  ellipse(x, y, rx, ry, rot, s, e, ac = false) {
    if (!finite(x, y, rx, ry, rot, s, e)) return;
    if (rx < 0 || ry < 0) throwDom("IndexSizeError", `The radius provided (${rx < 0 ? rx : ry}) is negative.`);
    this[PATH].ellipse(x, y, rx, ry, rot, s, e, !!ac);
  }
  arcTo(x1, y1, x2, y2, r) {
    if (!finite(x1, y1, x2, y2, r)) return;
    const d = this[PATH];
    d.ensure(x1, y1);
    if (r < 0) throwDom("IndexSizeError", `The radius provided (${r}) is negative.`);
    const res = arcToSegs(d.subpath ? d.subpath[1] : [0, 0], [x1, y1], [x2, y2], r);
    if (!res) d.lineTo(x1, y1); else { d.lineTo(res[0][0], res[0][1]); d.segments(res[1]); }
  }
}
function radiiList(radii) {
  const list = Array.isArray(radii) ? radii : [radii];
  if (list.length < 1 || list.length > 4) throwDom("RangeError", `${list.length} radii provided. Between one and four radii are necessary.`);
  const norm = [];
  for (const r of list) {
    const p = typeof r === "object" && r !== null ? { x: Number(r.x ?? 0), y: Number(r.y ?? 0) } : { x: Number(r), y: Number(r) };
    if (!finite(p.x, p.y)) return null;
    if (p.x < 0 || p.y < 0) throwDom("RangeError", `Radius value ${p.x < 0 ? p.x : p.y} is negative.`);
    norm.push(p);
  }
  return norm;
}
// The recorder's Path2D is the realm's: a module's `new Path2D()` must be
// one the recorder can paint, on the web too, where the realm's own exists.
globalThis.Path2D = Path2D;
const PATH_OPS = { M: OP.PathMoveTo, L: OP.PathLineTo, Q: OP.PathQuadTo, C: OP.PathCubicTo, Z: OP.PathClose };

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
    const norm = radiiList(radii);
    if (!norm || !M.invertible(g.state.transform)) return;
    const [start, segs] = roundRectSegs(x, y, w, h, norm);
    g.moveTo(start[0], start[1]); g.segments(segs); this.closePath(); g.moveTo(x, y);
  }
  // Painting
  _paintPath(path, code, rule, clip) {
    const g = this._;
    if (!clip && !g.paints()) return;
    const m = g.state.transform;
    for (const s of path[PATH].segs) { const t = transformSeg(s, m); g.op(PATH_OPS[t[0]], t.slice(1)); }
    g.op(code, rule === undefined ? [] : [rule === "evenodd" ? 1 : 0]);
  }
  fill(a = "nonzero", b = "nonzero") {
    if (a instanceof Path2D) return this._paintPath(a, OP.FillPath, b, false);
    const g = this._; if (g.paints()) g.op(OP.Fill, [a === "evenodd" ? 1 : 0]);
  }
  stroke(path) {
    if (path instanceof Path2D) return this._paintPath(path, OP.StrokePath, undefined, false);
    const g = this._; if (g.paints()) g.op(OP.Stroke, []);
  }
  clip(a = "nonzero", b = "nonzero") {
    if (a instanceof Path2D) return this._paintPath(a, OP.ClipPath, b, true);
    this._.op(OP.Clip, [a === "evenodd" ? 1 : 0]);
  }
  fillRect(x, y, w, h) { const g = this._; if (finite(x, y, w, h) && g.paints()) g.op(OP.FillRect, [x, y, w, h]); }
  strokeRect(x, y, w, h) { const g = this._; if (finite(x, y, w, h) && g.paints()) g.op(OP.StrokeRect, [x, y, w, h]); }
  clearRect(x, y, w, h) { const g = this._; if (finite(x, y, w, h) && g.paints()) g.op(OP.ClearRect, [x, y, w, h]); }
  // Line styles
  get lineWidth() { return this._.state.lineWidth; }
  set lineWidth(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && v > 0 && g.state.lineWidth !== v) { g.state.lineWidth = v; g.op(OP.LineWidth, [v]); } }
  get lineCap() { return CAPS_LINE[this._.state.cap]; }
  set lineCap(v) { const k = CAPS_LINE.indexOf(v), g = this._; if (k >= 0 && g.state.cap !== k) { g.state.cap = k; g.op(OP.LineCap, [k]); } }
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
    const g = this._, slot = fill ? "fill" : "stroke";
    let p;
    if (v instanceof CanvasGradient || v instanceof CanvasPattern) {
      if (v._inner !== g) { g.notes.push(`a ${v instanceof CanvasGradient ? "gradient" : "pattern"} from another canvas was ignored`); return; }
      p = v instanceof CanvasGradient ? { gradient: v._id } : { pattern: v._id };
      g.paintOp(fill, p);
      g.state[slot] = p;
      return;
    }
    const c = g.color(v);
    if (!c) return;
    p = { color: c, text: c.text };
    const old = g.state[slot];
    if (!samePaint(old, p)) {
      const changed = old.color === undefined || !sameColor(old.color, c);
      g.state[slot] = p;
      if (changed) g.paintOp(fill, p);
    }
  }
  _style(fill) {
    const p = fill ? this._.state.fill : this._.state.stroke;
    if (p.gradient !== undefined) return new CanvasGradient(this._, p.gradient);
    if (p.pattern !== undefined) return new CanvasPattern(this._, p.pattern);
    return p.text ?? serializeColor(p.color);
  }
  get fillStyle() { return this._style(true); }
  set fillStyle(v) { this._setStyle(true, v); }
  get strokeStyle() { return this._style(false); }
  set strokeStyle(v) { this._setStyle(false, v); }
  _gradient(code, operands) { const g = this._, id = g.nextObject(); g.op(code, [id, ...operands]); return new CanvasGradient(g, id); }
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
  createConicGradient(angle, x, y) {
    const v = [angle, x, y].map(Number);
    if (!finite(...v)) throwDom("TypeError", "The provided double value is non-finite.");
    return this._gradient(OP.ConicGradient, v);
  }
  // Compositing
  get globalAlpha() { return this._.state.alpha; }
  set globalAlpha(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && v >= 0 && v <= 1 && g.state.alpha !== v) { g.state.alpha = v; g.op(OP.GlobalAlpha, [v]); } }
  get globalCompositeOperation() { return COMPOSITE[this._.state.composite]; }
  set globalCompositeOperation(v) {
    const k = COMPOSITE.indexOf(v), g = this._;
    if (k >= 0 && g.state.composite !== k) { g.state.composite = k; g.op(OP.Composite, [k]); }
  }
  // Shadows
  get shadowColor() { const s = this._.state.shadow; return s.text ?? serializeColor(s.color); }
  set shadowColor(v) {
    const g = this._, c = g.color(v);
    if (!c) return;
    if (!sameColor(g.state.shadow.color, c)) g.op(OP.ShadowColor, c.slice(0, 4));
    g.state.shadow = { color: c, text: c.text };
  }
  get shadowBlur() { return this._.state.shadowBlur; }
  set shadowBlur(v) { v = Number(v); const g = this._; if (Number.isFinite(v) && v >= 0 && g.state.shadowBlur !== v) { g.state.shadowBlur = v; g.op(OP.ShadowBlur, [v]); } }
  _offset(i, v) {
    v = Number(v); const g = this._, next = g.state.shadowOffset.slice(); next[i] = v;
    if (Number.isFinite(v) && v !== g.state.shadowOffset[i]) { g.state.shadowOffset = next; g.op(OP.ShadowOffset, next); }
  }
  get shadowOffsetX() { return this._.state.shadowOffset[0]; }
  set shadowOffsetX(v) { this._offset(0, v); }
  get shadowOffsetY() { return this._.state.shadowOffset[1]; }
  set shadowOffsetY(v) { this._offset(1, v); }
  // Image smoothing
  _smoothing(next) { const g = this._, s = g.state.smoothing; if (s[0] !== next[0] || s[1] !== next[1]) { g.state.smoothing = next; g.op(OP.ImageSmoothing, [next[0] ? 1 : 0, next[1]]); } }
  get imageSmoothingEnabled() { return this._.state.smoothing[0]; }
  set imageSmoothingEnabled(v) { this._smoothing([!!v, this._.state.smoothing[1]]); }
  get imageSmoothingQuality() { return ["low", "medium", "high"][this._.state.smoothing[1]]; }
  set imageSmoothingQuality(v) { const k = ["low", "medium", "high"].indexOf(v); if (k >= 0) this._smoothing([this._.state.smoothing[0], k]); }
  // Text
  get font() { return serializeFont(this._.state.text.font); }
  set font(v) { const f = parseFont(v); if (f) this._.state.text.font = f; }
  get textAlign() { return ALIGN[this._.state.text.align]; }
  set textAlign(v) { const k = ALIGN.indexOf(v); if (k >= 0) this._.state.text.align = k; }
  get textBaseline() { return BASELINE[this._.state.text.baseline]; }
  set textBaseline(v) { const k = BASELINE.indexOf(v); if (k >= 0) this._.state.text.baseline = k; }
  get direction() { return this._.rtl() ? "rtl" : "ltr"; }
  set direction(v) { const k = DIRECTION.indexOf(v); if (k >= 0) this._.state.text.direction = k; }
  get fontKerning() { return KERNING[this._.state.text.kerning]; }
  set fontKerning(v) { const k = KERNING.indexOf(v); if (k >= 0) this._.state.text.kerning = k; }
  get textRendering() { return RENDERING[this._.state.text.rendering]; }
  set textRendering(v) { const k = RENDERING.indexOf(v); if (k >= 0) this._.state.text.rendering = k; }
  get fontVariantCaps() { return CAPS[this._.state.text.font.caps]; }
  set fontVariantCaps(v) { const k = CAPS.indexOf(v); if (k >= 0) this._.state.text.font.caps = k; }
  get fontStretch() { const s = this._.state.text.font.stretch; return (STRETCH.find(([, p]) => p === s) ?? ["normal"])[0]; }
  set fontStretch(v) { const k = STRETCH.find(([n]) => n === v); if (k) this._.state.text.font.stretch = k[1]; }
  get letterSpacing() { return this._.state.text.ls[0]; }
  set letterSpacing(v) { const s = spacing(v, this._.state.text.font.size); if (s) this._.state.text.ls = s; }
  get wordSpacing() { return this._.state.text.ws[0]; }
  set wordSpacing(v) { const s = spacing(v, this._.state.text.font.size); if (s) this._.state.text.ws = s; }
  fillText(text, x, y, maxWidth) { this._.drawText(OP.FillText, text, x, y, maxWidth); }
  strokeText(text, x, y, maxWidth) { this._.drawText(OP.StrokeText, text, x, y, maxWidth); }
  measureText(text) {
    const g = this._, t = g.state.text;
    return new TextMetrics(g.measureRaw(prepareText(text)), t.align, t.baseline, g.rtl());
  }
  // Images and pixels
  drawImage(image, ...a) {
    const g = this._;
    if (![3, 5, 9].includes(a.length + 1)) throw new TypeError(`Failed to execute 'drawImage': ${a.length + 1} arguments present.`);
    const n = a.map(Number);
    if (!finite(...n) || !g.paints()) return;
    const size = g.imageSize(image);
    if (!size) return;
    const [w, h] = size;
    let sx = 0, sy = 0, sw = w, sh = h, dx, dy, dw, dh;
    if (n.length === 2) { [dx, dy] = n; dw = w; dh = h; }
    else if (n.length === 4) [dx, dy, dw, dh] = n;
    else [sx, sy, sw, sh, dx, dy, dw, dh] = n;
    if (sw < 0) { sx += sw; sw = -sw; }
    if (sh < 0) { sy += sh; sh = -sh; }
    if (dw < 0) { dx += dw; dw = -dw; }
    if (dh < 0) { dy += dh; dh = -dh; }
    if (!sw || !sh || !dw || !dh) return;
    const kx = dw / sw, ky = dh / sh, x0 = Math.max(sx, 0), y0 = Math.max(sy, 0), x1 = Math.min(sx + sw, w), y1 = Math.min(sy + sh, h);
    if (x1 <= x0 || y1 <= y0) return;
    dx += (x0 - sx) * kx; dy += (y0 - sy) * ky; dw = (x1 - x0) * kx; dh = (y1 - y0) * ky;
    const id = g.imageId(image);
    g.op(OP.DrawImage, [id, x0, y0, x1 - x0, y1 - y0, dx, dy, dw, dh]);
  }
  createPattern(image, repetition) {
    repetition = repetition === null || repetition === undefined ? "" : String(repetition);
    const rep = { "": 0, repeat: 0, "repeat-x": 1, "repeat-y": 2, "no-repeat": 3 }[repetition];
    if (rep === undefined) throwDom("SyntaxError", `The provided type ('${repetition}') is not one of 'repeat', 'no-repeat', 'repeat-x', or 'repeat-y'.`);
    const g = this._;
    if (!g.imageSize(image)) return null;
    const img = g.imageId(image), id = g.nextObject();
    g.op(OP.Pattern, [id, img, rep]);
    return new CanvasPattern(g, id);
  }
  createImageData(a, b) {
    if (a && typeof a === "object") return new ImageDataClass(a.width, a.height);
    return new ImageDataClass(dimension(a, "width"), dimension(b, "height"));
  }
  putImageData(data, dx, dy, x = 0, y = 0, w = data.width, h = data.height) {
    const all = [dx, dy, x, y, w, h].map(Number);
    if (!finite(...all)) throwDom("TypeError", "Value is not of type 'long'.");
    [dx, dy, x, y, w, h] = all.map(Math.trunc);
    if (w < 0) { x += w; w = -w; }
    if (h < 0) { y += h; h = -h; }
    if (x < 0) { w += x; x = 0; }
    if (y < 0) { h += y; y = 0; }
    w = Math.min(w, data.width - x); h = Math.min(h, data.height - y);
    if (w <= 0 || h <= 0) return;
    const ops = [dx + x, dy + y, w, h], d = data.data;
    for (let row = y; row < y + h; row++) for (let col = x; col < x + w; col++) {
      const i = (row * data.width + col) * 4;
      ops.push(d[i] * 16777216 + d[i + 1] * 65536 + d[i + 2] * 256 + d[i + 3]);
    }
    this._.op(OP.PutImageData, ops);
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
 * `{lists: [base64…], frame, error}` out; a throw keeps what it recorded.
 * `host` measures text and answers image sizes, where the draw runs (LLP
 * 1056 D8, D9): `measure(json) -> json` and `image(json) -> json | ""`. */
export function canvasSeam(draw) {
  const recorders = new Map(); // canvas -> { generation, rec }
  return {
    draw(request, host) {
      const { canvas, generation, surface, args, frame } = request;
      let entry = recorders.get(canvas);
      if (!entry || entry.generation !== generation) { entry = { generation, rec: new Recorder() }; recorders.set(canvas, entry); }
      const inner = entry.rec._;
      inner.env = { canvas, currentColor: request.currentColor ?? null, rtl: request.rtl === true, host: host ?? null };
      let wants = false, error = null;
      try {
        const result = draw(surface, args, entry.rec, Object.freeze(frame));
        if (result && typeof result.then === "function") throw new TypeError("draw returned a promise; a draw must finish in its turn");
        wants = result === true;
      } catch (e) {
        error = e && typeof e === "object" && e.message !== undefined ? `${e.name ?? "Error"}: ${e.message}` : String(e);
      }
      inner.env = { ...inner.env, host: null };
      const notes = inner.notes.splice(0);
      return JSON.stringify({ lists: inner.take().map(base64), frame: wants && !error, error, notes });
    },
    retire(retired) { for (const [canvas, generation] of retired) if (recorders.get(canvas)?.generation === generation) recorders.delete(canvas); },
  };
}
