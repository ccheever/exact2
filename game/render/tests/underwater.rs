//! The underwater recipe's numbers (`game/recipes/underwater.md`): the frame's
//! mean luminance sits in a band that a wrong exposure leaves, at the start and
//! with the sub backed against the glass (still in the frame), and in a look
//! frame with no tell; the water's surface keeps the sand in sunlight where an
//! opaque lid shades it; and `Environment::underwater()`'s fog hides 90% of a
//! surface at the reach it states.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}

/// Mean relative luminance, linear: Rec. 709 weights over decoded sRGB, as LLP
/// 1127.000's look frame measures a frame.
fn mean_luminance(p: &fixture::Pixels) -> f32 {
    let sum: f64 = p
        .data
        .chunks_exact(4)
        .map(|c| f64::from(luminance([c[0], c[1], c[2]])))
        .sum();
    (sum / f64::from(p.width * p.height)) as f32
}
fn luminance(c: [u8; 3]) -> f32 {
    let [r, g, b] = c.map(decode);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
fn decode(v: u8) -> f32 {
    let c = f32::from(v) / 255.;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The band the recipe's frame is measured in.
const BAND: std::ops::RangeInclusive<f32> = 0.13..=0.33;

// The recipe's scene (`game/recipes/underwater.md`), formatted; keep the two in step.
const GLASS_MAX: Vec3 = Vec3::new(10., 8., 6.);
const BOOM: Vec3 = Vec3::new(0., 1.5, 5.);
fn tank(w: &mut World) {
    w.insert_resource(Environment::underwater());
    w.spawn_named(
        "sun",
        (
            Transform::at(0., 20., 0.).looking_at(Vec3::new(3., 0., -4.), Vec3::Z),
            DirectionalLight {
                color: [0.8, 0.95, 1.],
                illuminance: 6000.,
                shadows: true,
            },
        ),
    );
    w.spawn_named(
        "sand",
        (
            Transform::at(0., 0., -12.),
            Mesh::plane(20., 36.),
            Material::rgb(0.45, 0.38, 0.24),
        ),
    );
    // The surface: its caustics on the sand, its shafts in the water.
    w.insert_resource(Water::at(GLASS_MAX.y));
    for (i, z) in [-4., -12., -24.].into_iter().enumerate() {
        w.spawn_named(
            format!("rock-{i}"),
            (
                Transform::at(-3. + 3. * i as f32, 0.6, z),
                Mesh::sphere(1.2),
                Material::rgb(0.1, 0.1, 0.09).rough(0.9),
            ),
        );
    }
    w.spawn_named(
        "sub",
        (
            Transform::at(0., 2., 0.),
            Mesh::cuboid(Vec3::new(1.2, 1., 2.5)),
            Material::rgb(0.8, 0.55, 0.1),
        ),
    );
    w.spawn_named(
        "camera",
        (
            Transform::default(),
            Camera::default(),
            Follow::new("sub").offset(BOOM.x, BOOM.y, BOOM.z).lag(0.2),
        ),
    );
}
fn fit_boom(w: &World) {
    let sub = w.require::<Transform>("sub").position;
    let back = (GLASS_MAX.z - 0.5 - sub.z).clamp(0., BOOM.z);
    let lift = (BOOM.length_squared() - back * back)
        .sqrt()
        .min(GLASS_MAX.y - 0.5 - sub.y);
    w.require_mut::<Follow>("camera").offset = Vec3::new(0., lift, back);
}

#[derive(Default, Args)]
struct TankArgs {
    /// Multiplies the recipe's exposure: 1 is the recipe.
    exposure: f32,
    /// Close the tank with an opaque lid over the water.
    opaque_surface: bool,
}
struct Tank;
impl Game for Tank {
    const ID: &'static str = "underwater-tank";
    type Args = TankArgs;
    fn setup(w: &mut World, args: &TankArgs) {
        tank(w);
        w.resource_mut::<Environment>().exposure *= args.exposure;
        if args.opaque_surface {
            w.spawn_named(
                "lid",
                (
                    Transform::at(0., GLASS_MAX.y + 0.2, -12.),
                    Mesh::cuboid(Vec3::new(20., 0.2, 36.)),
                    Material::rgb(0.6, 0.9, 0.95),
                ),
            );
        }
    }
    fn tick(w: &mut World, _: &Input, _: &TankArgs) {
        let mut sub = w.require_mut::<Transform>("sub");
        sub.position.z = (sub.position.z + 4. * w.dt()).min(GLASS_MAX.z - 1.25);
        drop(sub);
        fit_boom(w);
    }
}

fn frame(width: f32, height: f32, now_ms: f64) -> Frame {
    Frame {
        width,
        height,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    }
}
/// The frame at `ms` on the simulation clock, the sub's screen rectangle
/// (x, y, width, height) in it, and the simulation's tick, sub and camera
/// positions as the world reports them after that frame.
fn shoot(
    gpu: &Gpu,
    exposure: f32,
    opaque_surface: bool,
    ms: f64,
    name: &str,
) -> (fixture::Pixels, Rect, (u64, Vec3, Vec3)) {
    let mut s = WorldSurface::<Tank>::default();
    s.bind(
        &[
            Value::Number(f64::from(exposure)),
            Value::Bool(opaque_surface),
        ],
        None,
    )
    .unwrap();
    let mut pixels = None;
    for at in [0., ms] {
        pixels = Some(
            fixture::render(gpu, &mut s, &frame(192., 108., at))
                .unwrap()
                .0,
        );
        assert!(s.error().is_none(), "{:?}", s.error());
    }
    let pixels = pixels.unwrap();
    pixels.save(name);
    let layout = s
        .agent(r#"{"op":"layout","entity":"sub","width":192,"height":108}"#)
        .unwrap();
    let sub = json::from_str::<Layout>(&layout).unwrap().entity.screen;
    let mut place = |name: &str| {
        let reply = s
            .agent(&format!(r#"{{"op":"state","entity":"{name}"}}"#))
            .unwrap();
        let state =
            json::from_str::<EntityState>(&reply).unwrap_or_else(|e| panic!("{e}: {reply}"));
        (state.tick, state.entity.components.Transform.position)
    };
    let ((tick, sub_at), (_, camera_at)) = (place("sub"), place("camera"));
    (pixels, sub, (tick, sub_at, camera_at))
}
#[derive(Default, Data)]
struct EntityState {
    tick: u64,
    entity: EntityComponents,
}
#[derive(Default, Data)]
struct EntityComponents {
    components: Components,
}
#[derive(Default, Data)]
#[allow(non_snake_case)]
struct Components {
    Transform: Transform,
}
#[derive(Default, Data, Debug)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
#[derive(Default, Data)]
struct LayoutEntity {
    screen: Rect,
}
#[derive(Default, Data)]
struct Layout {
    entity: LayoutEntity,
}
/// Mean luminance of the sand between the camera and the first rock: the
/// bottom eighth of the frame.
fn sand(p: &fixture::Pixels) -> f32 {
    let rows = p.height * 7 / 8..p.height;
    let n = (rows.len() as u32 * p.width) as f32;
    rows.flat_map(|y| (0..p.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let [r, g, b, _] = p.at(x, y);
            luminance([r, g, b])
        })
        .sum::<f32>()
        / n
}

#[test]
fn the_recipe_frame_is_in_its_band_and_a_wrong_exposure_is_not() {
    let Some(gpu) = gpu() else { return };
    let (recipe, _, _) = shoot(&gpu, 1., false, 0., "underwater");
    let (dark, _, _) = shoot(&gpu, 0.25, false, 0., "underwater-exposure-quarter");
    let (bright, _, _) = shoot(&gpu, 4., false, 0., "underwater-exposure-4x");
    let (shaded, _, _) = shoot(&gpu, 1., true, 0., "underwater-opaque-surface");
    // Three seconds in, the sub has backed against the front glass.
    let (glass, sub, (tick, sub_at, camera_at)) =
        shoot(&gpu, 1., false, 3000., "underwater-at-glass");
    // The frame is of the sub at the glass only if the simulation got there.
    assert_eq!(tick, 3 * u64::from(Tank::HZ), "3 s of ticks");
    assert_eq!(sub_at.z, GLASS_MAX.z - 1.25, "the sub against the glass");
    assert!(
        (camera_at.z - (GLASS_MAX.z - 0.5)).abs() < 0.01,
        "the camera on the glass's inner face: {camera_at}"
    );
    let means = [&recipe, &dark, &bright, &glass].map(mean_luminance);
    let sands = [&recipe, &shaded].map(sand);
    eprintln!(
        "underwater mean luminance: recipe {:.4}, exposure x0.25 {:.4}, x4 {:.4}, \
         sub at the glass {:.4}; sand: under the water {:.4}, under an opaque lid {:.4}; \
         sub on screen at the glass {sub:?}",
        means[0], means[1], means[2], means[3], sands[0], sands[1]
    );
    assert!(BAND.contains(&means[0]), "the recipe's frame: {means:?}");
    assert!(!BAND.contains(&means[1]), "a quarter exposure: {means:?}");
    assert!(
        !BAND.contains(&means[2]),
        "four times the exposure: {means:?}"
    );
    assert!(BAND.contains(&means[3]), "the sub at the glass: {means:?}");
    assert!(
        sub.w > 0. && sub.x >= 0. && sub.y >= 0. && sub.x + sub.w <= 192. && sub.y + sub.h <= 108.,
        "the sub is wholly in the frame at the glass: {sub:?}"
    );
    assert!(
        sands[1] < sands[0] * 0.8,
        "an opaque surface shades the sand: {sands:?}"
    );
}

/// The recipe's frame as the look oracle (LLP 1127.000) sees it, from the
/// GPU-less proof host's look frame: in the band, and no tell, the surface and
/// its shafts in it.
#[test]
fn the_recipe_frame_trips_no_tell_in_a_look_frame() {
    let Some(_gpu) = gpu() else { return };
    let mut s = WorldSurface::<Tank>::default();
    s.bind(&[Value::Number(1.), Value::Bool(false)], None)
        .unwrap();
    s.agent(r#"{"op":"clock","now":0,"width":192,"height":108}"#)
        .unwrap();
    s.agent(r#"{"op":"clock","now":500}"#).unwrap();
    let reply = s
        .agent(r#"{"op":"state","look":true,"now":500,"width":192,"height":108}"#)
        .unwrap();
    let look: serde_json::Value = serde_json::from_str(&reply).unwrap();
    let look = &look["look"];
    let mean = look["mean"].as_f64().unwrap() as f32;
    eprintln!(
        "underwater look frame at 500 ms: mean {mean:.4}, tells {}",
        look["tells"]
    );
    assert!(BAND.contains(&mean), "{look}");
    assert_eq!(look["tells"], serde_json::json!([]), "{look}");
    assert_eq!(look["gaps"], serde_json::json!([]), "{look}");
}

#[derive(Default, Args)]
struct ProbeArgs {}
/// Two black cubes in open water, every light off and tone mapping linear, so a
/// pixel decodes to `fog × (1 − transmittance)` and open water to `fog`. Each
/// cube's front face crosses its probe pixel's ray at the stated distance.
struct Probe;
const REACH: f32 = 32.9;
const PROBE: u32 = 128;
const EYE: Vec3 = Vec3::new(0., 2., 0.);
/// Probe pixels (column, row) and the distance along each one's ray.
const PROBES: [(u32, u32, f32); 2] = [(44, 64, REACH / 2.), (84, 64, REACH)];
/// The direction through a pixel's centre, for `Camera::default()`'s 60°
/// vertical field on a square target looking down −Z.
fn ray(x: u32, y: u32) -> Vec3 {
    let half = PROBE as f32 / 2.;
    let tan = 30f32.to_radians().tan();
    Vec3::new(
        (x as f32 + 0.5 - half) / half * tan,
        (half - y as f32 - 0.5) / half * tan,
        -1.,
    )
    .normalize()
}
impl Game for Probe {
    const ID: &'static str = "underwater-fog-probe";
    type Args = ProbeArgs;
    fn setup(w: &mut World, _: &ProbeArgs) {
        w.insert_resource(Environment {
            ambient: 0.,
            exposure: 4.,
            tone_mapping: ToneMapping::None,
            bloom: None,
            ..Environment::underwater()
        });
        w.spawn((
            Transform::at(EYE.x, EYE.y, EYE.z).looking_at(EYE - Vec3::Z, Vec3::Y),
            Camera::default(),
        ));
        let black = Material::rgb(0., 0., 0.).rough(1.);
        for (x, y, distance) in PROBES {
            let hit = EYE + ray(x, y) * distance;
            // Sized to cover about a dozen pixels at its distance.
            let half = distance / 16.;
            w.spawn((
                Transform::at(hit.x, hit.y, hit.z - half),
                Mesh::cube(2. * half),
                black,
            ));
        }
    }
    fn tick(_: &mut World, _: &Input, _: &ProbeArgs) {}
}

#[test]
fn the_underwater_fog_hides_nine_tenths_at_its_stated_reach() {
    let Some(gpu) = gpu() else { return };
    let mut s = WorldSurface::<Probe>::default();
    s.bind(&[], None).unwrap();
    let size = PROBE as f32;
    let (p, _) = fixture::render(&gpu, &mut s, &frame(size, size, 0.)).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    p.save("underwater-fog-probe");
    // Open water fills the top row.
    let water = luminance_at(&p, 64, 2);
    for (x, y, distance) in PROBES {
        let measured = 1. - luminance_at(&p, x, y) / water;
        let expected = (-0.07 * distance).exp();
        eprintln!(
            "underwater fog transmittance at {distance:.2} m: {measured:.3}, expected {expected:.3}"
        );
        assert!(
            (measured - expected).abs() < 0.02,
            "at {distance} m: {measured}"
        );
    }
}
fn luminance_at(p: &fixture::Pixels, x: u32, y: u32) -> f32 {
    let [r, g, b, _] = p.at(x, y);
    luminance([r, g, b])
}

/// The fog probe with `Water` 1 m under the eye: a ray that crosses the
/// surface is fogged over its submerged part only. The sky is black, so the
/// surface from above adds nothing, and its reflectance dims the probe and
/// the open water beside it alike (the surface is still, and the two rays
/// mirror each other about the frame's centre column).
struct CrossingProbe;
/// The probe pixel, 15° down, and the open-water pixel mirroring it.
const CROSSING: (u32, u32, f32) = (44, 94, 20.);
const OPEN: (u32, u32) = (84, 94);
const SURFACE: f32 = 1.;
impl Game for CrossingProbe {
    const ID: &'static str = "underwater-crossing-probe";
    type Args = ProbeArgs;
    fn setup(w: &mut World, _: &ProbeArgs) {
        w.insert_resource(Environment {
            zenith: [0.; 3],
            horizon: [0.; 3],
            ground: [0.; 3],
            ambient: 0.,
            exposure: 4.,
            tone_mapping: ToneMapping::None,
            bloom: None,
            fog: Some(Fog {
                color: Some([0.03, 0.16, 0.2].into()),
                ..Fog::new(0.07, 0.)
            }),
            ..Environment::underwater()
        });
        // Still, so the probe's and the open water's reflectance are the same.
        w.insert_resource(Water {
            waves: Waves {
                amplitude: 0.,
                ..Waves::default()
            },
            caustics: None,
            shafts: None,
            ..Water::at(SURFACE)
        });
        w.spawn((
            Transform::at(EYE.x, EYE.y, EYE.z).looking_at(EYE - Vec3::Z, Vec3::Y),
            Camera::default(),
        ));
        let (x, y, distance) = CROSSING;
        let hit = EYE + ray(x, y) * distance;
        let half = distance / 16.;
        w.spawn((
            Transform::at(hit.x, hit.y, hit.z - half),
            Mesh::cube(2. * half),
            Material::rgb(0., 0., 0.).rough(1.),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &ProbeArgs) {}
}

#[test]
fn with_water_a_ray_that_crosses_the_surface_is_fogged_only_under_it() {
    let Some(gpu) = gpu() else { return };
    let mut s = WorldSurface::<CrossingProbe>::default();
    s.bind(&[], None).unwrap();
    let size = PROBE as f32;
    let (p, _) = fixture::render(&gpu, &mut s, &frame(size, size, 0.)).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    p.save("underwater-crossing-probe");
    let (x, y, distance) = CROSSING;
    let direction = ray(x, y);
    let submerged = distance - (EYE.y - SURFACE) / -direction.y;
    let measured = 1. - luminance_at(&p, x, y) / luminance_at(&p, OPEN.0, OPEN.1);
    let expected = (-0.07 * submerged).exp();
    let whole = (-0.07 * distance).exp();
    eprintln!(
        "underwater fog across the surface: transmittance {measured:.3} over {distance} m, \
         {submerged:.2} m of it submerged: expected {expected:.3} (the whole ray's {whole:.3})"
    );
    assert!((measured - expected).abs() < 0.02, "{measured} {expected}");
}
