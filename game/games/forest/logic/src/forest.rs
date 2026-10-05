//! The forest: rolling terrain, a jittered grid of trees and the grid lookups
//! that collision and navigation use instead of scanning every tree.
use exact_game::asset::{MeshBuilder, MeshData};
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
    pub standing: u32,
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
/// `trees` trees outside the clearing. The art pass (`pass`) changes only how
/// they are drawn: the same colliders, cells and random draws in the same order.
pub fn grow(w: &mut World, trees: u32, primitives: bool, colliders: bool, pass: bool) {
    let half = world_half(trees);
    let side = (2.0 * half / CELL) as u32;
    let half = side as f32 * CELL * 0.5;
    let n = (side * side) as usize;
    let heightfield = |spacing: f32| {
        let samples = (2.0 * half / spacing) as u32 + 1;
        let heights: Vec<f32> = (0..samples * samples)
            .map(|k| {
                let (c, r) = (k % samples, k / samples);
                height(
                    -half + c as f32 * spacing * (2.0 * half) / ((samples - 1) as f32 * spacing),
                    -half + r as f32 * spacing * (2.0 * half) / ((samples - 1) as f32 * spacing),
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
    let (mut mesh, shape) = heightfield(SAMPLE);
    let ground = if pass {
        // Drawn every 2 m where the world is small enough to afford it, so the
        // floor's patches read at walking scale; the collider keeps 4 m.
        let drawn = if half <= 400.0 {
            heightfield(2.0).0
        } else {
            mesh
        };
        crate::art::terrain(w, drawn)
    } else {
        mesh.colors = mesh
            .positions
            .chunks_exact(3)
            .flat_map(|p| {
                let k = (p[1] * 0.25 + 0.5).clamp(0.0, 1.0);
                [0.05 + 0.04 * k, 0.11 + 0.07 * k, 0.04 + 0.02 * k, 1.0]
            })
            .collect();
        w.generated("terrain.model", mesh).expect("terrain model")
    };
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
    // The art pass draws five kinds of baked tree, each with a generated far level.
    let looks = (pass && !primitives).then(|| crate::art::tree_looks(w));
    let pine = looks
        .is_none()
        .then(|| w.generated("pine.model", pine()).expect("pine model"));
    let mut grove = Grove {
        side,
        half,
        x: vec![0.0; n],
        z: vec![0.0; n],
        scale: vec![0.0; n],
        hp: vec![0; n],
        trunk: vec![Entity::default(); n],
        crown: vec![Entity::default(); n],
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
    let mut turns = vec![0.0; n];
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
        turns[c] = w.rand(0.0..std::f32::consts::TAU);
    }
    for (c, &turn) in turns.iter().enumerate() {
        if grove.hp[c] == 0 {
            continue;
        }
        let s = grove.scale[c];
        let pose = Transform {
            position: grove.at(c as u32),
            rotation: Quat::from_rotation_y(turn),
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
        } else if let Some(looks) = &looks {
            let (mesh, lod) = looks.of(c as u32);
            w.spawn((pose, mesh, lod, tree))
        } else {
            w.spawn((pose, pine.clone().expect("generated pine"), tree))
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
    w.insert_resource(grove);
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

/// A low-poly pine: a hexagonal trunk and two cones, flat-shaded with vertex colours.
pub fn pine() -> MeshData {
    let mut m = MeshBuilder::flat();
    let bark = [0.22, 0.13, 0.07];
    let (dark, light) = ([0.04, 0.16, 0.06], [0.07, 0.24, 0.08]);
    frustum(&mut m, 6, 0.0, 0.32, 2.2, 0.26, bark);
    frustum(&mut m, 8, 1.6, 2.1, 5.2, 0.0, dark);
    frustum(&mut m, 8, 3.8, 1.5, 7.4, 0.0, light);
    m.finish()
}

/// Append an open-ended frustum (a cone when `r1` is zero) with a closed base.
pub(crate) fn frustum(
    m: &mut MeshBuilder,
    sides: u32,
    y0: f32,
    r0: f32,
    y1: f32,
    r1: f32,
    color: [f32; 3],
) {
    let ring = |k: u32, r: f32, y: f32| {
        let (s, c) = math::sin_cos(k as f32 / sides as f32 * std::f32::consts::TAU);
        Vec3::new(c * r, y, s * r)
    };
    for k in 0..sides {
        let (a, b) = (ring(k, r0, y0), ring(k + 1, r0, y0));
        let (c, d) = (ring(k, r1, y1), ring(k + 1, r1, y1));
        m.facet(a, c, b, color);
        if r1 > 0.0 {
            m.facet(b, c, d, color);
        }
        m.facet(Vec3::new(0.0, y0, 0.0), a, b, color);
    }
}
