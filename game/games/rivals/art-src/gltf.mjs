// A glTF 2.0 writer and the procedural geometry the art is built from: rounded
// boxes, cylinders, cones and prisms, with planar UVs in metres so tiling
// textures keep one texel density across every size.
import { writeFileSync } from 'node:fs';

/** A mesh part: flat arrays, one material. */
export class Geom {
  constructor() { this.p = []; this.n = []; this.uv = []; this.c = []; this.i = []; }
  get count() { return this.p.length / 3; }
  vertex(p, n, uv, c = [1, 1, 1, 1]) {
    this.p.push(...p); this.n.push(...n); this.uv.push(...uv); this.c.push(...c);
    return this.count - 1;
  }
  tri(a, b, c) { this.i.push(a, b, c); }
  quad(a, b, c, d) { this.i.push(a, b, c, a, c, d); }
  /** Append another part, optionally transformed by `m` (see `mat`). */
  add(g, m) {
    const base = this.count;
    for (let k = 0; k < g.count; k++) {
      let p = g.p.slice(k * 3, k * 3 + 3), n = g.n.slice(k * 3, k * 3 + 3);
      if (m) { p = m.point(p); n = m.normal(n); }
      this.vertex(p, n, g.uv.slice(k * 2, k * 2 + 2), g.c.slice(k * 4, k * 4 + 4));
    }
    for (const i of g.i) this.i.push(base + i);
    return this;
  }
  tint(rgba) { for (let k = 0; k < this.count; k++) this.c.splice(k * 4, 4, ...rgba); return this; }
}

const norm = (v) => { const l = Math.hypot(...v) || 1; return v.map((x) => x / l); };
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
export const quat = {
  axis: (axis, angle) => { const s = Math.sin(angle / 2), a = norm(axis); return [a[0] * s, a[1] * s, a[2] * s, Math.cos(angle / 2)]; },
  x: (a) => quat.axis([1, 0, 0], a), y: (a) => quat.axis([0, 1, 0], a), z: (a) => quat.axis([0, 0, 1], a),
  mul: (a, b) => [
    a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
    a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
    a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
    a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]],
  rotate: (q, v) => {
    const u = [q[0], q[1], q[2]], t = cross(u, v).map((x) => 2 * x);
    return v.map((x, i) => x + q[3] * t[i] + cross(u, t)[i]);
  },
};
/** A translate · rotate · scale transform for parts. */
export function mat(t = [0, 0, 0], r = [0, 0, 0, 1], s = [1, 1, 1]) {
  return {
    point: (p) => quat.rotate(r, p.map((x, i) => x * s[i])).map((x, i) => x + t[i]),
    normal: (n) => norm(quat.rotate(r, n.map((x, i) => x / s[i]))),
  };
}

/** A box with rounded edges (radius r), centred, planar UVs at `tile` metres. */
export function roundedBox(size, r = 0.02, tile = 1, band = 2) {
  const g = new Geom(), h = size.map((x) => x / 2), inner = h.map((x) => Math.max(0, x - r));
  const axis = (k) => {
    const out = [-h[k]];
    if (r > 0 && inner[k] > 0) {
      for (let i = 1; i < band; i++) out.push(-inner[k] - r * Math.sin((1 - i / band) * Math.PI / 2));
      out.push(-inner[k], inner[k]);
      for (let i = 1; i < band; i++) out.push(inner[k] + r * Math.sin((i / band) * Math.PI / 2));
    }
    out.push(h[k]);
    return out;
  };
  const faces = [[0, 1, 2, 1], [0, 1, 2, -1], [1, 2, 0, 1], [1, 2, 0, -1], [2, 0, 1, 1], [2, 0, 1, -1]];
  for (const [ax, u, v, sign] of faces) {
    const us = axis(u), vs = axis(v), base = g.count;
    for (const b of vs) for (const a of us) {
      const p = [0, 0, 0]; p[ax] = sign * h[ax]; p[u] = a; p[v] = b;
      const c = p.map((x, i) => Math.max(-inner[i], Math.min(inner[i], x)));
      const d = p.map((x, i) => x - c[i]), l = Math.hypot(...d);
      const n = l > 1e-6 ? d.map((x) => x / l) : [0, 0, 0].map((_, i) => (i === ax ? sign : 0));
      const q = l > 1e-6 ? c.map((x, i) => x + n[i] * r) : p;
      // Planar UVs along this face's two axes, in metres over the tile.
      g.vertex(q, n, [(sign > 0 ? q[u] : -q[u]) / tile + 0.5, -q[v] / tile + 0.5]);
    }
    const w = us.length;
    for (let j = 0; j + 1 < vs.length; j++) for (let i = 0; i + 1 < w; i++) {
      const a = base + j * w + i, b = a + 1, c = a + w + 1, d = a + w;
      // Winding: counterclockwise seen from outside (cross(u,v) = +ax for sign>0).
      if (sign > 0) g.quad(a, b, c, d); else g.quad(a, d, c, b);
    }
  }
  return g;
}
export const box = (size, tile = 1) => roundedBox(size, 0, tile);

