#!/usr/bin/env bun
// Writes every model, texture and sky the forest draws into ../art/, from code:
// bun game/games/forest/artgen/gen.mjs. Deterministic: the same bytes every run.
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  rng, fbm, noise2, image, png, glb, Mesh, tube, blob, cards, mix, clamp, smooth,
  add, scale, norm, rotX, rotY, rotZ, chain, move, stretch,
} from './lib.mjs';

const out = resolve(import.meta.dir, '../art');
mkdirSync(out, {recursive: true});
const written = [];
const save = (name, bytes) => { writeFileSync(resolve(out, name), bytes); written.push([name, bytes.length]); };
const hex = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16) / 255);
// sRGB-authored colours become linear for vertex-colour multipliers.
const lin = c => c.map(x => x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4);

// ---------------------------------------------------------------- textures
const bark = (seed, dark, light, w = 128, h = 256) => {
  const streak = fbm(seed, 4, 5), fine = noise2(seed + 7, 64), knot = fbm(seed + 3, 2, 3);
  return png(image(w, h, (u, v) => {
    const ridge = 1 - Math.abs(streak(u * 3 % 1, v * 0.25 % 1) * 2 - 1);
    const t = clamp(ridge ** 1.6 * 1.1 + (fine(u * 64, v * 16) - 0.5) * 0.25 - smooth(0.62, 0.75, knot(u, v)) * 0.5);
    const c = mix(hex(dark), hex(light), t);
    return [...c, 1];
  }));
};
const birch = () => {
  const n = fbm(41, 4, 4), lent = noise2(42, 32);
  return png(image(128, 256, (u, v) => {
    const mark = smooth(0.72, 0.8, lent(u * 6, v * 32)) * smooth(0.4, 0.6, n(u, v));
    const base = mix(hex('#e9e4d8'), hex('#b8b2a4'), n(u, v) * 0.6);
    return [...mix(base, hex('#24211d'), mark), 1];
  }));
};
// Needles: dense dark streaks with light tips; opaque (the cones are solid).
const needles = (seed, a, b) => {
  const s = noise2(seed, 64), m = fbm(seed + 1, 4, 4);
  return png(image(128, 128, (u, v) => {
    const strand = s(u * 64, v * 16 + u * 6), fine = s(u * 128 % 64, v * 64 % 64);
    const t = clamp(0.25 + strand * 0.35 + fine * 0.2 + m(u, v) * 0.35);
    return [...mix(hex(a), hex(b), t), 1];
  }));
};
// Leaves on a transparent card: scattered ovate leaves with a midrib.
const leafCard = (seed, a, b) => {
  const r = rng(seed), leaves = Array.from({length: 70}, () => ({x: r(), y: r(), s: r.range(0.05, 0.1), t: r() * Math.PI, k: r()}));
  return png(image(128, 128, (u, v) => {
    let hit = null;
    for (const l of leaves) for (const ox of [-1, 0, 1]) for (const oy of [-1, 0, 1]) {
      const dx = u - l.x + ox, dy = v - l.y + oy;
      const x = dx * Math.cos(l.t) + dy * Math.sin(l.t), y = -dx * Math.sin(l.t) + dy * Math.cos(l.t);
      if ((x / l.s) ** 2 + (y / (l.s * 0.5)) ** 2 < 1) hit = {l, rib: Math.abs(y) < l.s * 0.05};
    }
    if (!hit) return [0, 0, 0, 0];
    const c = mix(hex(a), hex(b), hit.l.k);
    return [...(hit.rib ? mix(c, [0.9, 0.95, 0.5], 0.3) : c), 1];
  }));
};
const fern = () => png(image(128, 256, (u, v) => {
  // One frond: a stem up the middle and paired leaflets shrinking toward the tip.
  const x = u - 0.5, y = 1 - v, width = 0.45 * Math.sin(Math.PI * Math.min(1, y * 1.05)) * (1 - y * 0.6);
  if (Math.abs(x) < 0.012 + 0.01 * (1 - y)) return [...hex('#4c6b2a'), 1];
  const pitch = 0.045, k = Math.floor(y / pitch), local = y - k * pitch - pitch / 2;
  const reach = Math.abs(x) / Math.max(width, 0.001);
  const leaf = reach < 1 && Math.abs(local - reach * 0.02) < pitch * 0.42 * (1 - reach * 0.7);
  if (!leaf) return [0, 0, 0, 0];
  return [...mix(hex('#2f5a1d'), hex('#6b8f34'), clamp(0.3 + reach * 0.6 + (k % 3) * 0.08)), 1];
}));
const grass = () => {
  const r = rng(77), blades = Array.from({length: 26}, () => ({x: r.range(0.08, 0.92), h: r.range(0.5, 0.98), w: r.range(0.015, 0.03), lean: r.range(-0.18, 0.18), k: r()}));
  return png(image(128, 128, (u, v) => {
    const y = 1 - v;
    for (const b of blades) {
      if (y > b.h) continue;
      const cx = b.x + b.lean * (y / b.h) ** 2, half = b.w * (1 - y / b.h);
      if (Math.abs(u - cx) < half) return [...mix(hex('#3d5a22'), hex('#a3b356'), clamp(y / b.h * 0.8 + b.k * 0.3)), 1];
    }
    return [0, 0, 0, 0];
  }));
};
const stone = (seed, a, b) => {
  const n = fbm(seed, 4, 6), c = noise2(seed + 9, 16);
  return png(image(128, 128, (u, v) => {
    const crack = smooth(0.47, 0.5, c(u * 16, v * 16)) * (1 - smooth(0.5, 0.53, c(u * 16, v * 16)));
    return [...mix(mix(hex(a), hex(b), n(u, v)), [0.12, 0.12, 0.12], crack * 0.7), 1];
  }));
};
const fur = (seed, a, b, stretchV = 8) => {
  const n = noise2(seed, 64), m = fbm(seed + 2, 4, 4);
  return png(image(128, 128, (u, v) => [...mix(hex(a), hex(b), clamp(n(u * 64, v * stretchV) * 0.7 + m(u, v) * 0.6 - 0.2)), 1]));
};
const cloth = (seed, a, b) => {
  const n = fbm(seed, 4, 4);
  return png(image(128, 128, (u, v, x, y) => {
    const weave = ((x >> 1) + (y >> 1)) % 2 ? 0.06 : -0.06;
    return [...mix(hex(a), hex(b), clamp(n(u, v) * 0.8 + weave + 0.1)), 1];
  }));
};
const endGrain = () => png(image(128, 128, (u, v) => {
  const r = Math.hypot(u - 0.5, v - 0.5) * 2, ring = 0.5 + 0.5 * Math.sin(r * 38 + noise2(5, 8)(u * 8, v * 8) * 3);
  if (r > 0.92) return [...hex('#3a2616'), 1];
  return [...mix(hex('#a07a4d'), hex('#c9a46c'), ring), 1];
}));

