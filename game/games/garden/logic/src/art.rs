//! Authored low-poly garden models. One immutable mesh per crop keeps a full
//! field instanced; scenery adds a fixed number of entities, never one per tile.
use crate::{
    crops::{balance, Crop},
    garden::paint,
};
use exact_game::{
    asset::{MeshBuilder, MeshData},
    *,
};

/// The classic look's shapes: flat facets over the engine's builder, every
/// colour authored and stored squared.
trait Classic {
    fn quad(&mut self, p: [Vec3; 4], color: [f32; 3]);
    fn block(&mut self, at: Vec3, size: Vec3, color: [f32; 3]);
    fn cone(&mut self, at: Vec3, bottom: f32, top: f32, height: f32, color: [f32; 3]);
    fn globe(&mut self, at: Vec3, radius: Vec3, color: [f32; 3]);
    fn leaf(&mut self, root: Vec3, tip: Vec3, width: f32, color: [f32; 3]);
}

fn model() -> MeshBuilder {
    MeshBuilder::flat().squared()
}

impl Classic for MeshBuilder {
    fn quad(&mut self, p: [Vec3; 4], color: [f32; 3]) {
        self.facet(p[0], p[1], p[2], color);
        self.facet(p[0], p[2], p[3], color);
    }

    fn block(&mut self, at: Vec3, size: Vec3, color: [f32; 3]) {
        self.cuboid(at, size, Quat::IDENTITY, |_| color);
    }

    fn cone(&mut self, at: Vec3, bottom: f32, top: f32, height: f32, color: [f32; 3]) {
        let ring = |k: u32, radius: f32, y: f32| {
            let (s, c) = math::sin_cos(k as f32 * std::f32::consts::TAU / 10.0);
            at + Vec3::new(c * radius, y, s * radius)
        };
        for k in 0..10 {
            let (a, b) = (ring(k, bottom, 0.0), ring(k + 1, bottom, 0.0));
            let (c, d) = (ring(k, top, height), ring(k + 1, top, height));
            self.facet(a, c, b, color);
            if top > 0.0 {
                self.facet(b, c, d, color);
                self.facet(at + Vec3::Y * height, d, c, color);
            }
            self.facet(at, a, b, color);
        }
    }

    fn globe(&mut self, at: Vec3, radius: Vec3, color: [f32; 3]) {
        self.ellipsoid(at, radius, Quat::IDENTITY, (6, 10), |_| color);
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
            self.facet(a, b, c, color);
            self.facet(c, b, a, color.map(|v| v * 0.82));
        }
    }
}

