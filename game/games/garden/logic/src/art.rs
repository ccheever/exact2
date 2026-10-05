//! Authored low-poly garden models. One immutable mesh per crop keeps a full
//! field instanced; scenery adds a fixed number of entities, never one per tile.
use crate::{crops::CROPS, garden::paint};
use exact_game::{asset::MeshData, *};

#[derive(Default)]
struct Model(MeshData);

impl Model {
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let i = (self.0.positions.len() / 3) as u32;
        for p in [a, b, c] {
            self.0.positions.extend(p.to_array());
            self.0.normals.extend(n.to_array());
            self.0.uvs.extend([0.0, 0.0]);
            self.0.colors.extend([
                color[0] * color[0],
                color[1] * color[1],
                color[2] * color[2],
                1.0,
            ]);
        }
        self.0.indices.extend([i, i + 1, i + 2]);
    }

    fn quad(&mut self, p: [Vec3; 4], color: [f32; 3]) {
        self.tri(p[0], p[1], p[2], color);
        self.tri(p[0], p[2], p[3], color);
    }

    fn block(&mut self, at: Vec3, size: Vec3, color: [f32; 3]) {
        let p = |x, y, z| at + size * Vec3::new(x, y, z) * 0.5;
        for face in [
            [
                p(-1., -1., 1.),
                p(1., -1., 1.),
                p(1., 1., 1.),
                p(-1., 1., 1.),
            ],
            [
                p(1., -1., -1.),
                p(-1., -1., -1.),
                p(-1., 1., -1.),
                p(1., 1., -1.),
            ],
            [
                p(1., -1., 1.),
                p(1., -1., -1.),
                p(1., 1., -1.),
                p(1., 1., 1.),
            ],
            [
                p(-1., -1., -1.),
                p(-1., -1., 1.),
                p(-1., 1., 1.),
                p(-1., 1., -1.),
            ],
            [
                p(-1., 1., 1.),
                p(1., 1., 1.),
                p(1., 1., -1.),
                p(-1., 1., -1.),
            ],
            [
                p(-1., -1., -1.),
                p(1., -1., -1.),
                p(1., -1., 1.),
                p(-1., -1., 1.),
            ],
        ] {
            self.quad(face, color);
        }
    }

    fn cone(&mut self, at: Vec3, bottom: f32, top: f32, height: f32, color: [f32; 3]) {
        let ring = |k: u32, radius: f32, y: f32| {
            let (s, c) = math::sin_cos(k as f32 * std::f32::consts::TAU / 10.0);
            at + Vec3::new(c * radius, y, s * radius)
        };
        for k in 0..10 {
            let (a, b) = (ring(k, bottom, 0.0), ring(k + 1, bottom, 0.0));
            let (c, d) = (ring(k, top, height), ring(k + 1, top, height));
            self.tri(a, c, b, color);
            if top > 0.0 {
                self.tri(b, c, d, color);
                self.tri(at + Vec3::Y * height, d, c, color);
            }
            self.tri(at, a, b, color);
        }
    }

    fn globe(&mut self, at: Vec3, radius: Vec3, color: [f32; 3]) {
        let point = |j: u32, k: u32| {
            let (sy, cy) = math::sin_cos(j as f32 * std::f32::consts::PI / 6.0);
            let (s, c) = math::sin_cos(k as f32 * std::f32::consts::TAU / 10.0);
            at + radius * Vec3::new(c * sy, cy, s * sy)
        };
        for j in 0..6 {
            for k in 0..10 {
                let (a, b, c, d) = (
                    point(j, k),
                    point(j, k + 1),
                    point(j + 1, k),
                    point(j + 1, k + 1),
                );
                if j > 0 {
                    self.tri(a, b, c, color);
                }
                if j < 5 {
                    self.tri(b, d, c, color);
                }
            }
        }
    }

    fn leaf(&mut self, root: Vec3, tip: Vec3, width: f32, color: [f32; 3]) {
        let axis = tip - root;
        let side = axis.cross(Vec3::Y).normalize_or_zero() * width;
        let mid = root.lerp(tip, 0.48) + Vec3::Y * width * 0.35;
        let left = mid + side;
        let right = mid - side;
        let ridge = mid + Vec3::Y * width * 0.3;
        for (a, b, c) in [
            (root, ridge, left),
            (left, ridge, tip),
            (tip, ridge, right),
            (right, ridge, root),
        ] {
            self.tri(a, b, c, color);
            self.tri(c, b, a, color.map(|v| v * 0.82));
        }
    }

    fn finish(mut self) -> MeshData {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in self.0.positions.chunks_exact(3) {
            let p = Vec3::new(p[0], p[1], p[2]);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        self.0.bounds = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
        self.0
    }
}

