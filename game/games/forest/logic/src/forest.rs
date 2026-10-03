//! The forest: rolling terrain, a jittered grid of trees and the grid lookups
//! that collision and navigation use instead of scanning every tree.
use exact_game::*;
use exact_game_physics::{Collider, Shape};

/// One tree at most per cell; lookups visit the 3×3 cells around a point.
pub const CELL: f32 = 5.0;
/// Treeless radius around the campfire.
pub const CLEARING: f32 = 15.0;
/// Trunk collision radius at unit tree scale.
pub const TRUNK: f32 = 0.42;
/// Terrain sample spacing in metres.
pub const SAMPLE: f32 = 4.0;
/// Blows to fell a tree.
pub const TREE_HP: u8 = 3;

/// Per-cell tree data. Cells without a tree, and felled trees, have zero hp.
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct Grove {
    pub side: u32,
    pub half: f32,
    pub x: Vec<f32>,
    pub z: Vec<f32>,
    pub scale: Vec<f32>,
    pub hp: Vec<u8>,
    pub trunk: Vec<Entity>,
    pub crown: Vec<Entity>,
    /// Model index into `KINDS` and the tree's yaw, for the wind's sway.
    pub kind: Vec<u8>,
    pub turn: Vec<f32>,
    pub standing: u32,
}

/// The tree models, with the share of the forest each takes.
pub const KINDS: [(&str, f32); 5] = [
    ("pine_a.model", 0.42),
    ("pine_b.model", 0.26),
    ("pine_c.model", 0.08),
    ("oak.model", 0.14),
    ("birch.model", 0.10),
];
/// Undergrowth models with their shares and scale ranges.
const UNDER: [(&str, f32, f32, f32); 5] = [
    ("fern.model", 0.32, 0.8, 1.4),
    ("bush.model", 0.2, 0.7, 1.3),
    ("grass.model", 0.26, 0.8, 1.5),
    ("rock_a.model", 0.1, 0.6, 1.6),
    ("rock_b.model", 0.12, 0.6, 1.4),
];
/// Every model and sky the forest loads before setup.
pub const ASSETS: &[&str] = &[
    "pine_a.model",
    "pine_b.model",
    "pine_c.model",
    "oak.model",
    "birch.model",
    "fern.model",
    "bush.model",
    "grass.model",
    "rock_a.model",
    "rock_b.model",
    "log.model",
    "firelog.model",
    "deer_body.model",
    "deer_leg.model",
    "wolf_body.model",
    "wolf_leg.model",
    "survivor_body.model",
    "survivor_arm.model",
    "survivor_leg.model",
    "beam.model",
    "sky_day.tex",
    "sky_night.tex",
];

