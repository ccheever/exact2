// The golden and storybook looks' models, authored as code and baked like the
// art pass's: `golden-<model>.glb` and `storybook-<model>.glb` in art/, for
// the gardener, the watering can, the orchard and fences, flowers, the meadow,
// the backdrop, the bed's rails, the barrel, and a plant and a fruit per crop
// (`golden-plant-carrot`, `golden-fruit-carrot`). Each vertex is painted, so
// shading (contact darkening, sunlit tips, a painted highlight) is in the mesh.
// Palettes and the sun come from assets/looks.level.json, crops from
// assets/garden.level.json. Run through art.mjs, which writes every model.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { MeshBuilder, V, IDENTITY, rotX, rotY, rotZ, rot, across, writeGlb } from './kit.mjs';

const PI = Math.PI, TAU = 2 * Math.PI;
const data = name => JSON.parse(readFileSync(resolve(import.meta.dir, 'assets', name), 'utf8'));

// ------------------------------------------------------------ colour
const mix = (a, b, t) => { t = Math.min(1, Math.max(0, t)); return [0, 1, 2].map(i => a[i] + (b[i] - a[i]) * t); };
const scale = (c, k) => c.map(v => v * k);
/** Pushes a colour away from its grey: `k > 1` saturates. */
const saturate = (c, k) => { const g = (c[0] + c[1] + c[2]) / 3; return c.map(v => Math.min(1, Math.max(0, g + (v - g) * k))); };
const grey = v => [v, v, v];
/** A stable value in [0, 1) for scattering scenery (u32 arithmetic). */
const hash = i => {
  let x = (Math.imul(i >>> 0, 0x9e3779b9) ^ 0x85ebca6b) >>> 0;
  x = (x ^ (x >>> 15)) >>> 0;
  x = Math.imul(x, 0x2c1b3c6d) >>> 0;
  x = (x ^ (x >>> 12)) >>> 0;
  x = Math.imul(x, 0x297a2d39) >>> 0;
  x = (x ^ (x >>> 15)) >>> 0;
  return (x >>> 8) / 16777216;
};
/** Rust's saturating `as u32`. */
const u32 = v => (v > 0 ? Math.min(Math.trunc(v), 4294967295) : 0);
/** Rust's `f32::round`: halves away from zero. */
const round = v => Math.sign(v) * Math.round(Math.abs(v));
const dir = a => [Math.cos(a), 0, Math.sin(a)];
const Y = [0, 1, 0];
const vy = y => [0, y, 0];

// ------------------------------------------------------------ a look
/** A look's palette with its painted light. */
function look(palette) {
  const l = { ...palette };
  const light = V.norm(l.sun);
  /** Leafy shading on a clump from its unit direction: dark beneath, the sun's side lifted. */
  l.foliage = (tone, d) => {
    const c = mix(scale(tone, l.toy ? 0.68 : 0.5), tone, d[1] * 0.5 + 0.5);
    const sun = Math.max(V.dot(d, light), 0);
    return mix(c, mix(tone, l.sunlit, 0.55), sun * sun * 0.75);
  };
  l.leafTones = c => {
    const tone = saturate(c.leaf, l.sat);
    return l.toy ? [scale(tone, 0.82), mix(tone, [0.9, 1.0, 0.6], 0.4)] : [scale(tone, 0.6), mix(tone, [0.86, 0.84, 0.36], 0.3)];
  };
  /** A soft painted body: darker underneath, lighter toward the sun. */
  l.soft = (c, d) => {
    const floor = l.toy ? 0.82 : 0.72;
    return scale(c, floor + (1 - floor) * (d[1] * 0.5 + 0.5) + 0.12 * Math.max(V.dot(d, light), 0));
  };
  return l;
}

// ------------------------------------------------------------ sculpting
/** A smooth mesh with authored colours (stored squared), and the (length,
 * width) every leaf is scaled by: the garden's leaves, blades, slabs and balls. */
class Sculpt extends MeshBuilder {
  constructor() { super({ squared: true }); this.leafScale = [1, 1]; }
  leafy(length, width) { this.leafScale = [length, width]; }
  /** A box with flat faces, darker toward its bottom by `under`. */
  slab(at, size, R, color, under) { this.cuboid(at, size, R, c => scale(color, 1 - under * (0.5 - c[1] * 0.5))); }
  /** A ball painted from `low` (underside) to `high`, with a painted highlight when `gloss` > 0. */
  ball(at, radius, low, high, gloss) {
    const light = V.norm([-0.45, 0.8, 0.4]);
    this.ellipsoid(at, radius, IDENTITY, [8, 14], d => {
      const c = mix(low, high, d[1] * 0.5 + 0.5), h = Math.max(V.dot(d, light), 0);
      return scale(c, 1 + gloss * h ** 6);
    });
  }
  /** A curved leaf from `root` toward `tip`: `round` 0 a blade, 1 a paddle;
   * `droop` bends the tip down; the halves fold up by `fold`. */
  leaf(root, tip, width, round, droop, fold, base, edge) {
    const axis = V.scale(V.sub(tip, root), this.leafScale[0]);
    width *= this.leafScale[1];
    const side = V.scale(across(axis), width);
    let up = V.norm(V.cross(side, axis));
    if (up[1] < 0) up = V.scale(up, -1);
    const length = V.len(axis), rows = [];
    for (let i = 0; i <= 7; i++) {
      const u = i / 7, profile = Math.max(Math.sin(u * PI), 0);
      let w = profile * (1 - round) * Math.sqrt(profile) + profile * round;
      w *= 1 - 0.25 * u * u * (1 - round);
      const mid = V.add(V.sub(V.add(root, V.scale(axis, u)), vy(droop * length * u * u)), V.scale(up, length * 0.12 * u * (1 - u)));
      const tint = mix(base, edge, u * 0.85);
      rows.push([-1, -0.5, 0, 0.5, 1].map(s => {
        const lift = V.scale(up, fold * width * w * s * s);
        let color = mix(tint, edge, Math.abs(s) * 0.35);
        if (s === 0) color = scale(color, 1.12);
        return [V.add(V.add(mid, V.scale(side, s * w)), lift), color];
      }));
    }
    this.sheet(rows, 0.8);
  }
  /** One tapering grass blade, dark at the root. */
  blade(root, tip, width, low, high) {
    const side = V.scale(across(V.sub(tip, root)), width);
    const bend = V.scale([tip[0] - root[0], 0, tip[2] - root[2]], 0.35), rows = [];
    for (let i = 0; i <= 3; i++) {
      const u = i / 3, mid = V.add(V.lerp(root, tip, u), V.scale(bend, u * u - u)), w = 1 - u * 0.92, c = mix(low, high, u);
      rows.push([[V.sub(mid, V.scale(side, w)), c], [V.add(mid, V.scale(side, w)), c]]);
    }
    this.sheet(rows, 0.8);
  }
}

