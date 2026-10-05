//! Alternative looks for the garden, chosen by the `art` argument at setup:
//! **golden** (late-afternoon light, long shadows, a warm haze and drifting
//! pollen) and **storybook** (bright, soft and toy-like). Both keep the
//! classic look's entity names, asset names and counts, so the game, its
//! gestures and its scale are unchanged; only meshes, materials, lights and
//! a fixed handful of scenery entities differ.
use crate::{
    crops::{Crop, CROPS},
    garden::paint,
    models::*,
    scenery::*,
    sculpt::*,
};
use exact_game::{emitter, *};
use std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Golden,
    Storybook,
}

pub fn style(name: &str) -> Option<Style> {
    match name {
        "golden" => Some(Style::Golden),
        "storybook" => Some(Style::Storybook),
        _ => None,
    }
}

/// Everything a look paints with. Colours are authored sRGB-ish.
pub(crate) struct Look {
    pub(crate) toy: bool,
    /// Where the sun is, for painted light that agrees with the real one.
    pub(crate) sun: Vec3,
    pub(crate) sat: f32,
    pub(crate) sunlit: Rgb,
    pub(crate) soil: [f32; 3],
    pub(crate) mound: (Rgb, Rgb),
    pub(crate) meadow: Rgb,
    pub(crate) grass: (Rgb, Rgb),
    pub(crate) wood: Rgb,
    pub(crate) bark: Rgb,
    pub(crate) fence: Rgb,
    pub(crate) accent: Rgb,
    pub(crate) metal: Rgb,
    pub(crate) shirt: Rgb,
    pub(crate) denim: Rgb,
    pub(crate) skin: Rgb,
    pub(crate) cheek: Rgb,
    pub(crate) hair: Rgb,
    pub(crate) straw: Rgb,
    pub(crate) band: Rgb,
    pub(crate) boot: Rgb,
    pub(crate) glove: Rgb,
    pub(crate) bag: Rgb,
    pub(crate) can: Rgb,
    pub(crate) trim: Rgb,
    pub(crate) canopy: [Rgb; 3],
    pub(crate) orchard_fruit: Rgb,
    pub(crate) blooms: [Rgb; 5],
    pub(crate) stone: Rgb,
    pub(crate) hills: [Rgb; 2],
}

const GOLDEN: Look = Look {
    toy: false,
    sun: Vec3::new(-18., 6.2, -6.),
    sat: 1.1,
    sunlit: [1.0, 0.84, 0.46],
    soil: [0.075, 0.042, 0.022],
    mound: ([0.20, 0.13, 0.08], [0.36, 0.25, 0.16]),
    meadow: [0.50, 0.60, 0.23],
    grass: ([0.18, 0.28, 0.07], [0.82, 0.74, 0.36]),
    wood: [0.62, 0.45, 0.29],
    bark: [0.40, 0.29, 0.20],
    fence: [0.66, 0.58, 0.47],
    accent: [0.55, 0.38, 0.24],
    metal: [0.30, 0.30, 0.31],
    shirt: [0.70, 0.24, 0.18],
    denim: [0.25, 0.37, 0.53],
    skin: [0.90, 0.66, 0.50],
    cheek: [0.93, 0.52, 0.45],
    hair: [0.38, 0.24, 0.13],
    straw: [0.92, 0.77, 0.46],
    band: [0.42, 0.15, 0.11],
    boot: [0.40, 0.25, 0.15],
    glove: [0.76, 0.60, 0.38],
    bag: [0.55, 0.36, 0.20],
    can: [0.64, 0.70, 0.68],
    trim: [0.45, 0.50, 0.50],
    canopy: [[0.20, 0.36, 0.13], [0.34, 0.52, 0.18], [0.62, 0.70, 0.26]],
    orchard_fruit: [0.86, 0.22, 0.14],
    blooms: [
        [0.97, 0.95, 0.88],
        [0.92, 0.28, 0.16],
        [0.60, 0.50, 0.86],
        [0.99, 0.82, 0.24],
        [0.95, 0.62, 0.70],
    ],
    stone: [0.62, 0.60, 0.55],
    hills: [[0.36, 0.48, 0.27], [0.52, 0.60, 0.36]],
};