fn gardener() -> MeshData {
    let mut m = model();
    let (blue, shirt, skin, straw) = (
        [0.20, 0.43, 0.52],
        [0.95, 0.81, 0.48],
        [0.89, 0.62, 0.43],
        [0.89, 0.71, 0.38],
    );
    for x in [-0.18, 0.18] {
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
    m.block(
        Vec3::new(0.32, -0.16, -0.21),
        Vec3::new(0.29, 0.39, 0.27),
        [0.51, 0.32, 0.19],
    );
    m.block(
        Vec3::new(0.32, 0.02, -0.22),
        Vec3::new(0.32, 0.10, 0.30),
        [0.65, 0.45, 0.25],
    );
    m.finish()
}

fn arm() -> MeshData {
    let mut m = model();
    m.block(
        Vec3::new(0., -0.23, 0.),
        Vec3::new(0.19, 0.47, 0.25),
        [0.95, 0.81, 0.48],
    );
    m.globe(
        Vec3::new(0., -0.49, 0.02),
        Vec3::splat(0.13),
        [0.89, 0.62, 0.43],
    );
    m.finish()
}

fn leg() -> MeshData {
    let mut m = model();
    m.block(
        Vec3::new(0., -0.20, 0.),
        Vec3::new(0.27, 0.42, 0.30),
        [0.20, 0.43, 0.52],
    );
    m.block(
        Vec3::new(0., -0.49, 0.08),
        Vec3::new(0.29, 0.25, 0.43),
        [0.29, 0.20, 0.14],
    );
    m.finish()
}

fn watering_can() -> MeshData {
    let mut m = model();
    let blue = [0.24, 0.62, 0.76];
    m.cone(Vec3::new(0., -0.20, 0.), 0.20, 0.18, 0.30, blue);
    m.block(
        Vec3::new(0., -0.06, 0.27),
        Vec3::new(0.075, 0.075, 0.35),
        blue,
    );
    m.globe(
        Vec3::new(0., -0.06, 0.45),
        Vec3::new(0.12, 0.035, 0.08),
        [0.69, 0.83, 0.81],
    );
    for x in [-0.12, 0.12] {
        m.block(Vec3::new(x, 0.18, 0.), Vec3::new(0.045, 0.25, 0.055), blue);
    }
    m.block(Vec3::new(0., 0.29, 0.), Vec3::new(0.28, 0.045, 0.055), blue);
    m.finish()
}

fn plant(c: &Crop) -> MeshData {
    let mut m = model();
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

fn fruit(c: &Crop) -> MeshData {
    let mut m = model();
    let r = c.fruit_size;
    match c.id.as_str() {
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
    let mut m = model();
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
    let mut m = model();
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

/// Every look's world at once, so the look switches live (`art` is a live
/// argument). What every look draws (the gardener, crops, ground, meadow,
/// barrel) keeps the classic look's models in the simulation, registered
/// here; a prop only some looks draw is a bare pose that each look's
/// `present` dresses, with models it makes the first time it is shown.
pub fn setup(w: &mut World) {
    let body = w
        .generated("gardener.model", gardener())
        .expect("gardener mesh");
    let player = w.named("player").unwrap();
    w.remove::<Mesh>(player);
    w.remove::<Material>(player);
    let body = w.spawn_named(
        "gardener",
        (
            Transform::default(),
            body,
            Material::default(),
            Parent(player),
        ),
    );
    let arm = w.generated("gardener-arm.model", arm()).expect("arm mesh");
    let leg = w.generated("gardener-leg.model", leg()).expect("leg mesh");
    for (name, x, y, mesh) in [
        ("arm-left", -0.38, 0.18, arm.clone()),
        ("arm-right", 0.38, 0.18, arm),
        ("leg-left", -0.18, -0.28, leg.clone()),
        ("leg-right", 0.18, -0.28, leg),
    ] {
        w.spawn_named(
            name,
            (
                Transform::at(x, y, 0.),
                mesh,
                Material::default(),
                Parent(body),
            ),
        );
    }
    let can = w
        .generated("watering-can.model", watering_can())
        .expect("can mesh");
    w.spawn_named(
        "watering-can",
        (
            Transform::at(0., -0.47, 0.16).with_scale(1.25),
            can,
            Material::default(),
            Parent(w.named("arm-left").unwrap()),
            Visible(false),
        ),
    );
    for (kind, c) in balance(w).crops.iter().enumerate() {
        w.generated(&format!("plant-{kind}.model"), plant(c))
            .expect("plant mesh");
        w.generated(&format!("fruit-{kind}.model"), fruit(c))
            .expect("fruit mesh");
    }
    // Every look dresses the meadow (`present`).
    w.spawn_named(
        "meadow",
        (
            Transform::at(0., -0.10, 0.),
            Mesh::plane(1600., 1600.),
            Material::default(),
        ),
    );
    // Classic props: bare poses `classic` dresses.
    for (name, position) in [
        ("orchard", Vec3::ZERO),
        ("flowers-west", Vec3::new(-4.0, 0., 1.9)),
        ("flowers-east", Vec3::new(8.0, 0., 1.9)),
        ("bed-edge", Vec3::ZERO),
    ] {
        w.spawn_named(
            name,
            (Transform::at(position.x, position.y, position.z), Ambient),
        );
    }
    // A second directional light after the sun: the art pass's moon, the
    // other looks' fill. Unlit in the classic look.
    w.spawn_named("moon", (Transform::default(), Ambient));
    crate::looks::setup(w);
    crate::pass::setup(w);
}

/// What `present` draws for each value of `art`.
pub fn present(p: &mut Present, args: &crate::Options) {
    match args.art.as_str() {
        "pass" => crate::pass::present(p, args.smooth),
        art => match crate::looks::style(art) {
            Some(style) => crate::looks::present(p, style),
            None => classic(p),
        },
    }
}

/// The classic look: the simulation's own models, its sky, sun, soil and
/// meadow from the looks data, and its props dressed.
fn classic(p: &mut Present) {
    let looks = crate::looks::looks(p);
    let lit = &looks.classic;
    if let Some(camera) = p.named("camera") {
        p.insert(
            camera,
            DrawnEnvironment {
                environment: lit.environment,
                ambient_occlusion: Some(lit.occlusion),
            },
        );
    }
    if let Some(sun) = p.named("sun") {
        p.insert(
            sun,
            DrawnLight::Directional(DirectionalLight {
                color: lit.sun.color,
                illuminance: lit.sun.illuminance,
                shadows: true,
            }),
        );
    }
    if let Some(ground) = p.named("ground") {
        let plane = p.get::<Mesh>(ground).map(|m| m.clone()).unwrap_or_default();
        let soil = Material::grid(lit.soil, crate::garden::TILE);
        p.insert(ground, DrawnMesh::new(plane).material(soil));
    }
    dress(
        p,
        "meadow",
        DrawnMesh::new(Mesh::plane(1600., 1600.)).material(paint(lit.meadow)),
    );
    let orchard = p.generated("orchard.model", orchard).expect("orchard mesh");
    let flowers = p
        .generated("flowers.model", flower_bank)
        .expect("flower mesh");
    dress(p, "orchard", DrawnMesh::new(orchard));
    dress(p, "flowers-west", DrawnMesh::new(flowers.clone()));
    dress(p, "flowers-east", DrawnMesh::new(flowers));
    if let Some(bed) = bed(p) {
        dress(p, "bed-edge", bed.material(paint(lit.bed)));
    }
    hide(p, "weather");
    hide(p, "ambience-golden");
    hide(p, "ambience-storybook");
}

/// The raised bed's border box around the garden's plots.
pub(crate) fn bed(p: &Present) -> Option<DrawnMesh> {
    let span = p.resource::<crate::farm::Farm>()?.size as f32 * crate::garden::TILE;
    Some(DrawnMesh::new(Mesh::cuboid(Vec3::new(
        span + 4.2,
        0.25,
        span + 4.2,
    ))))
}

/// Draw `drawn` on the named entity, if it is there.
pub(crate) fn dress(p: &mut Present, name: &str, drawn: DrawnMesh) {
    if let Some(e) = p.named(name) {
        p.insert(e, drawn);
    }
}

/// The named entity draws nothing in this look (an emitter of another look).
pub(crate) fn hide(p: &mut Present, name: &str) {
    if let Some(e) = p.named(name) {
        p.insert(e, Opacity(0.0));
    }
}

/// Draw an entity at `to` in place of its own local pose: the `Offset` that
/// takes its local pose there.
pub(crate) fn place(p: &mut Present, e: Entity, to: Transform) {
    let Some(local) = p.get::<Transform>(e).map(|t| *t) else {
        return;
    };
    // Crops are unturned and uniformly scaled: local⁻¹ · to in closed form.
    let s = local.scale.x;
    let offset = if local.rotation == Quat::IDENTITY && local.scale == Vec3::splat(s) && s != 0.0 {
        Transform {
            position: (to.position - local.position) / s,
            rotation: to.rotation,
            scale: to.scale / s,
        }
    } else {
        let affine = |t: Transform| {
            Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position)
        };
        let (scale, rotation, position) =
            (affine(local).inverse() * affine(to)).to_scale_rotation_translation();
        Transform {
            position,
            rotation,
            scale,
        }
    };
    p.insert(e, Offset(offset));
}

/// Places every look's props with the garden: the classic and styled borders,
/// the art pass's trees.
pub fn resize(w: &World, span: f32, mid: f32) {
    w.require_mut::<Transform>("bed-edge").position = Vec3::new(mid, -0.18, -mid);
    w.require_mut::<Transform>("orchard").position = Vec3::new(0., 0., -span - 2.0);
    crate::looks::resize(w, span, mid);
    crate::pass::resize(w, span);
}

/// After the garden grows: the art pass's fence is rebuilt around it.
pub fn regrow(w: &mut World) {
    crate::pass::build_fence(w);
}
