//! Emitters as the art passes needed them: stepped without being told, a box
//! volume, and particles that stay where they were born.
use exact_game::emitter::{self, Shape};
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
