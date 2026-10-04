// Procedural rigs: generated skinned models walk, blend, flinch and carry socketed
// props, deterministically through save/restore and a fresh game.
use exact_game::animation::{self, Layer, Layers};
use exact_game::rig::{self, Gait, Rig};
use exact_game::*;

fn humanoid() -> asset::Model {
    let r = Rig::humanoid(1.8);
    r.model([
        r.idle("idle"),
        r.walk("walk", Gait::walk(1.4)),
        r.walk("run", Gait::run(4.)),
        r.flinch("flinch"),
    ])
}
fn quadruped() -> asset::Model {
    let r = Rig::quadruped(1.4);
    r.model([r.idle("idle"), r.walk("walk", Gait::walk(1.2))])
}

struct Yard;
impl Game for Yard {
    const ID: &'static str = "rig-yard";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let hero = w.generated_model("hero.model", humanoid()).unwrap();
        let dog = w.generated_model("dog.model", quadruped()).unwrap();
        w.spawn_named(
            "hero",
            (
                Transform::default(),
                hero,
                Animator::new([rig::locomotion(
                    "move",
                    "idle",
                    [(1.4, "walk"), (4., "run")],
                )]),
                Layers(vec![Layer::new(Animation::play("flinch").once())
                    .additive()
                    .weight(0.)]),
            ),
        );
        w.spawn_named(
            "dog",
            (
                Transform::at(2., 0., 0.),
                dog,
                Animator::new([rig::locomotion("move", "idle", [(1.2, "walk")])]),
            ),
        );
        w.spawn_named(
            "sword",
            (
                Transform::default(),
                Mesh::cuboid(Vec3::new(0.04, 0.04, 0.9)),
                SocketFollow::new("hero", "hand_r").offset(Transform::at(0., -0.05, 0.4)),
            ),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let t = w.tick_end().seconds();
        // Idle, then walk, then a run past the fastest gait (speed-matched).
        let speed = if t < 0.5 {
            0.
        } else if t < 2. {
            1.4
        } else {
            5.
        };
        rig::drive(w, "hero", "move", speed);
        rig::drive(w, "dog", "move", 1.2);
        if w.tick() == 150 {
            let mut layers = w.require_mut::<Layers>("hero");
            layers.0[0] = Layer::new(Animation::play("flinch").once()).additive();
        }
        let motion = animation::step(w);
        if motion.crossed("hero", "step") {
            w.log("hero step");
        }
        w.require_mut::<Transform>("hero").position.z += speed * w.dt();
    }
}

#[test]
fn rigs_validate_with_normalized_smooth_weights_and_named_sockets() {
    for model in [humanoid(), quadruped()] {
        model.validate().unwrap();
        let mesh = &model.meshes[0];
        let shared = mesh.weights.chunks_exact(4).filter(|w| w[1] > 0.).count();
        assert!(shared > 0, "joints blend between bones");
        assert_eq!(model.skins[0].joints.len(), model.nodes.len() - 1);
    }
    let names: Vec<_> = humanoid().nodes.into_iter().map(|n| n.name).collect();
    for joint in ["hips", "head", "hand_l", "hand_r", "foot_l", "foot_r"] {
        assert!(names.iter().any(|n| n == joint), "{joint}");
    }
    // The same parameters produce the same bytes (generated identity is stable).
    assert_eq!(hash::of(&humanoid()), hash::of(&humanoid()));
    let walk = Rig::humanoid(1.8).walk("walk", Gait::walk(1.4));
    assert_eq!(walk.markers.len(), 2, "one footfall per foot");
    assert!((walk.duration() - Gait::walk(1.4).period()).abs() < 1e-6);
}

#[test]
fn locomotion_sorts_gaits_and_refuses_bad_speeds() {
    let state = rig::locomotion("move", "idle", [(4., "run"), (1.4, "walk")]);
    let Play::Blend(blend) = &state.play else {
        panic!("a blend")
    };
    let knots: Vec<_> = blend.clips.iter().map(|c| (c.0, c.1.as_str())).collect();
    assert_eq!(knots, [(0., "idle"), (1.4, "walk"), (4., "run")]);
    for bad in [
        [(0., "a"), (1., "b")],
        [(1., "a"), (1., "b")],
        [(f32::NAN, "a"), (1., "b")],
    ] {
        let refused = std::panic::catch_unwind(|| rig::locomotion("move", "idle", bad));
        assert!(refused.is_err(), "{bad:?}");
    }
}

