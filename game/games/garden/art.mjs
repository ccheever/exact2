#!/usr/bin/env bun
// The garden's "art pass" look (`art: "pass"`), authored as code: every plant
// stage, fruit shape, prop, the farmer and the stall keeper are procedural
// meshes written as binary glTF into art/, sampling six shared textures in
// art/textures/ (each bakes once, by name, however many models use it). Run
// `bun game/games/garden/art.mjs`; the game's bake turns art/*.glb into
// .model assets. Deterministic: the same script writes the same bytes.
//
// Naming is the contract with logic/src/pass.rs: plant-<crop>-<stage>[-far],
// fruit-<crop>[-unripe][-far]; a fruit's body is material 0, which the game
// recolours per mutation (MaterialOverrides), and a lantern's glass is
// material 0, which it lights at night.
import { mkdirSync, writeFileSync, readdirSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { encodePng } from '../../../scripts/png.mjs';

const ART = resolve(import.meta.dir, 'art');

// ------------------------------------------------------------ randomness
const rng = seed => () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const hash3 = (x, y, z) => {
  let h = Math.imul(x | 0, 374761393) + Math.imul(y | 0, 668265263) + Math.imul(z | 0, 2147483647);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
};
const smooth = t => t * t * (3 - 2 * t);
// Value noise in 2D, tiling with period `p`.
const noise2 = (x, y, p = 1e9) => {
  const xi = Math.floor(x), yi = Math.floor(y), xf = smooth(x - xi), yf = smooth(y - yi);
  const v = (i, j) => hash3(((xi + i) % p + p) % p, ((yi + j) % p + p) % p, 7);
  const a = v(0, 0) + (v(1, 0) - v(0, 0)) * xf, b = v(0, 1) + (v(1, 1) - v(0, 1)) * xf;
  return a + (b - a) * yf;
};
const fbm = (x, y, p, oct = 4) => {
  let s = 0, a = 0.5, f = 1;
  for (let i = 0; i < oct; i++) { s += a * noise2(x * f, y * f, p * f); a *= 0.5; f *= 2; }
  return s;
};

// ------------------------------------------------------------ colour
// Hex colours are sRGB and convert exactly; crop colours copy crops.rs and
// convert as the game's `paint` does (squared), so the game's mutation looks
// and these models agree on a fruit's hue.
const lin = c => c.map(v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
const hex = h => lin([1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16) / 255));
const sq = c => c.map(v => v * v);
const mix = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t);
const scl = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const grey = v => [v, v, v];

// ------------------------------------------------------------ vectors
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = a => { const l = Math.hypot(...a) || 1; return scl(a, 1 / l); };
// An affine transform: rotation/scale rows and a translation.
const I = { r: [[1, 0, 0], [0, 1, 0], [0, 0, 1]], t: [0, 0, 0] };
const apply = (M, p) => [0, 1, 2].map(i => M.r[i][0] * p[0] + M.r[i][1] * p[1] + M.r[i][2] * p[2] + M.t[i]);
const mul = (A, B) => ({ // A after B
  r: A.r.map(row => [0, 1, 2].map(j => row[0] * B.r[0][j] + row[1] * B.r[1][j] + row[2] * B.r[2][j])),
  t: apply(A, B.t),
});
const T = (x, y, z) => ({ r: I.r, t: [x, y, z] });
const S = (x, y = x, z = x) => ({ r: [[x, 0, 0], [0, y, 0], [0, 0, z]], t: [0, 0, 0] });
const RX = a => ({ r: [[1, 0, 0], [0, Math.cos(a), -Math.sin(a)], [0, Math.sin(a), Math.cos(a)]], t: [0, 0, 0] });
const RY = a => ({ r: [[Math.cos(a), 0, Math.sin(a)], [0, 1, 0], [-Math.sin(a), 0, Math.cos(a)]], t: [0, 0, 0] });
const RZ = a => ({ r: [[Math.cos(a), -Math.sin(a), 0], [Math.sin(a), Math.cos(a), 0], [0, 0, 1]], t: [0, 0, 0] });
const chain = (...ms) => ms.reduce((a, b) => mul(a, b), I);
// Normals transform by the inverse transpose.
const normalOf = (M, n) => {
  const r = M.r;
  const det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0]) + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
  const c = (i, j) => { const a = r[(i + 1) % 3], b = r[(i + 2) % 3]; return a[(j + 1) % 3] * b[(j + 2) % 3] - a[(j + 2) % 3] * b[(j + 1) % 3]; };
  return norm([0, 1, 2].map(i => [0, 1, 2].reduce((s, j) => s + c(i, j) / det * n[j], 0)));
};

// ------------------------------------------------------------ textures
// Shared material textures: art/textures/<name>.png bakes once as <name>.tex.
function texture(size, f) {
  const data = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const c = f(x / size, y / size), o = (y * size + x) * 4;
    for (let k = 0; k < 3; k++) data[o + k] = Math.max(0, Math.min(255, Math.round(c[k] * 255)));
    data[o + 3] = 255;
  }
  return encodePng({ width: size, height: size, data });
}
// Textures are near-white detail; vertex colours and factors give the hue.
const TEXTURES = {
  soil: () => texture(128, (u, v) => {
    const n = fbm(u * 8, v * 8, 8), clod = noise2(u * 24, v * 24, 24), grit = hash3(u * 128, v * 128, 3);
    const c = mix([0.55, 0.5, 0.46], [1, 0.95, 0.9], n);
    return clod > 0.7 ? mix(c, [1, 0.98, 0.92], 0.5) : grit > 0.93 ? scl(c, 0.7) : c;
  }),
  wood: () => texture(128, (u, v) => {
    const g = Math.sin((u * 6 + fbm(u * 4, v * 16, 4) * 1.5) * Math.PI * 2) * 0.5 + 0.5;
    return grey(0.78 + 0.14 * g + noise2(u * 64, v * 4, 64) * 0.08);
  }),
  bark: () => texture(128, (u, v) => {
    const ridge = Math.abs(Math.sin((u * 10 + fbm(u * 3, v * 6, 3) * 2) * Math.PI));
    return grey(0.45 + 0.45 * ridge + noise2(u * 32, v * 32, 32) * 0.1);
  }),
  cloth: () => texture(64, (u, v) => {
    const w = (Math.floor(u * 32) + Math.floor(v * 32)) % 2 ? 1 : 0.88;
    return grey(w * (0.92 + noise2(u * 64, v * 64, 64) * 0.08));
  }),
  straw: () => texture(64, (u, v) => {
    const a = Math.sin((u + v) * Math.PI * 24) * 0.5 + 0.5, b = Math.sin((u - v) * Math.PI * 24) * 0.5 + 0.5;
    return grey(0.7 + 0.3 * ((Math.floor(u * 12) + Math.floor(v * 12)) % 2 ? a : b));
  }),
  stone: () => texture(64, (u, v) => grey(0.62 + 0.38 * fbm(u * 4, v * 4, 4))),
  // Meadow turf seen from above: clumps, blade streaks and a little bare earth.
  grass: () => texture(128, (u, v) => {
    const clump = fbm(u * 6, v * 6, 6), blade = noise2(u * 96, v * 24, 96), dot = hash3(u * 128, v * 128, 9);
    const c = 0.62 + 0.28 * clump + 0.14 * (blade - 0.5);
    return dot > 0.985 ? [1, 0.95, 0.75] : grey(Math.min(1, c));
  }),
};

// ------------------------------------------------------------ materials
const M = {
  soil: { rough: 0.95, tex: 'soil' },
  wood: { rough: 0.8, tex: 'wood' },
  bark: { rough: 0.9, tex: 'bark' },
  stone: { rough: 0.85, tex: 'stone' },
  straw: { rough: 0.85, tex: 'straw' },
  cloth: { rough: 0.9, tex: 'cloth' },
  turf: { rough: 0.95, tex: 'grass' },
  leaf: { rough: 0.6, double: true },
  canopy: { rough: 0.75 },
  stem: { rough: 0.65 },
  skin: { rough: 0.5 },
  plain: { rough: 0.55 },
  shiny: { rough: 0.15 },
  // A fruit's body: the factor is its hue, vertex colours its pattern.
  body: { rough: 0.3 },
  iron: { color: hex('#34343a'), metal: 1, rough: 0.45 },
  tin: { color: hex('#b9c2c6'), metal: 0.85, rough: 0.32 },
  // Lit by the game at night (emission through MaterialOverrides).
  glass: { color: hex('#ffe2a8'), rough: 0.15 },
};