/** A Y cylinder (radius, height) with caps; `segments` around. */
export function cylinder(radius, height, segments = 16, { caps = true, top = radius, tile = 1 } = {}) {
  const g = new Geom(), y0 = -height / 2, y1 = height / 2;
  const slope = (radius - top) / height;
  for (let s = 0; s <= segments; s++) {
    const a = (s / segments) * Math.PI * 2, x = Math.cos(a), z = Math.sin(a);
    const n = norm([x, slope, z]);
    g.vertex([x * radius, y0, z * radius], n, [s / segments * 2 * Math.PI * radius / tile, 0]);
    g.vertex([x * top, y1, z * top], n, [s / segments * 2 * Math.PI * radius / tile, -height / tile]);
  }
  for (let s = 0; s < segments; s++) { const a = s * 2; g.quad(a, a + 1, a + 3, a + 2); }
  if (caps) for (const [y, rr, ny] of [[y1, top, 1], [y0, radius, -1]]) {
    if (rr <= 0) continue;
    const c = g.vertex([0, y, 0], [0, ny, 0], [0.5, 0.5]), base = g.count;
    for (let s = 0; s <= segments; s++) {
      const a = (s / segments) * Math.PI * 2;
      g.vertex([Math.cos(a) * rr, y, Math.sin(a) * rr], [0, ny, 0], [Math.cos(a) * rr / tile + 0.5, Math.sin(a) * rr / tile + 0.5]);
    }
    for (let s = 0; s < segments; s++) ny > 0 ? g.tri(c, base + s + 1, base + s) : g.tri(c, base + s, base + s + 1);
  }
  return g;
}
/** A cylinder along Z (the guns' barrels look down −Z). */
export const barrel = (radius, length, segments = 16, opts) => new Geom().add(cylinder(radius, length, segments, opts), mat([0, 0, 0], quat.x(Math.PI / 2)));
/** An extruded convex outline (XY points, counterclockwise) of `depth` along X. */
export function prism(outline, depth) {
  const g = new Geom(), x0 = -depth / 2, x1 = depth / 2;
  for (const [x, nx] of [[x1, 1], [x0, -1]]) {
    const base = g.count;
    for (const [y, z] of outline) g.vertex([x, y, z], [nx, 0, 0], [z, -y]);
    for (let i = 1; i + 1 < outline.length; i++) nx > 0 ? g.tri(base, base + i, base + i + 1) : g.tri(base, base + i + 1, base + i);
  }
  for (let i = 0; i < outline.length; i++) {
    const [ya, za] = outline[i], [yb, zb] = outline[(i + 1) % outline.length];
    const n = norm([0, zb - za, -(yb - ya)]), base = g.count, len = Math.hypot(yb - ya, zb - za);
    g.vertex([x0, ya, za], n, [0, 0]); g.vertex([x1, ya, za], n, [depth, 0]);
    g.vertex([x1, yb, zb], n, [depth, len]); g.vertex([x0, yb, zb], n, [0, len]);
    g.quad(base, base + 3, base + 2, base + 1);
  }
  return g;
}
/** A UV sphere. */
export function sphere(radius, rings = 10, segments = 16, squash = [1, 1, 1]) {
  const g = new Geom();
  for (let r = 0; r <= rings; r++) {
    const t = r / rings * Math.PI, y = Math.cos(t), s = Math.sin(t);
    for (let k = 0; k <= segments; k++) {
      const a = k / segments * Math.PI * 2, n = [Math.cos(a) * s, y, Math.sin(a) * s];
      g.vertex(n.map((x, i) => x * radius * squash[i]), norm(n.map((x, i) => x / squash[i])), [k / segments, r / rings]);
    }
  }
  const w = segments + 1;
  for (let r = 0; r < rings; r++) for (let k = 0; k < segments; k++) {
    const a = r * w + k;
    g.quad(a, a + 1, a + w + 1, a + w);
  }
  return g;
}

