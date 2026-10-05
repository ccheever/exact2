// The models: the arena dressed over its collision blocks, three first-person
// weapons with the shooter's hands, the rocket, the muzzle flash, and the bot
// soldier as rigid parts that the game poses.
import { Gltf, Geom, roundedBox, box, cylinder, barrel, prism, sphere, mat, quat } from './gltf.mjs';

const T = (t, r, s) => mat(t, r, s);

/** The arena from arena.json: one node, a few primitives merged per material. */
export function arena(layout) {
  const g = new Gltf();
  const m = {
    floor: g.material('floor', { map: 'floor.png', normal: 'floor_n.png', mr: 'floor_mr.png' }),
    panel: g.material('panel', { map: 'panel.png', normal: 'panel_n.png', mr: 'panel_mr.png' }),
    crate: g.material('crate', { map: 'crate.png', normal: 'crate_n.png', mr: 'crate_mr.png' }),
    deck: g.material('deck', { map: 'deck.png', normal: 'deck_n.png', mr: 'deck_mr.png', emissiveMap: 'deck_e.png', emissive: [3, 3, 3] }),
    trim: g.material('trim', { color: [0.1, 0.11, 0.13, 1], metal: 0.85, rough: 0.3 }),
    red: g.material('glow-red', { color: [0.2, 0.02, 0.02, 1], emissive: [9, 0.8, 0.5] }),
    blue: g.material('glow-blue', { color: [0.02, 0.05, 0.2, 1], emissive: [0.5, 2.6, 9] }),
    amber: g.material('glow-amber', { color: [0.2, 0.1, 0.02, 1], emissive: [8, 4, 0.6] }),
    banner: g.material('banner-red', { map: 'logo_red.png', emissiveMap: 'logo_red.png', emissive: [1.6, 1.6, 1.6], rough: 0.4 }),
    bannerB: g.material('banner-blue', { map: 'logo_blue.png', emissiveMap: 'logo_blue.png', emissive: [1.6, 1.6, 1.6], rough: 0.4 }),
  };
  const parts = Object.fromEntries(Object.keys(m).map((k) => [k, new Geom()]));
  const half = layout.half;
  parts.floor.add(box([2 * half + 2, 1, 2 * half + 2], 2), T([0, -0.5, 0]));
  for (const b of layout.blocks) {
    const at = b.at, size = b.size, rot = quat.x(b.tilt ?? 0);
    switch (b.kind) {
      case 'wall': {
        parts.panel.add(box(size, 2), T(at));
        // A metal cap and a glowing team line along each wall's inner top edge.
        const long = size[0] > size[2], inward = long ? [0, 0, -Math.sign(at[2])] : [-Math.sign(at[0]), 0, 0];
        const cap = long ? [size[0], 0.25, size[2] + 0.2] : [size[0] + 0.2, 0.25, size[2]];
        parts.trim.add(roundedBox(cap, 0.04), T([at[0], 5.1, at[2]]));
        const glow = long ? [size[0] - 2, 0.08, 0.06] : [0.06, 0.08, size[2] - 2];
        const face = (long ? size[2] : size[0]) / 2 + 0.03;
        const team = at[2] < -1 || at[0] < -1 ? parts.red : parts.blue;
        for (const y of [0.35, 3.9]) team.add(box(glow), T([at[0] + inward[0] * face, y, at[2] + inward[2] * face]));
        break;
      }
      case 'deck':
        parts.deck.add(roundedBox(size, 0.05, 2.5), T(at));
        parts.trim.add(roundedBox([size[0] + 0.15, 0.12, size[2] + 0.15], 0.04), T([at[0], at[1] + size[1] / 2, at[2]]));
        break;
      case 'ramp':
        parts.deck.add(roundedBox(size, 0.04, 2.5), T(at, rot));
        for (const side of [-1, 1]) parts.amber.add(box([0.06, 0.06, size[2]]), T([at[0] + side * (size[0] / 2 + 0.02), at[1] + 0.2, at[2]], rot));
        break;
      case 'slab':
        parts.panel.add(roundedBox(size, 0.06, 2), T(at));
        parts.trim.add(roundedBox([size[0] + 0.1, 0.14, size[2] + 0.1], 0.05), T([at[0], at[1] + size[1] / 2, at[2]]));
        parts.amber.add(box([size[0] + 0.02, 0.05, size[2] + 0.02]), T([at[0], at[1] - 0.4, at[2]]));
        break;
      case 'crate':
        parts.crate.add(roundedBox(size, 0.05, size[1]), T(at));
        break;
      case 'low':
        parts.panel.add(roundedBox(size, 0.08, 2), T(at));
        parts.trim.add(roundedBox([size[0] + 0.08, 0.1, size[2] + 0.08], 0.04), T([at[0], at[1] + size[1] / 2 + 0.03, at[2]]));
        break;
    }
  }
  // Banners on the four walls' inner faces: red north and west, blue south and east.
  for (const [x, z, ry, team] of [[0, -21.97, 0, 'banner'], [0, 21.97, Math.PI, 'bannerB'], [-21.97, 0, Math.PI / 2, 'banner'], [21.97, 0, -Math.PI / 2, 'bannerB']]) {
    const q = new Geom(), w = 8, h = 2, base = q.count;
    for (const [px, py, u, v] of [[-w / 2, -h / 2, 0, 1], [w / 2, -h / 2, 1, 1], [w / 2, h / 2, 1, 0], [-w / 2, h / 2, 0, 0]]) q.vertex([px, py, 0], [0, 0, 1], [u, v]);
    q.quad(base, base + 1, base + 2, base + 3);
    parts[team].add(q, T([x, 2.6, z], quat.y(ry)));
  }
  // Corner light pylons: posts with glowing rings, where the spotlights hang.
  for (const [x, z] of layout.lights) {
    parts.trim.add(cylinder(0.25, 6.4, 12), T([x, 3.2, z]));
    parts.amber.add(cylinder(0.27, 0.12, 12), T([x, 2.0, z]));
    parts.amber.add(cylinder(0.27, 0.12, 12), T([x, 4.0, z]));
    parts.trim.add(roundedBox([0.7, 0.35, 0.7], 0.06), T([x, 6.4, z]));
  }
  const mesh = g.mesh('arena', Object.entries(parts).map(([k, geom]) => [geom, m[k]]));
  g.node({ name: 'arena', mesh }, true);
  return g;
}