// ------------------------------------------------------------ meshes
// A model is primitives keyed by material, in first-use order: that order is
// the material index the game addresses.
class Model {
  constructor() { this.parts = new Map(); }
  material(name, spec = {}) {
    if (!this.parts.has(name)) this.parts.set(name, { spec: { ...(M[name] ?? M.plain), ...spec }, p: [], n: [], t: [], c: [], i: [] });
    return this.parts.get(name);
  }
  // Append a grid (rows × cols of vertices) through M.
  grid(mat, Mx, rows, cols, at, { color = () => [1, 1, 1], flip = false, uv } = {}) {
    const P = this.material(mat), base = P.p.length / 3;
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
      const u = c / (cols - 1), v = r / (rows - 1);
      const s = at(u, v);
      const wp = apply(Mx, s.p);
      P.p.push(...wp);
      P.n.push(...normalOf(Mx, flip ? scl(s.n, -1) : s.n));
      P.t.push(...(uv ? uv(wp, u, v) : s.uv ?? [u, v]));
      const col = color(wp, u, v);
      P.c.push(col[0], col[1], col[2]);
    }
    for (let r = 0; r < rows - 1; r++) for (let c = 0; c < cols - 1; c++) {
      const a = base + r * cols + c, b = a + 1, d = a + cols, e = d + 1;
      if (flip) P.i.push(a, b, d, b, e, d); else P.i.push(a, d, b, b, d, e);
    }
    return this;
  }
  // A surface of revolution around +Y from a profile of [radius, y].
  lathe(mat, Mx, profile, seg = 16, opts = {}) {
    const { wobble = () => 0 } = opts, rows = profile.length;
    const at = (u, v) => {
      const k = Math.round(v * (rows - 1)), [r0, y] = profile[k];
      const a = u * Math.PI * 2, r = r0 * (1 + wobble(a, v));
      const [rp, yp] = profile[Math.max(0, k - 1)], [rn, yn] = profile[Math.min(rows - 1, k + 1)];
      const dr = rn - rp, dy = yn - yp || 1e-4;
      return { p: [Math.cos(a) * r, y, Math.sin(a) * r], n: norm([Math.cos(a) * dy, -dr, Math.sin(a) * dy]), uv: [u * 2, y * 2] };
    };
    return this.grid(mat, Mx, rows, seg + 1, at, opts);
  }
  sphere(mat, Mx, seg = 12, rings = 8, opts = {}) {
    const prof = [];
    for (let i = 0; i <= rings; i++) { const t = -Math.PI / 2 + Math.PI * i / rings; prof.push([Math.max(1e-3, Math.cos(t)), Math.sin(t)]); }
    return this.lathe(mat, Mx, prof, seg, opts);
  }
  cylinder(mat, Mx, r0, r1, h, seg = 10, opts = {}) {
    const prof = [[1e-3, 0], [r0, 0]];
    for (let i = 1; i <= (opts.rows ?? 3); i++) prof.push([r0 + (r1 - r0) * i / (opts.rows ?? 3), h * i / (opts.rows ?? 3)]);
    prof.push([1e-3, h]);
    return this.lathe(mat, Mx, prof, seg, opts);
  }
  box(mat, Mx, w, h, d, opts = {}) {
    const faces = [[[1, 0, 0], [0, 0, -1], [0, 1, 0]], [[-1, 0, 0], [0, 0, 1], [0, 1, 0]], [[0, 1, 0], [1, 0, 0], [0, 0, -1]], [[0, -1, 0], [1, 0, 0], [0, 0, 1]], [[0, 0, 1], [1, 0, 0], [0, 1, 0]], [[0, 0, -1], [-1, 0, 0], [0, 1, 0]]];
    for (let [n, u, v] of faces) {
      if (cross(v, u).reduce((s, x, i) => s + x * n[i], 0) < 0) [u, v] = [v, u];
      const len = e => Math.abs(e[0]) * w + Math.abs(e[1]) * h + Math.abs(e[2]) * d;
      this.grid(mat, Mx, 2, 2, (a, b) => ({
        p: [0, 1, 2].map(i => (n[i] * 0.5 + u[i] * (a - 0.5) + v[i] * (b - 0.5)) * [w, h, d][i]),
        n, uv: [a * len(u), b * len(v)],
      }), opts);
    }
    return this;
  }
  // A leaf blade along +Z, cupped, curling down by `bend`; its material is
  // double-sided. The middle column is the midrib.
  leaf(mat, Mx, len, wid, bend = 0.4, opts = {}) {
    const at = (u, v) => {
      const z = v * len, w = wid * Math.sin(Math.PI * Math.min(1, v * 1.05)) ** 0.8;
      return { p: [(u - 0.5) * w, -bend * v * v * len + Math.abs(u - 0.5) * w * 0.3, z], n: norm([0, 1, bend * v * 2]) };
    };
    return this.grid(mat, Mx, opts.rows ?? 5, 3, at, opts);
  }
}
// Leaf shading: dark at the base, the crop's colour toward the tip, a pale midrib.
const veined = (base, lift = 1) => (p, u, v) => {
  const c = mix(scl(base, 0.55), mix(base, [0.9, 1, 0.55].map((x, i) => x * base[i] * 1.25), 0.25 * lift), 0.3 + 0.7 * v);
  return u === 0.5 ? mix(c, [0.85, 0.95, 0.6], 0.25) : c;
};
const flat = c => () => c;

// ------------------------------------------------------------ binary glTF
// Linear vertex colours as normalised bytes, alpha one.
const rgba8 = c => {
  const out = new Uint8Array((c.length / 3) * 4);
  for (let i = 0, o = 0; i < c.length; i += 3, o += 4) {
    for (let k = 0; k < 3; k++) out[o + k] = Math.round(Math.max(0, Math.min(1, c[i + k])) * 255);
    out[o + 3] = 255;
  }
  return out;
};
function writeGlb(name, model) {
  const json = { asset: { version: '2.0', generator: 'garden art.mjs' }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0, name }], meshes: [{ primitives: [] }], materials: [], accessors: [], bufferViews: [], buffers: [] };
  const chunks = []; let length = 0;
  const view = (bytes, target) => {
    const pad = (4 - (length % 4)) % 4;
    if (pad) { chunks.push(new Uint8Array(pad)); length += pad; }
    json.bufferViews.push({ buffer: 0, byteOffset: length, byteLength: bytes.byteLength, target });
    chunks.push(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)); length += bytes.byteLength;
    return json.bufferViews.length - 1;
  };
  const accessor = (typed, type, componentType, target, extra = {}) => {
    const width = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 }[type];
    json.accessors.push({ bufferView: view(typed, target), componentType, count: typed.length / width, type, ...extra });
    return json.accessors.length - 1;
  };
  const images = new Map();
  const tex = key => {
    if (!images.has(key)) {
      json.images ??= []; json.textures ??= []; json.samplers ??= [{ magFilter: 9729, minFilter: 9987, wrapS: 10497, wrapT: 10497 }];
      json.images.push({ uri: `textures/${key}.png` });
      json.textures.push({ source: json.images.length - 1, sampler: 0 });
      images.set(key, json.textures.length - 1);
    }
    return images.get(key);
  };
  const used = new Set();
  for (const [matName, P] of model.parts) {
    if (!P.i.length) continue;
    const m = P.spec, color = m.color ?? [1, 1, 1];
    const mat = { name: matName, pbrMetallicRoughness: { baseColorFactor: [...color.slice(0, 3), color[3] ?? 1], metallicFactor: m.metal ?? 0, roughnessFactor: m.rough ?? 0.5 } };
    if (m.tex) mat.pbrMetallicRoughness.baseColorTexture = { index: tex(m.tex) };
    if (m.emissive) {
      mat.emissiveFactor = m.emissive;
      if (m.strength && m.strength !== 1) { mat.extensions = { KHR_materials_emissive_strength: { emissiveStrength: m.strength } }; used.add('KHR_materials_emissive_strength'); }
    }
    if (m.double) mat.doubleSided = true;
    json.materials.push(mat);
    const count = P.p.length / 3, min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < P.p.length; i++) { min[i % 3] = Math.min(min[i % 3], P.p[i]); max[i % 3] = Math.max(max[i % 3], P.p[i]); }
    const attributes = {
      POSITION: accessor(new Float32Array(P.p), 'VEC3', 5126, 34962, { min, max }),
      NORMAL: accessor(new Float32Array(P.n), 'VEC3', 5126, 34962),
      COLOR_0: accessor(rgba8(P.c), 'VEC4', 5121, 34962, { normalized: true }),
    };
    if (m.tex) attributes.TEXCOORD_0 = accessor(new Float32Array(P.t), 'VEC2', 5126, 34962);
    const indices = count < 65536 ? accessor(new Uint16Array(P.i), 'SCALAR', 5123, 34963) : accessor(new Uint32Array(P.i), 'SCALAR', 5125, 34963);
    json.meshes[0].primitives.push({ attributes, indices, material: json.materials.length - 1 });
  }
  if (used.size) json.extensionsUsed = [...used];
  const pad4 = (b, fill) => Buffer.concat([b, Buffer.alloc((4 - (b.length % 4)) % 4, fill)]);
  const bin = pad4(Buffer.concat(chunks.map(c => Buffer.from(c))), 0);
  json.buffers.push({ byteLength: bin.length });
  const text = pad4(Buffer.from(JSON.stringify(json)), 0x20);
  const header = Buffer.alloc(12), jh = Buffer.alloc(8), bh = Buffer.alloc(8);
  header.writeUInt32LE(0x46546c67, 0); header.writeUInt32LE(2, 4); header.writeUInt32LE(12 + 8 + text.length + 8 + bin.length, 8);
  jh.writeUInt32LE(text.length, 0); jh.writeUInt32LE(0x4e4f534a, 4);
  bh.writeUInt32LE(bin.length, 0); bh.writeUInt32LE(0x004e4942, 4);
  const out = Buffer.concat([header, jh, text, bh, bin]);
  writeFileSync(resolve(ART, `${name}.glb`), out);
  return { bytes: out.length, triangles: [...model.parts.values()].reduce((s, P) => s + P.i.length / 3, 0) };
}

