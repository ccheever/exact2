//! Alternative looks for the garden, chosen live by the `art` argument:
//! **golden** (late-afternoon light, long shadows, a warm haze and drifting
//! pollen) and **storybook** (bright, soft and toy-like). Both draw on the
//! classic look's entities from `present` (their own generated models, a sun
//! and fill, a sky, and a handful of props placed in every look), so the
//! game, its gestures, its saves and its scale are the same in every look.
use crate::{
    art::{dress, hide, place},
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

const STYLES: [Style; 2] = [Style::Golden, Style::Storybook];

fn tag(style: Style) -> &'static str {
    match style {
        Style::Golden => "golden",
        Style::Storybook => "storybook",
    }
}

/// A look's generated model: `golden-orchard.model`.
fn model(style: Style, base: &str) -> String {
    format!("{}-{base}.model", tag(style))
}

/// A light's linear colour and lux.
type Lux = ([f32; 3], f32);

/// The sky, occlusion, sun and fill of a look.
fn lighting(style: Style) -> (Environment, AmbientOcclusion, Lux, Lux) {
    match style {
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
    }
}

/// Both looks' generated models, their ambience emitters, and their props as
/// bare poses (they share a layout).
pub fn setup(w: &mut World) {
    for style in STYLES {
        let l = look(style);
        let generated = [
            ("gardener", gardener(l)),
            ("gardener-arm", arm(l)),
            ("gardener-leg", leg(l)),
            ("watering-can", watering_can(l)),
            ("orchard", orchard(l)),
            ("flowers", flower_bank(l)),
            ("meadow-grass", meadow(l)),
            ("backdrop", backdrop(l)),
            ("rail", rail(l)),
            ("rain-barrel", barrel(l)),
        ];
        for (base, mesh) in generated {
            w.generated(&model(style, base), mesh).expect("look mesh");
        }
        for kind in 0..CROPS.len() {
            w.generated(&model(style, &format!("plant-{kind}")), plant(kind, l))
                .expect("plant mesh");
            w.generated_model(&model(style, &format!("fruit-{kind}")), fruit(kind, l))
                .expect("fruit mesh");
        }
        let (mote, additive, rise) = if l.toy {
            ([1.0, 1.0, 1.0, 0.85], false, 0.05)
        } else {
            ([2.2, 1.6, 0.8, 0.9], true, 0.03)
        };
        w.spawn_named(
            format!("ambience-{}", tag(style)),
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
                Ambient,
            ),
        );
    }
    for (name, position) in [
        ("styled-flowers-west", Vec3::new(-5.0, 0., 3.6)),
        ("styled-flowers-east", Vec3::new(6.0, 0., 3.6)),
        ("meadow-grass", Vec3::ZERO),
        ("backdrop", Vec3::ZERO),
    ] {
        w.spawn_named(
            name,
            (Transform::at(position.x, position.y, position.z), Ambient),
        );
    }
    for edge in RAILS {
        w.spawn_named(edge, (Transform::default(), Ambient));
    }
}

