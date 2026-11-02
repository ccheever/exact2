// The garden's offline mesh kit, shared by art.mjs (the art pass) and
// looks.mjs (the golden and storybook looks): a binary glTF writer, and a
// port of the engine's `asset::MeshBuilder` (smooth shapes painted per
// vertex: ellipsoids, tubes, lathes, sheets and boxes), so a look authored for
// it keeps its shapes once baked.
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const ART = resolve(import.meta.dir, 'art');

// ------------------------------------------------------------ vectors
export const V = {
  add: (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]],
  sub: (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]],
  scale: (a, k) => [a[0] * k, a[1] * k, a[2] * k],
  mul: (a, b) => [a[0] * b[0], a[1] * b[1], a[2] * b[2]],
  div: (a, b) => [a[0] / b[0], a[1] / b[1], a[2] / b[2]],
  dot: (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
  cross: (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]],
  len: a => Math.hypot(a[0], a[1], a[2]),
  /** Normalized, or zero for a degenerate vector (glam's `normalize_or_zero`). */
  norm: a => { const l = Math.hypot(a[0], a[1], a[2]); return l > 0 && Number.isFinite(1 / l) ? [a[0] / l, a[1] / l, a[2] / l] : [0, 0, 0]; },
  lerp: (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t],
  zero: a => a[0] === 0 && a[1] === 0 && a[2] === 0,
};

// Rotations as 3×3 row matrices; `rot(a, b)` is glam's `a * b` of quaternions.
export const IDENTITY = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
export const rotX = a => { const c = Math.cos(a), s = Math.sin(a); return [[1, 0, 0], [0, c, -s], [0, s, c]]; };
export const rotY = a => { const c = Math.cos(a), s = Math.sin(a); return [[c, 0, s], [0, 1, 0], [-s, 0, c]]; };
export const rotZ = a => { const c = Math.cos(a), s = Math.sin(a); return [[c, -s, 0], [s, c, 0], [0, 0, 1]]; };
export const rot = (A, B) => A.map(row => [0, 1, 2].map(j => row[0] * B[0][j] + row[1] * B[1][j] + row[2] * B[2][j]));
export const turn = (M, v) => M.map(row => row[0] * v[0] + row[1] * v[1] + row[2] * v[2]);

/** A unit vector perpendicular to `axis` (the engine's `asset::across`). */
export const across = axis => V.norm(V.cross(axis, Math.abs(axis[1]) > 0.95 ? [1, 0, 0] : [0, 1, 0]));

// ------------------------------------------------------------ the builder
/** The engine's smooth `MeshBuilder`: shapes keep their vertices and the
 * normals they give them; `squared` stores colours authored by eye squared. */