// ------------------------------------------------------------ the crops
// Mirrors crops.rs: id, look, mature height, leaf colour, fruit colour (both
// as crops.rs writes them), fruit radius, fruit shape.
const CROPS = [
  ['carrot', 'fronds', 0.4, [0.3, 0.7, 0.25], [0.95, 0.5, 0.1], 0.22, 'root'],
  ['strawberry', 'bush', 0.5, [0.25, 0.6, 0.25], [0.9, 0.12, 0.15], 0.14, 'berry'],
  ['blueberry', 'bush', 0.6, [0.2, 0.5, 0.3], [0.25, 0.3, 0.85], 0.12, 'blue'],
  ['tomato', 'staked', 0.9, [0.25, 0.55, 0.2], [0.85, 0.15, 0.1], 0.2, 'tomato'],
  ['corn', 'stalk', 1.4, [0.45, 0.65, 0.2], [0.95, 0.85, 0.25], 0.18, 'cob'],
  ['watermelon', 'vine', 0.5, [0.2, 0.5, 0.2], [0.2, 0.6, 0.25], 0.55, 'melon'],
  ['pumpkin', 'vine', 0.5, [0.3, 0.5, 0.2], [0.95, 0.55, 0.1], 0.5, 'pumpkin'],
  ['apple', 'tree', 2.4, [0.2, 0.45, 0.2], [0.8, 0.1, 0.12], 0.2, 'apple'],
  ['bamboo', 'bamboo', 2.8, [0.45, 0.75, 0.3], [0.5, 0.8, 0.35], 0.25, 'shoot'],
  ['coconut', 'palm', 3.2, [0.25, 0.5, 0.2], [0.45, 0.3, 0.15], 0.28, 'coconut'],
  ['cactus', 'cactus', 1.8, [0.3, 0.6, 0.35], [0.9, 0.4, 0.6], 0.2, 'pear'],
  ['dragon', 'dragon', 1.6, [0.35, 0.6, 0.3], [0.95, 0.2, 0.55], 0.24, 'dragon'],
  ['mango', 'tree', 2.6, [0.2, 0.5, 0.2], [1.0, 0.65, 0.15], 0.24, 'mango'],
  ['grape', 'trellis', 1.4, [0.25, 0.45, 0.25], [0.45, 0.15, 0.55], 0.16, 'grapes'],
];
// Shared with pass.rs, which hangs fruit on these: a tree's canopy centre
// height and radius when mature, a palm's lean and trunk length.
const CANOPY = { apple: { y: 1.78, r: 0.8 }, mango: { y: 1.95, r: 0.85 } };
const PALM = { lean: 0.12, trunk: 2.88 };
const Y0 = 0.08; // the top of the soil mound, where stems start
const SOIL = hex('#7a5232');
const BROWN = hex('#6d4c41');

function mound(m, d, r) {
  const n = d ? 4 : 2, prof = [];
  for (let i = 0; i <= n; i++) { const t = i / n; prof.push([Math.max(0.001, r * Math.cos(t * Math.PI / 2)), 0.1 * Math.sin(t * Math.PI / 2)]); }
  m.lathe('soil', I, prof, d ? 14 : 6, {
    wobble: (a, v) => 0.06 * Math.sin(a * 5 + v * 3) + 0.03 * Math.sin(a * 13),
    color: p => scl(SOIL, 0.8 + 0.25 * noise2(p[0] * 9 + 4, p[2] * 9 + 4)),
    uv: p => [p[0] * 1.6, p[2] * 1.6],
  });
  if (d) for (let i = 0; i < 4; i++) {
    const a = i * 1.7 + 0.5, rr = r * (0.55 + 0.1 * (i % 2));
    m.sphere('stone', chain(T(Math.cos(a) * rr, 0.06, Math.sin(a) * rr), S(0.035, 0.022, 0.03)), 5, 3, { color: flat(grey(0.55)) });
  }
}

