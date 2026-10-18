//! The look oracle's negative controls (LLP 1127.000 D6): three scenes, each
//! wrong in one way the garden bakeoff's was, must trip their tell, and the
//! same scenes with the defect removed must trip none. Each is read twice: from
//! a canvas that draws (the web and Apple hosts' path) and from a device-less
//! one (the GPU-less proof host's), whose look frame makes its own device.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Gpu, Surface};

/// Which defect a scene carries; `None` draws it as it should look.
#[derive(Clone, Copy, PartialEq)]
enum Defect {
    NearBlack,
    ParticlesUnderground,
    NoCaster,
}

fn scene<const D: u8>(w: &mut World, broken: bool) {
    let defect = [
        Defect::NearBlack,
        Defect::ParticlesUnderground,
        Defect::NoCaster,
    ][D as usize];
    let dark = broken && defect == Defect::NearBlack;
    w.insert_resource(if dark {
        // Sun at zero and no sky: nothing lights the scene.
        Environment {
            background: Some(Color::hex("black")),
            zenith: [0.; 3],
            horizon: [0.; 3],
            ground: [0.; 3],
            ambient: 0.,
            fog: None,
            bloom: None,
            ..Environment::default()
        }
    } else {
        Environment::default()
    });
    w.spawn_named(
        "sun",
        (
            Transform::at(0., 10., 0.).looking_at(Vec3::new(0.3, 0., 0.2), Vec3::Z),
            DirectionalLight {
                illuminance: if dark { 0. } else { 10_000. },
                ..DirectionalLight::default()
            },
        ),
    );
    w.spawn_named(
        "camera",
        (
            Transform::at(0., 6., 8.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ),
    );
    if broken && defect == Defect::NoCaster {
        // Shadows reach 5 m; every caster is 60 m off, beyond every cascade.
        w.insert_resource(Shadows {
            distance: 5.,
            ..Shadows::default()
        });
        w.spawn_named(
            "far",
            (
                Transform::at(0., 1., -60.),
                Mesh::cube(2.),
                Material::hex("#c8b48c"),
            ),
        );
        return;
    }
    w.spawn_named(
        "ground",
        (
            Transform::default(),
            Mesh::plane(20., 20.),
            Material::hex("#5d7a52"),
        ),
    );
    w.spawn_named(
        "crate",
        (
            Transform::at(0., 0.5, 0.),
            Mesh::cube(1.),
            Material::hex("#c8b48c"),
        ),
    );
    if defect == Defect::ParticlesUnderground {
        // Spawned 2 m under the ground plane, they rise half a metre and fall.
        let y = if broken { -2. } else { 1. };
        w.spawn_named(
            "sparks",
            (
                Transform::at(1.5, y, 0.),
                Emitter {
                    size: [0.4, 0.2],
                    ..Emitter::sparks().rate(400.).lifetime(2.)
                },
            ),
        );
    }
}

macro_rules! fixture {
    ($name:ident, $defect:literal, $broken:literal) => {
        struct $name;
        impl Game for $name {
            const ID: &'static str = stringify!($name);
            type Args = ();
            fn setup(w: &mut World, _: &()) {
                scene::<$defect>(w, $broken);
            }
            fn tick(w: &mut World, _: &Input, _: &()) {
                emitter::step(w);
            }
        }
    };
}
fixture!(Dark, 0, true);
fixture!(Lit, 0, false);
fixture!(Buried, 1, true);
fixture!(Sparks, 1, false);
fixture!(Casterless, 2, true);
fixture!(Cast, 2, false);

const SIZE: (f32, f32) = (320., 180.);

/// The tells one look frame trips at 500 ms, and the reply.
fn tells<G: Game<Args = ()>>(gpu: Option<&Gpu>) -> (Vec<String>, serde_json::Value) {
    let mut s = WorldSurface::<G>::default();
    s.bind(&[], None).unwrap();
    // The first time the clock reads is its epoch; the next moves it.
    let frame = |now_ms| Frame {
        width: SIZE.0,
        height: SIZE.1,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.,
    };
    match gpu {
        Some(gpu) => {
            fixture::render(gpu, &mut s, &frame(0.)).unwrap();
            fixture::render(gpu, &mut s, &frame(500.)).unwrap();
        }
        None => {
            s.agent(r#"{"op":"clock","now":0}"#).unwrap();
        }
    }
    let request = format!(
        r#"{{"op":"state","look":true,"now":500,"width":{},"height":{}}}"#,
        SIZE.0, SIZE.1
    );
    let reply: serde_json::Value = serde_json::from_str(&s.agent(&request).unwrap()).unwrap();
    let look = &reply["look"];
    assert!(look.is_object(), "{reply}");
    let names = (look["tells"].as_array().unwrap().iter())
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    // The tells reached the world's journal too.
    let logs: serde_json::Value =
        serde_json::from_str(&s.agent(r#"{"op":"logs"}"#).unwrap()).unwrap();
    for tell in look["tells"].as_array().unwrap() {
        let line = tell["line"].as_str().unwrap();
        let lines = logs["lines"].as_array().unwrap();
        assert!(
            lines.iter().any(|l| l.as_str().unwrap().ends_with(line)),
            "{logs}"
        );
    }
    assert!(reply["tick"].as_u64().unwrap() > 0, "{reply}");
    (names, reply)
}

#[test]
fn each_defect_trips_its_tell_and_its_correction_trips_none() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    for drawing in [Some(&gpu), None] {
        let host = if drawing.is_some() {
            "drawing"
        } else {
            "device-less"
        };
        let (dark, reply) = tells::<Dark>(drawing);
        assert_eq!(dark, ["near-black"], "{host}: {reply}");
        let (lit, reply) = tells::<Lit>(drawing);
        assert!(lit.is_empty(), "{host}: {reply}");
        let (buried, reply) = tells::<Buried>(drawing);
        assert_eq!(buried, ["particles-uncovered"], "{host}: {reply}");
        assert!(
            reply["look"]["particles"]["live"].as_u64().unwrap() > 0,
            "{reply}"
        );
        let (sparks, reply) = tells::<Sparks>(drawing);
        assert!(sparks.is_empty(), "{host}: {reply}");
        assert_eq!(reply["look"]["particles"]["covered"], true, "{reply}");
        let (casterless, reply) = tells::<Casterless>(drawing);
        assert_eq!(casterless, ["no-shadow-casters"], "{host}: {reply}");
        let (cast, reply) = tells::<Cast>(drawing);
        assert!(cast.is_empty(), "{host}: {reply}");
        assert!(reply["look"]["sunCasters"].as_u64().unwrap() > 0, "{reply}");
    }
}

fn field(reply: &str, key: &str) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_str(reply).unwrap();
    value[key].clone()
}

#[test]
fn a_look_frame_runs_no_tick_moves_no_hash_and_draws_the_same_bytes_twice() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    drop(gpu);
    let mut s = WorldSurface::<Sparks>::default();
    s.bind(&[], None).unwrap();
    s.agent(r#"{"op":"clock","now":0,"width":320,"height":180}"#)
        .unwrap();
    s.agent(r#"{"op":"clock","now":400}"#).unwrap();
    let world = |s: &mut WorldSurface<Sparks>| {
        let state = s.agent(r#"{"op":"state","now":400}"#).unwrap();
        (
            field(&state, "tick"),
            field(&state, "world")["hash"].clone(),
        )
    };
    let before = world(&mut s);
    let look = |s: &mut WorldSurface<Sparks>, view: &str| {
        let reply = s
            .agent(&format!(
                r#"{{"op":"screenshot","form":"look","now":400{view}}}"#
            ))
            .unwrap();
        assert!(field(&reply, "data").is_string(), "{reply}");
        reply
    };
    let orbit = look(&mut s, r#","yaw":35,"pitch":-20,"distance":12"#);
    let again = look(&mut s, r#","yaw":35,"pitch":-20,"distance":12"#);
    assert_eq!(field(&orbit, "data"), field(&again, "data"));
    let plan = look(&mut s, r#","top":12"#);
    let camera = look(&mut s, "");
    assert_ne!(field(&plan, "data"), field(&camera, "data"));
    assert_ne!(field(&orbit, "data"), field(&camera, "data"));
    assert_eq!(field(&orbit, "width"), 320);
    assert_eq!(world(&mut s), before);
    // The device-less surface is still device-less after its look frames.
    let state = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(state.contains("\"no device\""), "{state}");
}