// ------------------------------------------------------------ the gardener
function gardener(l) {
  const m = new Sculpt(), toy = l.toy;
  const [headAt, headR] = toy ? [[0, 0.47, 0], [0.32, 0.31, 0.30]] : [[0, 0.44, 0], [0.25, 0.27, 0.24]];
  const body = c => d => l.soft(c, d);
  // Shirt, then the overalls over its lower half, bib and straps.
  const [torsoAt, torsoR] = toy ? [[0, -0.06, 0], [0.28, 0.29, 0.22]] : [[0, -0.02, 0], [0.30, 0.33, 0.21]];
  m.ellipsoid(torsoAt, torsoR, IDENTITY, [12, 18], body(l.shirt));
  m.ellipsoid([0, -0.17, 0], [torsoR[0] + 0.015, 0.21, torsoR[2] + 0.015], IDENTITY, [10, 18], body(l.denim));
  m.slab([0, 0, torsoR[2] - 0.02], [0.30, 0.27, 0.05], rotX(-0.12), l.denim, 0.25);
  m.slab([0, -0.01, torsoR[2] + 0.008], [0.13, 0.09, 0.012], rotX(-0.12), scale(l.denim, 1.18), 0.1);
  for (const x of [-0.11, 0.11]) {
    m.slab([x, 0.18, 0.08], [0.06, 0.05, 0.24], rotX(0.35), l.denim, 0.2);
    m.ball([x, 0.11, torsoR[2] + 0.03], grey(0.026), scale(l.metal, 0.7), l.metal, 0.6);
  }
  if (toy) { // a neckerchief
    m.ellipsoid([0, 0.19, 0.06], [0.21, 0.06, 0.17], IDENTITY, [6, 14], body(l.band));
    m.ellipsoid([0, 0.13, 0.19], [0.07, 0.08, 0.03], rotZ(PI / 4), [6, 8], body(l.band));
  }
  // Head: skin with painted cheeks.
  m.ellipsoid(headAt, headR, IDENTITY, [14, 20], d => {
    let blush = 0;
    if (d[2] > 0.2) { const dx = Math.abs(d[0]) - 0.55, dy = d[1] + 0.2; blush = Math.exp(-(dx * dx + dy * dy) * (toy ? 14 : 20)); }
    return l.soft(mix(l.skin, l.cheek, blush * (toy ? 0.9 : 0.5)), d);
  });
  const front = headAt[2] + headR[2];
  const at = (x, y, inset) => [x, headAt[1] + y, front - inset];
  // Hair, showing below the hat at the sides and back.
  m.ellipsoid(V.add(headAt, [0, 0.06, -0.04]), V.mul(headR, [1.04, 0.74, 1.04]), IDENTITY, [8, 16], body(l.hair));
  // Nose, ears.
  m.ball(at(0, -0.02, 0), V.scale([0.05, 0.045, 0.045], toy ? 0.9 : 1), scale(l.skin, 0.85), mix(l.skin, l.cheek, 0.35), 0.3);
  for (const s of [-1, 1]) m.ball(V.add(headAt, [s * headR[0] * 0.98, -0.01, 0]), [0.035, 0.06, 0.045], scale(l.skin, 0.8), l.skin, 0);
  // Eyes with a catch-light.
  const [eyeX, eyeR] = toy ? [0.115, [0.042, 0.062, 0.03]] : [0.085, [0.026, 0.034, 0.02]];
  for (const s of [-1, 1]) {
    const eye = at(s * eyeX, 0.045, toy ? 0.035 : 0.022);
    m.ball(eye, eyeR, [0.10, 0.08, 0.07], [0.18, 0.14, 0.12], 0);
    m.ball(V.add(eye, [0.012, 0.018, eyeR[2] * 0.9]), grey(eyeR[0] * 0.38), grey(1.0), grey(1.2), 0);
    if (!toy) m.slab(at(s * 0.09, 0.10, 0.03), [0.08, 0.022, 0.025], rotZ(-s * 0.18), l.hair, 0);
  }
  if (toy) m.slab(at(0, -0.10, 0.03), [0.07, 0.016, 0.015], IDENTITY, [0.55, 0.22, 0.20], 0);
  else m.ellipsoid(at(0, -0.065, 0.012), [0.085, 0.024, 0.028], IDENTITY, [6, 12], body(l.hair)); // a moustache
  // Straw hat: a woven brim, crown and band.
  const brimY = headAt[1] + headR[1] * 0.82;
  const weave = d => l.soft(scale(l.straw, 0.92 + 0.08 * Math.sin(Math.hypot(d[0], d[2]) * 40)), d);
  m.ellipsoid(vy(brimY), V.scale([0.50, 0.03, 0.50], toy ? 1.06 : 1), rotX(-0.08), [6, 28], weave);
  const crown = [headR[0], 0.16, headR[2]];
  m.ellipsoid(vy(brimY + 0.10), crown, IDENTITY, [8, 20], weave);
  m.ellipsoid(vy(brimY + 0.045), [crown[0] + 0.012, 0.05, crown[2] + 0.012], IDENTITY, [6, 20], body(l.band));
  if (toy) { // a daisy tucked in the band
    const c = [crown[0] * 0.8, brimY + 0.06, crown[2] * 0.6];
    for (let i = 0; i < 6; i++) { const d = dir(i * TAU / 6); m.ball(V.add(c, [d[0] * 0.045, d[2] * 0.045, 0.02]), grey(0.03), grey(0.9), grey(1.0), 0); }
    m.ball(V.add(c, [0, 0, 0.035]), grey(0.028), [1.0, 0.7, 0.2], [1.0, 0.85, 0.3], 0);
  }
  // Satchel on the right hip.
  m.slab([0.32, -0.16, -0.21], [0.25, 0.32, 0.22], rotY(0.2), l.bag, 0.35);
  m.slab([0.33, -0.05, -0.19], [0.27, 0.14, 0.25], rotY(0.2), scale(l.bag, 1.12), 0.2);
  return m;
}
function arm(l) {
  const m = new Sculpt(), sleeve = l.shirt;
  const soft = c => (_, a) => scale(c, 0.85 + 0.15 * Math.cos(a));
  m.ball([0, 0, 0], grey(0.115), scale(sleeve, 0.8), sleeve, 0);
  m.tube([[[0, 0, 0], 0.11], [[0, -0.30, 0.01], 0.092]], 12, false, soft(sleeve));
  m.ellipsoid([0, -0.30, 0.01], [0.10, 0.035, 0.10], IDENTITY, [5, 12], d => l.soft(scale(sleeve, 1.12), d));
  m.tube([[[0, -0.30, 0.01], 0.075], [[0, -0.44, 0.02], 0.07]], 10, false, soft(l.skin));
  const hand = l.toy ? 0.11 : 0.088;
  m.ellipsoid([0, -0.49, 0.02], [hand, hand * 1.08, hand * 0.95], IDENTITY, [8, 12], d => l.soft(l.glove, d));
  return m;
}
function leg(l) {
  const m = new Sculpt(), soft = c => (_, a) => scale(c, 0.85 + 0.15 * Math.cos(a));
  m.tube([[[0, 0.04, 0], 0.13], [[0, -0.38, 0.01], 0.118]], 12, false, soft(l.denim));
  m.ellipsoid([0, -0.37, 0.01], [0.128, 0.035, 0.128], IDENTITY, [5, 12], d => l.soft(scale(l.denim, 1.2), d));
  m.ellipsoid([0, -0.49, 0.06], [0.14, 0.10, 0.21], IDENTITY, [8, 14], d => l.soft(l.boot, d));
  m.slab([0, -0.575, 0.06], [0.27, 0.04, 0.41], IDENTITY, scale(l.boot, 0.45), 0);
  return m;
}
function wateringCan(l) {
  const m = new Sculpt(), can = l.can;
  const metal = (_, a) => {
    const glint = Math.max(Math.cos(a - 2.2), 0) ** 12;
    return scale(can, 0.8 + 0.2 * Math.max(Math.cos(a - 2.2), 0) + 0.5 * glint);
  };
  m.lathe([0, -0.20, 0], [[0, 0], [0, 0.19], [0.015, 0.2], [0.03, 0.2], [0.27, 0.18], [0.29, 0.172], [0.30, 0.15], [0.30, 0]], 20, metal);
  for (const y of [-0.17, 0.06]) m.lathe([0, y, 0], [[0, 0.198], [0.02, 0.2], [0.04, 0.197]], 20, () => l.trim);
  m.tube([[[0, -0.12, 0.15], 0.032], [[0, -0.02, 0.30], 0.026], [[0, 0.06, 0.44], 0.022]], 8, false, (_, a) => scale(can, 0.85 + 0.15 * Math.cos(a)));
  m.ellipsoid([0, 0.075, 0.465], [0.06, 0.028, 0.06], rotX(0.75), [5, 12], d => scale(l.trim, 0.85 + 0.25 * Math.max(d[1], 0)));
  m.tube([[[0, 0.09, -0.13], 0.022], [[0, 0.24, -0.09], 0.022], [[0, 0.29, 0], 0.022], [[0, 0.24, 0.08], 0.022], [[0, 0.10, 0.10], 0.022]], 8, false, (_, a) => scale(l.trim, 0.85 + 0.15 * Math.cos(a)));
  return m;
}