// One stage of one crop (0 is the sprout, 4 mature). `d` 1 is the full
// model; 0 the far level of detail: fewer segments and leaves, no trim.
function plant([id, look, h, leaf0], stage, d) {
  const m = new Model(), R = rng(id.length * 131 + stage * 7 + 1);
  const leafC = sq(leaf0), g = stage / 4, hh = h * (0.3 + 0.7 * g);
  const seg = (a, b) => (d ? a : b), N = n => (d ? n : Math.max(2, Math.ceil(n / 2)));
  const rows = d ? 5 : 2, wide = d ? 1 : 1.3;
  const lc = veined(leafC), stemC = flat(scl(leafC, 0.75));
  mound(m, d, look === 'tree' || look === 'palm' ? 0.5 : 0.42);
  if (stage === 0) {
    m.cylinder('stem', T(0, Y0, 0), 0.014, 0.01, 0.08, seg(5, 3), { color: stemC, rows: 1 });
    for (const a of [0, Math.PI]) m.leaf('leaf', chain(T(0, Y0 + 0.07, 0), RY(a + 0.4), RX(-0.5)), 0.13, 0.09 * wide, 0.3, { color: lc, rows });
    return m;
  }
  const stem = (x, z, r0, r1, len, tilt = 0, yaw = 0, color = stemC) =>
    m.cylinder('stem', chain(T(x, Y0, z), RY(yaw), RZ(tilt)), r0, r1, len, seg(6, 3), { color, rows: d ? 3 : 1 });
  switch (look) {
    case 'fronds': { // carrot: feathery fronds arching out of the crown
      const n = N(3 + stage * 2);
      for (let i = 0; i < n; i++) {
        const a = (i / n) * Math.PI * 2 + R() * 0.4, up = 1.0 + R() * 0.35;
        const base = chain(T(0, Y0, 0), RY(a), RX(-up));
        m.leaf('leaf', base, hh * 0.95, 0.03, 0.55, { color: lc, rows });
        for (let k = 1; k <= (d ? 3 : 1); k++) for (const side of [-1, 1]) {
          m.leaf('leaf', chain(base, T(0, -0.02 * k * k, hh * 0.24 * k), RY(side * 0.9), RX(0.2)), hh * 0.28, 0.06 * wide, 0.3, { color: lc, rows: 3 });
        }
      }
      break;
    }
    case 'bush': case 'staked': {
      const n = N(5 + stage * 3), rad = 0.4 * (0.4 + 0.6 * g);
      if (look === 'staked') {
        m.box('wood', T(0.16, Y0 + hh * 0.55, 0.05), 0.035, hh * 1.1, 0.035, { color: flat(hex('#b08a5a')) });
        if (d) for (const y of [0.35, 0.7]) m.box('stem', T(0.14, Y0 + hh * y, 0.05), 0.08, 0.012, 0.05, { color: flat(hex('#d8c48c')) });
      }
      stem(0, 0, 0.03, 0.018, hh * 0.65);
      for (let i = 0; i < n; i++) {
        const a = i * 2.39996, y = Y0 + hh * (0.2 + 0.7 * (i / n)), r = rad * (0.5 + 0.5 * R());
        m.leaf('leaf', chain(T(Math.cos(a) * r * 0.3, y, Math.sin(a) * r * 0.3), RY(-a + Math.PI / 2), RX(-0.35 - R() * 0.4)),
          0.16 + 0.12 * g, (0.11 + 0.06 * g) * wide, 0.5, { color: lc, rows });
      }
      if (d && stage >= 3 && look === 'bush') for (let i = 0; i < 5; i++) { // blossoms
        const a = i * 1.9 + 0.3;
        m.sphere('plain', chain(T(Math.cos(a) * rad * 0.7, Y0 + hh * (0.55 + 0.08 * (i % 3)), Math.sin(a) * rad * 0.7), S(0.025)), 5, 3, { color: flat(hex('#fff8e6')) });
      }
      break;
    }
    case 'stalk': { // corn: stalks with long arching blades and tassels
      const n = 1 + Math.min(stage, 3);
      for (let s = 0; s < n; s++) {
        const x = (s - 1.5) * 0.14, z = (s % 2) * 0.1 - 0.05;
        const len = hh * (s % 2 ? 0.92 : 1);
        stem(x, z, 0.035, 0.02, len, 0, 0, flat(scl(leafC, 0.85)));
        for (let k = 0; k < N(2 + stage); k++) {
          m.leaf('leaf', chain(T(x, Y0 + len * (0.15 + 0.17 * k), z), RY(k * 2.4 + s), RX(-0.85)), len * 0.5, 0.07 * wide, 0.9, { color: lc, rows });
        }
        if (stage === 4) for (let k = 0; k < (d ? 5 : 2); k++) {
          m.cylinder('stem', chain(T(x, Y0 + len, z), RY(k * 1.25), RZ(0.35)), 0.01, 0.002, 0.2, 3, { color: flat(hex('#d8b55a')), rows: 1 });
        }
      }
      break;
    }
    case 'vine': { // melon and pumpkin: broad leaves sprawling over the soil
      const n = N(3 + stage * 2);
      for (let i = 0; i < n; i++) {
        const a = i * 2.2, r = 0.22 + 0.42 * g * (0.6 + 0.4 * R());
        m.cylinder('stem', chain(T(0, 0.1, 0), RY(a), RZ(Math.PI / 2 - 0.12)), 0.014, 0.009, r + 0.12, seg(5, 3), { color: stemC, rows: 1 });
        m.leaf('leaf', chain(T(Math.cos(a) * r, 0.12 + 0.05 * R(), -Math.sin(a) * r), RY(a + Math.PI / 2), RX(-0.1)),
          0.22 + 0.14 * g, (0.24 + 0.14 * g) * wide, 0.15, { color: lc, rows });
        if (d && i % 2) m.cylinder('stem', chain(T(Math.cos(a) * r * 1.3, 0.14, -Math.sin(a) * r * 1.3), RZ(0.5 + R())), 0.004, 0.002, 0.15, 3, { color: stemC, rows: 1 });
      }
      break;
    }
    case 'tree': { // apple and mango: a bark trunk under a lumpy canopy
      const C = CANOPY[id], k = 0.3 + 0.7 * g, top = C.y - C.r * 0.45;
      const at = (...ms) => chain(S(k), ...ms);
      m.cylinder('bark', at(T(0, Y0 - 0.02, 0)), 0.12, 0.075, top - Y0, seg(10, 5), { color: flat(hex('#8a6a50')), rows: d ? 3 : 1 });
      if (d) for (let i = 0; i < 3; i++) m.cylinder('bark', at(T(0, top - 0.1, 0), RY(i * 2.1), RZ(0.7)), 0.05, 0.025, C.r * 0.7, 6, { color: flat(hex('#8a6a50')), rows: 1 });
      const tone = (p, base) => {
        const up = (p[1] / k - C.y) / C.r;
        return scl(mix(scl(base, 0.55), mix(base, [0.85, 1, 0.5], 0.15), 0.5 + 0.5 * up), 0.85 + 0.3 * noise2(p[0] * 5 + 9, p[2] * 5 + p[1] * 3));
      };
      m.sphere('canopy', at(T(0, C.y, 0), S(C.r, C.r * 0.9, C.r)), seg(14, 7), seg(9, 5), {
        color: p => tone(p, leafC), wobble: (a, v) => 0.08 * Math.sin(a * 5 + v * 9) + 0.04 * Math.sin(a * 11 + v * 4),
      });
      if (d) {
        for (let i = 0; i < 6; i++) {
          const a = i * 1.05 + 0.3, e = 0.25 + 0.3 * (i % 2), s = C.r * (0.42 + 0.08 * R());
          const c = [Math.cos(a) * Math.cos(e) * C.r * 0.75, C.y + Math.sin(e) * C.r * 0.7, Math.sin(a) * Math.cos(e) * C.r * 0.75];
          m.sphere('canopy', at(T(...c), S(s, s * 0.85, s)), 9, 6, { color: p => tone(p, leafC) });
        }
        for (let i = 0; i < 18; i++) {
          const a = i * 2.39996, e = -0.2 + (i % 5) * 0.22;
          const c = [Math.cos(a) * Math.cos(e) * C.r * 0.97, C.y + Math.sin(e) * C.r * 0.88, Math.sin(a) * Math.cos(e) * C.r * 0.97];
          m.leaf('leaf', at(T(...c), RY(-a + Math.PI / 2), RX(-0.2)), 0.2, 0.1, 0.3, { color: lc, rows: 3 });
        }
      }
      break;
    }
    case 'palm': { // coconut: a ringed, leaning trunk and drooping fronds
      const k = 0.3 + 0.7 * g, lean = RZ(PALM.lean), rings = d ? 9 : 3;
      for (let i = 0; i < rings; i++) {
        const y0 = (PALM.trunk * i) / rings, y1 = (PALM.trunk * (i + 1)) / rings, r0 = 0.13 - 0.04 * (i / rings);
        m.lathe('bark', chain(S(k), T(0, Y0, 0), lean, T(0, y0, 0)), [[0.001, 0], [r0, 0], [r0 * 0.92, (y1 - y0) * 0.85], [r0 * 1.05, y1 - y0]], seg(10, 5), { color: flat(hex('#9a7a58')) });
      }
      const top = [-Math.sin(PALM.lean) * PALM.trunk, Y0 + Math.cos(PALM.lean) * PALM.trunk, 0];
      for (let i = 0; i < (d ? 9 : 6); i++) {
        m.leaf('leaf', chain(S(k), T(...top), RY(i * 0.7 + R() * 0.3), RX(-0.3 - (i % 3) * 0.15)), 0.7 + 0.9 * g, 0.3 * wide, 0.65, { color: lc, rows: d ? 7 : 3 });
      }
      m.sphere('bark', chain(S(k), T(top[0], top[1] - 0.04, 0), S(0.15, 0.12, 0.15)), seg(8, 5), 4, { color: flat(hex('#7a5a40')) });
      break;
    }
    case 'bamboo': {
      const n = 1 + stage;
      for (let s = 0; s < n; s++) {
        const a = s * 2.4, x = Math.cos(a) * 0.15 * (s ? 1 : 0), z = Math.sin(a) * 0.15 * (s ? 1 : 0);
        const len = hh * (0.7 + 0.3 * R()), joints = Math.max(2, Math.round(len / 0.45));
        for (let j = 0; j < joints; j++) {
          m.cylinder('stem', T(x, Y0 + (j * len) / joints, z), 0.04, 0.037, len / joints - 0.02, seg(8, 4), { color: flat(mix(hex('#9ccc65'), hex('#c5e1a5'), j / joints)), rows: 1 });
          if (d) m.cylinder('stem', T(x, Y0 + ((j + 1) * len) / joints - 0.025, z), 0.046, 0.046, 0.025, 8, { color: flat(hex('#7cb342')), rows: 1 });
          if (d && j > 0 && j % 2 === 0) m.leaf('leaf', chain(T(x, Y0 + (j * len) / joints, z), RY(a + j), RX(-0.6)), 0.3, 0.06, 0.5, { color: lc, rows: 3 });
        }
        m.leaf('leaf', chain(T(x, Y0 + len, z), RY(a), RX(-0.5)), 0.3, 0.06 * wide, 0.5, { color: lc, rows });
      }
      break;
    }
    case 'cactus': case 'dragon': {
      const ribs = (a2) => 0.12 * Math.abs(Math.sin(a2 * 4)), col = (p, u) => scl(leafC, (0.62 + 0.38 * Math.abs(Math.sin(u * Math.PI * 8))) * (0.85 + 0.2 * (p[1] / (hh + 0.1))));
      const thin = look === 'dragon' ? 0.75 : 1;
      m.lathe('stem', chain(T(0, Y0, 0), S(thin, 1, thin)), [[0.001, 0], [0.16, 0.02], [0.18, hh * 0.3], [0.17, hh * 0.8], [0.12, hh * 0.95], [0.001, hh]], seg(16, 8), { wobble: ribs, color: col });
      if (stage >= 2) for (const side of [-1, 1]) {
        const y = hh * (0.35 + 0.15 * side);
        const arm = look === 'dragon' ? [[0.001, 0], [0.07, 0.02], [0.08, hh * 0.35], [0.001, hh * 0.4]] : [[0.001, 0], [0.09, 0.02], [0.1, hh * 0.3], [0.001, hh * 0.34]];
        m.lathe('stem', chain(T(side * 0.14 * thin, Y0 + y, 0), RZ(-side * (look === 'dragon' ? 1.0 : 0.25))), arm, seg(12, 6), { wobble: ribs, color: col });
      }
      if (d) for (let i = 0; i < 10; i++) { // spines
        const a = i * 2.39996, y = Y0 + hh * (0.15 + 0.07 * i);
        m.cylinder('plain', chain(T(Math.cos(a) * 0.17 * thin, y, Math.sin(a) * 0.17 * thin), RY(-a), RZ(-Math.PI / 2)), 0.006, 0.001, 0.06, 3, { color: flat(hex('#f4ecd0')), rows: 1 });
      }
      if (d && look === 'cactus' && stage === 4) m.sphere('plain', chain(T(0, Y0 + hh + 0.02, 0), S(0.06, 0.04, 0.06)), 6, 3, { color: flat(hex('#ff9ec4')) });
      break;
    }
    case 'trellis': { // grape: vines climbing a wooden trellis
      const wood = flat(hex('#a7825a'));
      for (const x of [-0.5, 0.5]) m.box('wood', T(x, Y0 + hh * 0.5, -0.3), 0.06, hh, 0.06, { color: wood });
      m.box('wood', T(0, Y0 + hh * 0.98, -0.3), 1.1, 0.05, 0.05, { color: wood });
      if (d) m.box('wood', T(0, Y0 + hh * 0.55, -0.3), 1.0, 0.035, 0.035, { color: wood });
      stem(0, -0.22, 0.03, 0.015, hh * 0.92, 0.08, 0, flat(BROWN));
      for (let i = 0; i < N(4 + stage * 3); i++) {
        const x = (R() - 0.5) * 0.95, y = 0.25 + R() * hh * 0.78;
        m.leaf('leaf', chain(T(x, y, -0.25), RY(R() * 6), RX(-0.4)), 0.18, 0.17 * wide, 0.3, { color: lc, rows });
      }
      break;
    }
  }
  return m;
}

