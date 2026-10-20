//! Water (LLP 1139.002): exploration fixture.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}
fn decode(v: u8) -> f32 {
    let c = f32::from(v) / 255.;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn luminance(p: &fixture::Pixels, x: u32, y: u32) -> f32 {
    let [r, g, b, _] = p.at(x, y);
    0.2126 * decode(r) + 0.7152 * decode(g) + 0.0722 * decode(b)
}
fn mean(p: &fixture::Pixels, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> f32 {
    let n = (xs.len() * ys.len()) as f32;
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .map(|(x, y)| luminance(p, x, y))
        .sum::<f32>()
        / n
}

#[derive(Default, Args)]
struct PoolArgs {
    water: bool,
    caustics: bool,
    shafts: bool,
    y: f32,
    yaw: f32,
    pitch: f32,
    floor: f32,
    slab: bool,
    occluder: f32,
    flat: bool,
}
struct Pool;
const HEIGHT: f32 = 6.;
impl Game for Pool {
    const ID: &'static str = "water-pool";
    type Args = PoolArgs;
    fn setup(w: &mut World, a: &PoolArgs) {
        w.insert_resource(Environment::underwater());
        if a.water {
            let mut water = Water::at(HEIGHT);
            if !a.caustics {
                water.caustics = None;
            }
            if !a.shafts {
                water.shafts = None;
            }
            w.insert_resource(water);
        }
        // From ahead (−Z), 50° up.
        let to_sun = Vec3::new(0., 35f32.to_radians().sin(), -35f32.to_radians().cos());
        w.spawn_named(
            "sun",
            (
                Transform::at(to_sun.x * 30., to_sun.y * 30., to_sun.z * 30.)
                    .looking_at(Vec3::ZERO, Vec3::Y),
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
                Transform::at(0., a.floor, 0.),
                Mesh::plane(400., 400.),
                Material::rgb(0.45, 0.38, 0.24),
            ),
        );
        if a.slab {
            w.spawn_named(
                "slab",
                (
                    Transform::at(0., 5.3, 0.),
                    Mesh::cuboid(Vec3::new(60., 0.2, 60.)),
                    Material::rgb(0.2, 0.2, 0.2),
                ),
            );
        }
        if a.occluder != 0. {
            w.spawn_named(
                "occluder",
                (
                    Transform::at(a.occluder, 5.3, -3.),
                    Mesh::cuboid(Vec3::new(1.2, 0.4, 1.2)),
                    Material::rgb(0.1, 0.1, 0.1),
                ),
            );
        }
        if a.flat {
            w.spawn_named(
                "flat",
                (
                    Transform::at(0., HEIGHT, 0.),
                    Mesh::cuboid(Vec3::new(400., 0.01, 400.)),
                    Material::rgb(0.6, 0.9, 0.95),
                ),
            );
        }
        let rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            a.yaw.to_radians(),
            a.pitch.to_radians(),
            0.,
        );
        w.spawn_named(
            "camera",
            (
                Transform {
                    position: Vec3::new(0., a.y, 0.),
                    rotation,
                    scale: Vec3::ONE,
                },
                Camera::default(),
            ),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &PoolArgs) {}
}

#[derive(Clone, Copy)]
struct Shot {
    water: bool,
    caustics: bool,
    shafts: bool,
    y: f32,
    yaw: f32,
    pitch: f32,
    floor: f32,
    slab: bool,
    occluder: f32,
    flat: bool,
}
const BELOW: Shot = Shot {
    water: true,
    caustics: true,
    shafts: true,
    y: 4.,
    yaw: 0.,
    pitch: 0.,
    floor: 0.,
    slab: false,
    occluder: 0.,
    flat: false,
};
const W: u32 = 320;
const H: u32 = 180;
fn shoot(gpu: &Gpu, s: Shot, name: &str) -> fixture::Pixels {
    let mut surface = WorldSurface::<Pool>::default();
    let b = |v: bool| Value::Bool(v);
    let n = |v: f32| Value::Number(f64::from(v));
    surface
        .bind(
            &[
                b(s.water),
                b(s.caustics),
                b(s.shafts),
                n(s.y),
                n(s.yaw),
                n(s.pitch),
                n(s.floor),
                b(s.slab),
                n(s.occluder),
                b(s.flat),
            ],
            None,
        )
        .unwrap();
    let frame = |now_ms| Frame {
        width: W as f32,
        height: H as f32,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.,
    };
    let mut pixels = None;
    for at in [0., 500.] {
        pixels = Some(fixture::render(gpu, &mut surface, &frame(at)).unwrap().0);
        assert!(surface.error().is_none(), "{:?}", surface.error());
    }
    let p = pixels.unwrap();
    p.save(name);
    p
}

#[test]
fn explore() {
    let Some(gpu) = gpu() else { return };
    let up = Shot { pitch: 50., ..BELOW };
    let a = shoot(&gpu, up, "water-up");
    let b = shoot(&gpu, Shot { water: false, ..up }, "water-up-none");
    let c = shoot(&gpu, Shot { water: false, flat: true, ..up }, "water-up-flat");
    let q = |p: &fixture::Pixels| mean(p, 0..W, 0..H / 4);
    eprintln!("upper quarter: water {:.3} none {:.3} flat {:.3}", q(&a), q(&b), q(&c));
    for pitch in [0., 10., 20., 30.] {
        let toward = Shot { pitch, ..BELOW };
        let tag = format!("water-p{pitch}");
        let ahead = shoot(&gpu, toward, &tag);
        let none = shoot(&gpu, Shot { shafts: false, ..toward }, &format!("{tag}-none"));
        let off = shoot(&gpu, Shot { yaw: 40., ..toward }, &format!("{tag}-off"));
        let offnone = shoot(&gpu, Shot { yaw: 40., shafts: false, ..toward }, &format!("{tag}-offnone"));
        for rows in [H / 4..H / 2, H / 2..H * 3 / 4, H / 4..H * 3 / 4] {
            let col = |p: &fixture::Pixels| mean(p, W / 2 - 2..W / 2 + 2, rows.clone());
            eprintln!(
                "pitch {pitch} rows {rows:?}: on {:.3} off {:.3} diff {:.3}; none {:.3} offnone {:.3} diff {:.3}",
                col(&ahead), col(&off), col(&ahead) - col(&off), col(&none), col(&offnone), col(&none) - col(&offnone)
            );
        }
    }
    let down = Shot { pitch: -60., ..BELOW };
    shoot(&gpu, down, "water-down");
    shoot(&gpu, Shot { caustics: false, ..down }, "water-down-nocaustics");
    shoot(&gpu, Shot { slab: true, ..down }, "water-down-slab");
    let above = Shot { y: 7., pitch: -60., floor: 5., ..BELOW };
    shoot(&gpu, above, "water-above");
    shoot(&gpu, Shot { water: false, ..above }, "water-above-none");
}
