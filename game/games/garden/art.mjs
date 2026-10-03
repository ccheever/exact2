#!/usr/bin/env bun
// The garden's art, authored as code: every plant stage, fruit look, prop,
// the avatar and the shopkeeper are procedural meshes written as glTF 2.0
// (buffers and textures embedded), with generated PNG textures. Run
// `bun game/games/garden/art.mjs`; the game's bake turns art/*.gltf into
// .model assets. Deterministic: the same script writes the same bytes.
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
// Authored colours are sRGB; glTF factors and COLOR_0 are linear.
const lin = c => c.map(v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
const hex = h => lin([1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16) / 255));
const mix = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t);
const hsv = (h, s, v) => {
  const f = n => { const k = (n + h * 6) % 6; return v - v * s * Math.max(0, Math.min(k, 4 - k, 1)); };
  return lin([f(5), f(3), f(1)]);
};

// ------------------------------------------------------------ vectors
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const scl = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = a => { const l = Math.hypot(...a) || 1; return scl(a, 1 / l); };
// A transform: rotation matrix rows plus translation and scale.
const I = { r: [[1, 0, 0], [0, 1, 0], [0, 0, 1]], t: [0, 0, 0] };
const mul = (A, B) => ({ // A after B
  r: A.r.map(row => [0, 1, 2].map(j => row[0] * B.r[0][j] + row[1] * B.r[1][j] + row[2] * B.r[2][j])),
  t: add(apply(A, B.t), [0, 0, 0]),
});
function apply(M, p) { return [0, 1, 2].map(i => M.r[i][0] * p[0] + M.r[i][1] * p[1] + M.r[i][2] * p[2] + M.t[i]); }
const rotN = (M, n) => norm([0, 1, 2].map(i => M.r[i][0] * n[0] + M.r[i][1] * n[1] + M.r[i][2] * n[2]));
const T = (x, y, z) => ({ r: I.r, t: [x, y, z] });
const S = (x, y = x, z = x) => ({ r: [[x, 0, 0], [0, y, 0], [0, 0, z]], t: [0, 0, 0] });
const RX = a => ({ r: [[1, 0, 0], [0, Math.cos(a), -Math.sin(a)], [0, Math.sin(a), Math.cos(a)]], t: [0, 0, 0] });
const RY = a => ({ r: [[Math.cos(a), 0, Math.sin(a)], [0, 1, 0], [-Math.sin(a), 0, Math.cos(a)]], t: [0, 0, 0] });
const RZ = a => ({ r: [[Math.cos(a), -Math.sin(a), 0], [Math.sin(a), Math.cos(a), 0], [0, 0, 1]], t: [0, 0, 0] });
const chain = (...ms) => ms.reduce((a, b) => mul(a, b), I);
// Normals under non-uniform scale use the inverse transpose; for the
// diagonal scales used here, dividing by the scale is enough.
const normalOf = (M, n) => {
  const det = M.r[0][0] * (M.r[1][1] * M.r[2][2] - M.r[1][2] * M.r[2][1]) - M.r[0][1] * (M.r[1][0] * M.r[2][2] - M.r[1][2] * M.r[2][0]) + M.r[0][2] * (M.r[1][0] * M.r[2][1] - M.r[1][1] * M.r[2][0]);
  const c = (i, j) => { const a = M.r[(i + 1) % 3], b = M.r[(i + 2) % 3]; return a[(j + 1) % 3] * b[(j + 2) % 3] - a[(j + 2) % 3] * b[(j + 1) % 3]; };
  const inv = [0, 1, 2].map(i => [0, 1, 2].map(j => c(i, j) / det));
  return norm([0, 1, 2].map(i => inv[i][0] * n[0] + inv[i][1] * n[1] + inv[i][2] * n[2]));
};

