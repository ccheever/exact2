// The art toolkit: a seeded RNG, tileable value noise, a PNG writer, a mesh
// builder and a binary glTF writer. Everything the forest draws is made from these.
import { deflateSync } from 'node:zlib';

export function rng(seed) {
  let s = seed >>> 0 || 1;
  const next = () => {
    s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0;
    return s / 4294967296;
  };
  next.range = (a, b) => a + (b - a) * next();
  return next;
}

// Tileable value noise on a period-p lattice, smoothstep-interpolated.
export function noise2(seed, period) {
  const r = rng(seed), table = new Float32Array(period * period).map(() => r());
  const at = (i, j) => table[((j % period + period) % period) * period + ((i % period + period) % period)];
  const s = t => t * t * (3 - 2 * t);
  return (x, y) => {
    const i = Math.floor(x), j = Math.floor(y), fx = s(x - i), fy = s(y - j);
    const a = at(i, j), b = at(i + 1, j), c = at(i, j + 1), d = at(i + 1, j + 1);
    return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
  };
}
// Fractal sum over octaves; u, v in [0, 1) tile seamlessly at every octave.
export function fbm(seed, base = 4, octaves = 5) {
  const layers = Array.from({length: octaves}, (_, k) => [noise2(seed + k * 101, base << k), base << k, 0.5 ** k]);
  const total = layers.reduce((t, l) => t + l[2], 0);
  return (u, v) => layers.reduce((t, [n, p, w]) => t + n(u * p, v * p) * w, 0) / total;
}

const lerp = (a, b, t) => a + (b - a) * t;
export const mix = (a, b, t) => a.map((x, i) => lerp(x, b[i], t));
export const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
export const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };

// An RGBA8 image from a function of (u, v) returning [r, g, b, a] in 0..1 (sRGB).
export function image(w, h, f) {
  const px = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const c = f((x + 0.5) / w, (y + 0.5) / h, x, y);
    for (let k = 0; k < 4; k++) px[(y * w + x) * 4 + k] = Math.round(clamp(c[k] ?? 1) * 255);
  }
  return {w, h, px};
}