// ------------------------------------------------------------ crops
/** The hilled soil every plant stands in; it grows with the plant. */
function mound(m, l, base, radius) {
  const [low, high] = l.mound;
  m.ellipsoid(vy(base), [radius, 0.07 + radius * 0.06, radius], IDENTITY, [5, 14], d => mix(low, high, d[1] * 0.8));
}
/** A fan of leaves around a point, golden-angle spaced. */
function rosette(m, root, count, reach, lift, width, shape, tones, phase) {
  for (let i = 0; i < count; i++) {
    const a = phase + i * 2.39996, k = 0.8 + 0.4 * hash(i * 7 + u32(phase * 100));
    m.leaf(root, V.add(V.add(root, V.scale(dir(a), reach * k)), vy(lift * k)), width * k, shape[0], shape[1], shape[2], tones[0], tones[1]);
  }
}
function stalk(m, from, to, r, color, segs) {
  const mid = V.add(V.lerp(from, to, 0.5), [0.02, 0, -0.015]);
  m.tube([[from, r[0]], [mid, (r[0] + r[1]) * 0.5], [to, r[1]]], segs, true, (t, a) => scale(color, (0.78 + 0.22 * t) * (0.88 + 0.12 * Math.cos(a - 2.0))));
}
function plant(c, l) {
  const m = new Sculpt(), h = c.height, base = -h * 0.5, at = y => vy(base + y);
  const tones = l.leafTones(c), stem = scale(tones[0], 0.95), toy = l.toy;
  // Chunkier, rounder leaves in the storybook look.
  const rnd = r => (toy ? Math.min(r + 0.35, 1) : r), wide = toy ? 1.35 : 1;
  if (toy) m.leafy(1.35, 2.0); else m.leafy(1.3, 2.1);
  switch (c.id) {
    case 'carrot':
      mound(m, l, base, 0.30);
      rosette(m, at(0.03), toy ? 7 : 11, 0.14, h * 0.85, 0.035 * wide, [rnd(0.15), 0.25, 0.45], tones, 0.3);
      break;
    case 'strawberry':
      mound(m, l, base, 0.42);
      for (let i = 0; i < 7; i++) {
        const a = i * 2.39996, end = V.add(at(h * (0.45 + 0.3 * hash(i))), V.scale(dir(a), 0.26));
        stalk(m, at(0.04), end, [0.012, 0.009], stem, 5);
        for (let j = 0; j < 3; j++) {
          const b = a + (j - 1) * 0.75;
          m.leaf(end, V.add(V.add(end, V.scale(dir(b), 0.15)), vy(0.03)), 0.065 * wide, rnd(0.85), 0.2, 0.3, tones[0], tones[1]);
        }
      }
      break;
    case 'blueberry':
      mound(m, l, base, 0.40);
      for (let i = 0; i < 5; i++) {
        const a = i * TAU / 5 + 0.4, top = V.add(at(h * (0.8 + 0.2 * hash(i + 3))), V.scale(dir(a), 0.22));
        stalk(m, at(0.02), top, [0.022, 0.01], l.bark, 6);
        for (let j = 0; j < 7; j++) {
          const t = 0.35 + 0.65 * j / 6, p = V.lerp(at(0.02), top, t), b = a + j * 2.1;
          m.leaf(p, V.add(V.add(p, V.scale(dir(b), 0.11)), vy(0.05)), 0.04 * wide, rnd(0.6), 0.1, 0.2, tones[0], tones[1]);
        }
      }
      break;
    case 'tomato': {
      mound(m, l, base, 0.40);
      m.slab(V.add(at(h * 0.52), [-0.12, 0, -0.1]), [0.035, h * 1.04, 0.035], IDENTITY, l.wood, 0.35);
      const top = at(h * 0.95);
      stalk(m, at(0.02), top, [0.03, 0.015], stem, 7);
      for (let i = 0; i < 9; i++) {
        const t = 0.2 + 0.8 * i / 8, p = V.lerp(at(0.02), top, t), a = i * 2.39996;
        m.leaf(p, V.add(V.add(p, V.scale(dir(a), 0.30 - 0.12 * t)), vy(0.04)), 0.08 * wide, rnd(0.5), 0.45, 0.35, tones[0], tones[1]);
      }
      break;
    }
    case 'corn': {
      mound(m, l, base, 0.32);
      const top = at(h * 0.96);
      stalk(m, at(0), top, [0.045, 0.02], stem, 8);
      for (let i = 0; i < 8; i++) {
        const t = 0.12 + 0.75 * i / 7, p = V.lerp(at(0), top, t), a = i * PI + hash(i) * 0.6;
        m.leaf(p, V.add(V.add(p, V.scale(dir(a), 0.6 - 0.25 * t)), vy(0.35)), 0.055 * wide, rnd(0.05), 0.9, 0.5, tones[0], tones[1]);
      }
      const tassel = scale(l.straw, 0.9);
      for (let i = 0; i < 6; i++) m.leaf(top, V.add(V.add(top, V.scale(dir(i * TAU / 6), 0.12)), vy(0.2)), 0.012, 0, 0.3, 0, scale(tassel, 0.8), tassel);
      break;
    }
    case 'watermelon':
    case 'pumpkin':
      mound(m, l, base, 0.5);
      for (let i = 0; i < 4; i++) {
        const a = i * TAU / 4 + 0.3, p = (r, y) => V.add(at(y), V.scale(dir(a + r * 1.2), r));
        m.tube([[p(0, 0.04), 0.02], [p(0.3, 0.05), 0.016], [p(0.6, 0.03), 0.012]], 6, true, () => stem);
      }
      rosette(m, at(0.04), 7, 0.48, 0.08, 0.17 * wide, [1, 0.35, 0.25], tones, 0.1);
      rosette(m, at(0.06), 5, 0.22, h * 0.8, 0.13 * wide, [1, 0.2, 0.3], tones, 1.3);
      break;
    case 'apple':
    case 'mango': {
      mound(m, l, base, 0.5);
      const crown = at(h * 0.62);
      stalk(m, at(0), crown, [0.11, 0.065], l.bark, 10);
      const lobes = toy ? 3 : c.slots;
      for (let i = 0; i < lobes; i++) {
        // Between the fruit slots, so hanging fruit shows.
        const a = (i + 0.5) / lobes * TAU, centre = V.add(at(h * 0.78), V.scale(dir(a), toy ? 0.38 : 0.45));
        m.tube([[crown, 0.05], [centre, 0.025]], 6, false, () => l.bark);
        const tone = mix(l.canopy[i % 3], saturate(c.leaf, l.sat), 0.5), r = toy ? 0.5 : 0.36;
        m.ellipsoid(centre, [r, r * 0.82, r], rotY(a), [8, 12], d => l.foliage(tone, d));
      }
      const tone = mix(l.canopy[1], saturate(c.leaf, l.sat), 0.5);
      m.ellipsoid(at(h + 0.05), V.scale([0.48, 0.36, 0.48], toy ? 1.15 : 1), IDENTITY, [10, 14], d => l.foliage(tone, d));
      if (!toy) for (let i = 0; i < 14; i++) { // leaves breaking the silhouette
        const a = i * 2.39996, p = V.add(at(h * (0.75 + 0.3 * hash(i + 40))), V.scale(dir(a), 0.55));
        m.leaf(p, V.add(V.add(p, V.scale(dir(a + 0.4), 0.2)), vy(0.02)), 0.06, 0.55, 0.3, 0.3, tones[0], tones[1]);
      }
      break;
    }
    case 'coconut': {
      mound(m, l, base, 0.45);
      const pts = [];
      for (let i = 0; i <= 8; i++) { const t = i / 8; pts.push([V.add(at(h * 0.92 * t), [0.25 * t * t, 0, 0]), 0.11 - 0.05 * t]); }
      m.tube(pts, 10, true, t => scale(l.bark, (0.82 + 0.18 * Math.abs(Math.sin(t * 60))) * (0.8 + 0.3 * t)));
      const top = pts[8][0];
      rosette(m, top, 9, 1.1, -0.15, 0.13 * wide, [rnd(0.1), 0.75, 0.55], tones, 0.2);
      rosette(m, top, 5, 0.7, 0.35, 0.10 * wide, [rnd(0.1), 0.6, 0.55], tones, 1.0);
      break;
    }
    case 'bamboo':
      mound(m, l, base, 0.38);
      for (let i = 0; i < 5; i++) {
        const off = V.scale(dir(i * 2.39996), 0.08 + 0.12 * hash(i));
        const top = V.add(at(h * (0.75 + 0.25 * hash(i + 9))), V.scale(off, 1.6));
        const cane = saturate(c.fruit, l.sat * 0.9);
        m.tube([[V.add(at(0), off), 0.04], [top, 0.032]], 8, true, (t, a) => {
          const band = Math.abs(Math.sin(t * 8 * PI)) < 0.12 ? 0.72 : 1;
          return scale(cane, band * (0.82 + 0.18 * Math.cos(a - 2)));
        });
        for (let j = 0; j < 3; j++) {
          const p = V.add(V.lerp(at(0), top, 0.55 + 0.2 * j), V.scale(off, 0.2));
          rosette(m, p, 3, 0.26, 0.02, 0.028 * wide, [rnd(0.15), 0.4, 0.2], tones, i * 3 + j);
        }
      }
      break;
    case 'cactus': {
      mound(m, l, base, 0.38);
      const tone = saturate(c.leaf, l.sat), ribbed = (_, a) => scale(tone, 0.8 + 0.2 * Math.abs(Math.cos(a * 8)));
      const column = (from, height, r) => {
        const pts = [[from, r], [V.add(from, vy(height - r)), r]];
        for (let k = 1; k <= 4; k++) { const t = k / 4 * PI / 2; pts.push([V.add(from, vy(height - r + Math.sin(t) * r)), Math.cos(t) * r + 0.001]); }
        m.tube(pts, 16, false, ribbed);
      };
      column(at(0), h * 0.92, 0.16);
      for (const [s, y, len] of [[-1, 0.35, 0.35], [1, 0.5, 0.28]]) {
        const from = at(h * y), elbow = V.add(from, [s * 0.3, 0.05, 0]);
        m.tube([[from, 0.09], [elbow, 0.09]], 12, false, ribbed);
        column(V.sub(elbow, vy(0.04)), len, 0.09);
      }
      for (let i = 0; i < 3; i++) m.ball(V.add(at(h * 0.92), V.scale(dir(i * 2.1), 0.07)), grey(0.04), [0.95, 0.55, 0.70], [1.0, 0.75, 0.85], 0);
      break;
    }
    case 'dragon': {
      mound(m, l, base, 0.38);
      m.slab(at(h * 0.5), [0.09, h, 0.09], IDENTITY, l.wood, 0.4);
      const tone = saturate(c.leaf, l.sat);
      for (let i = 0; i < 7; i++) {
        const a = i * TAU / 7, top = at(h * 0.95), p = (r, y) => V.add(V.add(top, V.scale(dir(a), r)), vy(y));
        m.tube([[p(0.02, 0), 0.05], [p(0.28, 0.06), 0.05], [p(0.45, -0.15), 0.045], [p(0.52, -0.55 - 0.2 * hash(i)), 0.035]], 6, true,
          (t, a) => scale(tone, (0.75 + 0.25 * Math.abs(Math.cos(a * 3))) * (1 - 0.2 * t)));
      }
      break;
    }
    case 'grape': {
      mound(m, l, base, 0.42);
      for (const x of [-0.42, 0.42]) m.slab(V.add(at(h * 0.55), [x, 0, 0]), [0.06, h * 1.1, 0.06], IDENTITY, l.wood, 0.4);
      m.slab(at(h * 1.08), [1.0, 0.05, 0.06], IDENTITY, l.wood, 0.2);
      const vine = scale(l.bark, 1.1);
      m.tube([[at(0), 0.03], [V.add(at(h * 0.5), [0.05, 0, 0.04]), 0.025], [at(h * 1.0), 0.02]], 6, false, () => vine);
      for (let i = 0; i < 14; i++) {
        const x = -0.45 + 0.9 * i / 13, p = V.add(at(h * (1.0 + 0.06 * hash(i))), [x, 0, 0]), a = i * 2.39996;
        m.leaf(p, V.sub(V.add(p, V.scale(dir(a), 0.2)), vy(0.06)), 0.1 * wide, rnd(0.85), 0.35, 0.25, tones[0], tones[1]);
      }
      rosette(m, at(h * 0.45), 5, 0.22, 0.02, 0.09 * wide, [rnd(0.85), 0.3, 0.2], tones, 0.7);
      break;
    }
    default:
      mound(m, l, base, 0.35);
      rosette(m, at(0.02), 8, 0.3, h * 0.7, 0.07 * wide, [0.5, 0.3, 0.3], tones, 0);
  }
  return m;
}