// ------------------------------------------------------------ meshes
// A model is primitives keyed by material name.
class Model {
  constructor() { this.parts = new Map(); }
  part(mat) {
    if (!this.parts.has(mat)) this.parts.set(mat, { p: [], n: [], t: [], c: [], i: [] });
    return this.parts.get(mat);
  }
  // Append a local grid (rows × cols of vertices) through M.
  grid(mat, M, rows, cols, at, { color = () => [1, 1, 1], flip = false } = {}) {
    const P = this.part(mat), base = P.p.length / 3;
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
      const u = c / (cols - 1), v = r / (rows - 1);
      const { p, n, uv } = at(u, v);
      const wp = apply(M, p);
      P.p.push(...wp); P.n.push(...normalOf(M, flip ? scl(n, -1) : n)); P.t.push(...(uv ?? [u, v]));
      P.c.push(...color(wp, u, v), 1);
    }
    for (let r = 0; r < rows - 1; r++) for (let c = 0; c < cols - 1; c++) {
      const a = base + r * cols + c, b = a + 1, d = a + cols, e = d + 1;
      if (flip) P.i.push(a, b, d, b, e, d); else P.i.push(a, d, b, b, d, e);
    }
    return this;
  }
  // Surface of revolution around +Y from a profile of [radius, y].
  lathe(mat, M, profile, seg = 16, opts = {}) {
    const { wobble = () => 0 } = opts;
    const rows = profile.length;
    const at = (u, v) => {
      const k = Math.round(v * (rows - 1)), [r0, y] = profile[k];
      const a = u * Math.PI * 2, r = r0 * (1 + wobble(a, v));
      const [rp, yp] = profile[Math.max(0, k - 1)], [rn, yn] = profile[Math.min(rows - 1, k + 1)];
      const dr = rn - rp, dy = yn - yp || 1e-4;
      const n = norm([Math.cos(a) * dy, -dr, Math.sin(a) * dy]);
      return { p: [Math.cos(a) * r, y, Math.sin(a) * r], n, uv: [u * 2, y] };
    };
    return this.grid(mat, M, rows, seg + 1, at, opts);
  }
  sphere(mat, M, seg = 12, rings = 8, opts = {}) {
    const prof = [];
    for (let i = 0; i <= rings; i++) { const t = -Math.PI / 2 + Math.PI * i / rings; prof.push([Math.max(1e-3, Math.cos(t)), Math.sin(t)]); }
    return this.lathe(mat, M, prof, seg, opts);
  }
  cylinder(mat, M, r0, r1, h, seg = 10, opts = {}) {
    const prof = [[1e-3, 0], [r0, 0], [r0, 0.001]];
    for (let i = 1; i <= 4; i++) prof.push([r0 + (r1 - r0) * i / 4, h * i / 4]);
    prof.push([r1, h - 0.001 + 0.001], [1e-3, h]);
    return this.lathe(mat, M, prof, seg, opts);
  }
  box(mat, M, w, h, d, opts = {}) {
    const faces = [[[1, 0, 0], [0, 0, -1], [0, 1, 0]], [[-1, 0, 0], [0, 0, 1], [0, 1, 0]], [[0, 1, 0], [1, 0, 0], [0, 0, -1]], [[0, -1, 0], [1, 0, 0], [0, 0, 1]], [[0, 0, 1], [1, 0, 0], [0, 1, 0]], [[0, 0, -1], [-1, 0, 0], [0, 1, 0]]];
    for (let [n, u, v] of faces) {
      if (cross(v, u).reduce((s, x, i) => s + x * n[i], 0) < 0) [u, v] = [v, u];
      const len = d3 => Math.abs(d3[0]) * w + Math.abs(d3[1]) * h + Math.abs(d3[2]) * d;
      this.grid(mat, M, 2, 2, (a, b) => {
        const p = [0, 1, 2].map(i => (n[i] * 0.5 + u[i] * (a - 0.5) + v[i] * (b - 0.5)) * [w, h, d][i]);
        return { p, n, uv: [a * len(u), b * len(v)] };
      }, opts);
    }
    return this;
  }
  // A leaf blade along +Z, curling down by `bend`, both sides.
  leaf(mat, M, len, wid, bend = 0.4, opts = {}) {
    const at = (u, v) => {
      const z = v * len, w = wid * Math.sin(Math.PI * Math.min(1, v * 1.05)) ** 0.8;
      const x = (u - 0.5) * w, y = -bend * v * v * len + Math.abs(u - 0.5) * w * 0.25;
      return { p: [x, y, z], n: norm([0, 1, bend * v * 2]), uv: [u, v] };
    };
    this.grid(mat, M, 6, 3, at, opts);
    this.grid(mat, M, 6, 3, at, { ...opts, flip: true });
    return this;
  }
}

// ------------------------------------------------------------ textures
function texture(size, f) {
  const data = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const c = f(x / size, y / size), o = (y * size + x) * 4;
    for (let k = 0; k < 3; k++) data[o + k] = Math.max(0, Math.min(255, Math.round(c[k] * 255)));
    data[o + 3] = 255;
  }
  return encodePng({ width: size, height: size, data });
}
const TEXTURES = {
  soil: () => texture(128, (u, v) => {
    const n = fbm(u * 8, v * 8, 8), clod = noise2(u * 24, v * 24, 24);
    const base = [0.33, 0.21, 0.12], dark = [0.2, 0.12, 0.07];
    const c = mix(dark, base, n);
    return clod > 0.72 ? mix(c, [0.45, 0.32, 0.2], 0.6) : c;
  }),
  wood: () => texture(128, (u, v) => {
    const g = Math.sin((u * 6 + fbm(u * 4, v * 16, 4) * 3) * Math.PI * 2) * 0.5 + 0.5;
    return mix([0.42, 0.26, 0.14], [0.6, 0.42, 0.25], g * 0.7 + noise2(u * 64, v * 4, 64) * 0.3);
  }),
  bark: () => texture(128, (u, v) => {
    const ridge = Math.abs(Math.sin((u * 10 + fbm(u * 3, v * 6, 3) * 2) * Math.PI));
    return mix([0.18, 0.11, 0.07], [0.42, 0.3, 0.2], ridge * 0.8 + noise2(u * 32, v * 32, 32) * 0.2);
  }),
  cloth: () => texture(64, (u, v) => {
    const w = (Math.floor(u * 32) + Math.floor(v * 32)) % 2 ? 1 : 0.9;
    return [w, w, w].map(x => x * (0.92 + noise2(u * 64, v * 64, 64) * 0.08));
  }),
  stone: () => texture(64, (u, v) => {
    const n = fbm(u * 4, v * 4, 4);
    return mix([0.45, 0.44, 0.42], [0.7, 0.68, 0.64], n);
  }),
};

// ------------------------------------------------------------ materials
const M = {
  soil: { color: [1, 1, 1], rough: 0.95, tex: 'soil' },
  wood: { color: [1, 1, 1], rough: 0.8, tex: 'wood' },
  bark: { color: [1, 1, 1], rough: 0.9, tex: 'bark' },
  stone: { color: [1, 1, 1], rough: 0.85, tex: 'stone' },
  leaf: { color: [1, 1, 1], rough: 0.55, double: true },
  stem: { color: [1, 1, 1], rough: 0.6 },
  skin: { color: [1, 1, 1], rough: 0.35 },
  cloth: { color: [1, 1, 1], rough: 0.9, tex: 'cloth' },
  plain: { color: [1, 1, 1], rough: 0.5 },
  juicy: { color: [1, 1, 1], rough: 0.22 },
  // Untextured stand-ins: the bake names a texture per model, so a texture on
  // all 70 plant models would ship 70 copies; plants take noise in vertex colour.
  earth: { color: [1, 1, 1], rough: 0.95 },
  grain: { color: [1, 1, 1], rough: 0.8 },
  rind: { color: [1, 1, 1], rough: 0.9 },
  glass: { color: hex('#ffd27a'), rough: 0.2, emissive: hex('#ffb347'), strength: 3 },
  iron: { color: hex('#2b2b2e'), metal: 1, rough: 0.5 },
  gold: { color: hex('#ffcf4a'), metal: 1, rough: 0.2 },
  frozen: { color: hex('#cfefff'), rough: 0.05, emissive: hex('#5ab0ff'), strength: 0.35 },
  ice: { color: [0.8, 0.95, 1, 0.35], rough: 0.02, blend: true },
  wet: { color: [1, 1, 1], rough: 0.04 },
  drop: { color: [0.75, 0.88, 1, 0.55], rough: 0.02, blend: true },
  shocked: { color: hex('#fff6a8'), rough: 0.3, emissive: hex('#ffe14a'), strength: 4 },
  rainbow: { color: [1, 1, 1], rough: 0.15, emissive: [1, 1, 1], strength: 0.9, rainbow: true },
  chilled: { color: hex('#e3f4ff'), rough: 0.25 },
};