// The fruit of a crop, ripe or unripe. Radius 1 is the crop's fruit size.
// Material 0 is the body: its factor is the fruit's hue and its vertex
// colours a pattern (stripes, seeds, kernels) that every mutation keeps.
function fruit([id, , , , fruit0, size, shape], unripe, d) {
  const m = new Model(), R = rng(id.length * 7 + 3);
  const hue = unripe ? mix(sq(fruit0), sq([0.55, 0.78, 0.3]), 0.72) : sq(fruit0);
  m.material('body', { color: hue });
  const seg = (a, b) => (d ? a : b), S1 = S(size);
  const pat = f => (d ? f : () => grey(0.95));
  const LEAF = veined(sq([0.3, 0.62, 0.26])), STEM = flat(BROWN);
  const calyx = (y, n, len, wid, bend = -0.1) => { if (d) for (let i = 0; i < n; i++) m.leaf('leaf', chain(S1, T(0, y, 0), RY(i * ((Math.PI * 2) / n)), RX(-1.4)), len, wid, bend, { color: LEAF, rows: 3 }); };
  const stalk = (y, len) => { if (d) m.cylinder('stem', chain(S1, T(0, y, 0)), 0.06, 0.035, len, 5, { color: STEM, rows: 1 }); };
  switch (shape) {
    case 'root':
      m.lathe('body', S1, [[0.001, -1.4], [0.18, -1.1], [0.35, -0.4], [0.42, 0.1], [0.3, 0.3], [0.001, 0.32]], seg(12, 6), { color: pat((p, u, v) => grey(0.84 + 0.16 * Math.sin(v * 40))) });
      for (const [a, len] of [[0, 1.3], [2.1, 1.1], [4.2, 1.2]]) if (d || a === 0) m.leaf('leaf', chain(S1, T(0, 0.3, 0), RY(a), RX(-1.25)), len, 0.3, 0.25, { color: LEAF, rows: 4 });
      break;
    case 'berry':
      m.lathe('body', S1, [[0.001, -1], [0.45, -0.75], [0.85, -0.1], [0.9, 0.3], [0.6, 0.6], [0.001, 0.68]], seg(14, 6), { color: pat((p, u, v) => grey(hash3(u * 14, v * 9, 3) > 0.86 ? 0.62 : 1)) });
      calyx(0.62, 5, 0.45, 0.22);
      break;
    case 'blue':
      m.sphere('body', chain(S1, S(1, 0.86, 1)), seg(12, 6), seg(8, 4), { color: pat((p, u, v) => grey(0.86 + 0.14 * noise2(u * 9, v * 6))) });
      calyx(0.82, 5, 0.3, 0.16, 0.2);
      break;
    case 'tomato':
      m.sphere('body', chain(S1, S(1, 0.8, 1)), seg(16, 7), seg(9, 5), { color: pat((p, u, v) => grey(0.9 + 0.1 * v)), wobble: a => 0.05 * Math.cos(a * 5) });
      calyx(0.78, 5, 0.42, 0.15);
      stalk(0.78, 0.25);
      break;
    case 'cob':
      m.lathe('body', chain(S1, S(0.55, 1, 0.55)), [[0.001, -1.4], [0.7, -1.2], [0.85, 0], [0.7, 1.2], [0.001, 1.45]], seg(14, 6), { color: pat((p, u, v) => grey((Math.floor(u * 28) + Math.floor(v * 18)) % 2 ? 1 : 0.8)) });
      for (let i = 0; i < (d ? 3 : 1); i++) m.leaf('leaf', chain(S1, T(0, -1.3, 0), RY(i * 2.1), RX(-0.25)), 2.2, 0.6, -0.3, { color: veined(sq([0.62, 0.8, 0.4])), rows: 4 });
      break;
    case 'melon':
      m.sphere('body', chain(S1, S(1, 0.78, 1.25)), seg(18, 8), seg(10, 5), { color: pat((p, u) => grey(Math.sin(u * Math.PI * 22) > 0.3 ? 0.48 : 1)) });
      stalk(0.75, 0.2);
      break;
    case 'pumpkin':
      m.sphere('body', chain(S1, S(1, 0.7, 1)), seg(20, 8), seg(10, 5), { color: pat((p, u) => grey(0.8 + 0.2 * Math.cos(u * Math.PI * 16))), wobble: a => 0.08 * Math.cos(a * 8) });
      if (d) m.cylinder('stem', chain(S1, T(0, 0.6, 0), RZ(0.3)), 0.1, 0.06, 0.35, 6, { color: flat(hex('#6d5a2e')), rows: 1 });
      break;
    case 'apple':
      m.lathe('body', S1, [[0.001, -0.85], [0.55, -0.8], [0.95, -0.2], [0.9, 0.5], [0.4, 0.82], [0.001, 0.7]], seg(16, 7), { color: pat((p, u, v) => grey(0.82 + 0.18 * noise2(u * 18, v * 3))) });
      stalk(0.7, 0.35);
      if (d) m.leaf('leaf', chain(S1, T(0, 0.95, 0), RX(-0.9)), 0.7, 0.3, 0.2, { color: LEAF, rows: 3 });
      break;
    case 'shoot':
      m.lathe('body', S1, [[0.001, -0.2], [0.45, -0.2], [0.42, 0.3], [0.32, 0.8], [0.18, 1.25], [0.001, 1.5]], seg(10, 5), { color: pat((p, u, v) => grey(Math.floor(v * 6 + u * 1.5) % 2 ? 1 : 0.78)) });
      break;
    case 'coconut':
      m.sphere('body', chain(S1, S(1, 0.95, 1)), seg(12, 6), seg(8, 4), { color: pat((p, u, v) => grey(0.7 + 0.3 * noise2(u * 30, v * 5))) });
      break;
    case 'pear':
      m.lathe('body', S1, [[0.001, -0.9], [0.5, -0.75], [0.75, -0.1], [0.62, 0.55], [0.32, 0.8], [0.001, 0.82]], seg(12, 6), { color: pat((p, u, v) => grey(hash3(u * 12, v * 8, 5) > 0.88 ? 0.7 : 1)) });
      break;
    case 'dragon':
      m.sphere('body', chain(S1, S(0.85, 1.05, 0.85)), seg(14, 7), seg(9, 5), { color: pat(() => grey(1)) });
      for (let i = 0; i < (d ? 9 : 3); i++) m.leaf('leaf', chain(S1, RY(i * 0.7), RX(-0.4 - (i % 3) * 0.5), T(0, 0.2, 0.75)), 0.6, 0.25, -0.4, { color: flat(sq([0.6, 0.85, 0.45])), rows: 3 });
      break;
    case 'mango':
      m.sphere('body', chain(S1, RZ(0.3), S(0.8, 1.1, 0.75)), seg(14, 7), seg(9, 5), { color: pat((p, u, v) => grey(0.8 + 0.2 * v)) });
      stalk(1.0, 0.2);
      break;
    case 'grapes':
      for (let i = 0; i < (d ? 14 : 5); i++) {
        const t = i / (d ? 14 : 5), a = i * 2.4, r = 0.55 * (1 - t);
        m.sphere('body', chain(S1, T(Math.cos(a) * r, 0.9 - t * 1.9, Math.sin(a) * r), S(d ? 0.42 : 0.55)), seg(8, 5), seg(6, 3), { color: flat(grey(0.8 + 0.2 * R())) });
      }
      stalk(0.9, 0.4);
      break;
  }
  return m;
}