/** Shared weapon materials. */
function weaponMaterials(g) {
  return {
    metal: g.material('gunmetal', { map: 'gunmetal.png', mr: 'gunmetal_mr.png', normal: 'gunmetal_n.png' }),
    polymer: g.material('polymer', { color: [0.1, 0.105, 0.11, 1], rough: 0.55 }),
    accent: g.material('accent', { color: [0.95, 0.42, 0.06, 1], rough: 0.45, metal: 0.1 }),
    steel: g.material('steel', { color: [0.82, 0.84, 0.88, 1], metal: 1, rough: 0.18 }),
    lens: g.material('lens', { color: [0.1, 0.0, 0.0, 1], emissive: [3, 0.2, 0.1] }),
    olive: g.material('olive', { color: [0.18, 0.24, 0.14, 1], rough: 0.65, metal: 0.05 }),
    yellow: g.material('hazard', { color: [0.95, 0.72, 0.05, 1], rough: 0.5 }),
    grip: g.material('grip', { color: [0.11, 0.08, 0.06, 1], rough: 0.9 }),
    glove: g.material('glove', { color: [0.22, 0.18, 0.13, 1], rough: 0.8 }),
    sleeve: g.material('sleeve', { color: [0.08, 0.13, 0.21, 1], rough: 0.95 }),
    cuff: g.material('cuff', { color: [0.11, 0.12, 0.09, 1], rough: 0.85 }),
  };
}
const build = (g, name, parts) => g.node({ name, mesh: g.mesh(name, parts) }, true);