// RGBM equirect skies (+Y the top row, −Z the centre column): radiance = rgb × a × 8.
const RANGE = 8;
const srgb = x => x <= 0.0031308 ? x * 12.92 : 1.055 * x ** (1 / 2.4) - 0.055;
const sky = (f) => png(image(256, 128, (u, v) => {
  const phi = (u - 0.5) * Math.PI * 2, theta = (0.5 - v) * Math.PI;
  const dir = [Math.sin(phi) * Math.cos(theta), Math.sin(theta), -Math.cos(phi) * Math.cos(theta)];
  const L = f(dir, u, v);
  const m = clamp(Math.max(...L, 1e-4) / RANGE, 1 / 255, 1);
  const a = Math.ceil(m * 255) / 255;
  return [...L.map(x => srgb(clamp(x / (a * RANGE)))), a];
}));
const daySky = () => sky(([x, y, z]) => {
  const up = Math.max(y, 0), sunDir = norm([0.6, 0.55, 0.3]), cosS = x * sunDir[0] + y * sunDir[1] + z * sunDir[2];
  const base = y >= 0 ? mix([0.55, 0.65, 0.72], [0.18, 0.36, 0.75], up ** 0.6) : mix([0.25, 0.27, 0.2], [0.06, 0.08, 0.04], Math.min(1, -y * 3));
  return add(base, scale([1.0, 0.85, 0.6], 2.5 * Math.max(0, cosS) ** 24 + 0.4 * Math.max(0, cosS) ** 4));
});
const nightSky = () => {
  const r = rng(99), stars = noise2(98, 256), twinkle = noise2(97, 64), band = fbm(96, 2, 4);
  return sky(([x, y, z], u, v) => {
    const up = Math.max(y, 0), moonDir = norm([-0.35, 0.85, -0.4]), cosM = x * moonDir[0] + y * moonDir[1] + z * moonDir[2];
    let L = y >= 0 ? mix([0.012, 0.016, 0.03], [0.002, 0.004, 0.012], up ** 0.5) : [0.003, 0.004, 0.003];
    // A faint Milky Way band, then stars thresholded from high-frequency noise.
    const galaxy = Math.max(0, 1 - Math.abs(y - x * 0.6) * 3) * band(u, v);
    L = add(L, scale([0.02, 0.022, 0.03], galaxy * up));
    const s = stars(u * 256, v * 256);
    if (y > 0.05 && s > 0.985) L = add(L, scale([0.9, 0.9, 1.0], (s - 0.985) * 400 * (0.6 + 0.4 * twinkle(u * 64, v * 64))));
    L = add(L, scale([0.6, 0.7, 1.0], 6 * Math.max(0, cosM) ** 800 + 0.15 * Math.max(0, cosM) ** 12));
    return L;
  });
};

