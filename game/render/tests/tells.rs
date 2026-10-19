//! The look oracle's negative controls (LLP 1127.000 D6): five scenes, three
//! wrong in one way the garden bakeoff's was, one blown out and one whose
//! sparks draw nothing visible, must trip their tell, and the same scenes with
//! the defect removed must trip none. Each is read twice: from
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
    NearWhite,
    /// The sparks are prepared and submitted, every fragment fully transparent.
    Transparent,
}

fn scene<const D: u8>(w: &mut World, broken: bool) {
    let defect = [
        Defect::NearBlack,
        Defect::ParticlesUnderground,
        Defect::NoCaster,
        Defect::NearWhite,
        Defect::Transparent,
    ][D as usize];
    let dark = broken && defect == Defect::NearBlack;
    let blown = broken && defect == Defect::NearWhite;
    w.insert_resource(if blown {
        // A white sky and a sun a thousand times too bright, untone-mapped.
        Environment {
            background: Some(Color::hex("white")),
            tone_mapping: ToneMapping::None,
            bloom: None,
            ..Environment::default()
        }
    } else if dark {
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
                illuminance: if dark {
                    0.
                } else if blown {
                    1e7
                } else {
                    10_000.
                },
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
    if matches!(defect, Defect::ParticlesUnderground | Defect::Transparent) {
        // Spawned 2 m under the ground plane, they rise half a metre and fall.
        let y = if broken && defect == Defect::ParticlesUnderground {
            -2.
        } else {
            1.
        };
        let sparks = Emitter {
            size: [0.4, 0.2],
            ..Emitter::sparks().rate(400.).lifetime(2.)
        };
        let clear = broken && defect == Defect::Transparent;
        w.spawn_named(
            "sparks",
            (
                Transform::at(1.5, y, 0.),
                Emitter {
                    color: if clear {
                        [[1., 1., 1., 0.]; 2]
                    } else {
                        sparks.color
                    },
                    ..sparks
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
fixture!(Blown, 3, true);
fixture!(Exposed, 3, false);
fixture!(Clear, 4, true);

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
    // A tell is presentation: the save before the look frame is the save after it.
    s.agent(r#"{"op":"clock","now":500}"#).unwrap();
    let saved = s.carry().unwrap().unwrap();
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
    // The tells reached `logs` too, and not the journal a save carries.
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
    // Every measurement a tell reads was taken.
    assert_eq!(look["gaps"], serde_json::json!([]), "{reply}");
    let after = s.carry().unwrap().unwrap();
    assert!(after == saved, "a look frame changed the save");
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
        let draw = &reply["look"]["particles"][0];
        assert!(draw["live"].as_u64().unwrap() > 0, "{reply}");
        assert_eq!(draw["covered"], 0, "{reply}");
        let (sparks, reply) = tells::<Sparks>(drawing);
        assert!(sparks.is_empty(), "{host}: {reply}");
        let covered = reply["look"]["particles"][0]["covered"].as_u64();
        assert!(covered.unwrap() > 0, "{reply}");
        let (casterless, reply) = tells::<Casterless>(drawing);
        assert_eq!(casterless, ["no-shadow-casters"], "{host}: {reply}");
        let (cast, reply) = tells::<Cast>(drawing);
        assert!(cast.is_empty(), "{host}: {reply}");
        assert!(reply["look"]["sunCasters"].as_u64().unwrap() > 0, "{reply}");
        assert!(reply["look"]["sunTexels"].as_u64().unwrap() > 0, "{reply}");
        // The particles' own draw is what is counted: sparks above the ground
        // whose fragments are all transparent cover nothing.
        let (clear, reply) = tells::<Clear>(drawing);
        assert_eq!(clear, ["particles-uncovered"], "{host}: {reply}");
        let (blown, reply) = tells::<Blown>(drawing);
        assert!(blown.contains(&"near-white".to_owned()), "{host}: {reply}");
        let (exposed, reply) = tells::<Exposed>(drawing);
        assert!(exposed.is_empty(), "{host}: {reply}");
    }
}

/// The lit scene, whose ground and crate move 60 m off after tick 45, beyond
/// shadows that then reach 5 m: the same renderer's look frames before and after.
struct Clearing;
impl Game for Clearing {
    const ID: &'static str = "Clearing";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        scene::<2>(w, false);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        if w.tick() >= 45 {
            w.insert_resource(Shadows {
                distance: 5.,
                ..Shadows::default()
            });
            for (name, y) in [("ground", 0.), ("crate", 0.5)] {
                w.require_mut::<Transform>(name).position = Vec3::new(0., y, -60.);
            }
        }
    }
}

#[test]
fn a_casterless_look_after_a_lit_one_counts_no_texels() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    for drawing in [Some(&gpu), None] {
        let mut s = WorldSurface::<Clearing>::default();
        s.bind(&[], None).unwrap();
        let look = |s: &mut WorldSurface<Clearing>, now: u32| {
            if let Some(gpu) = drawing {
                let frame = Frame {
                    width: SIZE.0,
                    height: SIZE.1,
                    scale: 1.,
                    now_ms: f64::from(now),
                    seekable: true,
                    period_ms: 0.,
                    children_generation: 0,
                    shader_generation: 0,
                    headroom: 1.,
                };
                fixture::render(gpu, s, &frame).unwrap();
            }
            s.agent(&format!(r#"{{"op":"clock","now":{now}}}"#))
                .unwrap();
            let request = format!(
                r#"{{"op":"state","look":true,"now":{now},"width":{},"height":{}}}"#,
                SIZE.0, SIZE.1
            );
            let reply: serde_json::Value =
                serde_json::from_str(&s.agent(&request).unwrap()).unwrap();
            reply["look"].clone()
        };
        look(&mut s, 0);
        let lit = look(&mut s, 500);
        assert_eq!(lit["tells"], serde_json::json!([]), "{lit}");
        assert!(lit["sunTexels"].as_u64() > Some(0), "{lit}");
        // The look renderer and its counter are the same; the count is this frame's.
        let bare = look(&mut s, 1500);
        assert_eq!(bare["sunTexels"], 0, "{bare}");
        assert_eq!(bare["tells"][0]["name"], "no-shadow-casters", "{bare}");
    }
}

/// An unlit black world before the game's camera, and behind it a lit cube
/// that an orbit of the scene sees: the camera's frame is near-black, the
/// orbit's is not.
struct Behind;
impl Game for Behind {
    const ID: &'static str = "Behind";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some(Color::hex("black")),
            zenith: [0.; 3],
            horizon: [0.; 3],
            ground: [0.; 3],
            ambient: 0.,
            fog: None,
            bloom: None,
            ..Environment::default()
        });
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 0., 0.).looking_at(Vec3::new(0., 0., -1.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn_named(
            "lamp",
            (
                Transform::at(0., 0., 10.),
                Mesh::cube(6.),
                Material {
                    emissive: [0.5; 3],
                    ..Material::rgb(0., 0., 0.)
                },
            ),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

#[test]
fn an_orbit_neither_logs_a_tell_nor_forgets_the_cameras() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    for drawing in [Some(&gpu), None] {
        let mut s = WorldSurface::<Behind>::default();
        s.bind(&[], None).unwrap();
        match drawing {
            Some(gpu) => {
                for now_ms in [0., 100.] {
                    let frame = Frame {
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
                    fixture::render(gpu, &mut s, &frame).unwrap();
                }
            }
            None => {
                s.agent(r#"{"op":"clock","now":0}"#).unwrap();
                s.agent(r#"{"op":"clock","now":100}"#).unwrap();
            }
        }
        let size = format!(r#""width":{},"height":{}"#, SIZE.0, SIZE.1);
        let look = |s: &mut WorldSurface<Behind>, view: &str| {
            let request = format!(r#"{{"op":"screenshot","form":"look",{size}{view}}}"#);
            let reply: serde_json::Value =
                serde_json::from_str(&s.agent(&request).unwrap()).unwrap();
            reply["look"]["tells"].clone()
        };
        let camera = look(&mut s, "");
        assert_eq!(camera[0]["name"], "near-black", "{camera}");
        let orbit = look(&mut s, r#","yaw":0,"pitch":-10,"distance":14"#);
        assert_eq!(orbit, serde_json::json!([]), "an orbit is a picture");
        look(&mut s, "");
        let logs: serde_json::Value =
            serde_json::from_str(&s.agent(r#"{"op":"logs"}"#).unwrap()).unwrap();
        let told: Vec<_> = (logs["lines"].as_array().unwrap().iter())
            .filter(|l| l.as_str().unwrap().contains("look: "))
            .collect();
        assert_eq!(told.len(), 1, "{logs}");
        assert!(told[0].as_str().unwrap().ends_with("near black"), "{logs}");
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
