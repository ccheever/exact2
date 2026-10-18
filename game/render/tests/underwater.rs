//! The underwater recipe's numbers (`game/recipes/underwater.md`): the frame's
//! mean luminance sits in a band that a wrong exposure leaves, a translucent
//! water surface keeps the sand in sunlight where an opaque one shades it, and
//! `Environment::underwater()`'s fog hides 90% of a surface at the reach it states.
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
    // The surface faces down at the camera and, translucent, casts no shadow.
    w.spawn_named(
        "surface",
        (
            Transform::at(0., 8., -12.).looking_at(Vec3::new(0., 8., -13.), Vec3::NEG_Y),
            Mesh::plane(20., 36.),
            Material::rgb(0.6, 0.9, 0.95).alpha(0.25),
        ),
    );
    for (i, z) in [-4., -12., -24.].into_iter().enumerate() {
        w.spawn_named(
            &format!("rock-{i}"),
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
            Follow::new("sub")
                .offset(0., 1.5, 5.)
                .look_at_offset(0., 0., -6.)
                .lag(0.2),
        ),
    );
}

#[derive(Default, Args)]
struct TankArgs {
    /// Multiplies the recipe's exposure: 1 is the recipe.
    exposure: f32,
    /// Close the tank with an opaque lid in place of the translucent surface.
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
            *w.require_mut::<Mesh>("surface") = Mesh::cuboid(Vec3::new(20., 0.2, 36.));
            w.require_mut::<Material>("surface").color[3] = 1.;
        }
    }
    fn tick(_: &mut World, _: &Input, _: &TankArgs) {}
}

fn frame(width: f32, height: f32) -> Frame {
    Frame {
        width,
        height,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    }
}
fn shoot(gpu: &Gpu, exposure: f32, opaque_surface: bool, name: &str) -> fixture::Pixels {
    let mut s = WorldSurface::<Tank>::default();
    s.bind(
        &[
            Value::Number(f64::from(exposure)),
            Value::Bool(opaque_surface),
        ],
        None,
    )
    .unwrap();
    let (pixels, _) = fixture::render(gpu, &mut s, &frame(192., 108.)).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    pixels.save(name);
    pixels
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
    let recipe = shoot(&gpu, 1., false, "underwater");
    let (dark, bright) = (
        shoot(&gpu, 0.25, false, "underwater-exposure-quarter"),
        shoot(&gpu, 4., false, "underwater-exposure-4x"),
    );
    let shaded = shoot(&gpu, 1., true, "underwater-opaque-surface");
    let means = [&recipe, &dark, &bright].map(mean_luminance);
    let sands = [&recipe, &shaded].map(sand);
    eprintln!(
        "underwater mean luminance: recipe {:.4}, exposure x0.25 {:.4}, x4 {:.4}; \
         sand: translucent surface {:.4}, opaque surface {:.4}",
        means[0], means[1], means[2], sands[0], sands[1]
    );
    assert!(BAND.contains(&means[0]), "the recipe's frame: {means:?}");
    assert!(!BAND.contains(&means[1]), "a quarter exposure: {means:?}");
    assert!(
        !BAND.contains(&means[2]),
        "four times the exposure: {means:?}"
    );
    assert!(
        sands[1] < sands[0] * 0.8,
        "an opaque surface shades the sand: {sands:?}"
    );
}

#[derive(Default, Args)]
struct ProbeArgs {}
/// Two black cubes in open water at half the fog reach and at the reach, with
/// every light off and tone mapping linear, so a pixel decodes to
/// `fog × (1 − transmittance)` and the open water beside them to `fog`.
struct Probe;
const REACH: f32 = 32.9;
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
            Transform::at(0., 2., 0.).looking_at(Vec3::new(0., 2., -1.), Vec3::Y),
            Camera::default(),
        ));
        let black = Material::rgb(0., 0., 0.).rough(1.);
        w.spawn((Transform::at(-3., 2., -REACH / 2.), Mesh::cube(2.), black));
        w.spawn((Transform::at(6., 2., -REACH), Mesh::cube(4.), black));
    }
    fn tick(_: &mut World, _: &Input, _: &ProbeArgs) {}
}

#[test]
fn the_underwater_fog_hides_nine_tenths_at_its_stated_reach() {
    let Some(gpu) = gpu() else { return };
    let mut s = WorldSurface::<Probe>::default();
    s.bind(&[], None).unwrap();
    let (p, _) = fixture::render(&gpu, &mut s, &frame(128., 128.)).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    p.save("underwater-fog-probe");
    // The cubes' centres project to these pixels; open water fills the top row.
    let water = luminance_at(&p, 64, 2);
    let half = luminance_at(&p, 64 - 3 * 64 * 2 / 19, 64);
    let reach = luminance_at(&p, 64 + 6 * 64 / 19, 64);
    let transmittance = [1. - half / water, 1. - reach / water];
    eprintln!(
        "underwater fog transmittance: {:.3} at {:.2} m, {:.3} at {REACH} m",
        transmittance[0],
        REACH / 2.,
        transmittance[1]
    );
    assert!((transmittance[0] - 0.316).abs() < 0.03, "{transmittance:?}");
    assert!((transmittance[1] - 0.1).abs() < 0.03, "{transmittance:?}");
}
fn luminance_at(p: &fixture::Pixels, x: u32, y: u32) -> f32 {
    let [r, g, b, _] = p.at(x, y);
    luminance([r, g, b])
}