// ---------------------------------------------------------------- models
const ao = (y, h, lo = 0.45) => { const t = clamp(y / h); return [lo + (1 - lo) * t, lo + (1 - lo) * t, lo + (1 - lo) * t, 1]; };
const tint = (c, k) => [...lin(c).map(x => x * k), 1];

// A pine: a tapering, slightly crooked trunk and tiers of jagged cones.
function pine(seed, {tiers, height, spread, droop = 0.25, dead = false}) {
  const r = rng(seed), trunk = tube(
    Array.from({length: 6}, (_, k) => ({c: [Math.sin(k * 1.3 + seed) * 0.04 * k, (k / 5) * height * 0.92, 0], r: 0.34 * (1 - k / 6) + 0.05})),
    {sides: 9, around: 2, vScale: 0.35, color: t => ao(t, 0.25, 0.55)});
  const crown = new Mesh();
  for (let k = 0; k < tiers; k++) {
    const t = k / tiers, base = height * (0.18 + t * 0.7), top = base + height * (0.42 - t * 0.12);
    const radius = spread * (1 - t * 0.78) * r.range(0.9, 1.1), sides = 11;
    const cone = new Mesh(), rimJag = Array.from({length: sides}, () => r.range(0.75, 1.15));
    const tip = cone.vertex([0, top, 0], [0, 1, 0], [2.5, 0], [0.8, 0.85, 0.75, 1]);
    for (let s = 0; s <= sides; s++) {
      const a = (s / sides) * Math.PI * 2, j = rimJag[s % sides], rr = radius * j;
      const p = [Math.cos(a) * rr, base - droop * rr * (0.6 + 0.4 * j), Math.sin(a) * rr];
      const n = norm([Math.cos(a), 0.55, Math.sin(a)]);
      const shade = dead ? 0.55 : 0.5 + 0.5 * t;
      cone.vertex(p, n, [s / sides * 5, 2.2], [shade, shade, shade, 1]);
    }
    for (let s = 0; s < sides; s++) cone.tri(tip, tip + 2 + s, tip + 1 + s);
    // Underside, darker: closes the cone so it reads solid from below.
    const under = cone.vertex([0, base + radius * 0.15, 0], [0, -1, 0], [0.5, 0.5], [0.25, 0.28, 0.25, 1]);
    for (let s = 0; s < sides; s++) cone.tri(under, tip + 1 + s, tip + 2 + s);
    crown.append(cone, chain(rotY(r() * Math.PI)));
  }
  return {trunk, crown};
}
function broadleaf(seed, {height, canopy, birchBark = false}) {
  const r = rng(seed);
  const trunk = tube(Array.from({length: 5}, (_, k) => ({c: [Math.sin(k + seed) * 0.08 * k, (k / 4) * height * 0.62, 0], r: (birchBark ? 0.2 : 0.32) * (1 - k / 6) + 0.04})), {sides: 9, around: 2, vScale: 0.35, color: t => ao(t, 0.3, 0.6)});
  const branches = new Mesh(), leaves = new Mesh();
  for (let b = 0; b < 5; b++) {
    const a = b * 2.4 + r(), from = [0, height * r.range(0.4, 0.6), 0];
    const to = add(from, [Math.cos(a) * canopy * 0.7, height * r.range(0.18, 0.32), Math.sin(a) * canopy * 0.7]);
    branches.append(tube([{c: from, r: 0.1}, {c: to, r: 0.03}], {sides: 6, vScale: 0.5, caps: false}));
  }
  const clumps = birchBark ? 6 : 8;
  for (let k = 0; k < clumps; k++) {
    const a = k * 2.4 + r(), d = canopy * r.range(0.2, 0.75);
    const c = [Math.cos(a) * d, height * r.range(0.66, 0.92), Math.sin(a) * d], s = canopy * r.range(0.38, 0.55);
    const n = noise2(seed + k, 8);
    leaves.append(blob(2, dir => 1 + (n(dir[0] * 3 + 4, dir[2] * 3 + 4 + dir[1]) - 0.5) * 0.5, dir => { const l = 0.55 + 0.45 * clamp(dir[1] * 0.5 + 0.5); return [l, l, l, 1]; }, 1.4), p => add(c, scale(p, s)));
  }
  return {trunk, branches, leaves};
}
// A rock: a lumpy blob, flattened and half-buried, with moss on its upper faces.
const rock = (seed, w, h) => {
  const n = fbm(seed, 2, 4), f = noise2(seed + 1, 4);
  const lumps = d => 0.75 + n(d[0] * 0.25 + 0.5, d[2] * 0.25 + 0.5 + d[1] * 0.1) * 0.5 + (f(d[0] * 2 + 2, d[1] * 2 + d[2] * 2 + 2) - 0.5) * 0.2;
  const moss = d => [...mix([1, 1, 1], lin(hex('#5d7a33')), smooth(0.35, 0.8, d[1]) * 0.85), 1];
  return new Mesh().append(blob(2, lumps, moss, 0.9), p => [p[0] * w, Math.max(p[1] * h, -0.15 * h), p[2] * w * 0.85]);
};

