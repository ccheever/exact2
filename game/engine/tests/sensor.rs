//! Sampled sensors as a canvas's input (LLP 1133.013 D6): every sample reaches
//! the tick its stamp falls in, the latest holds through a gap and is gone
//! after an end, a tilt stick steers by gravity against its neutral, and the
//! samples and what they leave save, restore and replay.
use exact_game::*;

/// Each tick's own gravity samples, and the move stick it read.
#[derive(Default, Resource)]
struct Seen {
    samples: Vec<u32>,
    sticks: Vec<Vec2>,
}
struct Tilted;
impl Game for Tilted {
    const ID: &'static str = "input-tilt";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().stick(
            "move",
            Stick::wasd().or_tilt(Tilt {
                max_degrees: 20.0,
                dead_zone: 2.0,
            }),
        )
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", Transform::default());
        w.insert_resource(Seen::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        for m in input.messages() {
            let v: Vec<f32> = (m.strip_prefix("tilt ").into_iter())
                .flat_map(str::split_whitespace)
                .filter_map(|n| n.parse().ok())
                .collect();
            if let [x, y, z] = v[..] {
                w.set_tilt_neutral("move", Vec3::new(x, y, z));
            }
        }
        {
            let mut seen = w.resource_mut::<Seen>();
            seen.samples
                .push(input.sensor_samples(SensorKind::Gravity).count() as u32);
            seen.sticks.push(input.stick("move"));
        }
        let step = input.stick_xz("move") * 4.0 * w.dt();
        w.require_mut::<Transform>("player").position += step;
    }
}

/// No game here reads a sensor.
struct Plain;
impl Game for Plain {
    const ID: &'static str = "input-plain";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd())
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", Transform::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let step = input.stick_xz("move") * 4.0 * w.dt();
        w.require_mut::<Transform>("player").position += step;
    }
}

const LEVEL: [f32; 4] = [0.0, 4.9, 8.49, 0.0];
const TILTED: [f32; 4] = [3.0, 4.9, 7.9, 0.0];

fn gravity(at_ms: f64, values: [f32; 4]) -> InputEvent {
    InputEvent::Sensor {
        sensor: SensorKind::Gravity,
        values,
        dropped: 0,
        at_ms,
        player: 0,
    }
}
fn ended(at_ms: f64) -> InputEvent {
    InputEvent::SensorEnded {
        sensor: SensorKind::Gravity,
        at_ms,
        player: 0,
    }
}
fn started<G: Game<Args = ()>>() -> Sim<G> {
    let mut sim = Sim::<G>::new(()).unwrap();
    sim.advance(0., Clock::Seekable);
    sim
}
fn stick(sim: &Sim<Tilted>) -> Vec2 {
    *sim.world().resource::<Seen>().sticks.last().unwrap()
}

