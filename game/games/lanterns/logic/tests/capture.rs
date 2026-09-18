//! Simulation reproduction of the public native route; normalized inputs, not a UI claim.
use exact_game::{Args, Capture, CaptureLimits, Sim, Transform, Value};
use exact_game_physics::Character;
use lanterns_logic::{Lantern, Lanterns, Options, Session};

fn ticks(sim: &mut Sim<Lanterns>, count: u32) {
    let before = sim.world().tick();
    let reply = sim.agent(&format!(r#"{{"op":"clock","ticks":{count}}}"#));
    assert!(!reply.contains("error"), "{reply}");
    assert_eq!(sim.world().tick(), before + count as u64);
}
fn hold(sim: &mut Sim<Lanterns>, code: &str, count: u32) {
    sim.key_down(code);
    ticks(sim, count);
    sim.key_up(code);
}
fn press(sim: &mut Sim<Lanterns>, name: &str) {
    let index = <Options as Args>::FIELDS
        .iter()
        .position(|(field, _)| *field == name)
        .unwrap();
    let mut values = sim.args().values();
    let Value::Number(value) = &mut values[index] else {
        panic!("counter")
    };
    *value += 1.0;
    sim.bind(&values, None).unwrap();
}
fn steer(sim: &mut Sim<Lanterns>, x: f32, z: f32, tolerance: f32) {
    for _ in 0..160 {
        let at = sim.position("player").unwrap();
        let dx = x - at.x;
        let dz = z - at.z;
        if dx.hypot(dz) <= tolerance {
            return;
        }
        let x_axis = dx.abs() >= dz.abs();
        let gap = if x_axis { dx } else { dz };
        let key = if x_axis {
            if gap > 0.0 {
                "KeyD"
            } else {
                "KeyA"
            }
        } else if gap > 0.0 {
            "KeyS"
        } else {
            "KeyW"
        };
        let count =
            (((gap.abs() - tolerance * 0.45).max(0.075) / 0.075).floor() as u32).clamp(1, 10);
        hold(sim, key, count);
        ticks(sim, 1);
    }
    panic!(
        "route stalled toward {x},{z} at {:?}",
        sim.position("player")
    );
}
fn land(sim: &mut Sim<Lanterns>, floor: f32) {
    for _ in 0..72 {
        if sim.world().get::<Character>("player").unwrap().grounded
            && sim.position("player").unwrap().y - 0.65 > floor
        {
            return;
        }
        ticks(sim, 1);
    }
    panic!("landing failed at {:?}", sim.position("player"));
}
#[test]
fn crate_checkpoint_finishes_route_twice_and_restores_terminal_state() {
    let mut sim = Sim::<Lanterns>::new(Options {
        seed: 1_041_003,
        started: true,
        sound: true,
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json"),
            &lanterns_logic::scene_types(),
            <Lanterns as exact_game::Game>::assets(),
        )
        .unwrap()
        .content,
        ..Default::default()
    })
    .unwrap();
    ticks(&mut sim, 2);
    steer(&mut sim, -8.0, 12.0, 0.18);
    for index in 1..=11 {
        let name = format!("lantern-{index}");
        let at = sim
            .world()
            .get::<Transform>(name.as_str())
            .unwrap()
            .position;
        steer(&mut sim, at.x, at.z, 0.18);
        press(&mut sim, "light_press");
        ticks(&mut sim, 2);
        assert!(sim.world().get::<Lantern>(name.as_str()).unwrap().lit);
    }
    steer(&mut sim, 4.8, 8.0, 0.14);
    let before = sim.position("crate").unwrap();
    hold(&mut sim, "KeyD", 65);
    ticks(&mut sim, 15);
    let at = sim.position("crate").unwrap();
    assert!(
        at.x > before.x + 0.25 && at.x < 7.55 && (at.z - 8.0).abs() < 0.22,
        "{at:?}"
    );
    sim.start_capture(
        "test-executable:lanterns-route",
        CaptureLimits {
            bytes: 8 * 1024 * 1024,
            ..Default::default()
        },
    )
    .unwrap();
    steer(&mut sim, at.x - 1.5, at.z, 0.11);
    press(&mut sim, "jump_press");
    hold(&mut sim, "KeyD", 20);
    land(&mut sim, 0.9);
    assert!(
        (sim.position("player").unwrap().y - 0.65 - (sim.position("crate").unwrap().y + 0.6)).abs()
            < 0.18
    );
    press(&mut sim, "jump_press");
    hold(&mut sim, "KeyD", 35);
    land(&mut sim, 2.2);
    steer(&mut sim, 10.0, 8.0, 0.18);
    press(&mut sim, "light_press");
    ticks(&mut sim, 2);
    assert_eq!(sim.world().resource::<Session>().phase, 2);
    assert_eq!(
        sim.world()
            .query::<&Lantern>()
            .iter()
            .filter(|(_, l)| l.lit)
            .count(),
        12
    );
    let hash = sim.world().hash();
    let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
    for _ in 0..2 {
        let replay =
            Sim::<Lanterns>::replay_capture(&capture, "test-executable:lanterns-route", None)
                .unwrap();
        assert_eq!(replay.world().hash(), hash);
        assert_eq!(replay.world().resource::<Session>().phase, 2);
    }
    let terminal = sim.save();
    let restored = Sim::<Lanterns>::from_save(&terminal).unwrap();
    assert_eq!(restored.world().hash(), hash);
    sim.start_capture("test-executable:lanterns-route", CaptureLimits::default())
        .unwrap();
    ticks(&mut sim, 10);
    let terminal_capture = sim.stop_capture().unwrap();
    let replay =
        Sim::<Lanterns>::replay_capture(terminal_capture, "test-executable:lanterns-route", None)
            .unwrap();
    assert_eq!(replay.world().resource::<Session>().phase, 2);
}