export class MeshBuilder {
  constructor({ squared = false } = {}) { this.squared = squared; this.p = []; this.n = []; this.c = []; this.i = []; }
  isEmpty() { return this.p.length === 0; }
  vertex(p, n, color) {
    const k = this.p.length / 3, m = V.norm(n);
    this.p.push(p[0], p[1], p[2]);
    this.n.push(m[0], m[1], m[2]);
    const c = this.squared ? color.map(v => v * v) : color;
    this.c.push(c[0], c[1], c[2]);
    return k;
  }
  triangle(a, b, c) { this.i.push(a, b, c); }
  /** A box of `size` centred on `at`, turned by `R`; `shade(corner)`, each axis ±1. */
  cuboid(at, size, R, shade) {
    const p = (x, y, z) => { const corner = [x, y, z]; return [V.add(at, turn(R, V.scale(V.mul(size, corner), 0.5))), corner]; };
    for (const face of [
      [p(-1, -1, 1), p(1, -1, 1), p(1, 1, 1), p(-1, 1, 1)],
      [p(1, -1, -1), p(-1, -1, -1), p(-1, 1, -1), p(1, 1, -1)],
      [p(1, -1, 1), p(1, -1, -1), p(1, 1, -1), p(1, 1, 1)],
      [p(-1, -1, -1), p(-1, -1, 1), p(-1, 1, 1), p(-1, 1, -1)],
      [p(-1, 1, 1), p(1, 1, 1), p(1, 1, -1), p(-1, 1, -1)],
      [p(-1, -1, -1), p(1, -1, -1), p(1, -1, 1), p(-1, -1, 1)],
    ]) {
      const n = V.cross(V.sub(face[1][0], face[0][0]), V.sub(face[2][0], face[0][0]));
      const ids = face.map(([q, corner]) => this.vertex(q, n, shade(corner)));
      this.triangle(ids[0], ids[1], ids[2]);
      this.triangle(ids[0], ids[2], ids[3]);
    }
  }
  /** `rings` pole to pole and `segs` around; `shade(d)` from the unit direction in its own frame. */
  ellipsoid(at, radius, R, [rings, segs], shade) {
    const grid = [];
    for (let j = 0; j <= rings; j++) {
      const sy = Math.sin(j * Math.PI / rings), cy = Math.cos(j * Math.PI / rings);
      for (let k = 0; k <= segs; k++) {
        const a = k * 2 * Math.PI / segs, d = [Math.cos(a) * sy, cy, Math.sin(a) * sy];
        grid.push(this.vertex(V.add(at, turn(R, V.mul(radius, d))), turn(R, V.div(d, radius)), shade(d)));
      }
    }
    const row = segs + 1;
    for (let j = 0; j < rings; j++) for (let k = 0; k < segs; k++) {
      const a = grid[j * row + k], b = grid[j * row + k + 1], c = grid[(j + 1) * row + k], d = grid[(j + 1) * row + k + 1];
      if (j > 0) this.triangle(a, b, c);
      if (j + 1 < rings) this.triangle(b, d, c);
    }
  }
  /** A tube along `path` ([point, radius]), its frame carried so it does not
   * twist; `shade(t, angle)`. `cap` closes the far end. */
  tube(path, segs, cap, shade) {
    const n = path.length, rings = [];
    let side = across(V.sub(path[1][0], path[0][0])), last = [[0, 0, 0], [0, 0, 0]];
    path.forEach(([p, r], i) => {
      let axis = V.norm(i + 1 < n ? V.sub(path[i + 1][0], p) : V.sub(p, path[i - 1][0]));
      if (V.zero(axis)) axis = [0, 1, 0];
      side = V.norm(V.sub(side, V.scale(axis, V.dot(side, axis))));
      if (V.zero(side)) side = across(axis);
      const other = V.cross(axis, side);
      last = [side, other];
      const t = i / (n - 1);
      for (let k = 0; k <= segs; k++) {
        const a = k * 2 * Math.PI / segs, radial = V.add(V.scale(side, Math.cos(a)), V.scale(other, Math.sin(a)));
        rings.push(this.vertex(V.add(p, V.scale(radial, r)), radial, shade(t, a)));
      }
    });
    const row = segs + 1;
    for (let i = 0; i < n - 1; i++) for (let k = 0; k < segs; k++) {
      const a = i * row + k;
      this.triangle(rings[a], rings[a + 1], rings[a + row]);
      this.triangle(rings[a + 1], rings[a + row + 1], rings[a + row]);
    }
    const [end, r] = path[n - 1];
    if (cap && r > 0) {
      const axis = V.norm(V.sub(end, path[n - 2][0]));
      const mid = this.vertex(end, axis, shade(1, 0));
      const ring = [];
      for (let k = 0; k <= segs; k++) {
        const a = k * 2 * Math.PI / segs;
        ring.push(this.vertex(V.add(end, V.scale(V.add(V.scale(last[0], Math.cos(a)), V.scale(last[1], Math.sin(a))), r)), axis, shade(1, a)));
      }
      for (let k = 0; k < segs; k++) this.triangle(ring[k], ring[k + 1], mid);
    }
  }
  /** About +Y through `at`: [height, radius] from bottom to top; `shade(height, angle)`. */
  lathe(at, profile, segs, shade) {
    const n = profile.length, rings = [];
    for (let i = 0; i < n; i++) {
      const [y, r] = profile[i], [y0, r0] = profile[Math.max(0, i - 1)], [y1, r1] = profile[Math.min(n - 1, i + 1)];
      const dy = y1 - y0, dr = r1 - r0;
      for (let k = 0; k <= segs; k++) {
        const a = k * 2 * Math.PI / segs, radial = [Math.cos(a), 0, Math.sin(a)];
        const normal = V.sub(V.scale(radial, dy), [0, dr, 0]);
        rings.push(this.vertex(V.add(V.add(at, V.scale(radial, r)), [0, y, 0]), normal, shade(y, a)));
      }
    }
    const row = segs + 1;
    for (let i = 0; i < n - 1; i++) for (let k = 0; k < segs; k++) {
      const a = i * row + k;
      this.triangle(rings[a], rings[a + row], rings[a + 1]);
      this.triangle(rings[a + 1], rings[a + row], rings[a + row + 1]);
    }
  }
  /** Rows of [point, colour], normals from the surface; `back` adds the
   * reverse face, its colours scaled by that much. */
  sheet(rows, back) {
    const cols = rows[0].length, R = rows.length;
    const normal = (r, c) => {
      const p = (r, c) => rows[r][c][0];
      const du = V.sub(p(r, Math.min(c + 1, cols - 1)), p(r, Math.max(c - 1, 0)));
      const dv = V.sub(p(Math.min(r + 1, R - 1), c), p(Math.max(r - 1, 0), c));
      const m = V.cross(du, dv);
      return V.dot(m, m) < 1e-12 ? [0, 1, 0] : m;
    };
    for (const shade of back === undefined ? [null] : [null, back]) {
      const reverse = shade !== null, grid = [];
      rows.forEach((row, r) => row.forEach(([p, color], c) => {
        const m = normal(r, c);
        grid.push(this.vertex(p, reverse ? V.scale(m, -1) : m, shade === null ? color : color.map(v => v * shade)));
      }));
      for (let r = 0; r < R - 1; r++) for (let c = 0; c < cols - 1; c++) {
        const a = grid[r * cols + c], b = grid[r * cols + c + 1], d = grid[(r + 1) * cols + c], e = grid[(r + 1) * cols + c + 1];
        if (reverse) { this.triangle(a, d, b); this.triangle(b, d, e); } else { this.triangle(a, b, d); this.triangle(b, e, d); }
      }
    }
  }
}