// ------------------------------------------------------------ props
const PAINT = hex('#ece3cf'), TIMBER = hex('#a5794f');
function fencePost() {
  const m = new Model();
  m.box('wood', T(0, 0.47, 0), 0.13, 0.94, 0.13, { color: flat(PAINT) });
  m.lathe('wood', T(0, 0.94, 0), [[0.001, 0], [0.095, 0], [0.095, 0.02], [0.001, 0.11]], 4, { color: flat(PAINT) });
  return m;
}
function fenceRail() { // one 2 m span along +X, centred: two rails and three pickets
  const m = new Model();
  for (const y of [0.3, 0.66]) m.box('wood', T(0, y, 0), 2.0, 0.07, 0.04, { color: flat(scl(PAINT, 0.92)) });
  for (const x of [-0.5, 0, 0.5]) {
    m.box('wood', T(x, 0.4, 0.04), 0.09, 0.8, 0.025, { color: flat(PAINT) });
    m.lathe('wood', chain(T(x, 0.8, 0.04), RY(Math.PI / 4), S(1, 1, 0.3)), [[0.001, 0], [0.064, 0], [0.001, 0.07]], 4, { color: flat(PAINT) });
  }
  return m;
}
function lantern() { // a post with an arm and a hanging lamp; glass is material 0
  const m = new Model();
  m.material('glass');
  const dark = flat(hex('#5a4030'));
  m.box('wood', T(0, 0.95, 0), 0.11, 1.9, 0.11, { color: dark });
  m.box('wood', T(0.18, 1.82, 0), 0.42, 0.06, 0.06, { color: dark });
  m.box('wood', chain(T(0.07, 1.7, 0), RZ(-0.8)), 0.04, 0.24, 0.04, { color: dark });
  m.cylinder('iron', T(0.36, 1.71, 0), 0.008, 0.008, 0.1, 4, { rows: 1 });
  m.cylinder('iron', T(0.36, 1.38, 0), 0.1, 0.11, 0.035, 6, { rows: 1 });
  m.cylinder('glass', T(0.36, 1.41, 0), 0.085, 0.085, 0.2, 6, { rows: 1 });
  for (let i = 0; i < 6; i++) m.box('iron', chain(T(0.36, 1.51, 0), RY((i * Math.PI) / 3), T(0.088, 0, 0)), 0.012, 0.2, 0.012);
  m.lathe('iron', T(0.36, 1.61, 0), [[0.001, 0], [0.12, 0], [0.08, 0.06], [0.02, 0.1], [0.001, 0.11]], 6);
  return m;
}
function pathStone(seed) {
  const m = new Model(), R = rng(seed);
  for (let i = 0; i < 3; i++) {
    const x = (R() - 0.5) * 1.1, z = (R() - 0.5) * 1.1, r = 0.26 + R() * 0.12, ph = R() * 6;
    m.lathe('stone', T(x, 0, z), [[0.001, 0.0], [r, 0.0], [r * 0.94, 0.045], [0.001, 0.055]], 9, { wobble: a => 0.12 * Math.sin(a * 3 + ph), color: flat(grey(0.8 + 0.2 * R())), uv: p => [p[0] * 2, p[2] * 2] });
  }
  return m;
}
function tuft(seed) { // grass, and in every other tuft three flowers
  const m = new Model(), R = rng(seed);
  const g = sq([[0.34, 0.6, 0.2], [0.4, 0.66, 0.24], [0.28, 0.52, 0.17]][seed % 3]);
  for (let i = 0; i < 10; i++) m.leaf('leaf', chain(RY(R() * 6.28), RX(-1.15 - R() * 0.3)), 0.28 + R() * 0.22, 0.05, 0.3, { color: (p, u, v) => mix(scl(g, 0.55), g, v), rows: 3 });
  if (seed % 2) for (let i = 0; i < 3; i++) {
    const a = R() * 6.28, r = 0.12 + R() * 0.1, x = Math.cos(a) * r, z = Math.sin(a) * r, y = 0.24 + R() * 0.1;
    m.cylinder('stem', T(x, 0, z), 0.007, 0.005, y, 3, { color: flat(scl(g, 0.8)), rows: 1 });
    const petal = hex(['#ffffff', '#ffd54f', '#f48fb1', '#b39ddb'][(seed + i) % 4]);
    for (let k = 0; k < 5; k++) m.leaf('leaf', chain(T(x, y, z), RY(k * 1.256), RX(-1.4)), 0.065, 0.055, 0.0, { color: flat(petal), rows: 2 });
    m.sphere('plain', chain(T(x, y + 0.01, z), S(0.022)), 5, 3, { color: flat(hex('#f9a825')) });
  }
  return m;
}
function meadow() { // the ground beyond the garden: 600 m of turf in patches of colour
  const m = new Model(), n = 48, L = 600, g = sq([0.32, 0.55, 0.18]);
  m.grid('turf', T(0, 0, 0), n + 1, n + 1, (u, v) => ({ p: [(u - 0.5) * L, 0, (0.5 - v) * L], n: [0, 1, 0] }), {
    color: p => mix(scl(g, 0.78), mix(g, sq([0.5, 0.62, 0.22]), 0.5), fbm(p[0] / 40 + 50, p[2] / 40 + 50, 1e9)),
    uv: p => [p[0] / 2.5, p[2] / 2.5],
  });
  return m;
}
function grassPatch(seed) { // a 10 m square of blades and wild flowers, laid around the fence
  const m = new Model(), R = rng(seed);
  for (let i = 0; i < 420; i++) {
    const x = (R() - 0.5) * 10, z = (R() - 0.5) * 10, k = R();
    if (fbm(x / 4 + seed, z / 4, 1e9) < 0.38) continue;
    const g = sq(mix([0.26, 0.5, 0.14], [0.5, 0.7, 0.24], k));
    m.leaf('leaf', chain(T(x, 0, z), RY(R() * 6.28), RX(-1.1 - R() * 0.35)), 0.22 + R() * 0.25, 0.05, 0.3, { color: (p, u, v) => mix(scl(g, 0.5), g, v), rows: 3 });
  }
  for (let i = 0; i < 26; i++) {
    const x = (R() - 0.5) * 9.5, z = (R() - 0.5) * 9.5, y = 0.2 + R() * 0.15;
    m.cylinder('stem', T(x, 0, z), 0.007, 0.005, y, 3, { color: flat(sq([0.3, 0.5, 0.2])), rows: 1 });
    const petal = hex(['#ffffff', '#ffd54f', '#f48fb1', '#b39ddb', '#ff8a65'][i % 5]);
    for (let k = 0; k < 5; k++) m.leaf('leaf', chain(T(x, y, z), RY(k * 1.256), RX(-1.4)), 0.06, 0.05, 0.0, { color: flat(petal), rows: 2 });
  }
  return m;
}
function stall() { // the seed stall: counter, posts, striped awning, crates
  const m = new Model(), W = 3.4, D = 1.5, wood = flat(TIMBER), red = hex('#d84343'), white = hex('#f6f1e4');
  m.box('wood', T(0, 0.5, 0.1), W, 1.0, D * 0.55, { color: wood });
  m.box('wood', T(0, 1.03, 0.1), W + 0.15, 0.06, D * 0.68, { color: flat(scl(TIMBER, 1.15)) });
  for (let i = 0; i < 6; i++) m.box('wood', T(-W / 2 + 0.28 + i * ((W - 0.56) / 5), 0.5, 0.1 + D * 0.276), 0.05, 0.9, 0.02, { color: flat(scl(TIMBER, 0.8)) });
  for (const x of [-W / 2, W / 2]) for (const z of [-D / 2, D / 2]) m.box('wood', T(x, 1.35, z), 0.11, 2.7, 0.11, { color: wood });
  const awning = (flip) => m.grid('cloth', chain(T(0, 2.75, 0.1), RX(0.3)), 2, 25, (u, v) => ({ p: [(u - 0.5) * (W + 0.5), 0, (v - 0.5) * (D + 0.8)], n: [0, 1, 0] }), { color: (p, u) => (Math.floor(u * 12) % 2 ? white : red), flip });
  awning(false); awning(true);
  for (let i = 0; i < 12; i++) m.sphere('cloth', chain(T(-W / 2 - 0.1 + ((i + 0.5) * (W + 0.2)) / 12, 2.46, D / 2 + 0.43), S(0.16, 0.12, 0.03)), 8, 4, { color: flat(i % 2 ? white : red) });
  CROPS.slice(0, 8).forEach(([, , , , fruit0], i) => {
    const x = -W / 2 + 0.28 + i * 0.41;
    m.box('wood', T(x, 1.13, 0.15), 0.36, 0.14, 0.42, { color: flat(scl(TIMBER, 0.9)) });
    for (let k = 0; k < 5; k++) m.sphere('plain', chain(T(x + ((k % 3) - 1) * 0.1, 1.22 + (k > 2 ? 0.04 : 0), 0.15 + (k > 2 ? 0.05 : -0.06) + (k % 2) * 0.04), S(0.075)), 6, 4, { color: flat(sq(fruit0)) });
  });
  m.box('wood', T(0, 3.15, 0.35), 2.0, 0.55, 0.07, { color: flat(scl(TIMBER, 0.85)) });
  for (const x of [-0.75, 0.75]) m.box('wood', T(x, 2.95, 0.33), 0.05, 0.4, 0.05, { color: wood });
  for (let i = 0; i < 3; i++) m.box('wood', T(W / 2 + 0.45, 0.2 + i * 0.001, 0.6 - i * 0.45), 0.42, 0.4, 0.4, { color: flat(scl(TIMBER, 0.75 + 0.1 * i)) });
  for (let i = 0; i < 2; i++) m.cylinder('cloth', T(-W / 2 - 0.45, 0, 0.3 - i * 0.55), 0.18, 0.22, 0.42, 8, { color: flat(hex('#d9c08a')), rows: 2 });
  return m;
}
function barrel() { // the water barrel, centred on its origin like the classic cylinder
  const m = new Model();
  const prof = []; for (let i = 0; i <= 8; i++) { const t = i / 8; prof.push([0.56 + 0.07 * Math.sin(t * Math.PI), -0.5 + t]); }
  m.lathe('wood', I, [[0.001, -0.5], ...prof], 18, { color: (p, u) => scl(TIMBER, Math.floor(u * 18) % 2 ? 1 : 0.85), uv: (p, u, v) => [u * 6, v * 1.2] });
  for (const y of [-0.38, 0.0, 0.38]) { const r = 0.56 + 0.07 * Math.sin((y + 0.5) * Math.PI) + 0.008; m.lathe('iron', T(0, y, 0), [[r, -0.03], [r, 0.03]], 18); }
  m.lathe('wood', T(0, 0.4, 0), [[0.001, 0], [0.57, 0]], 18, { color: flat(scl(TIMBER, 0.6)), uv: p => [p[0], p[2]] });
  return m;
}
function can() { // a galvanised watering can, held by its handle
  const m = new Model();
  m.cylinder('tin', T(0, -0.2, 0), 0.17, 0.155, 0.28, 12, { rows: 2 });
  m.cylinder('tin', chain(T(0, -0.15, 0.12), RX(1.0)), 0.035, 0.022, 0.42, 6, { rows: 1 });
  m.sphere('tin', chain(T(0, 0.04, 0.46), RX(1.0), S(0.06, 0.02, 0.06)), 8, 4);
  for (const [y, z, a] of [[0.12, -0.12, 0.6], [0.2, 0, Math.PI / 2], [0.12, 0.12, 2.5]]) m.box('tin', chain(T(0, y, z), RX(a)), 0.03, 0.17, 0.035);
  m.lathe('tin', T(0, 0.08, 0), [[0.16, 0], [0.17, 0.015]], 12);
  return m;
}