// ------------------------------------------------------------ glTF
function write(name, model) {
  const json = { asset: { version: '2.0', generator: 'garden art.mjs' }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }], meshes: [{ primitives: [] }], materials: [], accessors: [], bufferViews: [], buffers: [], textures: [], images: [], samplers: [{ magFilter: 9729, minFilter: 9987, wrapS: 10497, wrapT: 10497 }] };
  const chunks = []; let length = 0;
  const view = (bytes, target) => {
    const pad = (4 - (length % 4)) % 4; if (pad) { chunks.push(new Uint8Array(pad)); length += pad; }
    json.bufferViews.push({ buffer: 0, byteOffset: length, byteLength: bytes.byteLength, target });
    chunks.push(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)); length += bytes.byteLength;
    return json.bufferViews.length - 1;
  };
  const accessor = (arr, type, comp, target, minmax) => {
    const typed = comp === 5125 ? new Uint32Array(arr) : new Float32Array(arr);
    const a = { bufferView: view(typed, target), componentType: comp, count: arr.length / { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 }[type], type };
    if (minmax) {
      a.min = [0, 1, 2].map(k => Math.min(...arr.filter((_, i) => i % 3 === k)));
      a.max = [0, 1, 2].map(k => Math.max(...arr.filter((_, i) => i % 3 === k)));
    }
    json.accessors.push(a); return json.accessors.length - 1;
  };
  const textures = new Map();
  const tex = key => {
    if (!textures.has(key)) {
      json.images.push({ uri: 'data:image/png;base64,' + Buffer.from(TEXTURES[key]()).toString('base64') });
      json.textures.push({ source: json.images.length - 1, sampler: 0 });
      textures.set(key, json.textures.length - 1);
    }
    return textures.get(key);
  };
  const used = new Set();
  for (const [matName, P] of model.parts) {
    const m = M[matName] ?? M.plain;
    const mat = { name: matName, pbrMetallicRoughness: { baseColorFactor: [...m.color.slice(0, 3), m.color[3] ?? 1], metallicFactor: m.metal ?? 0, roughnessFactor: m.rough ?? 0.5 } };
    if (m.tex) mat.pbrMetallicRoughness.baseColorTexture = { index: tex(m.tex) };
    if (m.emissive) {
      mat.emissiveFactor = m.emissive;
      if (m.strength && m.strength !== 1) { mat.extensions = { KHR_materials_emissive_strength: { emissiveStrength: m.strength } }; used.add('KHR_materials_emissive_strength'); }
    }
    if (m.double) mat.doubleSided = true;
    if (m.blend) mat.alphaMode = 'BLEND';
    json.materials.push(mat);
    json.meshes[0].primitives.push({
      attributes: {
        POSITION: accessor(P.p, 'VEC3', 5126, 34962, true),
        NORMAL: accessor(P.n, 'VEC3', 5126, 34962),
        TEXCOORD_0: accessor(P.t, 'VEC2', 5126, 34962),
        COLOR_0: accessor(P.c, 'VEC4', 5126, 34962),
      },
      indices: accessor(P.i, 'SCALAR', 5125, 34963),
      material: json.materials.length - 1,
    });
  }
  if (used.size) json.extensionsUsed = [...used];
  if (!json.textures.length) { delete json.textures; delete json.images; delete json.samplers; }
  const buf = Buffer.concat(chunks.map(c => Buffer.from(c)));
  json.buffers.push({ byteLength: buf.length, uri: 'data:application/octet-stream;base64,' + buf.toString('base64') });
  const text = JSON.stringify(json);
  writeFileSync(resolve(ART, `${name}.gltf`), text);
  return text.length;
}

// ------------------------------------------------------------ the crops
// Mirrors crops.rs: id, look, mature height, leaf and fruit colours (sRGB),
// fruit radius and shape.
const CROPS = [
  ['carrot', 'fronds', 0.4, '#4caf50', '#f28a1e', 0.22, 'root'],
  ['strawberry', 'bush', 0.5, '#3f9b46', '#e0262f', 0.14, 'berry'],
  ['blueberry', 'bush', 0.6, '#2f7d4f', '#4f6bd8', 0.12, 'round'],
  ['tomato', 'staked', 0.9, '#3e8e3a', '#e2352a', 0.2, 'tomato'],
  ['corn', 'stalk', 1.4, '#7cb342', '#f4d03f', 0.18, 'cob'],
  ['watermelon', 'vine', 0.5, '#2e7d32', '#2f8f3a', 0.55, 'melon'],
  ['pumpkin', 'vine', 0.5, '#558b2f', '#f08a24', 0.5, 'pumpkin'],
  ['apple', 'tree', 2.4, '#2e7d32', '#d32f2f', 0.2, 'apple'],
  ['bamboo', 'bamboo', 2.8, '#8bc34a', '#9ccc65', 0.25, 'shoot'],
  ['coconut', 'palm', 3.2, '#43a047', '#7a5230', 0.28, 'round'],
  ['cactus', 'cactus', 1.8, '#4f9a5e', '#e8669a', 0.2, 'berry'],
  ['dragon', 'dragon', 1.6, '#5aa36b', '#f0388f', 0.24, 'dragon'],
  ['mango', 'tree', 2.6, '#2e6b30', '#ffa726', 0.24, 'mango'],
  ['grape', 'trellis', 1.4, '#3d7a3d', '#7b3f9e', 0.16, 'grapes'],
];