fn gardener() -> MeshData {
    let mut m = Model::default();
    let (blue, shirt, skin, straw) = (
        [0.20, 0.43, 0.52],
        [0.95, 0.81, 0.48],
        [0.89, 0.62, 0.43],
        [0.89, 0.71, 0.38],
    );
    for x in [-0.18, 0.18] {
        m.block(
            Vec3::new(x, -0.77, 0.08),
            Vec3::new(0.29, 0.25, 0.43),
            [0.29, 0.20, 0.14],
        );
        m.block(Vec3::new(x, -0.48, 0.0), Vec3::new(0.27, 0.42, 0.30), blue);
        m.block(
            Vec3::new(x * 2.1, -0.05, 0.0),
            Vec3::new(0.19, 0.47, 0.25),
            shirt,
        );
        m.globe(Vec3::new(x * 2.1, -0.31, 0.02), Vec3::splat(0.13), skin);
        m.block(Vec3::new(x, 0.06, 0.21), Vec3::new(0.09, 0.40, 0.035), blue);
        m.globe(Vec3::new(x, 0.04, 0.24), Vec3::splat(0.035), straw);
    }
    m.block(Vec3::new(0., -0.04, 0.), Vec3::new(0.61, 0.61, 0.39), shirt);
    m.block(
        Vec3::new(0., -0.22, 0.22),
        Vec3::new(0.53, 0.29, 0.06),
        blue,
    );
    m.block(
        Vec3::new(0., -0.19, 0.26),
        Vec3::new(0.20, 0.14, 0.025),
        [0.29, 0.53, 0.61],
    );
    m.globe(Vec3::new(0., 0.44, 0.), Vec3::new(0.30, 0.34, 0.28), skin);
    m.globe(
        Vec3::new(0., 0.43, 0.28),
        Vec3::new(0.07, 0.055, 0.045),
        skin,
    );
    for x in [-0.105, 0.105] {
        m.block(
            Vec3::new(x, 0.50, 0.257),
            Vec3::new(0.039, 0.055, 0.025),
            [0.15, 0.17, 0.13],
        );
    }
    m.block(
        Vec3::new(0., 0.33, 0.261),
        Vec3::new(0.10, 0.022, 0.018),
        [0.48, 0.25, 0.18],
    );
    m.cone(Vec3::new(0., 0.69, 0.), 0.48, 0.49, 0.055, straw);
    m.cone(Vec3::new(0., 0.74, 0.), 0.30, 0.25, 0.20, straw);
    m.cone(
        Vec3::new(0., 0.74, 0.),
        0.305,
        0.292,
        0.055,
        [0.38, 0.29, 0.17],
    );
    m.finish()
}