// ------------------------------------------------------------ people
// Parts on the garden's gesture rig: the body hangs from the player's centre
// (0.9 m up), arms pivot at (±0.38, 0.18) and legs at (±0.18, −0.28), so the
// soles touch the ground. The same names as every look's gardener.
const PEOPLE = {
  farmer: { shirt: '#e8574a', pants: '#3f6fb5', skin: '#f2c49b', hair: '#7a4a24', hat: '#e9c46a', boot: '#5b3b26', bag: '#8a5a32' },
  keeper: { shirt: '#f3e7c6', pants: '#6d4c41', skin: '#c68863', hair: '#2b1b10', apron: '#2f8a4c', boot: '#3a2a20', scarf: '#f0a030' },
};
function head(m, p, y) {
  const skin = hex(p.skin), hair = hex(p.hair);
  m.sphere('skin', chain(T(0, y, 0), S(0.23, 0.25, 0.23)), 16, 10, { color: flat(skin) });
  m.sphere('plain', chain(T(0, y + 0.05, -0.045), S(0.236, 0.2, 0.22)), 14, 8, { color: flat(hair) });
  for (const x of [-0.08, 0.08]) {
    m.sphere('shiny', chain(T(x, y + 0.02, 0.2), S(0.036, 0.05, 0.03)), 8, 5, { color: flat(hex('#1d1a18')) });
    m.sphere('shiny', chain(T(x + 0.012, y + 0.04, 0.225), S(0.011)), 5, 3, { color: flat(grey(1)) });
    m.box('plain', chain(T(x, y + 0.095, 0.205), RZ(x > 0 ? -0.15 : 0.15)), 0.07, 0.016, 0.02, { color: flat(scl(hair, 0.8)) });
    m.sphere('skin', chain(T(x * 1.45, y - 0.05, 0.18), S(0.045, 0.028, 0.02)), 6, 3, { color: flat(mix(skin, hex('#f07070'), 0.55)) });
    m.sphere('skin', chain(T(x * 2.85, y, 0), S(0.035, 0.06, 0.03)), 6, 4, { color: flat(scl(skin, 0.92)) });
  }
  m.sphere('skin', chain(T(0, y - 0.015, 0.235), S(0.038, 0.034, 0.034)), 6, 4, { color: flat(scl(skin, 0.94)) });
  m.grid('plain', chain(T(0, y - 0.1, 0.212), RX(0.25)), 2, 6, (u, v) => ({ p: [(u - 0.5) * 0.1, -Math.sin(u * Math.PI) * 0.022 + v * 0.012, 0], n: [0, 0, 1] }), { color: flat(hex('#7a2e2e')) });
}
function body(who) {
  const p = PEOPLE[who], m = new Model(), shirt = hex(p.shirt), pants = hex(p.pants), skin = hex(p.skin);
  const squash = S(1, 1, 0.78);
  const lower = p.apron ? shirt : pants;
  m.lathe('cloth', squash, [[0.001, -0.4], [0.25, -0.39], [0.29, -0.3], [0.3, -0.1], [0.3, 0.02]], 16, { color: flat(lower) });
  m.lathe('cloth', squash, [[0.3, 0.0], [0.31, 0.12], [0.28, 0.22], [0.18, 0.28], [0.08, 0.31], [0.001, 0.32]], 16, { color: flat(shirt) });
  m.cylinder('skin', T(0, 0.27, 0), 0.085, 0.08, 0.1, 10, { color: flat(skin), rows: 1 });
  head(m, p, 0.55);
  if (p.hat) { // a straw hat with a red band
    const straw = flat(hex(p.hat));
    m.lathe('straw', T(0, 0.71, 0), [[0.2, 0.012], [0.44, -0.03], [0.46, -0.045], [0.44, -0.05], [0.2, -0.008]], 22, { color: straw, uv: pt => [pt[0] * 3, pt[2] * 3] });
    m.lathe('straw', T(0, 0.7, 0), [[0.205, 0], [0.215, 0.1], [0.18, 0.165], [0.001, 0.185]], 18, { color: straw });
    m.lathe('cloth', T(0, 0.705, 0), [[0.218, 0], [0.222, 0.05]], 18, { color: flat(hex('#c0392b')) });
    // Overall bib, straps, brass buttons and a satchel at the hip.
    m.box('cloth', T(0, 0.1, 0.222), 0.36, 0.26, 0.04, { color: flat(pants) });
    m.box('cloth', T(0, 0.12, 0.244), 0.13, 0.09, 0.012, { color: flat(scl(pants, 0.82)) });
    for (const x of [-0.13, 0.13]) {
      m.box('cloth', chain(T(x, 0.22, 0.05), RX(-0.35)), 0.055, 0.04, 0.38, { color: flat(pants) });
      m.sphere('shiny', chain(T(x, 0.21, 0.245), S(0.022, 0.022, 0.012)), 6, 3, { color: flat(hex('#d9a93a')) });
    }
    m.box('plain', chain(T(0.32, -0.16, -0.21), RY(0.3)), 0.29, 0.32, 0.22, { color: flat(hex(p.bag)) });
    m.box('plain', chain(T(0.32, -0.04, -0.21), RY(0.3)), 0.31, 0.1, 0.24, { color: flat(scl(hex(p.bag), 1.2)) });
    m.box('plain', chain(T(0.05, 0.02, 0.0), RZ(0.75), squash), 0.05, 0.85, 0.64, { color: flat(scl(hex(p.bag), 0.7)) });
  }
  if (p.apron) { // an apron, a headscarf and a moustache
    m.box('cloth', T(0, -0.13, 0.225), 0.4, 0.5, 0.03, { color: flat(hex(p.apron)) });
    m.box('cloth', T(0, 0.16, 0.21), 0.24, 0.16, 0.03, { color: flat(hex(p.apron)) });
    m.box('cloth', chain(T(0, 0.03, 0.0), squash), 0.62, 0.05, 0.62, { color: flat(scl(hex(p.apron), 0.8)) });
    m.lathe('cloth', T(0, 0.62, -0.01), [[0.245, 0], [0.24, 0.06], [0.2, 0.13], [0.001, 0.19]], 16, { color: flat(hex(p.scarf)) });
    m.sphere('cloth', chain(T(0, 0.6, -0.25), S(0.07, 0.06, 0.05)), 6, 4, { color: flat(hex(p.scarf)) });
    for (const x of [-0.05, 0.05]) m.sphere('plain', chain(T(x, 0.485, 0.222), RZ(x > 0 ? 0.35 : -0.35), S(0.06, 0.022, 0.025)), 7, 4, { color: flat(hex(p.hair)) });
  }
  return m;
}
function arm(who) { // hangs from the shoulder at the origin
  const p = PEOPLE[who], m = new Model(), shirt = flat(hex(p.shirt)), skin = flat(hex(p.skin));
  m.sphere('cloth', S(0.1), 10, 6, { color: shirt });
  m.cylinder('cloth', T(0, -0.27, 0), 0.08, 0.092, 0.27, 9, { color: shirt, rows: 1 });
  m.cylinder('cloth', T(0, -0.29, 0), 0.088, 0.088, 0.04, 9, { color: flat(scl(hex(p.shirt), 0.85)), rows: 1 });
  m.cylinder('skin', T(0, -0.45, 0), 0.058, 0.066, 0.18, 8, { color: skin, rows: 1 });
  m.sphere('skin', chain(T(0, -0.495, 0.01), S(0.07, 0.075, 0.065)), 8, 5, { color: skin });
  return m;
}
function leg(who) { // hangs from the hip at the origin; the sole is at -0.62
  const p = PEOPLE[who], m = new Model(), boot = flat(hex(p.boot));
  m.cylinder('cloth', T(0, -0.47, 0), 0.1, 0.105, 0.47, 9, { color: flat(hex(p.pants)), rows: 1 });
  m.cylinder('plain', T(0, -0.6, 0), 0.098, 0.1, 0.14, 9, { color: boot, rows: 1 });
  m.box('plain', T(0, -0.575, 0.06), 0.17, 0.09, 0.27, { color: boot });
  m.box('plain', T(0, -0.612, 0.06), 0.18, 0.016, 0.28, { color: flat(hex('#2a201a')) });
  return m;
}