const T = {};
T.bark = bark(1, '#2a1c12', '#6b4a30');
T.barkDark = bark(2, '#1d1712', '#4a3a2c');
T.oakBark = bark(3, '#2e261c', '#6e6050');
T.birch = birch();
T.needles = needles(4, '#0e2a17', '#3f6b33');
T.needlesBlue = needles(5, '#10241f', '#3b5f4f');
T.deadNeedles = needles(6, '#2a2618', '#6b5c38');
T.leaves = leafCard(7, '#23461a', '#7a9a2e');
T.birchLeaves = leafCard(8, '#4b6e20', '#c2b94a');
T.fern = fern();
T.grass = grass();
T.stone = stone(9, '#5d5a55', '#9a958c');
T.deerFur = fur(10, '#1a1512', '#4a3d31', 6);
T.bone = stone(11, '#b8ad96', '#e3dccb');
T.wolfFur = fur(12, '#3a3734', '#8a837a', 10);
T.jacket = cloth(13, '#6e2a1c', '#a8482c');
T.denim = cloth(14, '#1f2c3d', '#3e5370');
T.skin = fur(15, '#c08a68', '#d9a582', 2);
T.hair = fur(16, '#1f140c', '#4a3220', 20);
T.pack = cloth(17, '#3d4a2a', '#66773e');
T.endGrain = endGrain();

const M = (name, image, extra = {}) => ({name, image, roughness: 0.92, ...extra});
const model = (file, prims) => {
  const mats = [], parts = [{name: file, prims: prims.map(([mesh, mat]) => { mats.push(mat); return {mesh, material: mats.length - 1}; })}];
  save(`${file}.glb`, glb(parts, mats));
};

