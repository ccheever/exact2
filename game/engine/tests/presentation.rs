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
#[should_panic(expected = "Game::present changed simulation state (spawned an entity)")]
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

#[test]
#[should_panic(expected = "a tick read or wrote presentation component `Bob`")]
fn a_tick_cannot_read_presentation_state() {
    struct Peeks;
    impl Game for Peeks {
        const ID: &'static str = "peeks";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            // Branching on drawn state would diverge after a restore.
            if w.get::<Bob>("crate").is_some_and(|b| b.flash > 0.5) {
                w.require_mut::<Crate>("crate").hp = 0;
            }
        }
        fn present(w: &mut World, a: &()) {
            Dressed::present(w, a);
        }
    }
    Sim::<Peeks>::new(()).unwrap().run(100.);
}

#[test]
#[should_panic(expected = "a tick read or wrote presentation component `Bob`")]
fn a_tick_cannot_query_presentation_state() {
    struct Scans;
    impl Game for Scans {
        const ID: &'static str = "scans";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            let _ = w.query::<&Bob>().iter().count();
        }
    }
    Sim::<Scans>::new(()).unwrap().run(100.);
}

#[test]
fn present_rebuilds_from_nothing_so_a_stateful_present_cannot_drift() {
    // Counts its own presents: continuous and restored worlds would differ if
    // the previous present's rows survived into the next.
    #[derive(Default, Presentation)]
    struct Count(u32);
    struct Counts;
    impl Game for Counts {
        const ID: &'static str = "counts";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(w: &mut World, i: &Input, a: &()) {
            Plain::tick(w, i, a);
        }
        fn present(w: &mut World, _: &()) {
            let e = w.named("crate").unwrap();
            let seen = w.get::<Count>(e).map_or(0, |c| c.0);
            w.insert(e, Count(seen + 1));
        }
    }
    let mut a = Sim::<Counts>::new(()).unwrap();
    a.run(500.);
    assert_eq!(a.world().require::<Count>("crate").0, 1);
    let mut b = Sim::<Counts>::new(()).unwrap();
    b.restore(&a.save().unwrap()).unwrap();
    assert_eq!(b.world().require::<Count>("crate").0, 1);
}

/// A game whose present makes one simulation change; the panic names it.
fn presents(change: fn(&mut World)) -> String {
    thread_local!(static CHANGE: std::cell::Cell<Option<fn(&mut World)>> = const { std::cell::Cell::new(None) });
    struct Meddles;
    impl Game for Meddles {
        const ID: &'static str = "meddles";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(w: &mut World, i: &Input, a: &()) {
            Plain::tick(w, i, a);
        }
        fn present(w: &mut World, _: &()) {
            if w.tick() > 0 {
                CHANGE.with(|c| c.get().unwrap())(w);
            }
        }
    }
    CHANGE.with(|c| c.set(Some(change)));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Sim::<Meddles>::new(()).unwrap().run(100.);
    }));
    let error = result.expect_err("present's simulation change must panic");
    error.downcast_ref::<String>().cloned().unwrap_or_default()
}

#[test]
fn present_cannot_change_simulation_state() {
    type Change = fn(&mut World);
    let cases: [(Change, &str); 9] = [
        // Busy reasons are saved and hashed simulation state.
        (|w| w.busy("drawing"), "reported busy `drawing`"),
        (
            |w| w.require_mut::<Transform>("crate").position.x += 1.,
            "wrote component `Transform`",
        ),
        (
            |w| {
                let e = w.named("crate").unwrap();
                w.insert(e, Crate { hp: 9 });
            },
            "inserted component `Crate`",
        ),
        (
            |w| {
                for (_, t) in w.query::<&mut Transform>().iter() {
                    t.position.y = 1.;
                }
            },
            "queried `Transform` mutably",
        ),
        (
            |w| {
                w.rand(0..3u32);
            },
            "drew from World::rng",
        ),
        (|w| w.emit("hello"), "emitted a message"),
        (|w| w.log("note"), "journaled `note`"),
        (|w| w.publish("score", 3.0), "published `score`"),
        (|w| emitter::step(w), "queried `Emitter` mutably"),
    ];
    for (change, named) in cases {
        let message = presents(change);
        assert!(
            message.contains("Game::present changed simulation state") && message.contains(named),
            "{named}: {message}"
        );
    }
}