#[test]
fn drive_picks_the_gait_and_rate_for_the_ground_speed() {
    let mut w = World::new(60, 0);
    let hero = w.generated_model("hero.model", humanoid()).unwrap();
    w.spawn_named(
        "hero",
        (
            Transform::default(),
            hero,
            Animator::new([rig::locomotion(
                "move",
                "idle",
                [(1.4, "walk"), (4., "run")],
            )]),
        ),
    );
    let read = |w: &World| {
        let a = w.require::<Animator>("hero");
        let axis = match a.params.iter().find(|p| p.0 == "speed") {
            Some((_, animation::Param::Number(v))) => *v,
            _ => f32::NAN,
        };
        (axis, a.state_named("move").unwrap().speed)
    };
    // Idle at rest and backwards; the slowest gait slowed down below it; the
    // fastest sped up above it.
    for (speed, axis, rate) in [
        (0., 0., 1.),
        (-2., 0., 1.),
        (0.7, 1.4, 0.5),
        (1.4, 1.4, 1.),
        (6., 4., 1.5),
    ] {
        rig::drive(&w, "hero", "move", speed);
        assert_eq!(read(&w), (axis, rate), "{speed}");
    }
    // Between gaits the rate corrects the blended stride (shorter than the blend's speed).
    rig::drive(&w, "hero", "move", 2.7);
    let (axis, rate) = read(&w);
    assert!(axis == 2.7 && rate > 1.01 && rate < 1.3, "{rate}");
}

#[test]
fn feet_hold_the_ground_at_every_speed() {
    for speed in [0.1, 0.3, 0.7, 1.0, 1.4, 2.0, 2.7, 3.5, 4., 5.] {
        let samples = feet(speed, "", 300);
        // A planted foot: the lower one, at its rest height (the foot joint sits
        // 0.06 m up) on consecutive ticks. It must hold still in the world.
        let mut moved = Vec::new();
        for t in 90..samples.len() - 1 {
            for foot in 0..2 {
                let (a, b) = (samples[t][foot], samples[t + 1][foot]);
                let other = samples[t][1 - foot].y;
                if a.y < other && b.y < samples[t + 1][1 - foot].y && a.y < 0.075 && b.y < 0.075 {
                    moved.push(((b.z - a.z) * 60.).abs());
                }
            }
        }
        moved.sort_by(f32::total_cmp);
        let median = moved[moved.len() / 2];
        assert!(
            !moved.is_empty() && median < 0.1 * speed + 0.02,
            "{speed} m/s: a planted foot slides at {median} m/s (median of {})",
            moved.len()
        );
    }
}

#[test]
fn rigs_walk_flinch_and_carry_a_socket_identically_across_reloads() {
    let mut expected = None;
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = Sim::<Yard>::new(()).unwrap().paranoid(mode);
        let mut foot = Vec::new();
        let mut hand = Vec::new();
        for _ in 0..40 {
            sim.run(100.);
            let w = sim.world();
            let hero = w.require::<Transform>("hero").position;
            foot.push(animation::socket(w, "hero", "foot_l").unwrap().position - hero);
            hand.push(animation::socket(w, "hero", "hand_r").unwrap().position);
        }
        let w = sim.world();
        // Walking moves the foot forward and back relative to the body.
        let zs: Vec<f32> = foot[6..20].iter().map(|p| p.z).collect();
        let span = zs.iter().cloned().fold(f32::MIN, f32::max)
            - zs.iter().cloned().fold(f32::MAX, f32::min);
        assert!(span > 0.2, "foot swing {span}: {zs:?}");
        // The hand socket (where the sword attaches) travels with the body.
        let hero = w.require::<Transform>("hero").position;
        let last = *hand.last().unwrap();
        assert!(
            last.distance(hero + Vec3::Y * 0.9) < 0.6,
            "hand {last:?} hero {hero:?}"
        );
        assert!(w.journal().iter().any(|e| e.line.contains("hero step")));
        let state = (w.hash(), sim.save().unwrap(), foot, hand);
        match &expected {
            None => expected = Some(state),
            Some(e) => assert!(*e == state, "{mode:?} diverged"),
        }
    }
}

