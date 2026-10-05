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
    fn present(p: &mut Present<'_>, _: &()) {
        let crate_ = p.named("crate").unwrap();
        let mut rng = p.rng(7);
        let bob = Bob {
            height: math::sin(p.tick() as f32 * 0.1) * 0.05,
            flash: rng.range(0.0..1.0),
        };
        p.insert(crate_, bob);
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
        fn present(p: &mut Present<'_>, a: &()) {
            Dressed::present(p, a);
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
        fn present(p: &mut Present<'_>, _: &()) {
            let e = p.named("crate").unwrap();
            let seen = p.get::<Count>(e).map_or(0, |c| c.0);
            p.insert(e, Count(seen + 1));
        }
    }
    let mut a = Sim::<Counts>::new(()).unwrap();
    a.run(500.);
    assert_eq!(a.world().require::<Count>("crate").0, 1);
    let mut b = Sim::<Counts>::new(()).unwrap();
    b.restore(&a.save().unwrap()).unwrap();
    assert_eq!(b.world().require::<Count>("crate").0, 1);
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
        fn present(p: &mut Present<'_>, _: &()) {
            let crate_ = p.named("crate").expect("present needs the crate");
            p.insert(crate_, Bob::default());
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
        fn present(p: &mut Present<'_>, look: &Look) {
            let e = p.named("crate").unwrap();
            let hp = p.require::<Crate>(e).hp;
            p.insert(
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

#[test]
fn present_clears_sparse_rows_across_many_slots() {
    // Rows far apart in a large world: each present erases exactly the old ones.
    struct Sparse;
    impl Game for Sparse {
        const ID: &'static str = "sparse";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            for _ in 0..20_000 {
                w.spawn(Transform::default());
            }
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut Present<'_>, _: &()) {
            let n = p.tick() as u32;
            let wanted = [n % 64, 7_000 + n % 64, 19_999 - n % 64];
            let mut chosen = Vec::new();
            p.for_each::<Transform>(|e, _| {
                if wanted.contains(&e.index()) {
                    chosen.push(e);
                }
            });
            for e in chosen {
                p.insert(e, Bob::default());
            }
        }
    }
    let mut s = Sim::<Sparse>::new(()).unwrap();
    let started = std::time::Instant::now();
    s.run(1000.);
    eprintln!("60 presents over 20,000 slots: {:?}", started.elapsed());
    assert_eq!(s.world().query::<&Bob>().iter().count(), 3);
}

#[test]
#[should_panic(expected = "a tick read or wrote presentation component `Bob`")]
fn a_tick_cannot_watch_presentation_revisions() {
    struct Watches;
    impl Game for Watches {
        const ID: &'static str = "watches";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            let _ = w.revision::<Bob>();
        }
    }
    Sim::<Watches>::new(()).unwrap().run(100.);
}

#[test]
#[should_panic(expected = "Game::setup wrote presentation component `Bob`")]
fn setup_cannot_write_presentation_state() {
    struct Early;
    impl Game for Early {
        const ID: &'static str = "early";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
            let e = w.named("crate").unwrap();
            w.insert(e, Bob::default());
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let _ = Sim::<Early>::new(());
}

// Presents counted across a whole process, by game.
static PRESENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Counted;
impl Game for Counted {
    const ID: &'static str = "counted";
    type Args = ();
    fn setup(w: &mut World, a: &()) {
        Plain::setup(w, a);
    }
    fn tick(w: &mut World, i: &Input, a: &()) {
        Plain::tick(w, i, a);
    }
    fn present(p: &mut Present<'_>, a: &()) {
        PRESENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Dressed::present(p, a);
    }
}

#[test]
fn a_long_seek_presents_only_what_it_shows_and_paranoid_presents_every_tick() {
    let count = || PRESENTS.load(std::sync::atomic::Ordering::Relaxed);
    let mut seek = Sim::<Counted>::new(()).unwrap();
    let before = count();
    seek.run(10_000.); // 600 ticks
    let boundary_only = count() - before;
    assert!(boundary_only <= 3, "{boundary_only} presents for one seek");
    let mut every = Sim::<Counted>::new(()).unwrap().paranoid(Paranoid::Save);
    let before = count();
    every.run(10_000.);
    assert!(count() - before >= 600, "paranoid presents every tick");
    // Both present the same drawn state at the observed boundary.
    let bob = |s: &Sim<Counted>| {
        let b = s.world().require::<Bob>("crate");
        (b.height.to_bits(), b.flash.to_bits())
    };
    assert_eq!(bob(&seek), bob(&every));
    assert_eq!(seek.world().hash(), every.world().hash());
}

#[test]
fn restore_presents_after_publications_are_back() {
    struct Scores;
    impl Game for Scores {
        const ID: &'static str = "scores";
        type Args = ();
        fn setup(w: &mut World, a: &()) {
            Plain::setup(w, a);
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.publish("score", w.tick() as f64);
        }
        fn present(p: &mut Present<'_>, _: &()) {
            let score = match p.published("score") {
                Some(Value::Number(n)) => n as f32,
                _ => -1.,
            };
            let e = p.named("crate").unwrap();
            p.insert(
                e,
                Bob {
                    height: score,
                    flash: 0.,
                },
            );
        }
    }
    let mut a = Sim::<Scores>::new(()).unwrap();
    a.run(500.);
    let mut b = Sim::<Scores>::new(()).unwrap();
    b.restore(&a.save().unwrap()).unwrap();
    let height = |s: &Sim<Scores>| s.world().require::<Bob>("crate").height;
    assert_eq!(height(&a), 29.); // published during the 30th tick
    assert_eq!(height(&b), height(&a));
}

#[test]
fn presenting_is_not_a_simulation_mutation() {
    #[derive(Default, Args)]
    struct Look {
        #[live]
        glow: f64,
    }
    struct Glows;
    impl Game for Glows {
        const ID: &'static str = "glows-epoch";
        type Args = Look;
        fn setup(w: &mut World, a: &Look) {
            Plain::setup(w, &());
            let _ = a;
        }
        fn tick(_: &mut World, _: &Input, _: &Look) {}
        fn paused(_: &Look) -> bool {
            true
        }
        fn present(p: &mut Present<'_>, look: &Look) {
            let e = p.named("crate").unwrap();
            p.insert(
                e,
                Bob {
                    height: look.glow as f32,
                    flash: 0.,
                },
            );
        }
    }
    let mut s = Sim::<Glows>::new(Look::default()).unwrap();
    let epoch = s.world().mutation_epoch();
    s.bind(&[Value::Number(0.5)], None).unwrap();
    assert_eq!(s.world().require::<Bob>("crate").height, 0.5);
    assert_eq!(s.world().mutation_epoch(), epoch);
}

/// A look that swaps a mesh, a light and the sky from `present`, switched by
/// a live argument: the simulation, its hash and its save are the plain
/// game's in either look, and the swapped model is requested like a `Mesh`'s.
#[test]
fn a_live_look_swaps_meshes_lights_and_the_sky_without_moving_the_save() {
    #[derive(Default, Args)]
    struct Look {
        #[live]
        night: bool,
    }
    struct Lit;
    impl Game for Lit {
        const ID: &'static str = "presentation";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            Plain::setup(w, &());
            w.spawn_named("lamp", Transform::at(0., 2., 0.));
            w.spawn_named("camera", (Transform::default(), Camera::default()));
        }
        fn tick(w: &mut World, i: &Input, _: &()) {
            Plain::tick(w, i, &());
        }
    }
    struct Night;
    impl Game for Night {
        const ID: &'static str = "presentation";
        type Args = Look;
        fn setup(w: &mut World, _: &Look) {
            Lit::setup(w, &());
        }
        fn tick(w: &mut World, i: &Input, _: &Look) {
            Lit::tick(w, i, &());
        }
        fn present(p: &mut Present<'_>, look: &Look) {
            if !look.night {
                return;
            }
            let crate_ = p.named("crate").unwrap();
            let stage = p.require::<Crate>(crate_).hp;
            p.insert(
                crate_,
                DrawnMesh::model(format!("crate-{stage}.model"))
                    .material(Material::rgb(0.2, 0.2, 0.3)),
            );
            let lamp = p.named("lamp").unwrap();
            p.insert(
                lamp,
                DrawnLight::Point(PointLight {
                    intensity: 300.,
                    ..PointLight::default()
                }),
            );
            let camera = p.named("camera").unwrap();
            p.insert(
                camera,
                DrawnEnvironment {
                    environment: Environment {
                        exposure: 1.6,
                        ..Environment::default()
                    },
                    ambient_occlusion: None,
                },
            );
        }
    }
    let mut plain = Sim::<Lit>::new(()).unwrap();
    let mut night = Sim::<Night>::new(Look::default()).unwrap();
    night.bind(&[Value::Bool(true)], None).unwrap();
    plain.run(1000.);
    night.run(1000.);
    let w = night.world();
    assert_eq!(
        w.require::<DrawnMesh>("crate").mesh,
        Mesh::asset("crate-2.model"),
        "present reads the simulation it draws"
    );
    assert!(w.get::<DrawnLight>("lamp").is_some());
    assert_eq!(
        w.require::<DrawnEnvironment>("camera").environment.exposure,
        1.6
    );
    assert!(night.take_assets().contains(&"crate-2.model".to_owned()));
    // Unloaded, a drawn model never holds a save back: saves check simulated
    // meshes. The saves differ only in the arguments they carry.
    assert_eq!(plain.world().hash(), night.world().hash());
    night.save().unwrap();
    // Switching the look back is live: the same world, nothing drawn in its place.
    night.bind(&[Value::Bool(false)], None).unwrap();
    assert!(night.world().get::<DrawnMesh>("crate").is_none());
    assert!(night.world().get::<DrawnLight>("lamp").is_none());
    assert_eq!(plain.world().hash(), night.world().hash());
}