fn plant(kind: usize) -> MeshData {
    let c = &CROPS[kind];
    let mut m = Model::default();
    let base = -c.height * 0.5;
    let woody = c.height >= 2.0;
    let stem = if woody {
        [0.44, 0.29, 0.17]
    } else {
        c.leaf.map(|v| v * 0.72)
    };
    m.cone(
        Vec3::Y * base,
        if woody { 0.16 } else { 0.05 },
        0.035,
        c.height,
        stem,
    );
    let leaves = if woody { 9 } else { 7 };
    for i in 0..leaves {
        let angle = i as f32 * 2.4;
        let (s, co) = math::sin_cos(angle);
        let y = base + c.height * (0.25 + 0.65 * i as f32 / leaves as f32);
        let spread = if woody { 0.9 } else { 0.40 + c.height * 0.25 };
        let root = Vec3::new(0.0, y, 0.0);
        let tip = root + Vec3::new(co * spread, 0.18 + c.height * 0.18, s * spread);
        m.leaf(
            root,
            tip,
            spread * 0.32,
            c.leaf
                .map(|v| (v * (0.92 + (i % 3) as f32 * 0.06)).min(1.0)),
        );
    }
    if woody && c.id != "bamboo" {
        for i in 0..5 {
            let (s, co) = math::sin_cos(i as f32 * 1.2566371);
            m.globe(
                Vec3::new(co * 0.38, c.height * 0.39, s * 0.38),
                Vec3::new(0.65, 0.42, 0.65),
                c.leaf,
            );
        }
    }
    m.finish()
}

fn fruit(kind: usize) -> MeshData {
    let c = &CROPS[kind];
    let mut m = Model::default();
    let r = c.fruit_size;
    match c.id {
        "carrot" => {
            m.cone(Vec3::Y * (-r * 1.2), r * 0.08, r * 0.70, r * 2.3, [1.0; 3]);
            for y in [-0.45, 0.2, 0.65] {
                m.block(
                    Vec3::new(0., r * y, r * (0.34 + y * 0.16)),
                    Vec3::new(r * 0.55, r * 0.06, r * 0.09),
                    [0.75; 3],
                );
            }
        }
        "strawberry" => {
            m.cone(Vec3::Y * (-r * 1.1), r * 0.06, r * 0.82, r * 1.35, [1.0; 3]);
            m.globe(
                Vec3::Y * (r * 0.25),
                Vec3::new(r * 0.82, r * 0.48, r * 0.82),
                [1.0; 3],
            );
        }
        "banana" => {
            for i in 0..4 {
                let t = i as f32 / 3.0;
                m.globe(
                    Vec3::new((t - 0.5) * r * 2.3, t * t * r * 0.65, 0.0),
                    Vec3::new(r * 0.48, r * 0.36, r * 0.38),
                    [1.0; 3],
                );
            }
        }
        "pumpkin" => {
            for i in 0..8 {
                let (s, co) = math::sin_cos(i as f32 * std::f32::consts::TAU / 8.0);
                m.globe(
                    Vec3::new(co * r * 0.44, 0., s * r * 0.44),
                    Vec3::new(r * 0.64, r * 0.82, r * 0.64),
                    [1.0; 3],
                );
            }
            m.cone(Vec3::Y * r * 0.65, r * 0.12, r * 0.07, r * 0.5, [0.42; 3]);
        }
        "watermelon" => m.globe(Vec3::ZERO, Vec3::new(r * 1.2, r * 0.85, r * 0.85), [1.0; 3]),
        _ => m.globe(Vec3::ZERO, Vec3::splat(r), [1.0; 3]),
    }
    m.finish()
}

fn orchard() -> MeshData {
    let mut m = Model::default();
    for (x, z, size) in [
        (-5., -5., 1.4),
        (2., -4., 1.1),
        (8., -6., 1.7),
        (15., -3., 1.3),
        (23., -7., 1.8),
    ] {
        let p = Vec3::new(x, 0., z);
        m.cone(p, 0.24 * size, 0.16 * size, 2.6 * size, [0.42, 0.29, 0.17]);
        for (dx, dy, dz, r) in [
            (-0.55, 2.7, 0., 1.3),
            (0.60, 2.9, 0.1, 1.25),
            (0., 3.7, -0.1, 1.35),
        ] {
            m.globe(
                p + Vec3::new(dx, dy, dz) * size,
                Vec3::splat(r * size),
                [0.29 + dy * 0.018, 0.58, 0.26],
            );
        }
        for i in 0..5 {
            let (s, c) = math::sin_cos(i as f32 * 1.2566371);
            m.globe(
                p + Vec3::new(c * 1.1, 2.8 + s * 0.2, s * 1.1) * size,
                Vec3::splat(0.18 * size),
                [0.87, 0.33, 0.21],
            );
        }
    }
    for i in 0..25 {
        let x = i as f32 * 1.1 - 5.0;
        m.block(
            Vec3::new(x, 0.60, 0.),
            Vec3::new(0.11, 1.2, 0.15),
            [0.90, 0.86, 0.69],
        );
        m.cone(Vec3::new(x, 1.2, 0.), 0.08, 0.0, 0.12, [0.90, 0.86, 0.69]);
    }
    for y in [0.35, 0.88] {
        m.block(
            Vec3::new(8.2, y, 0.),
            Vec3::new(26.4, 0.11, 0.10),
            [0.80, 0.77, 0.59],
        );
    }
    m.finish()
}