/** The rotation taking +Z to `d`. */
const toward = (d) => {
  const l = Math.hypot(...d), n = d.map((x) => x / l), axis = [-n[1], n[0], 0];
  return Math.hypot(...axis) < 1e-6 ? [0, 0, 0, 1] : quat.axis(axis, Math.acos(Math.max(-1, Math.min(1, n[2]))));
};
/**
 * The shooter's gloved hands and sleeved forearms: each grip is a hand box at `at`
 * and the direction its forearm runs back from the wrist, towards the camera.
 */
function hands(m, grips) {
  const glove = new Geom(), sleeve = new Geom(), cuff = new Geom();
  for (const { at, size, arm, length = 0.36, wrap = 'grip' } of grips) {
    glove.add(roundedBox(size, Math.min(...size) * 0.4), T(at));
    // Four fingers: around the front of an upright grip, or over a handguard's side.
    for (let k = 0; k < 4; k++) {
      if (wrap === 'grip') {
        const y = at[1] + size[1] * (0.3 - k * 0.22);
        glove.add(roundedBox([size[0] * 0.9, 0.02, 0.028], 0.009), T([at[0] - 0.004, y, at[2] - size[2] / 2 - 0.006]));
      } else {
        const z = at[2] + size[2] * (0.32 - k * 0.21);
        glove.add(roundedBox([0.024, 0.034, 0.02], 0.01), T([at[0] + size[0] / 2 + 0.004, at[1] + 0.01, z]));
      }
    }
    const l = Math.hypot(...arm), n = arm.map((x) => x / l), q = toward(n);
    const wrist = at.map((x, i) => x + n[i] * size[2] * 0.45);
    cuff.add(roundedBox([0.084, 0.076, 0.04], 0.016), T(wrist.map((x, i) => x + n[i] * 0.018), q));
    sleeve.add(roundedBox([0.1, 0.094, length], 0.04), T(wrist.map((x, i) => x + n[i] * (0.034 + length / 2)), q));
  }
  return [[glove, m.glove], [cuff, m.cuff], [sleeve, m.sleeve]];
}