// Trees: the trunk base is the origin, as the game's collider expects.
[[ 'pine_a', 21, {tiers: 4, height: 7.4, spread: 2.2}, T.needles, T.bark],
 [ 'pine_b', 22, {tiers: 6, height: 8.8, spread: 1.7, droop: 0.35}, T.needlesBlue, T.bark],
 [ 'pine_c', 23, {tiers: 3, height: 6.0, spread: 1.6, droop: 0.5, dead: true}, T.deadNeedles, T.barkDark],
].forEach(([file, seed, o, leaf, wood]) => {
  const {trunk, crown} = pine(seed, o);
  model(file, [[trunk, M('bark', wood)], [crown, M('needles', leaf, {roughness: 0.85})]]);
});
{
  const oak = broadleaf(31, {height: 7.0, canopy: 2.6});
  model('oak', [[oak.trunk, M('bark', T.oakBark)], [oak.branches, M('branch', T.oakBark)], [oak.leaves, M('leaves', T.leaves, {mask: true, doubleSided: true})]]);
  const b = broadleaf(32, {height: 7.6, canopy: 1.8, birchBark: true});
  model('birch', [[b.trunk, M('bark', T.birch)], [b.branches, M('branch', T.birch)], [b.leaves, M('leaves', T.birchLeaves, {mask: true, doubleSided: true})]]);
}
// Undergrowth and props.
{
  const bush = new Mesh(), r = rng(41);
  for (let k = 0; k < 4; k++) bush.append(blob(1, () => 1, d => { const l = 0.5 + 0.5 * clamp(d[1] * 0.5 + 0.5); return [l, l, l, 1]; }, 1.2),
    p => add([r.range(-0.5, 0.5), 0.45 + r.range(0, 0.25), r.range(-0.5, 0.5)], scale(p, r.range(0.45, 0.7))));
  model('bush', [[bush, M('leaves', T.leaves, {mask: true, doubleSided: true})]]);
  const fernMesh = new Mesh();
  for (let k = 0; k < 7; k++) fernMesh.append(cards(1, 0.5, 1.1, {color: t => [0.6 + 0.4 * t, 0.6 + 0.4 * t, 0.6 + 0.4 * t, 1]}), chain(rotX(-0.9), rotY(k / 7 * Math.PI * 2)));
  model('fern', [[fernMesh, M('fern', T.fern, {mask: true, doubleSided: true})]]);
  model('grass', [[cards(3, 0.7, 0.55, {tilt: 0.1, color: t => [0.55 + 0.45 * t, 0.55 + 0.45 * t, 0.55 + 0.45 * t, 1]}), M('grass', T.grass, {mask: true, doubleSided: true})]]);
  model('rock_a', [[rock(51, 0.9, 0.6), M('stone', T.stone)]]);
  model('rock_b', [[rock(52, 0.5, 0.45), M('stone', T.stone)]]);
  // A log on its side along X: bark round, end grain at both ends.
  const log = tube([{c: [-0.55, 0, 0], r: 0.17}, {c: [0.55, 0, 0], r: 0.16}], {sides: 10, around: 2, vScale: 0.6, caps: false});
  const ends = new Mesh();
  for (const [x, r0, sign] of [[-0.55, 0.17, -1], [0.55, 0.16, 1]]) {
    const c = ends.vertex([x, 0, 0], [sign, 0, 0], [0.5, 0.5]);
    for (let s = 0; s <= 10; s++) { const a = s / 10 * Math.PI * 2; ends.vertex([x, Math.cos(a) * r0, Math.sin(a) * r0], [sign, 0, 0], [0.5 + 0.5 * Math.cos(a), 0.5 + 0.5 * Math.sin(a)]); }
    for (let s = 0; s < 10; s++) sign > 0 ? ends.tri(c, c + 1 + s, c + 2 + s) : ends.tri(c, c + 2 + s, c + 1 + s);
  }
  model('log', [[log, M('bark', T.bark)], [ends, M('grain', T.endGrain)]]);
  // A charred campfire log: the same log, darkened, with embers glowing at the ends.
  model('firelog', [[log, M('char', T.barkDark, {color: [0.35, 0.3, 0.28, 1]})], [ends, M('ember', T.endGrain, {color: [0.5, 0.3, 0.2, 1], emissive: [1.6, 0.45, 0.08]})]]);
}