/// A look as drawn: its sky, sun and fill, its models on what every look
/// shares and on its props, and its ambience; the others' emitters hidden.
pub fn present(p: &mut Present, style: Style) {
    let l = look(style);
    let (environment, occlusion, sun, fill) = lighting(style);
    if let Some(camera) = p.named("camera") {
        p.insert(
            camera,
            DrawnEnvironment {
                environment,
                ambient_occlusion: Some(occlusion),
            },
        );
    }
    let aim = |at: Vec3| Transform::at(at.x, at.y, at.z).looking_at(Vec3::ZERO, Vec3::Y);
    let opposite = Vec3::new(-l.sun.x, l.sun.y * 0.6, -l.sun.z);
    for (name, at, (color, illuminance), shadows) in
        [("sun", l.sun, sun, true), ("moon", opposite, fill, false)]
    {
        if let Some(e) = p.named(name) {
            place(p, e, aim(at));
            p.insert(
                e,
                DrawnLight::Directional(DirectionalLight {
                    color,
                    illuminance,
                    shadows,
                }),
            );
        }
    }
    if let Some(ground) = p.named("ground") {
        let plane = p.get::<Mesh>(ground).map(|m| m.clone()).unwrap_or_default();
        let soil = Material::grid(l.soil, crate::garden::TILE);
        p.insert(ground, DrawnMesh::new(plane).material(soil));
    }
    let named = |base: &str| DrawnMesh::model(model(style, base));
    for (name, base) in [
        ("gardener", "gardener"),
        ("arm-left", "gardener-arm"),
        ("arm-right", "gardener-arm"),
        ("leg-left", "gardener-leg"),
        ("leg-right", "gardener-leg"),
        ("watering-can", "watering-can"),
        ("orchard", "orchard"),
        ("styled-flowers-west", "flowers"),
        ("styled-flowers-east", "flowers"),
        ("meadow-grass", "meadow-grass"),
        ("backdrop", "backdrop"),
        ("bed-rail-north", "rail"),
        ("bed-rail-south", "rail"),
        ("bed-rail-west", "rail"),
        ("bed-rail-east", "rail"),
    ] {
        dress(p, name, named(base));
    }
    dress(
        p,
        "meadow",
        DrawnMesh::new(Mesh::plane(1600., 1600.)).material(paint(l.meadow)),
    );
    if let Some(bed) = crate::art::bed(p) {
        dress(p, "bed-edge", bed.material(paint(scale(l.mound.0, 0.8))));
    }
    dress(
        p,
        "water-barrel",
        named("rain-barrel").material(Material::default()),
    );
    if let Some(water) = p.named("barrel-water") {
        let still = Material {
            color: if l.toy {
                [0.20, 0.55, 0.85, 1.]
            } else {
                [0.05, 0.16, 0.20, 1.]
            },
            roughness: 0.06,
            ..Material::default()
        };
        p.insert(
            water,
            DrawnMesh::new(Mesh::cylinder(0.54, 0.04)).material(still),
        );
        let b = crate::farm::BARREL;
        place(p, water, Transform::at(b.x, 0.92, b.z));
    }
    // The crops: this look's model of each, in the simulation's colours and poses.
    let models: Vec<[DrawnMesh; 2]> = (0..CROPS.len())
        .map(|kind| {
            [
                named(&format!("plant-{kind}")),
                named(&format!("fruit-{kind}")),
            ]
        })
        .collect();
    let mut crops = Vec::new();
    p.for_each::<crate::garden::Plant>(|e, pl| crops.push((e, pl.kind, 0)));
    p.for_each::<crate::garden::Fruit>(|e, f| crops.push((e, f.kind, 1)));
    for (e, kind, part) in crops {
        p.insert(e, models[kind as usize][part].clone());
    }
    let picked = p
        .resource::<crate::feedback::Feedback>()
        .is_some_and(|f| f.cue == crate::feedback::Cue::Harvest);
    if picked {
        let last = p
            .resource::<crate::farm::Farm>()
            .and_then(|farm| farm.bag.last().map(|i| i.kind));
        if let Some(kind) = last {
            dress(p, "picked-fruit", named(&format!("fruit-{kind}")));
        }
    }
    hide(p, "weather");
    for other in STYLES.into_iter().filter(|&s| s != style) {
        hide(p, &format!("ambience-{}", tag(other)));
    }
}

const RAILS: [&str; 4] = [
    "bed-rail-north",
    "bed-rail-south",
    "bed-rail-west",
    "bed-rail-east",
];

/// Moves these looks' border, backdrop and ambience with the garden.
pub fn resize(w: &World, span: f32, mid: f32) {
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
    let radius = (span * 0.6 + 4.).min(40.);
    for style in STYLES {
        let name = format!("ambience-{}", tag(style));
        w.require_mut::<Transform>(name.as_str()).position = Vec3::new(mid, 1.6, -mid);
        let mut e = w.require_mut::<Emitter>(name.as_str());
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
}