fn flower_bank() -> MeshData {
    let mut m = Model::default();
    for i in 0..36 {
        let x = (i % 12) as f32 * 0.7;
        let z = (i / 12) as f32 * 0.75;
        let (s, c) = math::sin_cos(i as f32 * 2.4);
        let root = Vec3::new(x + c * 0.15, 0., z + s * 0.15);
        let h = 0.20 + (i % 3) as f32 * 0.08;
        m.cone(root, 0.025, 0.015, h, [0.26, 0.48, 0.19]);
        let color = match i % 4 {
            0 => [0.94, 0.72, 0.25],
            1 => [0.91, 0.48, 0.48],
            2 => [0.73, 0.59, 0.84],
            _ => [0.97, 0.91, 0.74],
        };
        for j in 0..5 {
            let (s, c) = math::sin_cos(j as f32 * 1.2566371);
            let center = root + Vec3::Y * h;
            let axis = Vec3::new(c, 0.0, s);
            let side = Vec3::new(-s, 0.0, c) * 0.055;
            m.quad(
                [
                    center,
                    center + axis * 0.08 + side,
                    center + axis * 0.16,
                    center + axis * 0.08 - side,
                ],
                color,
            );
        }
        m.cone(root + Vec3::Y * h, 0.045, 0.025, 0.045, [0.95, 0.75, 0.24]);
    }
    m.finish()
}

pub fn setup(w: &mut World) {
    let player = w
        .generated("gardener.model", gardener())
        .expect("gardener mesh");
    *w.require_mut::<Mesh>("player") = player;
    *w.require_mut::<Material>("player") = Material::default();
    for kind in 0..CROPS.len() {
        w.generated(&format!("plant-{kind}.model"), plant(kind))
            .expect("plant mesh");
        w.generated(&format!("fruit-{kind}.model"), fruit(kind))
            .expect("fruit mesh");
    }
    let trees = w
        .generated("orchard.model", orchard())
        .expect("orchard mesh");
    let flowers = w
        .generated("flowers.model", flower_bank())
        .expect("flower mesh");
    for (name, position, mesh, material) in [
        (
            "meadow",
            Vec3::new(0., -0.10, 0.),
            Mesh::plane(1600., 1600.),
            paint([0.49, 0.68, 0.34]),
        ),
        ("orchard", Vec3::ZERO, trees, Material::default()),
        (
            "flowers-west",
            Vec3::new(-4.0, 0., 1.9),
            flowers.clone(),
            Material::default(),
        ),
        (
            "flowers-east",
            Vec3::new(8.0, 0., 1.9),
            flowers,
            Material::default(),
        ),
        (
            "bed-edge",
            Vec3::ZERO,
            Mesh::cube(1.),
            paint([0.51, 0.34, 0.21]),
        ),
    ] {
        w.spawn_named(
            name,
            (
                Transform::at(position.x, position.y, position.z),
                mesh,
                material,
            ),
        );
    }
}

pub fn resize(w: &World, span: f32, mid: f32) {
    *w.require_mut::<Mesh>("bed-edge") = Mesh::cuboid(Vec3::new(span + 4.2, 0.25, span + 4.2));
    w.require_mut::<Transform>("bed-edge").position = Vec3::new(mid, -0.18, -mid);
    w.require_mut::<Transform>("orchard").position = Vec3::new(0., 0., -span - 2.0);
}