// ------------------------------------------------------------ binary glTF
// Linear vertex colours: unsigned bytes for the art pass's, unsigned shorts
// for a look's (squared dark tones need the precision), floats where a
// painted highlight goes past one.
const COLOR = {
  u8: { type: 'VEC4', component: 5121, width: 4, make: c => quantize(c, Uint8Array, 255) },
  u16: { type: 'VEC4', component: 5123, width: 4, make: c => quantize(c, Uint16Array, 65535) },
  f32: { type: 'VEC3', component: 5126, width: 3, make: c => new Float32Array(c) },
};
function quantize(c, Typed, max) {
  const out = new Typed((c.length / 3) * 4);
  for (let i = 0, o = 0; i < c.length; i += 3, o += 4) {
    for (let k = 0; k < 3; k++) out[o + k] = Math.round(Math.max(0, Math.min(1, c[i + k])) * max);
    out[o + 3] = max;
  }
  return out;
}
/** Write `art/<name>.glb`: one node, one mesh, a primitive per part, in
 * order, each with its material (`spec`: color, metal, rough, tex, double,
 * emissive, strength, colors 'u8' | 'u16' | 'f32' (float when unset and a
 * colour exceeds one)). Material `i` is part `i`. `instances` adds meshes
 * placed many times, each `{part, at: [[position, yaw], …]}` drawn with the
 * first part's material: a meadow of tufts is a few tufts and their places,
 * which the renderer merges into one mesh when the model loads. */
export function writeGlb(name, parts, instances = []) {
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
  for (const [matName, P] of parts) {
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
    json.meshes[0].primitives.push(primitive(P, json.materials.length - 1));
  }
  function primitive(P, material) {
    const m = P.spec, count = P.p.length / 3, min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < P.p.length; i++) { min[i % 3] = Math.min(min[i % 3], P.p[i]); max[i % 3] = Math.max(max[i % 3], P.p[i]); }
    const colors = COLOR[m.colors ?? (P.c.some(v => v > 1) ? 'f32' : 'u16')];
    const attributes = {
      POSITION: accessor(new Float32Array(P.p), 'VEC3', 5126, 34962, { min, max }),
      NORMAL: accessor(new Float32Array(P.n), 'VEC3', 5126, 34962),
      COLOR_0: accessor(colors.make(P.c), colors.type, colors.component, 34962, colors.component === 5126 ? {} : { normalized: true }),
    };
    if (m.tex) attributes.TEXCOORD_0 = accessor(new Float32Array(P.t), 'VEC2', 5126, 34962);
    const indices = count < 65536 ? accessor(new Uint16Array(P.i), 'SCALAR', 5123, 34963) : accessor(new Uint32Array(P.i), 'SCALAR', 5125, 34963);
    return { attributes, indices, material };
  }
  for (const { part, at } of instances) {
    json.meshes.push({ primitives: [primitive(part, 0)] });
    const mesh = json.meshes.length - 1;
    for (const [translation, yaw] of at) {
      json.nodes.push({ mesh, translation, rotation: [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)] });
      json.scenes[0].nodes.push(json.nodes.length - 1);
    }
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
  let triangles = 0;
  for (const [, P] of parts) triangles += P.i.length / 3;
  for (const { part, at } of instances) triangles += part.i.length / 3 * at.length;
  return { bytes: out.length, triangles };
}