/** Fruit is tinted by its entity's material (ripeness, mutations), so the mesh
 * carries only shading, in greys. The skin is material 0, as glossy as its
 * crop (glossier in the toy look); stems and leaves matte, seeds gilt. */
function fruit(c, l) {
  const skin = new Sculpt(), m = new Sculpt(), gilt = new Sculpt(), r = c.fruit_size;
  const shine = (d, base) => grey(base * (0.7 + 0.3 * (d[1] * 0.5 + 0.5)));
  const rough = { tomato: 0.2, apple: 0.25, grape: 0.25, strawberry: 0.3, watermelon: 0.3, mango: 0.3, dragon: 0.3, blueberry: 0.45, corn: 0.45, pumpkin: 0.45, bamboo: 0.5, cactus: 0.5, carrot: 0.6, coconut: 0.85 }[c.id] ?? 0.35;
  const roughness = rough * (l.toy ? 0.75 : 1), dark = grey(0.42);
  const lathed = (y, a) => shine(V.norm([Math.cos(a), y / r, Math.sin(a)]), 1);
  switch (c.id) {
    case 'carrot': {
      const profile = [];
      for (let i = 0; i <= 10; i++) { const t = i / 10; profile.push([-r * 1.25 + t * r * 2.4, r * 0.68 * t ** 0.6]); }
      profile.push([r * 1.17, r * 0.4], [r * 1.18, 0]);
      skin.lathe([0, 0, 0], profile, 12, (y, a) => grey((0.86 + 0.14 * Math.abs(Math.sin(y * 95))) * (0.85 + 0.15 * Math.cos(a - 2.2))));
      m.tube([[vy(r * 1.1), 0.03], [vy(r * 1.5), 0.02]], 6, true, () => dark);
      break;
    }
    case 'strawberry':
      skin.lathe([0, 0, 0], [[-r * 1.1, 0], [-r * 0.9, r * 0.35], [-r * 0.4, r * 0.72], [r * 0.1, r * 0.86], [r * 0.45, r * 0.78], [r * 0.6, r * 0.5], [r * 0.62, 0]], 14, lathed);
      for (let i = 0; i < 18; i++) {
        const a = i * 2.39996, y = -r * 0.8 + r * 1.2 * hash(i + 5);
        const rad = r * (0.86 - 0.5 * Math.min(Math.abs((y / r - 0.1) * 0.9), 1)) * 0.98;
        gilt.ball(V.add(vy(y), V.scale(dir(a), rad)), grey(r * 0.06), [0.85, 0.68, 0.3], [1.0, 0.85, 0.45], 0);
      }
      rosette(m, vy(r * 0.58), 6, r * 0.6, -r * 0.1, r * 0.2, [0.4, 0.2, 0.1], [dark, grey(0.5)], 0);
      break;
    case 'blueberry':
      skin.ellipsoid([0, 0, 0], [r, r * 0.9, r], IDENTITY, [10, 14], d => shine(d, 0.95));
      rosette(m, vy(r * 0.85), 5, r * 0.3, r * 0.15, r * 0.12, [0.4, 0, 0], [dark, grey(0.6)], 0);
      break;
    case 'tomato':
      skin.ellipsoid([0, 0, 0], [r, r * 0.82, r], IDENTITY, [12, 18], d => shine(d, 1 - 0.1 * Math.abs(Math.cos(Math.atan2(d[2], d[0]) * 5)) * (1 - Math.abs(d[1]))));
      rosette(m, vy(r * 0.8), 5, r * 0.55, -r * 0.05, r * 0.14, [0.2, 0.3, 0.1], [dark, grey(0.45)], 0);
      m.tube([[vy(r * 0.78), 0.022], [vy(r * 1.05), 0.016]], 6, true, () => dark);
      break;
    case 'corn':
      skin.ellipsoid([0, 0, 0], [r * 0.55, r * 1.3, r * 0.55], IDENTITY, [16, 16], d => {
        const row = Math.abs(Math.sin(Math.atan2(d[2], d[0]) * 8) * Math.sin(d[1] * 14));
        return shine(d, 0.86 + 0.18 * row);
      });
      for (let i = 0; i < 3; i++) m.leaf(vy(-r * 1.25), V.add(vy(r * 0.4), V.scale(dir(i * TAU / 3), r * 0.62)), r * 0.42, 0.4, -0.05, 0.6, grey(0.55), grey(0.7));
      break;
    case 'watermelon':
      skin.ellipsoid([0, 0, 0], [r * 1.2, r * 0.85, r * 0.85], IDENTITY, [14, 26], d => {
        const stripe = Math.sin(Math.atan2(d[2], d[1]) * 9 + Math.sin(d[0] * 7) * 0.35);
        return shine(d, stripe > 0.25 ? 0.5 : 1.0);
      });
      break;
    case 'pumpkin':
      for (let i = 0; i < 9; i++) { const a = i * TAU / 9; skin.ellipsoid(V.scale(dir(a), r * 0.42), [r * 0.6, r * 0.78, r * 0.48], rotY(-a), [10, 12], d => shine(d, 0.95)); }
      m.tube([[vy(r * 0.6), r * 0.12], [[r * 0.05, r * 0.92, 0], r * 0.08], [[r * 0.18, r * 1.05, 0], r * 0.06]], 8, true, (_, a) => grey(0.38 + 0.08 * Math.cos(a * 4)));
      break;
    case 'apple':
    case 'mango':
      if (c.id === 'apple') skin.lathe([0, 0, 0], [[-r * 0.85, 0], [-r * 0.9, r * 0.2], [-r * 0.8, r * 0.55], [-r * 0.4, r * 0.92], [r * 0.15, r * 1.0], [r * 0.6, r * 0.88], [r * 0.82, r * 0.5], [r * 0.78, r * 0.15], [r * 0.62, 0]], 16, lathed);
      else skin.ellipsoid([0, 0, 0], [r * 0.8, r * 1.05, r * 0.68], rotZ(0.35), [12, 16], d => shine(d, 0.85 + 0.15 * d[1]));
      m.tube([[vy(r * 0.6), 0.016], [[0.02, r * 1.1, 0], 0.012]], 5, true, () => dark);
      m.leaf([0.02, r * 1.0, 0], [r * 0.7, r * 1.2, r * 0.1], r * 0.22, 0.5, 0.2, 0.3, grey(0.45), grey(0.6));
      break;
    case 'bamboo': {
      const profile = [];
      for (let i = 0; i <= 8; i++) { const t = i / 8; profile.push([-r + t * r * 2.6, r * 0.62 * (1 - t * 0.9)]); }
      profile.push([r * 1.62, 0]);
      skin.lathe([0, 0, 0], profile, 10, (y, a) => grey((0.78 + 0.22 * Math.abs(Math.sin((y / r) * 9))) * (0.85 + 0.15 * Math.cos(a - 2.2))));
      break;
    }
    case 'coconut':
      skin.ellipsoid([0, 0, 0], [r, r * 1.08, r], IDENTITY, [12, 16], d => {
        const q = d.map(v => round(v * 7));
        return shine(d, (0.82 + 0.3 * hash(u32(q[0] * 31 + q[1] * 17 + q[2] * 7 + 400))) * 0.9);
      });
      for (let i = 0; i < 3; i++) m.ball(V.add(vy(-r * 0.95), V.scale(dir(i * TAU / 3), r * 0.22)), grey(r * 0.09), grey(0.3), grey(0.35), 0);
      break;
    case 'cactus':
      skin.ellipsoid([0, 0, 0], [r * 0.72, r, r * 0.72], IDENTITY, [10, 14], d => shine(d, 0.95));
      for (let i = 0; i < 12; i++) {
        const a = i * 2.39996, y = -0.7 + 1.4 * hash(i + 11), ring = Math.sqrt(1 - y * y);
        skin.ball([Math.cos(a) * ring * r * 0.72, y * r, Math.sin(a) * ring * r * 0.72], grey(r * 0.06), grey(0.9), grey(1.0), 0);
      }
      break;
    case 'dragon':
      skin.ellipsoid([0, 0, 0], [r * 0.78, r, r * 0.78], IDENTITY, [10, 14], d => shine(d, 1.0));
      for (let i = 0; i < 10; i++) {
        const a = i * 2.39996, y = -0.6 + 1.2 * (i / 9), ring = Math.sqrt(1 - y * y);
        const root = [Math.cos(a) * ring * r * 0.7, y * r, Math.sin(a) * ring * r * 0.7];
        m.leaf(root, V.add(V.scale(root, 1.45), vy(r * 0.35)), r * 0.2, 0.3, -0.2, 0.4, grey(0.9), grey(0.6));
      }
      break;
    case 'grape':
      for (let i = 0; i < 14; i++) {
        const t = i / 13, ring = (1 - t) * 0.75 + 0.1, a = i * 2.39996;
        skin.ellipsoid(V.add(vy(r * 0.9 - t * r * 2.0), V.scale(dir(a), ring * r)), [r * 0.36, r * 0.4, r * 0.36], IDENTITY, [6, 10], d => shine(d, 0.95));
      }
      m.tube([[vy(r * 0.9), 0.018], [[0.02, r * 1.4, 0], 0.012]], 5, true, () => dark);
      break;
    default:
      skin.ellipsoid([0, 0, 0], [r, r, r], IDENTITY, [10, 14], d => shine(d, 1));
  }
  return [[skin, { metal: 0, rough: roughness }], [m, { metal: 0, rough: 0.8 }], [gilt, { metal: 1, rough: 0.35 }]];
}

