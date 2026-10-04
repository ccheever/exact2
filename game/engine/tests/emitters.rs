//! Emitters as the art passes needed them: stepped without being told, a box
//! volume, and particles that stay where they were born.
use exact_game::emitter::{self, Particle, Shape};
use exact_game::*;

struct Rain<const EXPLICIT: bool>;
impl<const EXPLICIT: bool> Game for Rain<EXPLICIT> {
    const ID: &'static str = "rain";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "rain",
            (Transform::default(), Emitter::sparks().rate(120.).seed(3)),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        if EXPLICIT {
            emitter::step(w);
        }
    }
}

// Grow a Garden's rain emitter had no particles and no error until the tick
// called emitter::step.
#[test]
fn a_running_emitter_no_tick_steps_is_stepped_once_a_tick() {
    let mut forgot = Sim::<Rain<false>>::new(()).unwrap();
    let mut explicit = Sim::<Rain<true>>::new(()).unwrap();
    forgot.run(500.);
    explicit.run(500.);
    let age = |w: &World| w.require::<Emitter>("rain").state.age;
    assert_eq!(age(forgot.world()), forgot.world().tick());
    assert_eq!(
        age(explicit.world()),
        explicit.world().tick(),
        "never twice a tick"
    );
    assert!(forgot.world().require::<Emitter>("rain").state.alive > 0);
    assert_eq!(forgot.world().hash(), explicit.world().hash());
}

#[test]
fn a_box_emitter_spawns_inside_its_box() {
    let mut w = World::new(60, 0);
    let e = w.spawn(Emitter {
        shape: Shape::Box(Vec3::new(10., 2., 4.)),
        speed: 0.,
        gravity: Vec3::ZERO,
        ..Emitter::sparks().rate(0.).burst(500).seed(5)
    });
    emitter::step(&w);
    let mut seen = 0;
    let mut extent = Vec3::ZERO;
    w.get::<Emitter>(e).unwrap().particles(60, 1., |p| {
        seen += 1;
        extent = extent.max(p.position.abs());
    });
    assert_eq!(seen, 500);
    assert!(extent.x <= 5. && extent.y <= 1. && extent.z <= 2.);
    assert!(extent.x > 4.5 && extent.z > 1.8, "fills the box: {extent}");
    assert!(Emitter {
        shape: Shape::Box(Vec3::new(-1., 1., 1.)),
        ..Emitter::default()
    }
    .validate()
    .is_err());
}

struct Trail<const WORLD: bool>;
impl<const WORLD: bool> Game for Trail<WORLD> {
    const ID: &'static str = "trail";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let e = w.spawn_named(
            "rocket",
            (
                Transform::default(),
                Emitter {
                    speed: 0.,
                    gravity: Vec3::ZERO,
                    ..Emitter::sparks().rate(60.).lifetime(2.).seed(1)
                },
            ),
        );
        if WORLD {
            w.insert(e, emitter::WorldSpace::default());
        }
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.require_mut::<Transform>("rocket").position.x += 0.5;
    }
}
fn trail<const WORLD: bool>() -> (Vec<Particle>, u64) {
    let mut s = Sim::<Trail<WORLD>>::new(()).unwrap();
    s.run(500.);
    let mut out = Vec::new();
    s.world()
        .require::<Emitter>("rocket")
        .particles(60, 1., |p| out.push(p));
    (out, s.world().hash())
}
// Rivals spawned an emitter entity per smoke puff because particles moved with
// their emitter. With WorldSpace each batch stays where it was born.
#[test]
fn world_space_particles_stay_where_they_were_born() {
    let (local, local_hash) = trail::<false>();
    let (world, _) = trail::<true>();
    assert!(local.iter().all(|p| !p.world && p.position.x.abs() < 1e-4));
    assert!(world.iter().all(|p| p.world));
    let xs: Vec<_> = world.iter().map(|p| p.position.x).collect();
    let (min, max) = xs
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    assert!(min < 2. && max > 10., "a trail from {min} to {max}");
    // The local emitter's encoding is untouched: the marker is the only new state.
    let mut again = Sim::<Trail<false>>::new(()).unwrap();
    again.run(500.);
    assert_eq!(again.world().hash(), local_hash);
}

#[test]
fn a_restored_trail_draws_where_the_continuous_one_does() {
    let mut a = Sim::<Trail<true>>::new(()).unwrap();
    a.run(250.);
    let mut b = Sim::<Trail<true>>::new(()).unwrap();
    b.restore(&a.save().unwrap()).unwrap();
    a.run(250.);
    b.run(250.);
    let drawn = |s: &Sim<Trail<true>>| {
        let mut out = Vec::new();
        s.world()
            .require::<Emitter>("rocket")
            .particles(60, 1., |p| out.push(p));
        out
    };
    assert!(!drawn(&a).is_empty());
    assert_eq!(drawn(&a), drawn(&b));
    assert_eq!(a.world().hash(), b.world().hash());
}

#[test]
fn a_world_space_batch_keeps_the_emitter_scale_it_was_born_at() {
    struct Scaled;
    impl Game for Scaled {
        const ID: &'static str = "scaled";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let e = w.spawn_named(
                "puff",
                (
                    Transform::default().with_scale(3.),
                    Emitter {
                        speed: 0.,
                        gravity: Vec3::ZERO,
                        size: [0.1, 0.1],
                        ..Emitter::sparks().rate(0.).lifetime(5.).burst(4)
                    },
                ),
            );
            w.insert(e, emitter::WorldSpace::default());
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            // Born at scale 3 on the first tick; shrunk afterwards.
            if w.tick() > 5 {
                w.require_mut::<Transform>("puff").scale = Vec3::ONE;
            }
        }
    }
    let mut s = Sim::<Scaled>::new(()).unwrap();
    s.run(500.);
    let mut out = Vec::new();
    s.world()
        .require::<Emitter>("puff")
        .particles(60, 1., |p| out.push(p));
    assert_eq!(out.len(), 4);
    assert!(out.iter().all(|p| (p.size - 0.3).abs() < 1e-5), "{out:?}");
}

// A child emitter's batch is born at this tick's parent pose, including the
// tick it is spawned in, and at its scale then.
struct Exhaust;
impl Game for Exhaust {
    const ID: &'static str = "exhaust";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("ship", Transform::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.require_mut::<Transform>("ship").position.x += 1.;
        if w.tick() == 10 {
            let ship = w.named("ship").unwrap();
            let e = w.spawn_named(
                "exhaust",
                (
                    Transform::at(0., 2., 0.).with_scale(3.),
                    Parent(ship),
                    Emitter {
                        speed: 0.,
                        gravity: Vec3::ZERO,
                        size: [0.1, 0.1],
                        ..Emitter::sparks().rate(0.).lifetime(5.).burst(4)
                    },
                ),
            );
            w.insert(e, emitter::WorldSpace::default());
        }
    }
}
#[test]
fn a_parented_world_space_emitter_is_born_at_its_current_pose_and_scale() {
    let mut s = Sim::<Exhaust>::new(()).unwrap();
    s.run(500.);
    let mut out = Vec::new();
    s.world()
        .require::<Emitter>("exhaust")
        .particles(60, 1., |p| out.push(p));
    assert_eq!(out.len(), 4);
    for p in out {
        // The ship was at x = 11 during tick 10, the exhaust 2 above it.
        assert!(
            (p.position - Vec3::new(11., 2., 0.)).length() < 1e-4,
            "{p:?}"
        );
        assert!((p.size - 0.3).abs() < 1e-5, "{p:?}");
    }
}