const leafy = (base, r) => (p, u, v) => mix(mix(base, scl(base, 0.55), 1 - v), [0.9, 1, 0.6].map((x, i) => x * base[i] * 1.2), noise2(p[0] * 9, p[2] * 9) * 0.25 * r);
const earthy = p => { const n = fbm(p[0] * 6 + 9, p[2] * 6 + 9, 1e9); return hash3(p[0] * 40, p[2] * 40, 2) > 0.85 ? hex('#7a5a3a') : mix(hex('#3b2414'), hex('#6b4426'), n); };
const grainy = p => mix(hex('#6d4626'), hex('#9a6b3f'), Math.sin((p[0] + p[2]) * 40 + fbm(p[1] * 8, p[0] * 8, 1e9) * 6) * 0.5 + 0.5);
const barky = p => mix(hex('#3e2717'), hex('#7a5a40'), Math.abs(Math.sin(Math.atan2(p[2], p[0]) * 9 + fbm(p[1] * 5, 3, 1e9) * 3)));
function mound(m) {
  const prof = [];
  for (let i = 0; i <= 6; i++) { const t = i / 6; prof.push([Math.max(0.001, 0.64 * Math.cos(t * Math.PI / 2)), 0.12 * Math.sin(t * Math.PI / 2)]); }
  m.lathe('earth', I, prof, 18, { wobble: (a, v) => 0.05 * Math.sin(a * 5 + v * 3) + 0.03 * Math.sin(a * 13), color: earthy });
}
// One stage of one crop: g in [0,1] is growth; stage 0 is the sprout.
function plant(id, look, h, leafHex, fruitHex, stage) {
  const m = new Model(), R = rng(id.length * 131 + stage);
  const leafC = hex(leafHex), g = stage / 4, hh = h * (0.3 + 0.7 * g);
  mound(m);
  const leafCol = leafy(leafC, 1);
  if (stage === 0) {
    for (const a of [0, Math.PI]) m.leaf('leaf', chain(T(0, 0.1, 0), RY(a + R()), RX(-0.6)), 0.14, 0.09, 0.3, { color: leafCol });
    m.cylinder('stem', T(0, 0.08, 0), 0.012, 0.01, 0.06, 5, { color: () => leafC });
    return m;
  }
  const stem = (x, z, r0, r1, len, tilt = 0, yaw = 0, mat = 'stem', col = () => scl(leafC, 0.8)) => m.cylinder(mat, chain(T(x, 0.1, z), RY(yaw), RZ(tilt)), r0, r1, len, 7, { color: col });
  switch (look) {
    case 'fronds': {
      const n = 3 + stage * 2;
      for (let i = 0; i < n; i++) {
        const a = i / n * Math.PI * 2 + R() * 0.4;
        stem(0, 0, 0.012, 0.008, hh * 0.8, 0.25 + R() * 0.2, a);
        for (let k = 1; k <= 3; k++) m.leaf('leaf', chain(T(0, 0.1, 0), RY(a), RX(-1.2 + 0.25 * k), T(0, 0, hh * 0.25 * k)), hh * 0.25, 0.05, 0.6, { color: leafCol });
      }
      break;
    }
    case 'bush': case 'staked': {
      const n = 5 + stage * 3, rad = 0.42 * (0.4 + 0.6 * g);
      if (look === 'staked') { m.box('grain', T(0.18, 0.1 + hh * 0.55, 0.05), 0.04, hh * 1.1, 0.04, { color: grainy }); }
      stem(0, 0, 0.03, 0.02, hh * 0.6);
      for (let i = 0; i < n; i++) {
        const a = i * 2.39996, y = 0.1 + hh * (0.25 + 0.65 * (i / n)), r = rad * (0.5 + 0.5 * R());
        m.leaf('leaf', chain(T(Math.cos(a) * r * 0.3, y, Math.sin(a) * r * 0.3), RY(-a + Math.PI / 2), RX(-0.3 - R() * 0.4)), 0.16 + 0.12 * g, 0.12 + 0.06 * g, 0.5, { color: leafCol });
      }
      break;
    }
    case 'stalk': {
      const n = 1 + Math.min(stage, 3);
      for (let s = 0; s < n; s++) {
        const x = (s - (n - 1) / 2) * 0.14, z = (s % 2) * 0.1 - 0.05;
        stem(x, z, 0.035, 0.022, hh, (R() - 0.5) * 0.08);
        for (let k = 0; k < 2 + stage; k++) m.leaf('leaf', chain(T(x, 0.1 + hh * (0.15 + 0.17 * k), z), RY(k * 2.4 + s), RX(-0.9)), hh * 0.45, 0.07, 0.9, { color: leafCol });
        if (stage === 4) m.cylinder('stem', T(x, 0.1 + hh, z), 0.02, 0.002, 0.18, 6, { color: () => hex('#d8b55a') });
      }
      break;
    }
    case 'vine': {
      const n = 3 + stage * 2;
      for (let i = 0; i < n; i++) {
        const a = i * 2.2, r = 0.25 + 0.45 * g * R();
        m.cylinder('stem', chain(T(0, 0.12, 0), RY(a), RZ(Math.PI / 2 - 0.15)), 0.015, 0.01, r + 0.15, 5, { color: () => scl(leafC, 0.7) });
        m.leaf('leaf', chain(T(Math.cos(a) * r, 0.12 + 0.05 * R(), -Math.sin(a) * r), RY(a + Math.PI / 2), RX(-0.15)), 0.22 + 0.12 * g, 0.24 + 0.12 * g, 0.2, { color: leafCol });
      }
      break;
    }
    case 'tree': case 'palm': {
      const trunkH = look === 'palm' ? hh * 0.9 : hh * 0.55;
      const lean = look === 'palm' ? 0.12 : 0;
      m.cylinder('rind', chain(T(0, 0.08, 0), RZ(lean)), 0.09 + 0.05 * g, 0.06, trunkH, 12, { color: barky });
      const top = [Math.sin(-lean) * trunkH, 0.08 + trunkH, 0];
      if (look === 'palm') {
        for (let i = 0; i < 7; i++) m.leaf('leaf', chain(T(...top), RY(i * 0.9), RX(-0.35)), 0.6 + 0.9 * g, 0.28, 0.7, { color: leafCol });
      } else {
        const blobs = 3 + stage;
        for (let i = 0; i < blobs; i++) {
          const a = i * 2.39996, r = (0.2 + 0.25 * g) * (i ? 1 : 0);
          const s = (0.35 + 0.35 * g) * (0.8 + 0.3 * R());
          m.sphere('leaf', chain(T(top[0] + Math.cos(a) * r, top[1] + hh * 0.12 * (i % 3), top[2] + Math.sin(a) * r), S(s, s * 0.85, s)), 10, 7, { color: leafy(leafC, 2), wobble: (a2, v) => 0.08 * Math.sin(a2 * 6 + v * 9) });
        }
      }
      break;
    }
    case 'bamboo': {
      const n = 1 + stage;
      for (let s = 0; s < n; s++) {
        const a = s * 2.4, x = Math.cos(a) * 0.15 * (s ? 1 : 0), z = Math.sin(a) * 0.15 * (s ? 1 : 0);
        const len = hh * (0.7 + 0.3 * R());
        const joints = Math.max(2, Math.round(len / 0.45));
        for (let j = 0; j < joints; j++) {
          m.cylinder('stem', T(x, 0.1 + j * len / joints, z), 0.04, 0.038, len / joints - 0.02, 8, { color: () => mix(hex('#9ccc65'), hex('#c5e1a5'), j / joints) });
          m.cylinder('stem', T(x, 0.1 + (j + 1) * len / joints - 0.025, z), 0.046, 0.046, 0.025, 8, { color: () => hex('#7cb342') });
        }
        m.leaf('leaf', chain(T(x, 0.1 + len, z), RY(a), RX(-0.5)), 0.3, 0.06, 0.5, { color: leafCol });
      }
      break;
    }
    case 'cactus': case 'dragon': {
      const ribs = (a2, v) => 0.12 * Math.abs(Math.sin(a2 * 4));
      const col = () => leafC;
      m.lathe('stem', T(0, 0.08, 0), [[0.001, 0], [0.16, 0.02], [0.18, hh * 0.3], [0.17, hh * 0.8], [0.12, hh * 0.95], [0.001, hh]], 16, { wobble: ribs, color: col });
      if (stage >= 2) for (const side of [-1, 1]) {
        const y = hh * (0.35 + 0.15 * side);
        const arm = look === 'dragon' ? [[0.001, 0], [0.07, 0.02], [0.08, hh * 0.35], [0.001, hh * 0.4]] : [[0.001, 0], [0.09, 0.02], [0.1, hh * 0.3], [0.001, hh * 0.34]];
        m.lathe('stem', chain(T(side * 0.15, 0.08 + y, 0), RZ(-side * (look === 'dragon' ? 1.0 : 0.25))), arm, 12, { wobble: ribs, color: col });
      }
      break;
    }
    case 'trellis': {
      for (const x of [-0.5, 0.5]) m.box('grain', T(x, 0.1 + hh * 0.5, -0.3), 0.06, hh, 0.06, { color: grainy });
      m.box('grain', T(0, 0.1 + hh * 0.98, -0.3), 1.1, 0.05, 0.05, { color: grainy });
      stem(0, -0.2, 0.03, 0.015, hh * 0.9, 0.1);
      for (let i = 0; i < 4 + stage * 3; i++) {
        const x = (R() - 0.5) * 0.9, y = 0.25 + R() * hh * 0.75;
        m.leaf('leaf', chain(T(x, y, -0.25), RY(R() * 6), RX(-0.4)), 0.18, 0.16, 0.3, { color: leafCol });
      }
      break;
    }
  }
  return m;
}