const STORYBOOK: Look = Look {
    toy: true,
    sun: Vec3::new(7., 20., 12.),
    sat: 1.35,
    sunlit: [0.92, 1.0, 0.70],
    soil: [0.14, 0.062, 0.024],
    mound: ([0.42, 0.26, 0.16], [0.62, 0.42, 0.28]),
    meadow: [0.44, 0.76, 0.28],
    grass: ([0.24, 0.56, 0.16], [0.64, 0.90, 0.34]),
    wood: [0.80, 0.57, 0.36],
    bark: [0.58, 0.38, 0.24],
    fence: [0.98, 0.97, 0.93],
    accent: [0.90, 0.32, 0.30],
    metal: [0.98, 0.84, 0.36],
    shirt: [0.99, 0.83, 0.32],
    denim: [0.26, 0.56, 0.88],
    skin: [0.99, 0.79, 0.65],
    cheek: [1.0, 0.55, 0.58],
    hair: [0.56, 0.32, 0.18],
    straw: [1.0, 0.87, 0.52],
    band: [0.94, 0.30, 0.36],
    boot: [0.92, 0.30, 0.26],
    glove: [0.45, 0.78, 0.40],
    bag: [0.97, 0.72, 0.38],
    can: [0.96, 0.40, 0.36],
    trim: [1.0, 0.86, 0.36],
    canopy: [[0.28, 0.58, 0.44], [0.44, 0.80, 0.40], [0.78, 0.96, 0.52]],
    orchard_fruit: [1.0, 0.62, 0.20],
    blooms: [
        [1.0, 1.0, 0.98],
        [1.0, 0.45, 0.55],
        [0.70, 0.60, 1.0],
        [1.0, 0.88, 0.30],
        [1.0, 0.68, 0.86],
    ],
    stone: [0.82, 0.80, 0.76],
    hills: [[0.44, 0.74, 0.46], [0.66, 0.88, 0.52]],
};

fn look(s: Style) -> &'static Look {
    match s {
        Style::Golden => &GOLDEN,
        Style::Storybook => &STORYBOOK,
    }
}

impl Look {
    pub(crate) fn light(&self) -> Vec3 {
        self.sun.normalize()
    }
    /// Leafy shading on a clump from its unit direction: dark beneath, the
    /// sun's side lifted toward `sunlit`.
    pub(crate) fn foliage(&self, tone: Rgb, d: Vec3) -> Rgb {
        let under = if self.toy { 0.68 } else { 0.5 };
        let c = mix(scale(tone, under), tone, d.y * 0.5 + 0.5);
        let sun = d.dot(self.light()).max(0.);
        mix(c, mix(tone, self.sunlit, 0.55), sun * sun * 0.75)
    }
    pub(crate) fn leaf_tones(&self, c: &Crop) -> (Rgb, Rgb) {
        let tone = saturate(c.leaf, self.sat);
        if self.toy {
            (scale(tone, 0.82), mix(tone, [0.9, 1.0, 0.6], 0.4))
        } else {
            (scale(tone, 0.6), mix(tone, [0.86, 0.84, 0.36], 0.3))
        }
    }
    /// A soft painted body: darker underneath, lighter toward the sun.
    pub(crate) fn soft(&self, c: Rgb, d: Vec3) -> Rgb {
        let floor = if self.toy { 0.82 } else { 0.72 };
        scale(
            c,
            floor + (1. - floor) * (d.y * 0.5 + 0.5) + 0.12 * d.dot(self.light()).max(0.),
        )
    }
}

pub(crate) fn dir(angle: f32) -> Vec3 {
    let (s, c) = math::sin_cos(angle);
    Vec3::new(c, 0., s)
}

// ------------------------------------------------------------- the world