const crcTable = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc = bytes => { let c = 0xffffffff; for (const b of bytes) c = crcTable[(c ^ b) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
export function png({w, h, px}) {
  const raw = new Uint8Array(h * (w * 4 + 1));
  for (let y = 0; y < h; y++) raw.set(px.subarray(y * w * 4, (y + 1) * w * 4), y * (w * 4 + 1) + 1);
  const chunk = (type, data) => {
    const out = new Uint8Array(12 + data.length), view = new DataView(out.buffer);
    view.setUint32(0, data.length); out.set(new TextEncoder().encode(type), 4); out.set(data, 8);
    view.setUint32(8 + data.length, crc(out.subarray(4, 8 + data.length)));
    return out;
  };
  const ihdr = new Uint8Array(13), v = new DataView(ihdr.buffer);
  v.setUint32(0, w); v.setUint32(4, h); ihdr.set([8, 6, 0, 0, 0], 8);
  const parts = [new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw, {level: 9})), chunk('IEND', new Uint8Array())];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0; for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

// --- Meshes: indexed triangles with normals, UVs and linear vertex colours ---
export class Mesh {
  constructor() { this.p = []; this.n = []; this.uv = []; this.c = []; this.i = []; }
  get count() { return this.p.length / 3; }
  vertex(p, n, uv, c = [1, 1, 1, 1]) {
    this.p.push(...p); this.n.push(...n); this.uv.push(...uv); this.c.push(...c);
    return this.count - 1;
  }
  tri(a, b, c) { this.i.push(a, b, c); }
  append(m, f = x => x, nf = x => x) {
    const base = this.count;
    for (let k = 0; k < m.count; k++) {
      const p = f(m.p.slice(k * 3, k * 3 + 3)), n = norm(nf(m.n.slice(k * 3, k * 3 + 3)));
      this.vertex(p, n, m.uv.slice(k * 2, k * 2 + 2), m.c.slice(k * 4, k * 4 + 4));
    }
    for (const i of m.i) this.i.push(base + i);
    return this;
  }
  // Recompute smooth normals from the triangles (welded by index only).
  smoothNormals() {
    const n = new Float64Array(this.p.length);
    for (let t = 0; t < this.i.length; t += 3) {
      const [a, b, c] = [this.i[t], this.i[t + 1], this.i[t + 2]];
      const pa = this.p.slice(a * 3, a * 3 + 3), pb = this.p.slice(b * 3, b * 3 + 3), pc = this.p.slice(c * 3, c * 3 + 3);
      const f = cross(sub(pb, pa), sub(pc, pa));
      for (const v of [a, b, c]) for (let k = 0; k < 3; k++) n[v * 3 + k] += f[k];
    }
    for (let v = 0; v < this.count; v++) {
      const m = norm([n[v * 3], n[v * 3 + 1], n[v * 3 + 2]]);
      this.n.splice(v * 3, 3, ...m);
    }
    return this;
  }
  bounds() {
    const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
    for (let k = 0; k < this.p.length; k++) { lo[k % 3] = Math.min(lo[k % 3], this.p[k]); hi[k % 3] = Math.max(hi[k % 3], this.p[k]); }
    return [lo, hi];
  }
}
export const sub = (a, b) => a.map((x, i) => x - b[i]);
export const add = (a, b) => a.map((x, i) => x + b[i]);
export const scale = (a, s) => a.map(x => x * s);
export const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
export const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const norm = a => { const l = Math.hypot(...a) || 1; return a.map(x => x / l); };

// Rotation helpers returning point transforms.
export const rotX = t => p => [p[0], p[1] * Math.cos(t) - p[2] * Math.sin(t), p[1] * Math.sin(t) + p[2] * Math.cos(t)];
export const rotY = t => p => [p[0] * Math.cos(t) + p[2] * Math.sin(t), p[1], -p[0] * Math.sin(t) + p[2] * Math.cos(t)];
export const rotZ = t => p => [p[0] * Math.cos(t) - p[1] * Math.sin(t), p[0] * Math.sin(t) + p[1] * Math.cos(t), p[2]];
export const chain = (...fs) => p => fs.reduce((q, f) => f(q), p);
export const move = d => p => add(p, d);
export const stretch = s => p => p.map((x, i) => x * (Array.isArray(s) ? s[i] : s));

// A tube along a path of rings: [{c: centre, r: radius}], with a cap at each end.
// `shape(angle, ring)` may perturb the radius; UV u wraps `around` times.
export function tube(rings, {sides = 8, around = 1, vScale = 1, color = () => [1, 1, 1, 1], shape = () => 1, caps = true} = {}) {
  const m = new Mesh();
  let v = 0;
  const frames = rings.map((ring, k) => {
    const next = rings[Math.min(k + 1, rings.length - 1)].c, prev = rings[Math.max(k - 1, 0)].c;
    const t = norm(sub(next, prev).some(x => x) ? sub(next, prev) : [0, 1, 0]);
    const ref = Math.abs(t[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
    const a = norm(cross(t, ref)), b = cross(t, a);
    return {t, a, b};
  });
  for (let k = 0; k < rings.length; k++) {
    const {c, r} = rings[k], {a, b} = frames[k];
    if (k > 0) v += Math.hypot(...sub(c, rings[k - 1].c)) * vScale;
    for (let s = 0; s <= sides; s++) {
      const ang = (s / sides) * Math.PI * 2, rr = r * shape(ang, k);
      const dir = add(scale(a, Math.cos(ang)), scale(b, Math.sin(ang)));
      m.vertex(add(c, scale(dir, rr)), dir, [(s / sides) * around, v], color(k / Math.max(1, rings.length - 1), ang));
    }
  }
  for (let k = 0; k + 1 < rings.length; k++) for (let s = 0; s < sides; s++) {
    const a = k * (sides + 1) + s, b = a + 1, c = a + sides + 1, d = c + 1;
    m.tri(a, b, c); m.tri(b, d, c);
  }
  if (caps) for (const [k, sign] of [[0, -1], [rings.length - 1, 1]]) {
    if (rings[k].r <= 0.001) continue;
    const {c, r} = rings[k], {t, a, b} = frames[k], n = scale(t, sign);
    const centre = m.vertex(c, n, [0.5, 0.5], color(k ? 1 : 0, 0));
    const first = m.count;
    for (let s = 0; s <= sides; s++) {
      const ang = (s / sides) * Math.PI * 2, dir = add(scale(a, Math.cos(ang)), scale(b, Math.sin(ang)));
      m.vertex(add(c, scale(dir, r * shape(ang, k))), n, [0.5 + 0.5 * Math.cos(ang), 0.5 + 0.5 * Math.sin(ang)], color(k ? 1 : 0, ang));
    }
    for (let s = 0; s < sides; s++) sign > 0 ? m.tri(centre, first + s, first + s + 1) : m.tri(centre, first + s + 1, first + s);
  }
  return m;
}

// A subdivided icosahedron, displaced along its normals by `bump(direction)`,
// box-projected UVs, smooth normals.
export function blob(subdiv, bump = () => 1, color = () => [1, 1, 1, 1], uvScale = 1) {
  const t = (1 + Math.sqrt(5)) / 2;
  let verts = [[-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0], [0, -1, t], [0, 1, t], [0, -1, -t], [0, 1, -t], [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1]].map(norm);
  let faces = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8], [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
  for (let s = 0; s < subdiv; s++) {
    const cache = new Map(), mid = (a, b) => {
      const key = a < b ? `${a},${b}` : `${b},${a}`;
      if (!cache.has(key)) { verts.push(norm(add(verts[a], verts[b]))); cache.set(key, verts.length - 1); }
      return cache.get(key);
    };
    faces = faces.flatMap(([a, b, c]) => { const ab = mid(a, b), bc = mid(b, c), ca = mid(c, a); return [[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]; });
  }
  const m = new Mesh();
  for (const d of verts) {
    const p = scale(d, bump(d)), ax = d.map(Math.abs);
    const uv = ax[1] > ax[0] && ax[1] > ax[2] ? [p[0], p[2]] : ax[0] > ax[2] ? [p[2], p[1]] : [p[0], p[1]];
    m.vertex(p, d, scale(uv, uvScale), color(d));
  }
  for (const [a, b, c] of faces) m.tri(a, b, c);
  return m.smoothNormals();
}

// Two or three crossed vertical quads, for ferns, grass and leaf cards.
export function cards(n, w, h, {tilt = 0, color = () => [1, 1, 1, 1]} = {}) {
  const m = new Mesh();
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI, dx = Math.cos(a) * w / 2, dz = Math.sin(a) * w / 2;
    const lean = [Math.sin(a + 1.7) * tilt * h, 0, Math.cos(a + 1.7) * tilt * h];
    const nrm = norm([-Math.sin(a), 0.6, Math.cos(a)]);
    const b = m.count;
    m.vertex([-dx, 0, -dz], nrm, [0, 1], color(0)); m.vertex([dx, 0, dz], nrm, [1, 1], color(0));
    m.vertex(add([-dx, h, -dz], lean), nrm, [0, 0], color(1)); m.vertex(add([dx, h, dz], lean), nrm, [1, 0], color(1));
    m.tri(b, b + 1, b + 2); m.tri(b + 1, b + 3, b + 2);
  }
  return m;
}

// --- Binary glTF: one node per part (all at the origin) or one mesh with a
// primitive per material. Materials: {name, image (PNG bytes) | null, color,
// roughness, metallic, emissive, mask, doubleSided}. ---
export function glb(parts, materials) {
  const chunks = [], views = [], accessors = [];
  let offset = 0;
  const pushView = (bytes, target) => {
    const pad = (4 - (bytes.byteLength % 4)) % 4;
    chunks.push(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength), new Uint8Array(pad));
    views.push({buffer: 0, byteOffset: offset, byteLength: bytes.byteLength, ...(target ? {target} : {})});
    offset += bytes.byteLength + pad;
    return views.length - 1;
  };
  const accessor = (array, type, comps, minmax) => {
    const view = pushView(array, comps === 5125 ? 34963 : 34962);
    const count = array.length / ({SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4})[type];
    const a = {bufferView: view, componentType: comps, count, type};
    if (minmax) { const [lo, hi] = minmax; a.min = lo; a.max = hi; }
    accessors.push(a);
    return accessors.length - 1;
  };
  const images = [], textures = [];
  const mats = materials.map(m => {
    let tex;
    if (m.image) {
      images.push({bufferView: pushView(m.image), mimeType: 'image/png'});
      textures.push({source: images.length - 1, sampler: 0});
      tex = {index: textures.length - 1};
    }
    return {
      name: m.name,
      pbrMetallicRoughness: {baseColorFactor: m.color ?? [1, 1, 1, 1], metallicFactor: m.metallic ?? 0, roughnessFactor: m.roughness ?? 0.9, ...(tex ? {baseColorTexture: tex} : {})},
      ...(m.emissive ? {emissiveFactor: m.emissive} : {}),
      ...(m.mask ? {alphaMode: 'MASK', alphaCutoff: 0.5} : {}),
      ...(m.blend ? {alphaMode: 'BLEND'} : {}),
      ...(m.doubleSided ? {doubleSided: true} : {}),
    };
  });
  const meshes = parts.map(({name, prims}) => ({
    name,
    primitives: prims.map(({mesh, material}) => ({
      attributes: {
        POSITION: accessor(new Float32Array(mesh.p), 'VEC3', 5126, mesh.bounds()),
        NORMAL: accessor(new Float32Array(mesh.n), 'VEC3', 5126),
        TEXCOORD_0: accessor(new Float32Array(mesh.uv), 'VEC2', 5126),
        COLOR_0: accessor(new Float32Array(mesh.c), 'VEC4', 5126),
      },
      indices: accessor(new Uint32Array(mesh.i), 'SCALAR', 5125),
      material,
    })),
  }));
  const json = {
    asset: {version: '2.0', generator: 'forest artgen'},
    scene: 0, scenes: [{nodes: meshes.map((_, k) => k)}],
    nodes: meshes.map((m, k) => ({name: m.name, mesh: k})),
    meshes, accessors, bufferViews: views, materials: mats,
    ...(images.length ? {images, textures, samplers: [{magFilter: 9729, minFilter: 9987, wrapS: 10497, wrapT: 10497}]} : {}),
    buffers: [{byteLength: offset}],
  };
  let text = new TextEncoder().encode(JSON.stringify(json));
  const jpad = (4 - (text.length % 4)) % 4;
  text = new Uint8Array([...text, ...new Array(jpad).fill(32)]);
  const total = 12 + 8 + text.length + 8 + offset, out = new Uint8Array(total), dv = new DataView(out.buffer);
  dv.setUint32(0, 0x46546c67, true); dv.setUint32(4, 2, true); dv.setUint32(8, total, true);
  dv.setUint32(12, text.length, true); dv.setUint32(16, 0x4e4f534a, true); out.set(text, 20);
  let o = 20 + text.length;
  dv.setUint32(o, offset, true); dv.setUint32(o + 4, 0x004e4942, true); o += 8;
  for (const c of chunks) { out.set(c, o); o += c.length; }
  return out;
}