/** Collects materials, meshes, nodes, images and animations; writes one .gltf. */
export class Gltf {
  /** PNG bytes by name, set once by the generator; models embed what they use. */
  static images = {};
  constructor(generator = 'rivals art-src') {
    this.json = { asset: { version: '2.0', generator }, scene: 0, scenes: [{ nodes: [] }], nodes: [], meshes: [],
      materials: [], accessors: [], bufferViews: [], buffers: [] };
    this.bin = []; this.length = 0; this.names = new Map();
  }
  view(bytes, target) {
    const pad = (4 - (this.length % 4)) % 4;
    if (pad) { this.bin.push(new Uint8Array(pad)); this.length += pad; }
    this.json.bufferViews.push({ buffer: 0, byteOffset: this.length, byteLength: bytes.byteLength, ...(target ? { target } : {}) });
    this.bin.push(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)); this.length += bytes.byteLength;
    return this.json.bufferViews.length - 1;
  }
  accessor(array, type, componentType, target, minmax = false) {
    const width = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 }[type];
    const a = { bufferView: this.view(array, target), componentType, count: array.length / width, type };
    if (minmax) {
      a.min = []; a.max = [];
      for (let k = 0; k < width; k++) {
        let lo = Infinity, hi = -Infinity;
        for (let i = k; i < array.length; i += width) { lo = Math.min(lo, array[i]); hi = Math.max(hi, array[i]); }
        a.min.push(lo); a.max.push(hi);
      }
    }
    this.json.accessors.push(a);
    return this.json.accessors.length - 1;
  }
  /** A texture embedded in the buffer (PNG bytes from `Gltf.images`), repeating. */
  texture(name, repeat = true) {
    const key = `tex:${name}:${repeat}`;
    if (this.names.has(key)) return this.names.get(key);
    const bytes = Gltf.images[name];
    if (!bytes) throw new Error(`no image ${name}`);
    (this.json.images ??= []).push({ name, mimeType: 'image/png', bufferView: this.view(bytes) });
    (this.json.samplers ??= []).push({ magFilter: 9729, minFilter: 9987, wrapS: repeat ? 10497 : 33071, wrapT: repeat ? 10497 : 33071 });
    (this.json.textures ??= []).push({ source: this.json.images.length - 1, sampler: this.json.samplers.length - 1 });
    this.names.set(key, this.json.textures.length - 1);
    return this.json.textures.length - 1;
  }
  /** A PBR material by name: {color, metal, rough, emissive, map, normal, mr, emissiveMap, alpha}. */
  material(name, m) {
    if (this.names.has(`mat:${name}`)) return this.names.get(`mat:${name}`);
    const pbr = { baseColorFactor: m.color ?? [1, 1, 1, 1], metallicFactor: m.metal ?? 0, roughnessFactor: m.rough ?? 0.6 };
    if (m.map) pbr.baseColorTexture = { index: this.texture(m.map) };
    if (m.mr) pbr.metallicRoughnessTexture = { index: this.texture(m.mr) };
    const out = { name, pbrMetallicRoughness: pbr };
    if (m.normal) out.normalTexture = { index: this.texture(m.normal), scale: m.normalScale ?? 1 };
    if (m.emissive) out.emissiveFactor = m.emissive.map((x) => Math.min(1, x));
    if (m.emissive && Math.max(...m.emissive) > 1) {
      const k = Math.max(...m.emissive);
      out.emissiveFactor = m.emissive.map((x) => x / k);
      out.extensions = { KHR_materials_emissive_strength: { emissiveStrength: k } };
      (this.json.extensionsUsed ??= []).includes('KHR_materials_emissive_strength') || this.json.extensionsUsed.push('KHR_materials_emissive_strength');
    }
    if (m.emissiveMap) out.emissiveTexture = { index: this.texture(m.emissiveMap) };
    if (m.alpha) { out.alphaMode = 'BLEND'; out.doubleSided = true; }
    this.json.materials.push(out);
    this.names.set(`mat:${name}`, this.json.materials.length - 1);
    return this.json.materials.length - 1;
  }
  /** A mesh of [Geom, material] parts. */
  mesh(name, parts) {
    const primitives = parts.filter(([g]) => g.count).map(([g, material]) => ({
      attributes: {
        POSITION: this.accessor(new Float32Array(g.p), 'VEC3', 5126, 34962, true),
        NORMAL: this.accessor(new Float32Array(g.n), 'VEC3', 5126, 34962),
        TEXCOORD_0: this.accessor(new Float32Array(g.uv), 'VEC2', 5126, 34962),
        // Vertex colours only where a part was tinted: white is the default.
        ...(g.c.some((x) => x !== 1) ? { COLOR_0: this.accessor(new Float32Array(g.c), 'VEC4', 5126, 34962) } : {}),
      },
      indices: this.accessor(new Uint32Array(g.i), 'SCALAR', 5125, 34963),
      material,
    }));
    this.json.meshes.push({ name, primitives });
    return this.json.meshes.length - 1;
  }
  node(n, root = false) {
    this.json.nodes.push(n);
    const i = this.json.nodes.length - 1;
    if (root) this.json.scenes[0].nodes.push(i);
    return i;
  }
  /** A clip: channels of {node, path: rotation|translation|scale, times, values (flat)}. */
  animation(name, channels) {
    const samplers = [], out = [];
    for (const c of channels) {
      samplers.push({ input: this.accessor(new Float32Array(c.times), 'SCALAR', 5126, undefined, true),
        output: this.accessor(new Float32Array(c.values), c.path === 'rotation' ? 'VEC4' : 'VEC3', 5126), interpolation: 'LINEAR' });
      out.push({ sampler: samplers.length - 1, target: { node: c.node, path: c.path } });
    }
    (this.json.animations ??= []).push({ name, samplers, channels: out });
  }
  write(path) {
    const bytes = new Uint8Array(this.length);
    let at = 0;
    for (const b of this.bin) { bytes.set(b, at); at += b.length; }
    this.json.buffers = [{ byteLength: bytes.length, uri: `data:application/octet-stream;base64,${Buffer.from(bytes).toString('base64')}` }];
    if (!this.json.meshes.length) delete this.json.meshes;
    writeFileSync(path, JSON.stringify(this.json));
    return bytes.length;
  }
}