fn environment(w: &mut World, style: Style, l: &Look) {
    let (env, ao, sun, fill) = match style {
        Style::Golden => (
            Environment {
                zenith: [0.16, 0.30, 0.58],
                horizon: [1.0, 0.72, 0.48],
                ground: [0.18, 0.13, 0.06],
                ambient: 0.3,
                sun_disc: 0.012,
                exposure: 1.08,
                fog: Some(Fog {
                    color: Some([0.96, 0.70, 0.46]),
                    ..Fog::new(0.0032, 0.07)
                }),
                bloom: Some(Bloom {
                    threshold: 0.85,
                    intensity: 0.26,
                    radius: 2.2,
                }),
                ..Environment::default()
            },
            AmbientOcclusion {
                radius: 0.7,
                intensity: 1.15,
                ..AmbientOcclusion::default()
            },
            ([1.0, 0.60, 0.30], 21000.),
            ([0.36, 0.52, 1.0], 4200.),
        ),
        Style::Storybook => (
            Environment {
                zenith: [0.30, 0.58, 0.95],
                horizon: [0.80, 0.92, 1.0],
                ground: [0.35, 0.45, 0.20],
                ambient: 0.55,
                sun_disc: 0.,
                exposure: 1.0,
                fog: Some(Fog {
                    color: Some([0.82, 0.93, 1.0]),
                    ..Fog::new(0.0016, 0.06)
                }),
                bloom: Some(Bloom {
                    threshold: 1.0,
                    intensity: 0.18,
                    radius: 2.0,
                }),
                ..Environment::default()
            },
            AmbientOcclusion {
                radius: 0.5,
                intensity: 0.75,
                ..AmbientOcclusion::default()
            },
            ([1.0, 0.95, 0.86], 11000.),
            ([1.0, 0.78, 0.80], 2400.),
        ),
    };
    w.insert_resource(env);
    w.insert_resource(ao);
    *w.require_mut::<Transform>("sun") =
        Transform::at(l.sun.x, l.sun.y, l.sun.z).looking_at(Vec3::ZERO, Vec3::Y);
    *w.require_mut::<DirectionalLight>("sun") = DirectionalLight {
        color: sun.0,
        illuminance: sun.1,
        shadows: true,
    };
    let opposite = Vec3::new(-l.sun.x, l.sun.y * 0.6, -l.sun.z);
    w.spawn_named(
        "fill-light",
        (
            Transform::at(opposite.x, opposite.y, opposite.z).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: fill.0,
                illuminance: fill.1,
                shadows: false,
            },
        ),
    );
    *w.require_mut::<Material>("ground") = Material::grid(l.soil, crate::garden::TILE);
}