// ------------------------------------------------------------ scenery
function tree(m, l, p, size, seed) {
  const bark = l.bark, lean = V.scale([hash(seed) - 0.5, 0, hash(seed + 1) - 0.5], 0.4);
  const trunkTop = V.add(p, V.scale(V.add(vy(2.5), lean), size));
  m.tube([[V.sub(p, vy(0.1)), 0.3 * size], [V.add(p, vy(0.4 * size)), 0.22 * size], [V.add(p, V.scale(V.add(vy(1.5), V.scale(lean, 0.5)), size)), 0.17 * size], [trunkTop, 0.11 * size]], 12, true,
    (t, a) => scale(bark, (0.86 + 0.14 * Math.sin(a * 7 + t * 3)) * (0.65 + 0.35 * t)));
  const crown = V.add(trunkTop, vy(0.9 * size));
  const clumps = l.toy
    ? [[[0, 0.35, 0], 1.45], [[-0.95, -0.25, 0.25], 1.0], [[0.9, -0.15, -0.2], 1.05], [[0.15, -0.35, 0.9], 0.9]]
    : Array.from({ length: 9 }, (_, i) => {
      const a = i * 2.39996 + hash(seed + i) * 0.5, up = -0.5 + 1.3 * hash(seed + i + 20), out = 0.6 + 0.6 * hash(seed + i + 40);
      return [V.add(V.scale(dir(a), out), vy(up)), 0.8 + 0.4 * hash(seed + i + 60)];
    });
  clumps.forEach(([off, r], i) => {
    const centre = V.add(crown, V.scale(off, size)), tone = mix(l.canopy[(i + seed) % 3], l.canopy[1], 0.4);
    if (!l.toy) m.tube([[V.sub(trunkTop, vy(0.3 * size)), 0.08 * size], [centre, 0.04 * size]], 6, false, () => bark);
    m.ellipsoid(centre, V.scale([1, 0.84, 1], r * size), rotY(i), l.toy ? [12, 18] : [9, 14], d => l.foliage(tone, d));
  });
  for (let i = 0; i < 7; i++) {
    const a = i * 2.39996 + seed, off = clumps[i % clumps.length];
    const d = V.norm(V.add(V.scale(dir(a), 0.8), vy(hash(i + seed) - 0.6)));
    m.ball(V.add(crown, V.scale(V.add(off[0], V.scale(d, off[1] * 0.97)), size)), grey(0.17 * size), scale(l.orchard_fruit, 0.7), l.orchard_fruit, 0.6);
  }
}
function mushroom(m, at, s) {
  m.lathe(at, [[0, 0], [0, 0.06 * s], [0.18 * s, 0.05 * s], [0.2 * s, 0]], 10, y => grey(0.92 + y));
  m.ellipsoid(V.add(at, vy(0.2 * s)), V.scale([0.16, 0.1, 0.16], s), IDENTITY, [6, 14], d => scale([0.92, 0.22, 0.2], 0.8 + 0.25 * d[1]));
  for (let i = 0; i < 5; i++) {
    const d = V.norm(V.add(V.scale(dir(i * 2.39996), 0.7), vy(0.75)));
    m.ball(V.add(V.add(at, vy(0.2 * s)), V.mul(d, V.scale([0.16, 0.1, 0.16], s))), V.scale([0.03, 0.015, 0.03], s), grey(0.95), grey(1.0), 0);
  }
}
function orchard(l) {
  const m = new Sculpt();
  [[-5, -5, 1.4], [2, -4, 1.1], [8, -6, 1.7], [15, -3, 1.3], [23, -7, 1.8]].forEach(([x, z, size], i) => {
    const p = [x, 0, z];
    tree(m, l, p, size, i * 97);
    if (l.toy) for (let j = 0; j < 3; j++) mushroom(m, V.add(p, V.scale(dir((i * 5 + j) * 2.1), 0.7 + 0.4 * hash(j + i))), 1.2 + 0.6 * hash(j + 9));
  });
  const fence = l.fence;
  if (l.toy) { // white pickets with rounded rails
    for (let i = 0; i < 49; i++) {
      const x = -5.0 + i * 0.55;
      m.slab([x, 0.5, 0], [0.13, 1.0, 0.05], IDENTITY, fence, 0.25);
      m.slab([x, 1.0, 0], [0.092, 0.092, 0.05], rotZ(PI / 4), fence, 0);
    }
    for (const y of [0.32, 0.74]) m.tube([[[-5.2, y, -0.05], 0.045], [[21.6, y, -0.05], 0.045]], 8, true, (_, a) => scale(fence, 0.85 + 0.15 * Math.cos(a - 1.5)));
  } else { // weathered split rails
    for (let x = -5.0, i = 0; x < 21.5; x += 2.2, i++) {
      const tilt = rot(rotZ((hash(i) - 0.5) * 0.08), rotX((hash(i + 50) - 0.5) * 0.06));
      m.slab([x, 0.62, 0], [0.15, 1.25 + 0.1 * hash(i + 3), 0.15], tilt, scale(fence, 0.9 + 0.15 * hash(i + 7)), 0.45);
      if (x + 2.2 < 21.6) [0.45, 0.92].forEach((y, k) => {
        m.slab([x + 1.1, y, 0.03], [2.3, 0.095, 0.075], rotZ((hash(i * 3 + k) - 0.5) * 0.05), scale(fence, 0.85 + 0.2 * hash(i * 5 + k)), 0.3);
      });
    }
  }
  return m;
}
function flower(m, l, root, kind, k) {
  const s = (l.toy ? 2.3 : 1.8) * k, h = (0.22 + 0.12 * hash(kind * 13 + u32(root[0] * 10))) * s;
  const stem = scale(l.grass[0], 1.2), head = V.add(V.add(root, vy(h)), V.scale([0.02, 0, 0.01], s));
  m.tube([[root, 0.012 * s], [head, 0.009 * s]], 5, false, () => stem);
  for (let i = 0; i < 2; i++) {
    const a = i * PI + hash(kind + i) * 1.5, p = V.add(root, vy(0.03 * s));
    m.leaf(p, V.add(V.add(p, V.scale(dir(a), 0.09 * s)), vy(0.06 * s)), 0.022 * s, 0.3, 0.2, 0.3, l.grass[0], l.grass[1]);
  }
  const blooms = l.blooms, centre = [0.98, 0.74, 0.22];
  switch (kind % 4) {
    case 0: { // daisy-like: a ring of petals around a gold centre
      const petal = blooms[(Math.floor(kind / 4) % 2) * 3];
      for (let i = 0; i < 9; i++) m.leaf(head, V.add(V.add(head, V.scale(dir(i * TAU / 9), 0.075 * s)), vy(0.01 * s)), 0.018 * s, 0.6, 0.15, 0.1, scale(petal, 0.9), petal);
      m.ball(V.add(head, vy(0.01 * s)), V.scale([0.026, 0.018, 0.026], s), scale(centre, 0.8), centre, 0);
      break;
    }
    case 1: { // cup: tulip or poppy
      const petal = blooms[l.toy ? 4 : 1];
      for (let i = 0; i < 5; i++) m.leaf(head, V.add(V.add(head, V.scale(dir(i * TAU / 5), 0.045 * s)), vy(0.075 * s)), 0.035 * s, 0.9, -0.1, 0.5, scale(petal, 0.75), petal);
      m.ball(V.add(head, vy(0.02 * s)), grey(0.018 * s), grey(0.2), grey(0.25), 0);
      break;
    }
    case 2: // a spike of small buds (lavender)
      for (let i = 0; i < 6; i++) { const t = i / 5; m.ball(V.add(head, vy(t * 0.11 * s)), grey((0.022 - 0.01 * t) * s), scale(blooms[2], 0.75), blooms[2], 0); }
      break;
    default:
      if (l.toy) m.ball(V.add(head, vy(0.03 * s)), grey(0.05 * s), scale(blooms[1], 0.75), blooms[1], 0.4); // a pom-pom
      else for (let i = 0; i < 5; i++) m.leaf(head, V.add(V.add(head, V.scale(dir(i * TAU / 5), 0.05 * s)), vy(0.02 * s)), 0.028 * s, 0.9, 0, 0.4, scale(blooms[3], 0.85), blooms[3]);
  }
}
function flowerBank(l) {
  const m = new Sculpt();
  for (let i = 0; i < 46; i++) flower(m, l, [hash(i) * 8.2, 0, hash(i + 100) * 1.4], i, 0.85 + 0.35 * hash(i + 200));
  return m;
}
function rock(m, l, at, size, seed) {
  const stone = l.stone, moss = l.grass[0];
  m.ellipsoid(at, size, rot(rotY(hash(seed) * TAU), rotZ((hash(seed + 1) - 0.5) * 0.4)), [7, 11], d => {
    const q = d.map(v => round(v * 3));
    const grain = 0.88 + 0.14 * hash(u32(q[0] * 13 + q[1] * 7 + q[2] * 3 + 50) + seed);
    const c = scale(stone, grain * (0.65 + 0.35 * (d[1] * 0.5 + 0.5)));
    return l.toy ? c : mix(c, scale(moss, 1.1), Math.min(1, Math.max(0, (d[1] - 0.55) * 2.5)) * 0.8);
  });
}
/** The meadow south and west of the garden: those edges never move as it
 * grows. Its thousands of grass tufts are `TUFTS` tufts, each placed many
 * times at a turn: a few hundred kilobytes, not tens of megabytes, merged
 * into one mesh when the model loads. Rocks and flowers are its own mesh. */