fn pick<T: Copy>(w: &World, table: &[(&'static str, f32, T)]) -> (usize, &'static str, T) {
    let mut roll = w.rand(0.0..1.0);
    for (k, &(name, share, extra)) in table.iter().enumerate() {
        roll -= share;
        if roll <= 0.0 {
            return (k, name, extra);
        }
    }
    let last = table.len() - 1;
    (last, table[last].0, table[last].2)
}

/// The tree a cell holds, if one is standing there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Tree {
    pub cell: u32,
}

/// Gentle hills that flatten into the campfire clearing.
pub fn height(x: f32, z: f32) -> f32 {
    let hills = 2.4 * math::sin(x * 0.021 + 0.7) * math::cos(z * 0.017 - 0.4)
        + 0.9 * math::sin((x + z) * 0.043)
        + 0.35 * math::cos(x * 0.11 - z * 0.07);
    let r = (x * x + z * z).sqrt();
    hills * math::smoothstep(CLEARING * 0.7, CLEARING * 2.0, r)
}

/// Side length of the square world holding `trees` at roughly 80% cell occupancy.
pub fn world_half(trees: u32) -> f32 {
    let clearing_cells = std::f32::consts::PI * CLEARING * CLEARING / (CELL * CELL);
    let cells = trees as f32 / 0.8 + clearing_cells + 16.0;
    (cells.sqrt() * CELL * 0.5).ceil().max(40.0)
}

impl Grove {
    pub fn cell_of(&self, x: f32, z: f32) -> Option<(i32, i32)> {
        let i = math::floor((x + self.half) / CELL) as i32;
        let j = math::floor((z + self.half) / CELL) as i32;
        let n = self.side as i32;
        (i >= 0 && j >= 0 && i < n && j < n).then_some((i, j))
    }
    /// Standing trunks in the 3×3 cells around a point: (cell, x, z, radius).
    pub fn around(&self, x: f32, z: f32) -> impl Iterator<Item = (u32, f32, f32, f32)> + '_ {
        let (ci, cj) = self.cell_of(x, z).unwrap_or((-9, -9));
        let n = self.side as i32;
        (-1..=1).flat_map(move |dj| {
            (-1..=1).filter_map(move |di| {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= n || j >= n {
                    return None;
                }
                let c = (j * n + i) as usize;
                (self.hp[c] > 0).then(|| (c as u32, self.x[c], self.z[c], TRUNK * self.scale[c]))
            })
        })
    }
    /// The nearest standing tree within `reach` of a point.
    pub fn nearest(&self, x: f32, z: f32, reach: f32) -> Option<(u32, f32)> {
        self.around(x, z)
            .map(|(c, tx, tz, r)| {
                let (dx, dz) = (tx - x, tz - z);
                (c, (dx * dx + dz * dz).sqrt() - r)
            })
            .filter(|&(_, d)| d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }
    /// Push a circle of `radius` out of every trunk it overlaps.
    pub fn resolve(&self, mut x: f32, mut z: f32, radius: f32) -> (f32, f32) {
        for _ in 0..2 {
            for (_, tx, tz, r) in self.around(x, z).collect::<Vec<_>>() {
                let (dx, dz) = (x - tx, z - tz);
                let d = (dx * dx + dz * dz).sqrt();
                let min = r + radius;
                if d < min {
                    let (nx, nz) = if d > 1e-4 {
                        (dx / d, dz / d)
                    } else {
                        (1.0, 0.0)
                    };
                    x = tx + nx * min;
                    z = tz + nz * min;
                }
            }
        }
        let edge = self.half - 1.0;
        (x.clamp(-edge, edge), z.clamp(-edge, edge))
    }
    /// Bend a planar velocity around trunks ahead: the steering half of navigation.
    pub fn steer(&self, x: f32, z: f32, vx: f32, vz: f32, radius: f32) -> (f32, f32) {
        let speed = (vx * vx + vz * vz).sqrt();
        if speed < 1e-4 {
            return (vx, vz);
        }
        let (fx, fz) = (vx / speed, vz / speed);
        let (mut ax, mut az) = (0.0, 0.0);
        let look = 3.5;
        for (_, tx, tz, r) in self.around(x + fx * 2.0, z + fz * 2.0) {
            let (dx, dz) = (tx - x, tz - z);
            let ahead = dx * fx + dz * fz;
            if ahead <= 0.0 || ahead > look {
                continue;
            }
            let side = dx * -fz + dz * fx;
            let clear = r + radius + 0.4;
            if side.abs() < clear {
                // Turn away from the trunk; ties turn left so the choice is stable.
                let push = (clear - side.abs()) / clear * (1.0 - ahead / look);
                let sign = if side > 0.0 { 1.0 } else { -1.0 };
                ax += fz * sign * push * 2.0;
                az += -fx * sign * push * 2.0;
            }
        }
        let (nx, nz) = (fx + ax, fz + az);
        let n = (nx * nx + nz * nz).sqrt().max(1e-4);
        (nx / n * speed, nz / n * speed)
    }
    /// Position a standing tree at this cell.
    pub fn at(&self, cell: u32) -> Vec3 {
        let c = cell as usize;
        Vec3::new(self.x[c], height(self.x[c], self.z[c]), self.z[c])
    }
}

/// Terrain, trees and the grove resource. Selection sampling places exactly
/// `trees` trees outside the clearing.
pub fn grow(w: &mut World, trees: u32, primitives: bool, colliders: bool) {
    let half = world_half(trees);
    let side = (2.0 * half / CELL) as u32;
    let half = side as f32 * CELL * 0.5;
    let n = (side * side) as usize;
    // The collider samples every 4 m; the drawn ground every 2 m where the world
    // is small enough to afford it, so its colour noise reads at walking scale.
    let heightfield = |spacing: f32| {
        let samples = (2.0 * half / spacing) as u32 + 1;
        let step = 2.0 * half / (samples - 1) as f32;
        let heights: Vec<f32> = (0..samples * samples)
            .map(|k| {
                height(
                    -half + (k % samples) as f32 * step,
                    -half + (k / samples) as f32 * step,
                )
            })
            .collect();
        Shape::heightfield(
            samples,
            samples,
            heights,
            Vec3::new(2.0 * half, 1.0, 2.0 * half),
        )
        .expect("terrain")
    };
    let (_, shape) = heightfield(SAMPLE);
    let (mut mesh, _) = heightfield(if half <= 400.0 { 2.0 } else { SAMPLE });
    mesh.colors = mesh
        .positions
        .chunks_exact(3)
        .flat_map(|p| ground_color(p[0], p[1], p[2]))
        .collect();
    let ground = w.generated("terrain.model", mesh).expect("terrain model");
    let terrain = w.spawn_named("terrain", (Transform::default(), ground));
    if colliders {
        w.insert(
            terrain,
            Collider {
                shape,
                ..Default::default()
            },
        );
    }
    let mut grove = Grove {
        side,
        half,
        x: vec![0.0; n],
        z: vec![0.0; n],
        scale: vec![0.0; n],
        hp: vec![0; n],
        trunk: vec![Entity::default(); n],
        crown: vec![Entity::default(); n],
        kind: vec![0; n],
        turn: vec![0.0; n],
        standing: 0,
    };
    let open: Vec<usize> = (0..n)
        .filter(|&c| {
            let (i, j) = ((c as u32 % side) as f32, (c as u32 / side) as f32);
            let (x, z) = (-half + (i + 0.5) * CELL, -half + (j + 0.5) * CELL);
            (x * x + z * z).sqrt() > CLEARING + CELL * 0.5
                && x.abs() < half - CELL
                && z.abs() < half - CELL
        })
        .collect();
    let mut left = trees.min(open.len() as u32);
    for (k, &c) in open.iter().enumerate() {
        let remaining = (open.len() - k) as f32;
        if left == 0 || !w.chance(left as f32 / remaining) {
            continue;
        }
        left -= 1;
        let (i, j) = ((c as u32 % side) as f32, (c as u32 / side) as f32);
        grove.x[c] = -half + (i + w.rand(0.2..0.8)) * CELL;
        grove.z[c] = -half + (j + w.rand(0.2..0.8)) * CELL;
        grove.scale[c] = w.rand(0.75..1.35);
        grove.hp[c] = TREE_HP;
        grove.standing += 1;
        grove.turn[c] = w.rand(0.0..std::f32::consts::TAU);
        let kinds = KINDS.map(|(name, share)| (name, share, ()));
        grove.kind[c] = pick(w, &kinds).0 as u8;
    }
    for c in 0..n {
        if grove.hp[c] == 0 {
            continue;
        }
        let s = grove.scale[c];
        let pose = Transform {
            position: grove.at(c as u32),
            rotation: Quat::from_rotation_y(grove.turn[c]),
            scale: Vec3::splat(s),
        };
        let tree = Tree { cell: c as u32 };
        let trunk = if primitives {
            let trunk = w.spawn((
                Transform {
                    position: pose.position + Vec3::Y * 1.6 * s,
                    ..pose
                },
                Mesh::cylinder(0.35, 3.2),
                Material::rgb(0.24, 0.15, 0.08).rough(0.9),
                tree,
            ));
            grove.crown[c] = w.spawn((
                Transform {
                    position: pose.position + Vec3::Y * 5.0 * s,
                    ..pose
                },
                Mesh::sphere(2.0),
                Material::rgb(0.07, 0.2, 0.08).rough(0.95),
            ));
            trunk
        } else {
            w.spawn((
                pose,
                Mesh::asset(KINDS[grove.kind[c] as usize].0),
                tree,
                Visible(true),
            ))
        };
        if colliders {
            w.insert(
                trunk,
                Collider {
                    shape: Shape::Cylinder {
                        radius: TRUNK,
                        height: 6.0,
                    },
                    offset: if primitives {
                        Vec3::ZERO
                    } else {
                        Vec3::Y * 3.0
                    },
                    ..Default::default()
                },
            );
        }
        grove.trunk[c] = trunk;
    }
    // Undergrowth: about half the cells carry a fern, bush, tuft or rock, kept
    // off the trunks; tufts thicken toward the clearing's edge.
    let under = UNDER.map(|(name, share, lo, hi)| (name, share, (lo, hi)));
    for c in 0..n {
        let (i, j) = ((c as u32 % side) as f32, (c as u32 / side) as f32);
        let (cx, cz) = (-half + (i + 0.5) * CELL, -half + (j + 0.5) * CELL);
        let r = (cx * cx + cz * cz).sqrt();
        if r < 7.0 || cx.abs() > half - CELL || cz.abs() > half - CELL || !w.chance(0.5) {
            continue;
        }
        let x = cx + w.rand(-2.2..2.2);
        let z = cz + w.rand(-2.2..2.2);
        let (_, name, (lo, hi)) = pick(w, &under);
        let s = w.rand(lo..hi);
        let turn = w.rand(0.0..std::f32::consts::TAU);
        let (x, z) = grove.resolve(x, z, 0.7);
        w.spawn((
            Transform {
                position: Vec3::new(x, height(x, z) - 0.03, z),
                rotation: Quat::from_rotation_y(turn),
                scale: Vec3::splat(s),
            },
            Mesh::asset(name),
            Ambient,
        ));
    }
    w.insert_resource(grove);
}

/// Forest-floor colour: moss, bare soil and leaf litter in drifting patches,
/// worn earth around the fire. Linear vertex colours.
fn ground_color(x: f32, y: f32, z: f32) -> [f32; 4] {
    let n1 = math::sin(x * 0.31 + z * 0.17) * math::sin(x * 0.13 - z * 0.29 + 1.3);
    let n2 = math::sin(x * 0.9 + 2.0 * math::sin(z * 0.23)) * math::cos(z * 0.77 - x * 0.11);
    let moss = [0.03, 0.09, 0.018];
    let soil = [0.055, 0.036, 0.02];
    let litter = [0.13, 0.07, 0.022];
    let a = math::smoothstep(-0.3, 0.6, n1);
    let b = math::smoothstep(0.2, 0.9, n2) * 0.7;
    let mut c = [0.0; 3];
    for k in 0..3 {
        c[k] = (moss[k] * a + soil[k] * (1.0 - a)) * (1.0 - b) + litter[k] * b;
    }
    let r = (x * x + z * z).sqrt();
    let worn = 1.0 - math::smoothstep(3.0, 9.0, r);
    let lift = 1.0 + y * 0.05;
    [
        (c[0] * (1.0 - worn) + 0.07 * worn) * lift,
        (c[1] * (1.0 - worn) + 0.05 * worn) * lift,
        (c[2] * (1.0 - worn) + 0.035 * worn) * lift,
        1.0,
    ]
}

/// Wind: the standing trees within `reach` of a point lean and recover on slow,
/// position-shifted waves. Trees outside it keep their last lean. Trees standing
/// between the camera (behind and above the player, toward +Z) and the player are
/// hidden so the player is never lost behind a crown; nothing fades, it is a cut.
pub fn sway(w: &World, around: Vec3, reach: f32) {
    let g = w.resource::<Grove>();
    let t = w.tick_end().seconds() as f32;
    let span = (reach / CELL).ceil() as i32;
    let Some((ci, cj)) = g.cell_of(around.x, around.z) else {
        return;
    };
    let n = g.side as i32;
    for j in (cj - span).max(0)..=(cj + span).min(n - 1) {
        for i in (ci - span).max(0)..=(ci + span).min(n - 1) {
            let c = (j * n + i) as usize;
            if g.hp[c] == 0 {
                continue;
            }
            let (x, z) = (g.x[c], g.z[c]);
            let gust = 0.6 + 0.4 * math::sin(t * 0.37 + x * 0.02);
            let lean = 0.022 * gust * math::sin(t * 1.3 + x * 0.11 + z * 0.07);
            let side = 0.012 * gust * math::sin(t * 0.9 + z * 0.13 + 1.7);
            let (dx, dz) = (x - around.x, z - around.z);
            let between = dz > -1.5 && dz < 9.0 && dx.abs() < 2.2 + dz * 0.15;
            // Read first: even an unwritten mutable borrow tells the renderer to rebatch.
            if w.get::<Visible>(g.trunk[c]).is_some_and(|v| v.0 == between) {
                w.get_mut::<Visible>(g.trunk[c]).unwrap().0 = !between;
            }
            if let Some(mut pose) = w.get_mut::<Transform>(g.trunk[c]) {
                pose.rotation = Quat::from_rotation_x(lean)
                    * Quat::from_rotation_z(side)
                    * Quat::from_rotation_y(g.turn[c]);
            }
        }
    }
}

/// Fell one tree: remove its entities and collider. Returns the stump position.
pub fn fell(w: &mut World, cell: u32) -> Vec3 {
    let (at, trunk, crown) = {
        let mut g = w.resource_mut::<Grove>();
        let c = cell as usize;
        g.hp[c] = 0;
        g.standing -= 1;
        let at = Vec3::new(g.x[c], 0.0, g.z[c]);
        let pair = (g.trunk[c], g.crown[c]);
        g.trunk[c] = Entity::default();
        g.crown[c] = Entity::default();
        (at, pair.0, pair.1)
    };
    w.despawn(trunk);
    if w.contains(crown) {
        w.despawn(crown);
    }
    Vec3::new(at.x, height(at.x, at.z), at.z)
}