// A humanoid moving along +Z at `speed`, its gait driven by rig::drive (or, with
// `clip`, one clip played at its own rate). Records both foot joints in world space.
#[derive(Default, Args)]
struct Walk {
    speed: f32,
    clip: String,
}
struct Walker;
impl Game for Walker {
    const ID: &'static str = "rig-walker";
    type Args = Walk;
    fn setup(w: &mut World, args: &Walk) {
        let hero = w.generated_model("hero.model", humanoid()).unwrap();
        let animator = if args.clip.is_empty() {
            Animator::new([rig::locomotion(
                "move",
                "idle",
                [(1.4, "walk"), (4., "run")],
            )])
        } else {
            Animator::new([State::clip("move", args.clip.as_str())])
        };
        w.spawn_named("hero", (Transform::default(), hero, animator));
    }
    fn tick(w: &mut World, _: &Input, args: &Walk) {
        if args.clip.is_empty() {
            rig::drive(w, "hero", "move", args.speed);
        }
        animation::step(w);
        w.require_mut::<Transform>("hero").position.z += args.speed * w.dt();
    }
}
fn feet(speed: f32, clip: &str, ticks: usize) -> Vec<[Vec3; 2]> {
    let mut sim = Sim::<Walker>::new(Walk {
        speed,
        clip: clip.into(),
    })
    .unwrap();
    (0..ticks)
        .map(|_| {
            sim.run(1000. / 60.);
            let w = sim.world();
            ["foot_l", "foot_r"].map(|f| animation::socket(w, "hero", f).unwrap().position)
        })
        .collect()
}

#[test]
fn a_planted_foot_holds_still_through_its_stance() {
    for (clip, gait) in [("walk", Gait::walk(1.4)), ("run", Gait::run(4.))] {
        let samples = feet(gait.speed, clip, 240);
        let period = gait.period();
        // foot_l touches down at phase 0, foot_r at one half; check mid-stance only.
        for (foot, phase) in [(0, 0.), (1, 0.5)] {
            let mut worst = 0f32;
            for t in 60..samples.len() - 1 {
                let c = ((t + 1) as f32 / 60. / period - phase).rem_euclid(1.);
                if c > 0.15 * gait.duty && c < 0.85 * gait.duty {
                    let v = (samples[t + 1][foot].z - samples[t][foot].z) * 60.;
                    worst = worst.max(v.abs());
                }
            }
            assert!(
                worst < 0.08 * gait.speed,
                "{clip} foot {foot}: planted foot moves {worst} m/s"
            );
        }
    }
}

#[test]
fn the_planted_foot_stays_on_the_ground_through_the_cycle() {
    // At rest the foot joints sit 0.06 m up (the foot bone's head).
    let rest = 0.06;
    for (clip, gait) in [("walk", Gait::walk(1.4)), ("run", Gait::run(4.))] {
        let samples = feet(gait.speed, clip, 240);
        let period = gait.period();
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for (t, f) in samples.iter().enumerate().skip(60) {
            let c = ((t + 1) as f32 / 60. / period).rem_euclid(1.);
            // Any foot in stance: the lower one carries the body and touches down.
            let planted = [c, (c - 0.5).rem_euclid(1.)].iter().any(|c| *c < gait.duty);
            if planted {
                let y = f[0].y.min(f[1].y);
                low = low.min(y);
                high = high.max(y);
            }
        }
        eprintln!("{clip}: planted foot height {low}..{high}");
        assert!(
            (low - rest).abs() < 0.02 && (high - rest).abs() < 0.02,
            "{clip}: the planted foot ranges {low}..{high} m against {rest}"
        );
    }
}