// The fruit of a crop, in one of its looks. Radius 1 is the crop's fruit size.
const VARIANTS = ['unripe', 'plain', 'gold', 'rainbow', 'frozen', 'wet', 'shocked', 'chilled'];
function fruit(id, shape, fruitHex, size, variant) {
  const m = new Model(), R = rng(id.length * 7 + VARIANTS.indexOf(variant));
  let base = hex(fruitHex);
  if (variant === 'unripe') base = mix(base, hex('#8bc34a'), 0.65);
  if (variant === 'chilled') base = mix(base, hex('#cfe9ff'), 0.45);
  if (variant === 'wet') base = scl(base, 0.8);
  const mat = { plain: 'juicy', unripe: 'plain', gold: 'gold', rainbow: 'rainbow', frozen: 'frozen', wet: 'wet', shocked: 'shocked', chilled: 'chilled' }[variant];
  const flat = ['gold', 'frozen', 'shocked'].includes(variant);
  const tint = (fn) => flat ? () => [1, 1, 1] : variant === 'rainbow' ? (p, u, v) => hsv((v * 1.5 + u * 0.5) % 1, 0.75, 1) : fn;
  const S1 = S(size);
  const solid = c => tint(() => c);
  switch (shape) {
    case 'root': m.lathe(mat, S1, [[0.001, -1.4], [0.18, -1.1], [0.35, -0.4], [0.42, 0.1], [0.3, 0.3], [0.001, 0.32]], 12, { color: tint((p, u, v) => scl(base, 0.85 + 0.15 * Math.sin(v * 40))) });
      m.leaf('leaf', chain(S1, T(0, 0.3, 0), RX(-1.3)), 1.2, 0.3, 0.2, { color: () => hex('#4caf50') });
      m.leaf('leaf', chain(S1, T(0, 0.3, 0), RY(2.1), RX(-1.2)), 1.0, 0.3, 0.2, { color: () => hex('#43a047') }); break;
    case 'berry': m.lathe(mat, S1, [[0.001, -1], [0.45, -0.75], [0.85, -0.1], [0.9, 0.3], [0.6, 0.6], [0.001, 0.68]], 14, { color: tint((p, u, v) => mix(base, [1, 0.9, 0.5], hash3(u * 14, v * 9, 3) > 0.9 ? 0.6 : 0)) });
      for (let i = 0; i < 5; i++) m.leaf('leaf', chain(S1, T(0, 0.62, 0), RY(i * 1.26), RX(-1.4)), 0.45, 0.22, -0.1, { color: () => hex('#388e3c') }); break;
    case 'round': m.sphere(mat, S1, 14, 9, { color: solid(base) }); m.cylinder('stem', chain(S1, T(0, 0.95, 0)), 0.06, 0.04, 0.25, 5, { color: () => hex('#5d4037') }); break;
    case 'tomato': m.sphere(mat, chain(S1, S(1, 0.8, 1)), 16, 9, { color: solid(base), wobble: a => 0.05 * Math.cos(a * 5) });
      for (let i = 0; i < 5; i++) m.leaf('leaf', chain(S1, T(0, 0.78, 0), RY(i * 1.26), RX(-1.45)), 0.4, 0.15, -0.1, { color: () => hex('#2e7d32') }); break;
    case 'cob': m.lathe(mat, chain(S1, S(0.55, 1, 0.55)), [[0.001, -1.4], [0.7, -1.2], [0.85, 0], [0.7, 1.2], [0.001, 1.45]], 14, { color: tint((p, u, v) => scl(base, (Math.floor(u * 28) + Math.floor(v * 18)) % 2 ? 1 : 0.82)) });
      for (let i = 0; i < 3; i++) m.leaf('leaf', chain(S1, T(0, -1.3, 0), RY(i * 2.1), RX(-0.25)), 2.2, 0.6, -0.3, { color: () => hex('#9ccc65') }); break;
    case 'melon': m.sphere(mat, chain(S1, S(1, 0.78, 1.25)), 18, 10, { color: tint((p, u) => mix(base, hex('#a5d6a7'), Math.sin(u * Math.PI * 22) > 0.3 ? 0.55 : 0)) }); break;
    case 'pumpkin': m.sphere(mat, chain(S1, S(1, 0.7, 1)), 20, 10, { color: tint((p, u) => scl(base, 0.85 + 0.15 * Math.cos(u * Math.PI * 16))), wobble: a => 0.08 * Math.cos(a * 8) });
      m.cylinder('stem', chain(S1, T(0, 0.6, 0), RZ(0.3)), 0.1, 0.06, 0.35, 6, { color: () => hex('#6d4c41') }); break;
    case 'apple': m.lathe(mat, S1, [[0.001, -0.85], [0.55, -0.8], [0.95, -0.2], [0.9, 0.5], [0.4, 0.82], [0.001, 0.7]], 16, { color: tint((p, u, v) => mix(base, hex('#ffd54f'), Math.max(0, Math.sin(u * 6.28)) * 0.35 * v)) });
      m.cylinder('stem', chain(S1, T(0, 0.7, 0)), 0.05, 0.03, 0.35, 5, { color: () => hex('#5d4037') });
      m.leaf('leaf', chain(S1, T(0, 0.95, 0), RX(-0.9)), 0.7, 0.3, 0.2, { color: () => hex('#43a047') }); break;
    case 'shoot': m.lathe(mat, S1, [[0.001, -0.2], [0.45, -0.2], [0.4, 0.5], [0.2, 1.2], [0.001, 1.5]], 10, { color: tint((p, u, v) => mix(base, hex('#fff59d'), v * 0.5)) }); break;
    case 'dragon': m.sphere(mat, chain(S1, S(0.85, 1.05, 0.85)), 14, 9, { color: solid(base) });
      for (let i = 0; i < 9; i++) m.leaf(variant === 'plain' || variant === 'unripe' ? 'leaf' : mat, chain(S1, RY(i * 0.7), RX(-0.4 - (i % 3) * 0.5), T(0, 0.2, 0.75)), 0.6, 0.25, -0.4, { color: tint(() => hex('#c5e1a5')) }); break;
    case 'mango': m.sphere(mat, chain(S1, S(0.8, 1.1, 0.75), RZ(0.3)), 14, 9, { color: tint((p, u, v) => mix(base, hex('#e53935'), v * 0.5)) }); break;
    case 'grapes':
      for (let i = 0; i < 14; i++) { const t = i / 14, a = i * 2.4, r = 0.55 * (1 - t); m.sphere(mat, chain(S1, T(Math.cos(a) * r, 0.9 - t * 1.9, Math.sin(a) * r), S(0.42)), 8, 6, { color: solid(scl(base, 0.85 + 0.3 * R())) }); }
      m.cylinder('stem', chain(S1, T(0, 0.9, 0)), 0.05, 0.03, 0.4, 5, { color: () => hex('#6d4c41') }); break;
  }
  // The looks layered over the shape.
  if (variant === 'frozen') m.sphere('ice', S(size * 1.18), 10, 7, { color: () => [1, 1, 1], wobble: (a, v) => 0.1 * hash3(Math.round(a * 3), Math.round(v * 8), 1) });
  if (variant === 'wet') for (let i = 0; i < 9; i++) { const a = R() * 6.28, y = R() * 1.4 - 0.7, r = Math.sqrt(1 - y * y) * 0.98; m.sphere('drop', chain(S1, T(Math.cos(a) * r, y, Math.sin(a) * r), S(0.13)), 6, 4, { color: () => [1, 1, 1] }); }
  if (variant === 'shocked') for (let i = 0; i < 6; i++) m.box('shocked', chain(S1, RY(i * 1.05), RZ(0.9 + R() * 0.4), T(0, 1.05, 0)), 0.08, 0.55, 0.08);
  if (variant === 'gold') for (let i = 0; i < 4; i++) m.box('gold', chain(S1, RY(i * 1.6 + 0.4), T(0.9, 0.3 + R() * 0.5, 0), RZ(0.785)), 0.14, 0.14, 0.03);
  return m;
}

