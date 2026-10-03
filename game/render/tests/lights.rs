//! Local and directional lights on the GPU: spot cones, more than sixteen
//! lights, the second directional light and photometric units.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}

#[derive(Default, Args)]
struct Scene {
    kind: u32,
}
const SPOT: u32 = 0;
const POINT: u32 = 1;
const MANY: u32 = 2;
const FILL: u32 = 3;
// A top-down orthographic view: 20 m tall, so 18 px per metre at 640 × 360,
// with +X right and +Z down the screen.
const PX: f32 = 18.;
struct Lights;
impl Game for Lights {
    const ID: &'static str = "render-lights";
    type Args = Scene;
    fn setup(w: &mut World, args: &Scene) {
        w.insert_resource(Environment {
            ambient: 0.,
            bloom: None,
            fog: None,
            background: Some([0.; 3]),
            ..Default::default()
        });
        w.spawn((
            Transform::default(),
            Mesh::plane(40., 40.),
            Material::rgb(0.6, 0.6, 0.6),
        ));
        w.spawn((
            Transform::at(0., 10., 0.).looking_at(Vec3::ZERO, -Vec3::Z),
            Camera::orthographic(20.),
        ));
        let down = |x: f32, y: f32, z: f32| {
            Transform::at(x, y, z).looking_at(Vec3::new(x, 0., z), Vec3::Z)
        };
        let sun = DirectionalLight {
            illuminance: 0.,
            ..Default::default()
        };
        w.spawn((down(0., 5., 0.), sun));
        match args.kind {
            SPOT => {
                w.spawn((
                    down(0., 4., 0.),
                    SpotLight {
                        intensity: 20_000.,
                        range: 12.,
                        inner: 0.2,
                        outer: 0.3,
                        shadows: false,
                        ..Default::default()
                    },
                ));
            }
            POINT => {
                w.spawn((
                    down(0., 4., 0.),
                    PointLight {
                        intensity: 20_000.,
                        range: 12.,
                        ..Default::default()
                    },
                ));
            }
            MANY => {
                for i in 0..40 {
                    let (x, z) = ((i % 8) as f32 * 2. - 7., (i / 8) as f32 * 2. - 4.);
                    w.spawn((
                        Transform::at(x, 0.5, z),
                        PointLight {
                            intensity: 2_000.,
                            range: 0.9,
                            ..Default::default()
                        },
                    ));
                }
            }
            _ => {
                w.spawn((
                    down(0., 5., 0.),
                    DirectionalLight {
                        illuminance: 10_000.,
                        shadows: true,
                        ..Default::default()
                    },
                ));
            }
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Scene) {}
}

fn render(gpu: &Gpu, kind: u32, name: &str) -> (fixture::Pixels, String) {
    let mut surface = WorldSurface::<Lights>::default();
    surface
        .bind(&[Value::Number(f64::from(kind))], None)
        .unwrap();
    let frame = Frame {
        width: 640.,
        height: 360.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let (pixels, _) = fixture::render(gpu, &mut surface, &frame).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    pixels.save(name);
    let state = surface.agent(r#"{"op":"state"}"#).unwrap();
    (pixels, state)
}
// Mean red channel over a 5 × 5 patch at world (x, z) on the floor.
fn at(image: &fixture::Pixels, x: f32, z: f32) -> f64 {
    let (cx, cy) = ((320. + x * PX) as u32, (180. + z * PX) as u32);
    let mut sum = 0.;
    for y in cy - 2..=cy + 2 {
        for x in cx - 2..=cx + 2 {
            sum += f64::from(image.at(x, y)[0]);
        }
    }
    sum / 25.
}

#[test]
fn a_spot_light_lights_its_cone_and_a_point_light_everywhere_in_range() {
    let Some(gpu) = gpu() else { return };
    let (spot, _) = render(&gpu, SPOT, "lights-spot");
    let (point, _) = render(&gpu, POINT, "lights-point");
    // The cone's edge on the floor is 4 m × tan 0.3 ≈ 1.24 m from its axis.
    let values = [at(&spot, 0., 0.), at(&spot, 0.6, 0.), at(&spot, 2.5, 0.), at(&point, 2.5, 0.)];
    eprintln!("spot axis, inside, outside; point outside the cone: {values:?}");
    assert!(values[0] > 60. && values[1] > 60., "{values:?}");
    assert_eq!(values[2], 0., "{values:?}");
    assert!(values[3] > 30., "{values:?}");
    // Inside the inner cone a spot equals the point light of the same intensity.
    assert!((values[0] - at(&point, 0., 0.)).abs() <= 1., "{values:?}");
}

#[test]
fn forty_point_lights_all_light_the_floor_and_none_are_dropped() {
    let Some(gpu) = gpu() else { return };
    let (image, state) = render(&gpu, MANY, "lights-forty");
    let mut dark = Vec::new();
    for i in 0..40 {
        let (x, z) = ((i % 8) as f32 * 2. - 7., (i / 8) as f32 * 2. - 4.);
        if at(&image, x, z) < 40. {
            dark.push(i);
        }
        // Between the lights the floor is out of every range.
        assert_eq!(at(&image, x + 1., z + 1.), 0., "light {i}");
    }
    assert!(dark.is_empty(), "unlit under lights {dark:?}");
    assert!(state.contains(r#""lights":{"drawn":40,"dropped":0}"#), "{state}");
}

#[test]
fn the_second_directional_light_is_an_unshadowed_fill() {
    let Some(gpu) = gpu() else { return };
    let (image, _) = render(&gpu, FILL, "lights-fill");
    // The first light (the sun) is dark; the second lights the floor alone.
    assert!(at(&image, 0., 0.) > 100., "{}", at(&image, 0., 0.));
}