const TUFTS = 24;
function meadow(l) {
  const m = new Sculpt(), [low, high] = l.grass, count = l.toy ? 1400 : 2600;
  const spot = i => {
    const u = hash(i * 2 + 1), v = hash(i * 2 + 2), near = t => t * t; // denser toward the garden's edges
    return i % 3 === 0 ? [-3.5 - near(u) * 18.5, 0, -38 + v * 41.4] : [-22 + u * 58, 0, 3.5 + near(v) * 13];
  };
  const tufts = Array.from({ length: TUFTS }, (_, i) => {
    const t = new Sculpt(), p = vy(-0.1), tip = l.toy ? high : mix(high, [0.86, 0.78, 0.48], hash(i + 7000) * 0.6);
    for (let b = 0; b < (l.toy ? 3 : 5); b++) {
      const a = (i * 5 + b) * 2.39996, lean = V.scale(dir(a), 0.05 + 0.08 * hash(i * 9 + b));
      const height = (0.16 + 0.22 * hash(i * 11 + b + 3)) * (l.toy ? 1.3 : 1), root = V.add(p, V.scale(dir(a + 1.3), 0.04));
      if (l.toy) t.leaf(root, V.add(V.add(root, V.scale(lean, 1.5)), vy(height)), 0.045, 0.35, 0.25, 0.3, low, tip);
      else t.blade(root, V.add(V.add(root, lean), vy(height)), 0.016, low, tip);
    }
    return { part: t, at: [] };
  });
  for (let i = 0; i < count; i++) tufts[Math.floor(hash(i + 9000) * TUFTS)].at.push([spot(i), hash(i + 8000) * TAU]);
  m.tufts = tufts;
  for (let i = 0; i < 10; i++) {
    const s = 0.25 + 0.45 * hash(i + 300);
    rock(m, l, V.sub(spot(i * 37 + 5), vy(0.12)), [s, s * 0.6, s * 0.8], i);
  }
  if (l.toy) for (let i = 0; i < 9; i++) { // round bushes
    const p = spot(i * 53 + 11), s = 0.5 + 0.4 * hash(i + 500);
    for (let j = 0; j < 3; j++) {
      const off = V.add(V.scale(dir(j * 2.2 + i), s * 0.45), vy(s * (0.45 + 0.1 * j))), tone = l.canopy[(i + j) % 3];
      m.ellipsoid(V.add(p, off), grey(s * (0.6 - 0.08 * j)), IDENTITY, [8, 12], d => l.foliage(tone, d));
    }
  }
  else for (let i = 0; i < 30; i++) flower(m, l, spot(i * 29 + 3), i * 4 + 1 + (i % 2) * 2, 0.9);
  return m;
}
/** Hills and a treeline north of the orchard, placed relative to it. */
function backdrop(l) {
  const m = new Sculpt();
  for (let i = 0; i < 16; i++) {
    const x = -70 + 150 * hash(i + 900), z = -22 - 26 * hash(i + 950), rx = 12 + 14 * hash(i + 990), ry = 4 + 7 * hash(i + 1030);
    const tone = mix(l.hills[0], l.hills[1], hash(i + 1070));
    m.ellipsoid([x, -ry * 0.25, z], [rx, ry, rx * 0.6], IDENTITY, [8, 16], d => l.foliage(tone, d));
  }
  for (let i = 0; i < 44; i++) {
    const p = [-40 + 100 * hash(i + 1200), 0, -10 - 9 * hash(i + 1300)], tone = l.canopy[i % 3], s = 0.8 + 0.6 * hash(i + 1400);
    if (l.toy) {
      m.tube([[p, 0.2 * s], [V.add(p, vy(1.6 * s)), 0.14 * s]], 6, false, () => l.bark);
      m.ellipsoid(V.add(p, vy(2.6 * s)), grey(1.4 * s), IDENTITY, [8, 12], d => l.foliage(tone, d));
    } else m.ellipsoid(V.add(p, vy(3.2 * s)), V.scale([0.95, 3.4, 0.95], s), IDENTITY, [8, 10], d => l.foliage(scale(tone, 0.85), d));
  }
  return m;
}
/** A unit-length border rail along X, stretched to each edge's length. */
function rail(l) {
  const m = new Sculpt(), wood = l.wood;
  if (l.toy) m.tube([[[-0.5, 0.13, 0], 0.14], [[0.5, 0.13, 0], 0.14]], 12, false, (_, a) => scale(wood, 0.8 + 0.2 * Math.cos(a - 1.4)));
  else for (const [y, h, tone] of [[0.07, 0.13, 0.92], [0.2, 0.12, 1.05]]) m.slab([0, y, 0], [1, h, 0.22], IDENTITY, scale(wood, tone), 0.35);
  return m;
}
function barrel(l) {
  const m = new Sculpt(), wood = l.toy ? l.accent : l.wood, metal = l.metal, profile = [[0, 0]];
  for (let i = 0; i <= 40; i++) { const y = i / 40, bulge = 1 - (2 * y - 1) * (2 * y - 1); profile.push([y, 0.58 + 0.08 * bulge]); }
  profile.push([1, 0.53], [0.9, 0.53], [0.9, 0]);
  m.lathe(vy(-0.5), profile, 36, (y, a) => {
    if ([0.1, 0.3, 0.7, 0.9].some(h => Math.abs(y - h) < 0.035)) return scale(metal, 0.9 + 0.2 * Math.cos(a - 2.2));
    const stave = Math.trunc(a / (TAU / 18)) % 2;
    return scale(wood, (stave === 0 ? 0.92 : 1.04) * (0.8 + 0.2 * Math.cos(a - 2.2)) * (0.85 + 0.15 * y));
  });
  return m;
}