// ------------------------------------------------------------ write
mkdirSync(resolve(ART, 'textures'), { recursive: true });
for (const f of readdirSync(ART)) if (f.endsWith('.glb') || f.endsWith('.gltf')) rmSync(resolve(ART, f));
for (const [name, make] of Object.entries(TEXTURES)) writeFileSync(resolve(ART, 'textures', `${name}.png`), make());
let files = 0, bytes = 0, tris = 0;
const out = (name, m) => { const r = writeGlb(name, m); files++; bytes += r.bytes; tris += r.triangles; return r.triangles; };
const table = [];
for (const crop of CROPS) {
  const id = crop[0], row = [id];
  for (let s = 0; s <= 4; s++) { row.push(out(`plant-${id}-${s}`, plant(crop, s, 1))); out(`plant-${id}-${s}-far`, plant(crop, s, 0)); }
  for (const unripe of [false, true]) {
    const name = `fruit-${id}${unripe ? '-unripe' : ''}`;
    row.push(out(name, fruit(crop, unripe, 1)));
    out(`${name}-far`, fruit(crop, unripe, 0));
  }
  table.push(row.join(' '));
}
out('fence-post', fencePost());
out('fence-rail', fenceRail());
out('lantern', lantern());
for (let i = 0; i < 3; i++) out(`path-${i}`, pathStone(i + 1));
for (let i = 0; i < 4; i++) out(`tuft-${i}`, tuft(i + 1));
out('stall', stall());
out('meadow', meadow());
for (let i = 0; i < 2; i++) out(`grass-${i}`, grassPatch(i + 3));
out('barrel', barrel());
out('can', can());
for (const who of Object.keys(PEOPLE)) {
  out(`${who}-body`, body(who));
  out(`${who}-arm`, arm(who));
  out(`${who}-leg`, leg(who));
}
if (process.env.ART_VERBOSE) console.log(`triangles (stages 0-4, fruit, unripe):\n${table.join('\n')}`);
console.log(`art: ${files} glb files, ${Object.keys(TEXTURES).length} textures, ${(bytes / 1024).toFixed(0)} KiB, ${tris} triangles`);