// ------------------------------------------------------------ props
function tuft(seed) {
  const m = new Model(), R = rng(seed);
  const g = hex(['#4c8c2b', '#5c9e33', '#3f7a24'][seed % 3]);
  for (let i = 0; i < 9; i++) m.leaf('leaf', chain(RY(R() * 6.28), RX(-1.2 - R() * 0.3)), 0.25 + R() * 0.2, 0.05, 0.25, { color: (p, u, v) => mix(scl(g, 0.6), g, v) });
  if (seed % 2) for (let i = 0; i < 3; i++) {
    const a = R() * 6.28, r = 0.12 + R() * 0.1, x = Math.cos(a) * r, z = Math.sin(a) * r, y = 0.22 + R() * 0.1;
    m.cylinder('stem', T(x, 0, z), 0.008, 0.006, y, 4, { color: () => scl(g, 0.8) });
    const petal = hex(['#ffffff', '#ffd54f', '#f48fb1', '#b39ddb'][(seed + i) % 4]);
    for (let k = 0; k < 5; k++) m.leaf('leaf', chain(T(x, y, z), RY(k * 1.256), RX(-1.45)), 0.06, 0.05, 0.0, { color: () => petal });
    m.sphere('plain', chain(T(x, y + 0.01, z), S(0.02)), 5, 3, { color: () => hex('#f9a825') });
  }
  return m;
}
function fencePost() {
  const m = new Model();
  m.box('wood', T(0, 0.45, 0), 0.14, 0.9, 0.14);
  m.lathe('wood', T(0, 0.9, 0), [[0.09, 0], [0.09, 0.02], [0.001, 0.12]], 4);
  return m;
}
function fenceRail() { // spans one tile (2 m) along +X, centred
  const m = new Model();
  for (const y of [0.35, 0.68]) m.box('wood', T(0, y, 0), 2.0, 0.09, 0.05);
  return m;
}
function lantern(lit) {
  const m = new Model();
  m.box('wood', T(0, 0.9, 0), 0.12, 1.8, 0.12);
  m.box('wood', T(0.18, 1.72, 0), 0.4, 0.06, 0.06);
  m.cylinder('iron', T(0.33, 1.42, 0), 0.11, 0.13, 0.04, 8);
  m.cylinder(lit ? 'glass' : 'plain', T(0.33, 1.46, 0), 0.09, 0.09, 0.16, 8, { color: () => lit ? [1, 1, 1] : hex('#c9b27a') });
  m.lathe('iron', T(0.33, 1.62, 0), [[0.13, 0], [0.09, 0.06], [0.001, 0.12]], 8);
  return m;
}
function pathStone(seed) {
  const m = new Model(), R = rng(seed);
  for (let i = 0; i < 3; i++) m.lathe('stone', T((R() - 0.5) * 1.2, 0, (R() - 0.5) * 1.2), [[0.001, 0], [0.32 + R() * 0.15, 0.0], [0.3, 0.05], [0.001, 0.06]], 9, { wobble: (a) => 0.12 * Math.sin(a * 3 + R() * 6) });
  return m;
}
function stall() {
  const m = new Model();
  const W = 3.6, D = 1.6;
  m.box('wood', T(0, 0.5, 0), W, 1.0, D * 0.6);
  m.box('wood', T(0, 1.02, 0), W + 0.15, 0.06, D * 0.7);
  for (const x of [-W / 2, W / 2]) for (const z of [-D / 2, D / 2]) m.box('wood', T(x, 1.4, z), 0.12, 2.8, 0.12);
  // A striped awning, sloping to the front.
  m.grid('cloth', chain(T(0, 2.85, 0), RX(0.28)), 2, 25, (u, v) => ({ p: [(u - 0.5) * (W + 0.5), 0, (v - 0.5) * (D + 0.8)], n: [0, 1, 0] }), { color: (p, u) => Math.floor(u * 12) % 2 ? hex('#fafafa') : hex('#d84343') });
  m.grid('cloth', chain(T(0, 2.85, 0), RX(0.28)), 2, 25, (u, v) => ({ p: [(u - 0.5) * (W + 0.5), 0, (v - 0.5) * (D + 0.8)], n: [0, 1, 0] }), { color: (p, u) => Math.floor(u * 12) % 2 ? hex('#fafafa') : hex('#d84343'), flip: true });
  // Scalloped valance along the front edge.
  for (let i = 0; i < 12; i++) m.sphere('cloth', chain(T(-W / 2 - 0.1 + (i + 0.5) * (W + 0.2) / 12, 2.55, D / 2 + 0.35), S(0.17, 0.12, 0.03)), 8, 4, { color: () => i % 2 ? hex('#fafafa') : hex('#d84343') });
  // Seed crates on the counter, each a colour of the catalogue.
  CROPS.slice(0, 8).forEach(([, , , , fruitHex], i) => {
    const x = -W / 2 + 0.3 + i * 0.43;
    m.box('wood', T(x, 1.13, 0.1), 0.36, 0.16, 0.4);
    m.sphere('plain', chain(T(x, 1.22, 0.1), S(0.15, 0.06, 0.17)), 8, 4, { color: () => hex(fruitHex) });
  });
  // A sign over the awning.
  m.box('wood', T(0, 3.25, 0.2), 2.2, 0.55, 0.08);
  return m;
}
// A person in parts, so the parts can move: head, torso, arm, leg.
const PERSON = {
  farmer: { shirt: '#4f8fd6', pants: '#3b5998', skin: '#f2c49b', hair: '#5d3a1a', hat: '#e3c16f', strap: '#3b5998' },
  keeper: { shirt: '#f3e5c0', pants: '#6d4c41', skin: '#c68863', hair: '#2b1b10', hat: null, apron: '#2e7d32' },
};
function head(p) {
  const m = new Model();
  m.sphere('skin', S(0.24, 0.26, 0.24), 16, 10, { color: () => hex(p.skin) });
  m.sphere('plain', chain(T(0, 0.06, -0.01), S(0.25, 0.22, 0.25)), 14, 8, { color: () => hex(p.hair) });
  for (const x of [-0.08, 0.08]) {
    m.sphere('plain', chain(T(x, 0.02, 0.21), S(0.045, 0.06, 0.03)), 8, 5, { color: () => hex('#1b1b1b') });
    m.sphere('plain', chain(T(x + 0.012, 0.035, 0.235), S(0.012)), 5, 3, { color: () => [1, 1, 1] });
    m.sphere('skin', chain(T(x * 1.4, -0.06, 0.19), S(0.04, 0.025, 0.02)), 6, 3, { color: () => hex('#f08c8c') });
  }
  m.sphere('skin', chain(T(0, -0.03, 0.24), S(0.035)), 6, 4, { color: () => scl(hex(p.skin), 0.9) });
  m.grid('plain', chain(T(0, -0.11, 0.215), RX(0.2)), 2, 6, (u, v) => ({ p: [(u - 0.5) * 0.1, -Math.sin(u * Math.PI) * 0.02 + v * 0.01, 0], n: [0, 0, 1] }), { color: () => hex('#7a2e2e') });
  if (p.hat) {
    m.cylinder('cloth', T(0, 0.13, 0), 0.45, 0.45, 0.025, 20, { color: () => hex(p.hat) });
    m.lathe('cloth', T(0, 0.15, 0), [[0.2, 0], [0.19, 0.12], [0.001, 0.14]], 16, { color: () => hex(p.hat) });
    m.cylinder('cloth', T(0, 0.15, 0), 0.205, 0.205, 0.04, 16, { color: () => hex('#c0392b') });
  }
  return m;
}
function torso(p) {
  const m = new Model();
  m.lathe('cloth', T(0, -0.35, 0), [[0.001, 0], [0.2, 0.01], [0.22, 0.2], [0.24, 0.45], [0.16, 0.62], [0.07, 0.66], [0.001, 0.67]], 14, { color: () => hex(p.shirt) });
  if (p.apron) m.grid('cloth', T(0, -0.05, 0.235), 2, 2, (u, v) => ({ p: [(u - 0.5) * 0.36, (v - 0.9) * 0.6, 0], n: [0, 0, 1] }), { color: () => hex(p.apron) });
  if (p.strap) for (const x of [-0.1, 0.1]) m.box('cloth', T(x, 0.08, 0.2), 0.05, 0.4, 0.04, { color: () => hex(p.strap) });
  m.cylinder('skin', T(0, 0.3, 0), 0.07, 0.07, 0.1, 8, { color: () => hex(p.skin) });
  return m;
}
function arm(p) { // hangs down from the shoulder at the origin
  const m = new Model();
  m.cylinder('cloth', chain(RX(Math.PI), T(0, 0, 0)), 0.075, 0.065, 0.3, 8, { color: () => hex(p.shirt) });
  m.cylinder('skin', T(0, -0.52, 0), 0.05, 0.055, 0.22, 8, { color: () => hex(p.skin) });
  m.sphere('skin', chain(T(0, -0.55, 0), S(0.065)), 8, 5, { color: () => hex(p.skin) });
  return m;
}
function leg(p) { // hangs from the hip at the origin
  const m = new Model();
  m.cylinder('cloth', T(0, -0.42, 0), 0.075, 0.09, 0.42, 8, { color: () => hex(p.pants) });
  m.box('plain', T(0, -0.48, 0.04), 0.13, 0.1, 0.24, { color: () => hex('#4e342e') });
  return m;
}