/// D6's map, in f64 from the formula, against the engine's f32.
#[test]
fn the_tilt_map_reads_roll_and_pitch_from_the_neutral() {
    let tilt = Tilt {
        max_degrees: 20.0,
        dead_zone: 2.0,
    };
    let deg = |c: f64, v: [f64; 3]| {
        (c / (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .asin()
            .to_degrees()
    };
    let (g, n) = ([3.0, 4.9, 7.9], [0.0, 4.9, 8.49]);
    let roll = deg(g[0], g) - deg(n[0], n);
    let pitch = deg(g[1], g) - deg(n[1], n);
    assert!((roll - 17.88).abs() < 0.01, "{roll}");
    assert!(pitch.abs() < 0.2, "{pitch}");
    let raw = [-roll / 20.0, -pitch / 20.0];
    let length = (raw[0] * raw[0] + raw[1] * raw[1]).sqrt();
    let scale = (length.min(1.0) - 0.1) / 0.9 / length;
    let expected = Vec2::new((raw[0] * scale) as f32, (raw[1] * scale) as f32);
    let got = tilt_stick(Vec3::new(3.0, 4.9, 7.9), Vec3::new(0.0, 4.9, 8.49), tilt);
    assert!((got - expected).length() < 1e-4, "{got} vs {expected}");
    assert!((got.x + 0.88).abs() < 0.01 && got.y.abs() < 0.01, "{got}");
    // Within the dead zone it reads nothing; past max_degrees, the unit circle.
    let level = Vec3::new(0.0, 0.0, 9.8);
    assert_eq!(
        tilt_stick(Vec3::new(0.3, 0.0, 9.8), level, tilt),
        Vec2::ZERO
    );
    // Screen-frame gravity points away from Earth: lowering the right edge
    // gives negative x, which must steer right. Pitch keeps its forward sign.
    for (g, direction) in [
        (Vec3::new(-4.9, 0.0, 8.49), Vec2::X),
        (Vec3::new(4.9, 0.0, 8.49), -Vec2::X),
        (Vec3::new(0.0, 4.9, 8.49), -Vec2::Y),
        (Vec3::new(0.0, -4.9, 8.49), Vec2::Y),
    ] {
        assert!((tilt_stick(g, level, tilt) - direction).length() < 1e-5);
        assert_eq!(tilt_stick(g, g, tilt), Vec2::ZERO);
    }
    let steep = tilt_stick(Vec3::new(9.8, 0.0, 0.1), level, tilt);
    assert!((steep.length() - 1.0).abs() < 1e-6, "{steep}");
    assert_eq!(tilt_stick(Vec3::ZERO, level, tilt), Vec2::ZERO);
}

/// The first sample is the neutral; a tilt steers; the latest holds through a
/// gap; an end leaves the stick at zero until a fresh sample starts again
/// from a fresh neutral; a neutral the game set holds across an end.
#[test]
fn a_tilt_stick_steers_holds_through_a_gap_and_stops_at_an_end() {
    let mut sim = started::<Tilted>();
    sim.run(100.);
    assert_eq!(stick(&sim), Vec2::ZERO, "nothing before the first sample");
    assert!(sim.world().try_resource::<SensorState>().is_none());
    sim.input(gravity(100., LEVEL));
    sim.run(50.);
    assert_eq!(stick(&sim), Vec2::ZERO, "the first sample is level");
    sim.input(gravity(150., TILTED));
    sim.run(50.);
    let tilted = stick(&sim);
    assert!((tilted.x + 0.88).abs() < 0.01, "{tilted}");
    // A second with no sample: the device is still the input.
    sim.run(1000.);
    assert_eq!(stick(&sim), tilted);
    // A dropped run is still a sample.
    sim.input(InputEvent::Sensor {
        sensor: SensorKind::Gravity,
        values: TILTED,
        dropped: 7,
        at_ms: 1200.,
        player: 0,
    });
    sim.run(50.);
    assert_eq!(stick(&sim), tilted);
    sim.input(ended(1250.));
    sim.run(50.);
    assert_eq!(stick(&sim), Vec2::ZERO, "an end takes the sensor away");
    // After an end, the next first sample is the new neutral.
    sim.input(gravity(1300., TILTED));
    sim.run(50.);
    assert_eq!(stick(&sim), Vec2::ZERO);
    // A neutral the game set (the app's Set level) holds through an end.
    sim.post("tilt 0 4.9 8.49");
    sim.run(50.);
    sim.input(ended(1400.));
    sim.run(50.);
    sim.input(gravity(1450., TILTED));
    sim.run(50.);
    assert_eq!(stick(&sim), tilted);
    let mut journal = sim.world().journal().into_iter();
    assert!(!journal.any(|e| e.line.contains("refused")));
}

/// A 100 Hz sensor reaches a 60 Hz world as one or two samples a tick, every
/// one of them, never coalesced.
#[test]
fn every_sample_reaches_its_tick() {
    let mut sim = started::<Tilted>();
    for i in 0..100 {
        sim.input(gravity(f64::from(i) * 10., LEVEL));
    }
    sim.run(1000.);
    let seen = &sim.world().resource::<Seen>().samples;
    assert_eq!(seen.iter().sum::<u32>(), 100);
    assert!(seen.iter().all(|n| (1..=2).contains(n)), "{seen:?}");
}

/// Queued samples and the state they leave save and restore, and two worlds
/// fed the same samples are the same world; a different tilt is another.
#[test]
fn samples_save_restore_and_replay() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let feed = |sim: &mut Sim<Tilted>, tilt: [f32; 4]| {
            sim.input(gravity(16., LEVEL));
            for i in 1..30 {
                sim.input(gravity(16. + f64::from(i) * 16.6, tilt));
            }
        };
        let mut a = Sim::<Tilted>::new(()).unwrap().paranoid(mode);
        a.advance(0., Clock::Seekable);
        feed(&mut a, TILTED);
        a.advance(200., Clock::Seekable);
        // Mid-stream: a save holds the samples still queued.
        let mut b = Sim::<Tilted>::new(()).unwrap().paranoid(mode);
        b.restore(&a.save().unwrap()).unwrap();
        b.advance(0., Clock::Seekable);
        a.advance(800., Clock::Seekable);
        b.advance(600., Clock::Seekable);
        assert_eq!(a.world().hash(), b.world().hash());
        assert_eq!(
            a.get::<Transform>("player").unwrap().position,
            b.get::<Transform>("player").unwrap().position
        );
        let mut c = started::<Tilted>();
        feed(&mut c, TILTED);
        c.advance(800., Clock::Seekable);
        assert_eq!(a.world().hash(), c.world().hash());
        let mut d = started::<Tilted>();
        feed(&mut d, [2.0, 4.9, 8.0, 0.0]);
        d.advance(800., Clock::Seekable);
        assert_ne!(a.world().hash(), d.world().hash());
    }
}

/// A world that never receives a sample gains no resource, so its saves and
/// hashes are what they were before sensors (the pins hold that).
#[test]
fn a_world_without_sensors_gains_no_sensor_state() {
    let mut sim = started::<Plain>();
    sim.hold("KeyW", 500.);
    sim.run(200.);
    assert!(sim.world().try_resource::<SensorState>().is_none());
    let mut tilted = started::<Tilted>();
    tilted.hold("KeyW", 500.);
    tilted.run(200.);
    assert!(tilted.world().try_resource::<SensorState>().is_none());
}
