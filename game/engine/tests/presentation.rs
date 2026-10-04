//! Presentation-only state: visual changes that never move a pin.
use exact_game::*;

#[derive(Default, Component)]
struct Crate {
    hp: u32,
}
// Bob and flash are visual only: Grow a Garden's sway, Rivals' muzzle flash.
#[derive(Default, Presentation)]
struct Bob {
    height: f32,
    flash: f32,
}

struct Plain;
impl Game for Plain {
    const ID: &'static str = "presentation";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("crate", (Transform::default(), Crate { hp: 3 }));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        if w.tick() == 30 {
            w.require_mut::<Crate>("crate").hp -= 1;
        }
    }
}
struct Dressed;
impl Game for Dressed {
    const ID: &'static str = "presentation";
    type Args = ();
    fn setup(w: &mut World, a: &()) {
        Plain::setup(w, a);
    }
    fn tick(w: &mut World, i: &Input, a: &()) {
        Plain::tick(w, i, a);
    }
    fn present(w: &mut World, _: &()) {
        let crate_ = w.named("crate").unwrap();
        let mut rng = w.presentation_rng(7);
        let bob = Bob {
            height: math::sin(w.tick() as f32 * 0.1) * 0.05,
            flash: rng.range(0.0..1.0),
        };
        w.insert(crate_, bob);
    }
}

#[test]
fn presentation_state_moves_no_hash_save_or_rest() {
    let mut plain = Sim::<Plain>::new(()).unwrap();
    let mut dressed = Sim::<Dressed>::new(()).unwrap();
    plain.run(1000.);
    dressed.run(1000.);
    assert_eq!(plain.world().hash(), dressed.world().hash());
    assert_eq!(plain.save().unwrap(), dressed.save().unwrap());
    assert!(dressed.world().get::<Bob>("crate").is_some());
    // A world whose only change is visual comes to rest.
    assert!(dressed.settle());
    assert_eq!(plain.world().hash(), dressed.world().hash());
}

#[test]
fn presentation_is_rebuilt_at_every_boundary_from_the_tick() {
    let mut a = Sim::<Dressed>::new(()).unwrap();
    a.run(500.);
    let bob = |s: &Sim<Dressed>| {
        let b = s.world().require::<Bob>("crate");
        (b.height.to_bits(), b.flash.to_bits())
    };
    let at = bob(&a);
    let saved = a.save().unwrap();
    let mut b = Sim::<Dressed>::new(()).unwrap();
    b.restore(&saved).unwrap();
    assert_eq!(bob(&b), at, "restore presents the same boundary");
    // The presentation stream is the tick's own, never the world's.
    let before = a.world().rng().next_u32();
    let mut c = Sim::<Plain>::new(()).unwrap();
    c.run(500.);
    assert_eq!(c.world().rng().next_u32(), before);
    for mode in [Paranoid::Save, Paranoid::FreshGame] {
        let mut p = Sim::<Dressed>::new(()).unwrap().paranoid(mode);
        p.run(500.);
        assert_eq!(bob(&p), at, "{mode:?}");
    }
}

#[test]
#[should_panic(expected = "Game::present spawned or despawned an entity")]
fn presenting_cannot_spawn() {
    struct Spawns;
    impl Game for Spawns {
        const ID: &'static str = "spawns";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(w: &mut World, _: &()) {
            w.spawn(());
        }
    }
    let _ = Sim::<Spawns>::new(());
}