// ------------------------------------------------------------ write
/** One look model per part: (builder, material) pairs, empty parts left out,
 * matte (`metal` 0, `rough` 1) unless given. */
const part = (b, spec) => ({ spec: { metal: 0, rough: 1, ...spec }, p: b.p, n: b.n, c: b.c, i: b.i });
const parts = list => list.filter(([b]) => !b.isEmpty()).map(([b, spec], i) => [`part-${i}`, part(b, spec)]);

/** Every golden and storybook model, written to art/ through `out(name,
 * parts, instances)`. */
export function writeLooks(out) {
  const looks = data('looks.level.json'), crops = data('garden.level.json').crops;
  for (const style of ['golden', 'storybook']) {
    const l = look({ ...looks[style].palette, sun: looks[style].sun.at }), name = base => `${style}-${base}`;
    for (const [base, make] of Object.entries({ gardener, 'gardener-arm': arm, 'gardener-leg': leg, 'watering-can': wateringCan, orchard, flowers: flowerBank, 'meadow-grass': meadow, backdrop, rail, 'rain-barrel': barrel })) {
      const m = make(l);
      out(name(base), parts([[m, {}]]), (m.tufts ?? []).map(t => ({ part: part(t.part, {}), at: t.at })));
    }
    for (const c of crops) {
      out(name(`plant-${c.id}`), parts([[plant(c, l), {}]]));
      out(name(`fruit-${c.id}`), parts(fruit(c, l)));
    }
  }
}
export { writeGlb };