#[test]
fn a_save_carrying_presentation_rows_is_refused_by_name() {
    // A save written where `Bob` was simulation state, read where it is drawn.
    mod sim_side {
        #[derive(Default, exact_game::Component)]
        pub struct Bob {
            pub height: f32,
            pub flash: f32,
        }
    }
    let mut writer = World::new(60, 1);
    let e = writer.spawn(Transform::default());
    writer.insert(
        e,
        sim_side::Bob {
            height: 1.,
            flash: 0.,
        },
    );
    let bytes = writer.save();
    let mut reader = World::new(60, 1);
    reader.register::<Bob>();
    let error = reader.load(&bytes).unwrap_err().to_string();
    assert!(error.contains("presentation component `Bob`"), "{error}");
}

#[test]
fn presentation_streams_differ_for_every_tick_and_salt() {
    // `tick ^ rotate(salt)` gave tick 1 salt 0 and tick 0 salt 2^35 one stream.
    let mut sim = Sim::<Plain>::new(()).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        for salt in (0..64u64).chain([1 << 35, 1 << 40, u64::MAX]) {
            let mut rng = sim.world().presentation_rng(salt);
            assert!(seen.insert((rng.next_u32(), rng.next_u32())));
        }
        sim.run(1000. / 60.);
    }
}

#[test]
fn a_present_that_fails_on_a_restored_world_refuses_the_restore() {
    // The same game ID saved by a build with no crate: this build's present
    // requires one, so the untrusted save is refused instead of crashing.
    struct Bare;
    impl Game for Bare {
        const ID: &'static str = "picky";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    struct Picky;
    impl Game for Picky {
        const ID: &'static str = "picky";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(w: &mut World, _: &()) {
            let crate_ = w.named("crate").expect("present needs the crate");
            w.insert(crate_, Bob::default());
        }
    }
    let mut bare = Sim::<Bare>::new(()).unwrap();
    bare.run(50.);
    let mut picky = Sim::<Picky>::new(()).unwrap();
    let before = picky.world().hash();
    let error = picky
        .restore(&bare.save().unwrap())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Game::present failed on the restored world: present needs the crate"),
        "{error}"
    );
    assert_eq!(
        picky.world().hash(),
        before,
        "the refused restore changed nothing"
    );
}

#[test]
fn drawn_state_follows_live_arguments_and_edits_without_a_tick() {
    #[derive(Default, Args)]
    struct Look {
        #[live]
        glow: f64,
    }
    struct Glows;
    impl Game for Glows {
        const ID: &'static str = "glows";
        type Args = Look;
        fn setup(w: &mut World, _: &Look) {
            w.spawn_named("crate", (Transform::default(), Crate { hp: 3 }));
        }
        fn tick(_: &mut World, _: &Input, _: &Look) {}
        fn paused(_: &Look) -> bool {
            true
        }
        fn present(w: &mut World, look: &Look) {
            let e = w.named("crate").unwrap();
            let hp = w.require::<Crate>(e).hp;
            w.insert(
                e,
                Bob {
                    height: hp as f32,
                    flash: look.glow as f32,
                },
            );
        }
    }
    let mut s = Sim::<Glows>::new(Look::default()).unwrap();
    s.bind(&[Value::Number(0.5)], None).unwrap();
    assert_eq!(s.world().require::<Bob>("crate").flash, 0.5);
    s.world_mut().require_mut::<Crate>("crate").hp = 7;
    s.run(0.);
    assert_eq!(s.world().require::<Bob>("crate").height, 7.);
}