/** The rifle (without its magazine), muzzle at z = −0.52, grip near the origin. */
export function rifle() {
  const g = new Gltf(), m = weaponMaterials(g);
  const metal = new Geom(), polymer = new Geom(), accent = new Geom(), lens = new Geom();
  metal.add(roundedBox([0.055, 0.075, 0.3], 0.008), T([0, 0, -0.1]));                 // receiver
  metal.add(barrel(0.011, 0.22, 12), T([0, 0.012, -0.42]));                           // barrel
  metal.add(barrel(0.018, 0.05, 12), T([0, 0.012, -0.53]));                           // muzzle brake
  polymer.add(roundedBox([0.06, 0.06, 0.2], 0.015), T([0, 0.005, -0.32]));            // handguard
  polymer.add(prism([[0.02, 0.0], [-0.09, 0.03], [-0.1, 0.07], [0.0, 0.06]], 0.04), T([0, -0.03, 0.02]));   // grip
  polymer.add(prism([[0.02, 0.05], [0.02, 0.25], [-0.07, 0.25], [-0.02, 0.05]], 0.045), T([0, -0.01, 0])); // stock
  metal.add(roundedBox([0.03, 0.03, 0.09], 0.006), T([0, 0.058, -0.08]));             // optic base
  metal.add(barrel(0.02, 0.07, 14), T([0, 0.085, -0.08]));                            // optic tube
  lens.add(barrel(0.006, 0.003, 10), T([0, 0.085, -0.044]));                          // red dot glow
  accent.add(roundedBox([0.062, 0.012, 0.18], 0.004), T([0, 0.034, -0.32]));          // stripe
  accent.add(roundedBox([0.047, 0.03, 0.03], 0.004), T([0, -0.015, -0.03]));          // trigger guard block
  const arms = hands(m, [
    { at: [0.006, -0.085, 0.06], size: [0.07, 0.09, 0.1], arm: [0.45, -0.55, 0.7] },     // trigger hand on the grip
    { at: [-0.012, -0.035, -0.31], size: [0.07, 0.05, 0.11], arm: [-0.72, -0.42, 0.55], length: 0.46, wrap: 'guard' }, // support hand
  ]);
  build(g, 'rifle', [[metal, m.metal], [polymer, m.polymer], [accent, m.accent], [lens, m.lens], ...arms]);
  return g;
}
/** The rifle's curved magazine, a separate model so reloads can drop it. */
export function rifleMag() {
  const g = new Gltf(), m = weaponMaterials(g);
  const body = new Geom(), base = new Geom();
  body.add(roundedBox([0.032, 0.09, 0.05], 0.006), T([0, -0.045, 0]));
  body.add(roundedBox([0.032, 0.08, 0.05], 0.006), T([0, -0.12, 0.015], quat.x(0.3)));
  base.add(roundedBox([0.04, 0.015, 0.065], 0.004), T([0, -0.165, 0.03], quat.x(0.3)));
  build(g, 'mag', [[body, m.metal], [base, m.accent]]);
  return g;
}
/** The rocket launcher: an olive tube with a flared muzzle and a hazard band. */
export function launcher() {
  const g = new Gltf(), m = weaponMaterials(g);
  const olive = new Geom(), metal = new Geom(), yellow = new Geom(), polymer = new Geom(), lens = new Geom();
  olive.add(barrel(0.065, 0.7, 20), T([0, 0, -0.2]));
  metal.add(barrel(0.085, 0.09, 20, { top: 0.066 }), T([0, 0, -0.58]));
  metal.add(barrel(0.075, 0.08, 20, { top: 0.065 }), T([0, 0, 0.18]));
  yellow.add(barrel(0.067, 0.05, 20), T([0, 0, -0.42]));
  yellow.add(barrel(0.067, 0.02, 20), T([0, 0, -0.34]));
  polymer.add(prism([[0.0, 0.0], [-0.1, 0.03], [-0.11, 0.07], [0.0, 0.06]], 0.04), T([0, -0.07, -0.05]));
  metal.add(roundedBox([0.04, 0.06, 0.12], 0.01), T([0.055, 0.07, -0.22]));
  lens.add(roundedBox([0.03, 0.035, 0.004], 0.003), T([0.055, 0.075, -0.162]));
  const arms = hands(m, [
    { at: [0.006, -0.125, -0.0], size: [0.07, 0.09, 0.1], arm: [0.45, -0.6, 0.65] },
    { at: [-0.012, -0.085, -0.36], size: [0.075, 0.055, 0.11], arm: [-0.72, -0.45, 0.53], length: 0.46, wrap: 'guard' },
  ]);
  build(g, 'launcher', [[olive, m.olive], [metal, m.metal], [yellow, m.yellow], [polymer, m.polymer], [lens, m.lens], ...arms]);
  return g;
}
/** The knife: a clip-point steel blade, guard and a wrapped grip. */
export function knife() {
  const g = new Gltf(), m = weaponMaterials(g);
  const steel = new Geom(), metal = new Geom(), grip = new Geom(), accent = new Geom();
  steel.add(prism([[0.0, 0.0], [0.025, 0.0], [0.03, -0.12], [0.012, -0.21], [-0.004, -0.23], [-0.006, -0.15], [0.0, -0.05]], 0.006), T([0, -0.012, -0.02]));
  metal.add(roundedBox([0.022, 0.05, 0.012], 0.004), T([0, 0.0, -0.01]));
  for (let i = 0; i < 6; i++) (i % 2 ? accent : grip).add(barrel(0.014, 0.018, 10), T([0, 0.0, 0.01 + i * 0.018]));
  metal.add(barrel(0.016, 0.012, 10), T([0, 0, 0.12]));
  const arms = hands(m, [{ at: [0.004, -0.004, 0.065], size: [0.065, 0.075, 0.11], arm: [0.4, -0.55, 0.72] }]);
  build(g, 'knife', [[steel, m.steel], [metal, m.metal], [grip, m.grip], [accent, m.accent], ...arms]);
  return g;
}
/** The rocket in flight: a long body, nose cone, four fins and a burning tail. */
export function rocket() {
  const g = new Gltf(), m = weaponMaterials(g);
  const body = new Geom(), nose = new Geom(), fins = new Geom(), tail = new Geom();
  const flame = g.material('flame', { color: [1, 0.5, 0.1, 1], emissive: [14, 5, 0.8] });
  body.add(cylinder(0.05, 0.36, 14));
  nose.add(cylinder(0.05, 0.12, 14, { top: 0 }), T([0, 0.24, 0]));
  for (let k = 0; k < 4; k++) fins.add(prism([[0, 0], [0.08, 0], [0, 0.12]], 0.008), T([0, -0.18, 0], quat.y(k * Math.PI / 2), [1, 1, 1]));
  tail.add(cylinder(0.035, 0.05, 10, { top: 0.045 }), T([0, -0.2, 0]));
  build(g, 'rocket', [[body, m.steel], [nose, m.accent], [fins, m.olive], [tail, flame]]);
  return g;
}
/** A muzzle flash: three crossed, blended petals of light along −Z. */
export function flash() {
  const g = new Gltf(), mat0 = g.material('flash', { color: [1, 0.7, 0.3, 0.9], emissive: [16, 9, 3], alpha: true });
  const petals = new Geom();
  for (let k = 0; k < 3; k++) {
    const q = new Geom(), base = q.count;
    for (const [x, z] of [[-0.05, 0], [0.05, 0], [0.012, -0.22], [-0.012, -0.22]]) q.vertex([x, 0, z], [0, 1, 0], [0, 0]);
    q.quad(base, base + 3, base + 2, base + 1);
    petals.add(q, T([0, 0, 0], quat.z(k * Math.PI / 3)));
  }
  petals.add(sphere(0.035, 6, 10));
  build(g, 'flash', [[petals, mat0]]);
  return g;
}