// ------------------------------------------------------------ write
mkdirSync(ART, { recursive: true });
for (const f of readdirSync(ART)) if (f.endsWith('.gltf')) rmSync(resolve(ART, f));
let files = 0, bytes = 0;
const names = [];
const out = (name, m) => { bytes += write(name, m); files++; names.push(`${name}.model`); };
for (const [id, look, h, leaf, fruitHex, size, shape] of CROPS) {
  for (let s = 0; s <= 4; s++) out(`plant-${id}-${s}`, plant(id, look, h, leaf, fruitHex, s));
  for (const v of VARIANTS) out(`fruit-${id}-${v}`, fruit(id, shape, fruitHex, size, v));
}
out('fence-post', fencePost());
out('fence-rail', fenceRail());
out('lantern-lit', lantern(true));
out('lantern-dark', lantern(false));
for (let i = 0; i < 3; i++) out(`path-${i}`, pathStone(i + 1));
for (let i = 0; i < 4; i++) out(`tuft-${i}`, tuft(i + 1));
out('stall', stall());
for (const [who, p] of Object.entries(PERSON)) {
  out(`${who}-head`, head(p));
  out(`${who}-torso`, torso(p));
  out(`${who}-arm`, arm(p));
  out(`${who}-leg`, leg(p));
}
// Every model is declared before setup: a save (and a paranoid proof, which
// saves every tick) refuses a mesh whose model has not arrived.
writeFileSync(resolve(import.meta.dir, 'logic/src/models.rs'),
  `//! Generated by art.mjs: every model the garden can show.\npub const MODELS: &[&str] = &[\n${names.map(n => `    "${n}",`).join('\n')}\n];\n`);
console.log(`art: ${files} glTF files, ${(bytes / 1024).toFixed(0)} KiB`);