pub fn setup(w: &mut World, style: Style) {
    let l = look(style);
    environment(w, style, l);
    let body = w
        .generated("gardener.model", gardener(l))
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
    let arm = w.generated("gardener-arm.model", arm(l)).expect("arm mesh");
    let leg = w.generated("gardener-leg.model", leg(l)).expect("leg mesh");
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
        .generated("watering-can.model", watering_can(l))
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
    for kind in 0..CROPS.len() {
        w.generated(&format!("plant-{kind}.model"), plant(kind, l))
            .expect("plant mesh");
        w.generated(&format!("fruit-{kind}.model"), fruit(kind, l))
            .expect("fruit mesh");
    }
    let trees = w
        .generated("orchard.model", orchard(l))
        .expect("orchard mesh");
    let flowers = w
        .generated("flowers.model", flower_bank(l))
        .expect("flower mesh");
    let grass = w.generated("meadow.model", meadow(l)).expect("meadow mesh");
    let hills = w
        .generated("backdrop.model", backdrop(l))
        .expect("backdrop mesh");
    let rail = w.generated("rail.model", rail(l)).expect("rail mesh");
    let tub = w.generated("barrel.model", barrel(l)).expect("barrel mesh");
    let bed = scale(l.mound.0, 0.8);
    for (name, position, mesh, material) in [
        (
            "meadow",
            Vec3::new(0., -0.10, 0.),
            Mesh::plane(1600., 1600.),
            paint(l.meadow),
        ),
        ("orchard", Vec3::ZERO, trees, Material::default()),
        (
            "flowers-west",
            Vec3::new(-5.0, 0., 3.6),
            flowers.clone(),
            Material::default(),
        ),
        (
            "flowers-east",
            Vec3::new(6.0, 0., 3.6),
            flowers,
            Material::default(),
        ),
        ("bed-edge", Vec3::ZERO, Mesh::cube(1.), paint(bed)),
        ("meadow-grass", Vec3::ZERO, grass, Material::default()),
        ("backdrop", Vec3::ZERO, hills, Material::default()),
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
    for edge in RAILS {
        w.spawn_named(
            edge,
            (Transform::default(), rail.clone(), Material::default()),
        );
    }
    *w.require_mut::<Mesh>("water-barrel") = tub;
    *w.require_mut::<Material>("water-barrel") = Material::default();
    *w.require_mut::<Mesh>("barrel-water") = Mesh::cylinder(0.54, 0.04);
    w.require_mut::<Transform>("barrel-water").position.y = 0.92;
    *w.require_mut::<Material>("barrel-water") = Material {
        color: if l.toy {
            [0.20, 0.55, 0.85, 1.]
        } else {
            [0.05, 0.16, 0.20, 1.]
        },
        roughness: 0.06,
        ..Material::default()
    };
    let (mote, additive, rise) = if l.toy {
        ([1.0, 1.0, 1.0, 0.85], false, 0.05)
    } else {
        ([2.2, 1.6, 0.8, 0.9], true, 0.03)
    };
    w.spawn_named(
        "ambience",
        (
            Transform::default(),
            Emitter {
                shape: emitter::Shape::Sphere(10.),
                rate: 24.,
                lifetime: 7.,
                speed: 0.12,
                spread: PI,
                gravity: Vec3::Y * rise,
                drag: 0.3,
                size: [0.045, 0.03],
                color: [mote, [mote[0], mote[1], mote[2], 0.]],
                ease: emitter::Ease::Smooth,
                seed: 91,
                bound: [-12., -12., -12., 12., 12., 12.],
                additive,
                running: true,
                ..Emitter::default()
            },
        ),
    );
}

const RAILS: [&str; 4] = [
    "bed-rail-north",
    "bed-rail-south",
    "bed-rail-west",
    "bed-rail-east",
];

/// Moves this look's border, backdrop and ambience with the garden. The
/// classic look has none of them.
pub fn resize(w: &World, span: f32, mid: f32) {
    if w.named(RAILS[0]).is_none() {
        return;
    }
    let half = (span + 4.) / 2.;
    let length = span + 4.3;
    for (name, x, z, turn) in [
        (RAILS[0], mid, -mid - half, false),
        (RAILS[1], mid, -mid + half, false),
        (RAILS[2], mid - half, -mid, true),
        (RAILS[3], mid + half, -mid, true),
    ] {
        let mut t = Transform::at(x, -0.02, z).with_scale(Vec3::new(length, 1., 1.));
        if turn {
            t.rotation = Quat::from_rotation_y(PI / 2.);
        }
        *w.require_mut::<Transform>(name) = t;
    }
    w.require_mut::<Transform>("backdrop").position = Vec3::new(0., 0., -span - 2.0);
    w.require_mut::<Transform>("ambience").position = Vec3::new(mid, 1.6, -mid);
    let radius = (span * 0.6 + 4.).min(40.);
    let mut e = w.require_mut::<Emitter>("ambience");
    e.shape = emitter::Shape::Sphere(radius);
    e.rate = (radius * radius * 0.25).clamp(24., 300.);
    e.bound = [
        -radius - 2.,
        -radius - 2.,
        -radius - 2.,
        radius + 2.,
        radius + 2.,
        radius + 2.,
    ];
}