/**
 * The bot soldier, 1.8 m tall, facing −Z, as rigid parts the game assembles and
 * poses (art.rs, art_present.rs): hips, torso, head, gun, and an arm, forearm,
 * thigh and shin each drawn twice. Every part has the same three materials, so
 * one `MaterialOverrides` dresses any of them in a team's colours: the armour
 * (material 0) takes the colour and the visor (material 2) its glow; cloth, skin
 * and kit share one white material (1) tinted by vertex colour, for few draws.
 * Each part's origin is its joint.
 */
export function soldier() {
  const cloth = [0.13, 0.14, 0.13, 1], skin = [0.78, 0.56, 0.42, 1], kit = [0.06, 0.065, 0.07, 1], steel = [0.14, 0.15, 0.17, 1];
  const tinted = (geom, c, t) => new Geom().add(geom, t).tint(c);
  const part = (name, pieces) => {
    const g = new Gltf();
    const m = {
      armour: g.material('armour', { color: [0.7, 0.7, 0.72, 1], rough: 0.35, metal: 0.25 }),
      body: g.material('body', { color: [1, 1, 1, 1], rough: 0.75, metal: 0.1 }),
      visor: g.material('visor', { color: [0.02, 0.02, 0.03, 1], emissive: [0.4, 0.4, 0.4], metal: 0.8, rough: 0.1 }),
    };
    // Merge every cloth, skin and kit piece into the one tinted body primitive.
    const body = new Geom(), rest = [];
    for (const [geom, slot] of pieces) slot === 'body' ? body.add(geom) : rest.push([geom, m[slot]]);
    build(g, name, [[body, m.body], ...rest]);
    return g;
  };
  return {
    hips: part('hips', [[tinted(roundedBox([0.36, 0.18, 0.22], 0.06), cloth), 'body'],
      [tinted(roundedBox([0.38, 0.06, 0.24], 0.02), kit, T([0, 0.07, 0])), 'body'],
      [tinted(roundedBox([0.1, 0.12, 0.06], 0.02), kit, T([0.16, -0.02, -0.08])), 'body']]),
    torso: part('torso', [
      [tinted(roundedBox([0.4, 0.5, 0.24], 0.08), cloth, T([0, 0.25, 0])), 'body'],
      [new Geom().add(roundedBox([0.42, 0.34, 0.28], 0.06), T([0, 0.3, 0])), 'armour'],
      [new Geom().add(roundedBox([0.2, 0.06, 0.29], 0.02), T([0, 0.5, 0])), 'armour'],
      [tinted(roundedBox([0.3, 0.12, 0.08], 0.03), kit, T([0, 0.36, -0.15])), 'body'],
      [tinted(roundedBox([0.24, 0.22, 0.1], 0.04), kit, T([0, 0.3, 0.17])), 'body'],
      [new Geom().add(roundedBox([0.06, 0.03, 0.01], 0.008), T([-0.12, 0.42, -0.146])), 'visor'],
    ]),
    head: part('head', [
      [tinted(sphere(0.12, 8, 14, [1, 1.1, 1.05]), skin), 'body'],
      [tinted(roundedBox([0.1, 0.1, 0.1], 0.04), skin, T([0, -0.13, 0.01])), 'body'],
      [new Geom().add(sphere(0.145, 8, 14, [1.05, 0.85, 1.1]), T([0, 0.05, 0.01])), 'armour'],
      [new Geom().add(roundedBox([0.2, 0.06, 0.06], 0.025), T([0, 0.0, -0.11])), 'visor'],
    ]),
    arm: part('arm', [[tinted(roundedBox([0.11, 0.3, 0.11], 0.04), cloth, T([0, -0.15, 0])), 'body'],
      [new Geom().add(sphere(0.085, 6, 10, [1.2, 0.9, 1.1]), T([0, -0.01, 0])), 'armour']]),
    fore: part('fore', [[tinted(roundedBox([0.09, 0.26, 0.09], 0.035), cloth, T([0, -0.13, 0])), 'body'],
      [tinted(roundedBox([0.1, 0.1, 0.11], 0.03), kit, T([0, -0.29, 0])), 'body']]),
    thigh: part('thigh', [[tinted(roundedBox([0.15, 0.42, 0.17], 0.05), cloth, T([0, -0.21, 0])), 'body'],
      [new Geom().add(roundedBox([0.155, 0.12, 0.13], 0.03), T([0, -0.12, -0.06])), 'armour']]),
    shin: part('shin', [[tinted(roundedBox([0.13, 0.36, 0.14], 0.04), cloth, T([0, -0.18, 0])), 'body'],
      [tinted(roundedBox([0.15, 0.12, 0.25], 0.04), kit, T([0, -0.39, -0.04])), 'body'],
      [new Geom().add(roundedBox([0.14, 0.14, 0.05], 0.02), T([0, -0.12, -0.075])), 'armour']]),
    // A rifle carried level across the chest; its muzzle is at z = −0.65.
    gun: part('gun', [[tinted(roundedBox([0.06, 0.08, 0.5], 0.01), steel, T([0, 0, -0.2])), 'body'],
      [tinted(barrel(0.012, 0.2, 8), steel, T([0, 0.01, -0.55])), 'body'],
      [tinted(roundedBox([0.04, 0.12, 0.06], 0.01), kit, T([0, -0.08, -0.1])), 'body'],
      [new Geom().add(roundedBox([0.062, 0.012, 0.16], 0.004), T([0, 0.045, -0.25])), 'armour']]),
  };
}