// The Deer: long-legged, hunched, black-furred, with a bone skull, a crown of
// antlers and ember eyes. The body faces +Z; legs are separate parts so the game
// can swing them at the hip.
{
  const body = tube([
    {c: [0, 1.62, -0.75], r: 0.2}, {c: [0, 1.78, -0.35], r: 0.33}, {c: [0, 1.9, 0.15], r: 0.36}, {c: [0, 2.0, 0.5], r: 0.3},
  ], {sides: 12, around: 2, vScale: 1, shape: (a, k) => 1 + 0.12 * Math.sin(a * 3 + k), color: t => ao(t, 1, 0.7)});
  const neck = tube([{c: [0, 1.98, 0.48], r: 0.17}, {c: [0, 2.3, 0.78], r: 0.12}, {c: [0, 2.48, 0.92], r: 0.1}], {sides: 9, vScale: 1});
  const tail = tube([{c: [0, 1.7, -0.75], r: 0.08}, {c: [0, 1.25, -0.95], r: 0.02}], {sides: 6});
  const skull = tube([{c: [0, 2.56, 0.84], r: 0.13}, {c: [0, 2.53, 1.0], r: 0.12}, {c: [0, 2.43, 1.22], r: 0.07}, {c: [0, 2.38, 1.33], r: 0.04}],
    {sides: 10, vScale: 2, shape: a => 1 - 0.25 * Math.abs(Math.sin(a))});
  const jaw = tube([{c: [0, 2.44, 0.92], r: 0.07}, {c: [0, 2.33, 1.22], r: 0.035}], {sides: 7});
  const antlers = new Mesh();
  for (const side of [-1, 1]) {
    const base = [side * 0.09, 2.66, 0.86], pts = [base];
    for (let k = 1; k <= 5; k++) pts.push(add(base, [side * (0.13 * k + 0.02 * k * k), 0.21 * k - 0.012 * k * k, -0.07 * k]));
    antlers.append(tube(pts.map((c, k) => ({c, r: 0.045 * (1 - k / 6) + 0.008})), {sides: 6, vScale: 3}));
    for (const k of [1, 2, 3, 4]) {
      const from = pts[k], to = add(from, [side * 0.08, 0.32 - k * 0.03, 0.12 - k * 0.06]);
      antlers.append(tube([{c: from, r: 0.028}, {c: to, r: 0.006}], {sides: 5, vScale: 3}));
    }
  }
  const eyes = new Mesh();
  for (const side of [-1, 1]) eyes.append(blob(1), p => add([side * 0.085, 2.6, 1.06], scale(p, 0.035)));
  model('deer_body', [[body, M('fur', T.deerFur)], [neck, M('fur', T.deerFur)], [tail, M('fur', T.deerFur)], [skull, M('bone', T.bone, {roughness: 0.6})], [jaw, M('bone', T.bone)], [antlers, M('antler', T.bone, {color: [0.82, 0.78, 0.7, 1]})], [eyes, M('eyes', null, {color: [1, 0.15, 0.05, 1], emissive: [12, 1.2, 0.3]})]]);
  // A leg hangs from its hip at the origin: thigh, a backward knee, a long shin, a hoof.
  const leg = tube([{c: [0, 0, 0], r: 0.11}, {c: [0, -0.55, 0.08], r: 0.07}, {c: [0, -0.95, -0.12], r: 0.045}, {c: [0, -1.55, -0.02], r: 0.035}, {c: [0, -1.62, 0.0], r: 0.05}], {sides: 7, vScale: 1.5});
  model('deer_leg', [[leg, M('fur', T.deerFur)]]);
}
// A wolf: a lean grey body facing +Z, with a head, ears and tail; legs separate.
{
  const body = tube([{c: [0, 0.62, -0.5], r: 0.16}, {c: [0, 0.68, -0.15], r: 0.22}, {c: [0, 0.72, 0.25], r: 0.25}, {c: [0, 0.74, 0.45], r: 0.2}], {sides: 10, vScale: 1.5, shape: a => 1 + 0.15 * Math.max(0, Math.sin(a))});
  const head = tube([{c: [0, 0.82, 0.48], r: 0.15}, {c: [0, 0.84, 0.62], r: 0.14}, {c: [0, 0.76, 0.82], r: 0.07}, {c: [0, 0.74, 0.9], r: 0.04}], {sides: 9, vScale: 2});
  const ears = new Mesh();
  for (const s of [-1, 1]) ears.append(tube([{c: [s * 0.08, 0.93, 0.55], r: 0.045}, {c: [s * 0.1, 1.06, 0.53], r: 0.005}], {sides: 4}));
  const tail = tube([{c: [0, 0.66, -0.52], r: 0.07}, {c: [0, 0.55, -0.8], r: 0.08}, {c: [0, 0.42, -0.98], r: 0.03}], {sides: 7, vScale: 2});
  const eyes = new Mesh();
  for (const side of [-1, 1]) eyes.append(blob(1), p => add([side * 0.06, 0.88, 0.7], scale(p, 0.022)));
  model('wolf_body', [[body, M('fur', T.wolfFur)], [head, M('fur', T.wolfFur)], [ears, M('fur', T.wolfFur)], [tail, M('fur', T.wolfFur)], [eyes, M('eyes', null, {color: [1, 0.85, 0.3, 1], emissive: [3, 2.2, 0.4]})]]);
  model('wolf_leg', [[tube([{c: [0, 0, 0], r: 0.06}, {c: [0, -0.3, 0.03], r: 0.04}, {c: [0, -0.58, -0.02], r: 0.03}, {c: [0, -0.62, 0.02], r: 0.035}], {sides: 6, vScale: 2}), M('fur', T.wolfFur)]]);
}
// The survivor: a jacketed torso with a head, hair and a backpack, facing +Z; arms
// and legs hang from shoulder and hip pivots at the origin. Children reuse it, tinted.
{
  const torso = tube([{c: [0, 0.82, 0], r: 0.17}, {c: [0, 1.05, 0], r: 0.2}, {c: [0, 1.35, 0], r: 0.21}, {c: [0, 1.5, 0], r: 0.13}], {sides: 12, vScale: 1.5, shape: a => 1 - 0.25 * Math.abs(Math.cos(a))});
  const neck = tube([{c: [0, 1.48, 0], r: 0.06}, {c: [0, 1.58, 0], r: 0.055}], {sides: 8});
  const head = new Mesh().append(blob(2, d => 1 + 0.08 * d[2] - 0.05 * Math.abs(d[0])), p => add([0, 1.72, 0.01], stretch([0.11, 0.135, 0.12])(p)));
  const hair = new Mesh().append(blob(2, () => 1), p => add([0, 1.76, -0.015], stretch([0.118, 0.12, 0.125])(p)).map((x, i) => i === 1 ? Math.max(x, 1.71) : x));
  const pack = new Mesh().append(blob(1, () => 1), p => add([0, 1.15, -0.27], stretch([0.17, 0.22, 0.1])(p)));
  const strap = tube([{c: [0.12, 1.43, -0.18], r: 0.02}, {c: [0.14, 1.43, 0.14], r: 0.02}, {c: [0.15, 1.0, 0.18], r: 0.02}], {sides: 5, caps: false})
    .append(tube([{c: [-0.12, 1.43, -0.18], r: 0.02}, {c: [-0.14, 1.43, 0.14], r: 0.02}, {c: [-0.15, 1.0, 0.18], r: 0.02}], {sides: 5, caps: false}));
  model('survivor_body', [[torso, M('jacket', T.jacket)], [neck, M('skin', T.skin)], [head, M('skin', T.skin, {roughness: 0.7})], [hair, M('hair', T.hair)], [pack, M('pack', T.pack)], [strap, M('strap', T.pack, {color: [0.4, 0.35, 0.3, 1]})]]);
  const arm = tube([{c: [0, 0, 0], r: 0.065}, {c: [0, -0.3, 0.02], r: 0.055}, {c: [0, -0.56, 0.06], r: 0.045}], {sides: 7, vScale: 2});
  const hand = new Mesh().append(blob(1), p => add([0, -0.62, 0.07], scale(p, 0.05)));
  model('survivor_arm', [[arm, M('jacket', T.jacket)], [hand, M('skin', T.skin)]]);
  const leg = tube([{c: [0, 0, 0], r: 0.085}, {c: [0, -0.42, 0.01], r: 0.07}, {c: [0, -0.8, -0.01], r: 0.055}], {sides: 8, vScale: 2});
  const boot = new Mesh().append(blob(1), p => add([0, -0.84, 0.05], stretch([0.07, 0.05, 0.13])(p)));
  model('survivor_leg', [[leg, M('denim', T.denim)], [boot, M('boot', T.barkDark, {color: [0.5, 0.42, 0.35, 1]})]]);
}

// The flashlight's beam: an open cone along −Z (the spot's axis), faint and
// fading with distance; drawn blended, it gives the light a body in the dark.
{
  const rings = Array.from({length: 9}, (_, k) => { const z = (k / 8) ** 1.3 * 10; return {c: [0, 0, -z], r: 0.06 + z * Math.tan(0.38)}; });
  const beam = tube(rings, {sides: 18, caps: false, color: t => [1, 1, 1, 0.06 * smooth(0, 0.15, t) * (1 - t) ** 1.4]});
  model('beam', [[beam, M('beam', null, {color: [1, 0.95, 0.82, 1], emissive: [0.35, 0.33, 0.27], blend: true, doubleSided: true})]]);
}

save('sky_day.png', daySky());
save('sky_night.png', nightSky());

const total = written.reduce((n, [, b]) => n + b, 0);
for (const [n, b] of written) console.log(`${n.padEnd(20)} ${String(b).padStart(8)} bytes`);
console.log(`${written.length} files, ${total} bytes`);
