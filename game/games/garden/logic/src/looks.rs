//! Alternative looks for the garden, chosen live by the `art` argument:
//! **golden** (late-afternoon light, long shadows, a warm haze and drifting
//! pollen) and **storybook** (bright, soft and toy-like). Both draw on the
//! classic look's entities from `present` (their own baked models, from
//! `looks.mjs` through `art.mjs`; a sun and fill; a sky; and a handful of props
//! placed in every look), so the game, its gestures, its saves and its scale
//! are the same in every look.
use crate::{
    art::{dress, hide, place},
    crops,
    garden::paint,
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

/// What every look's `present` reads (`assets/looks.level.json`): presentation
/// data, outside saves and hashes, so an edit redraws the same world.
pub const LOOKS: &str = "looks.level.json";

#[derive(Clone, Default, Data)]
pub struct Looks {
    pub classic: Lighting,
    pub golden: Lighting,
    pub storybook: Lighting,
    pub pass: crate::pass::PassLook,
}

/// A directional light as drawn: its colour, lux and where it shines from
/// (the golden and storybook suns; `looks.mjs` paints the same light).
#[derive(Clone, Copy, Default, Data)]
pub struct Light {
    pub color: [f32; 3],
    pub illuminance: f32,
    pub at: [f32; 3],
}

#[derive(Clone, Copy, Default, Data)]
pub struct Water {
    pub color: [f32; 3],
    pub roughness: f32,
}

/// A look's sky, occlusion, sun and fill, and the colours it paints what every
/// look shares (the soil's grid, the meadow, the bed, the barrel's water). Its
/// `palette` is the models' (`looks.mjs`), baked into them.
#[derive(Clone, Default, Data)]
pub struct Lighting {
    pub environment: Environment,
    pub occlusion: AmbientOcclusion,
    pub sun: Light,
    pub fill: Light,
    pub soil: [f32; 3],
    pub meadow: [f32; 3],
    pub bed: [f32; 3],
    pub water: Water,
}

/// Every look's data, from `Game::present`.
pub fn looks(p: &Present) -> std::sync::Arc<Looks> {
    p.level::<Looks>(LOOKS).expect("the declared looks")
}

fn lit(looks: &Looks, s: Style) -> &Lighting {
    match s {
        Style::Golden => &looks.golden,
        Style::Storybook => &looks.storybook,
    }
}

// ------------------------------------------------------------- the world

const STYLES: [Style; 2] = [Style::Golden, Style::Storybook];

fn tag(style: Style) -> &'static str {
    match style {
        Style::Golden => "golden",
        Style::Storybook => "storybook",
    }
}

/// This look's baked model `base` (`plant-carrot`, `orchard`): streamed, and
/// fetched only while this look is chosen (`Game::prefetch`).
fn drawn(style: Style, base: &str) -> DrawnMesh {
    DrawnMesh::model(format!("{}-{base}.model", tag(style)))
}

/// Setup places both looks' ambience emitters and their props as bare poses
/// (they share a layout); `present` draws them.
pub fn setup(w: &mut World) {
    for style in STYLES {
        let (mote, additive, rise) = if style == Style::Storybook {
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
    let looks = looks(p);
    let lit = lit(&looks, style);
    if let Some(camera) = p.named("camera") {
        p.insert(
            camera,
            DrawnEnvironment {
                environment: lit.environment,
                ambient_occlusion: Some(lit.occlusion),
            },
        );
    }
    let aim = |at: Vec3| Transform::at(at.x, at.y, at.z).looking_at(Vec3::ZERO, Vec3::Y);
    let sun = Vec3::from(lit.sun.at);
    let opposite = Vec3::new(-sun.x, sun.y * 0.6, -sun.z);
    for (
        name,
        at,
        Light {
            color, illuminance, ..
        },
        shadows,
    ) in [
        ("sun", sun, lit.sun, true),
        ("moon", opposite, lit.fill, false),
    ] {
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
        let soil = Material::grid(lit.soil, crate::garden::TILE);
        p.insert(ground, DrawnMesh::new(plane).material(soil));
    }
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
        dress(p, name, drawn(style, base));
    }
    dress(
        p,
        "meadow",
        DrawnMesh::new(Mesh::plane(1600., 1600.)).material(paint(lit.meadow)),
    );
    if let Some(bed) = crate::art::bed(p) {
        dress(p, "bed-edge", bed.material(paint(lit.bed)));
    }
    let barrel = drawn(style, "rain-barrel").material(Material::default());
    dress(p, "water-barrel", barrel);
    if let Some(water) = p.named("barrel-water") {
        let [r, g, b] = lit.water.color;
        let still = Material {
            color: [r, g, b, 1.],
            roughness: lit.water.roughness,
            ..Material::default()
        };
        p.insert(
            water,
            DrawnMesh::new(Mesh::cylinder(0.54, 0.04)).material(still),
        );
        let b = crate::farm::BARREL;
        place(p, water, Transform::at(b.x, 0.92, b.z));
    }
    // The crops: this look's model of each, in the simulation's colours and
    // poses. Kept per crop: a present redraws only the plants and fruit that
    // appeared.
    let b = crops::shown_balance(p);
    let mut crops: Vec<[Option<DrawnMesh>; 2]> = vec![[None, None]; b.crops.len()];
    let mut crop = |kind: u8, part: usize| {
        crops[kind as usize][part]
            .get_or_insert_with(|| {
                let base = ["plant", "fruit"][part];
                drawn(style, &format!("{base}-{}", b.crop(kind).id))
            })
            .clone()
    };
    p.each::<crate::garden::Plant>(|p, e| {
        let kind = p.require::<crate::garden::Plant>(e).kind;
        p.insert(e, crop(kind, 0));
        Derived::Kept
    });
    // The fruit's simulated colour tints it; a ripe Gold one is metal too.
    p.each::<crate::garden::Fruit>(|p, e| {
        let (kind, gold) = {
            let f = p.require::<crate::garden::Fruit>(e);
            let gold = f.ripe && f.muts & (crops::RAINBOW | crops::GOLD) == crops::GOLD;
            (f.kind, gold)
        };
        p.insert(e, crop(kind, 1));
        if gold {
            p.insert(e, MaterialOverrides(vec![crate::pass::gilded(&looks.pass)]));
        }
        Derived::Kept
    });
    let picked = p
        .resource::<crate::feedback::Feedback>()
        .is_some_and(|f| f.cue == crate::feedback::Cue::Harvest);
    if picked {
        let last = p
            .resource::<crate::farm::Farm>()
            .and_then(|farm| farm.bag.last().map(|i| i.kind));
        if let Some(kind) = last {
            let fruit = drawn(style, &format!("fruit-{}", b.crop(kind).id));
            dress(p, "picked-fruit", fruit);
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
