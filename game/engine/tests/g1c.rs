use exact_game::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Default, Args)]
struct Options {
    #[live]
    paused: bool,
}
#[derive(Default, Resource)]
struct Counter(u32);
struct Still;
impl Game for Still {
    type Args = Options;
    const ID: &'static str = "g1c";
    fn setup(w: &mut World, _: &Options) {
        w.spawn_named("player", Transform::default());
        w.insert_resource(Counter(0));
    }
    fn paused(a: &Options) -> bool {
        a.paused
    }
    fn tick(_: &mut World, _: &Input, _: &Options) {}
}
#[derive(Default, Data)]
#[allow(non_snake_case)]
struct Reply {
    quiescent: bool,
    changing: Vec<String>,
    settleAt: f64,
}
fn reply<G: Game>(s: &mut Sim<G>, settle: bool) -> Reply {
    json::from_str(&s.agent(&format!(r#"{{"op":"clock","settle":{settle}}}"#))).unwrap()
}
#[test]
fn leases_invalidate_observation_and_restart_backoff() {
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    for edit in 0..5 {
        assert!(s.settle());
        let tick = s.world().tick();
        match edit {
            0 => s.world().get_mut::<Transform>("player").unwrap().position.x += 1.0,
            1 => {
                for mut t in s.world().query::<&mut Transform>() {
                    t.position.x += 1.0;
                }
            }
            2 => s.world().resource_mut::<Counter>().0 += 1,
            3 => {
                s.world().rng();
            }
            _ => s.world().busy("external work"),
        }
        assert!(!s.quiescent(), "lease {edit}");
        reply(&mut s, true);
        reply(&mut s, true);
        s.world().resource_mut::<Counter>().0 += 1;
        assert_eq!(
            reply(&mut s, false).settleAt,
            tick as f64 * 1000.0 / 60.0 + 100.0
        );
        assert!(s.settle());
        assert!(s.world().tick() > tick);
    }
}
#[test]
fn rng_draws_are_observed_without_changing_hash_format() {
    struct Random;
    impl Game for Random {
        type Args = ();
        const ID: &'static str = "rng";
        fn setup(_: &mut World, _: &()) {}
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.rand(0u32..10);
        }
    }
    let mut s = Sim::<Random>::new(()).unwrap();
    s.run(100.0);
    let r = reply(&mut s, false);
    assert!(!r.quiescent);
    assert!(r.changing.contains(&"resource.Rng".into()));
    let hash = s.world().hash();
    let mut restored = World::new(60, 0);
    restored.load(&s.world().save()).unwrap();
    assert_eq!(hash, restored.hash());
}
#[test]
fn pause_explains_unobserved_rest_and_unpause_requires_a_sample() {
    let mut s = Sim::<Still>::new(Options { paused: true }).unwrap();
    let r = reply(&mut s, false);
    assert!(r.quiescent);
    assert_eq!(r.changing, ["paused"]);
    s.bind(&Options::default().values(), None).unwrap();
    assert!(!s.quiescent());
    assert!(s.settle());
}
#[test]
fn parent_only_marker_observes_ambient_conveyor() {
    struct Conveyor;
    impl Game for Conveyor {
        type Args = ();
        const ID: &'static str = "conveyor";
        fn setup(w: &mut World, _: &()) {
            let p = w.spawn_named("belt", (Transform::default(), Ambient));
            w.spawn_named("marker", Parent(p));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.get_mut::<Transform>("belt").unwrap().position.x += 1.0;
        }
    }
    let mut s = Sim::<Conveyor>::new(()).unwrap();
    s.run(100.0);
    let r = reply(&mut s, false);
    assert!(!r.quiescent);
    assert!(r.changing.contains(&"marker.global".into()));
}
static MOVES: AtomicUsize = AtomicUsize::new(0);
static WRITES: AtomicUsize = AtomicUsize::new(0);
#[derive(Default)]
struct Counted;
impl Component for Counted {
    const NAME: &'static str = "Counted";
}
impl Data for Counted {
    fn moving(&self, _: Now) -> bool {
        MOVES.fetch_add(1, Ordering::Relaxed);
        true
    }
    fn settle_tick(&self, _: Now) -> Option<u64> {
        None
    }
    fn write(&self, w: &mut dyn Writer) {
        WRITES.fetch_add(1, Ordering::Relaxed);
        0u32.write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        u32::default().read(r)
    }
}
#[test]
fn clock_streams_eight_reasons_and_serializes_each_row_only_twice() {
    struct Many;
    impl Game for Many {
        type Args = ();
        const ID: &'static str = "many";
        fn setup(w: &mut World, _: &()) {
            for _ in 0..200 {
                w.spawn(Counted);
            }
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut s = Sim::<Many>::new(()).unwrap();
    WRITES.store(0, Ordering::Relaxed);
    s.run(100.0);
    MOVES.store(0, Ordering::Relaxed);
    let r = reply(&mut s, false);
    assert_eq!(r.changing.len(), 8);
    assert!(
        MOVES.load(Ordering::Relaxed) <= 9,
        "reason visits {}",
        MOVES.load(Ordering::Relaxed)
    );
    assert_eq!(WRITES.load(Ordering::Relaxed), 400);
}
#[test]
fn helper_matches_hosts_initial_read_and_fifteen_followups() {
    struct Deadline<const END: u64>;
    impl<const END: u64> Game for Deadline<END> {
        type Args = ();
        const ID: &'static str = "last-round";
        fn setup(_: &mut World, _: &()) {}
        fn tick(w: &mut World, _: &Input, _: &()) {
            if w.tick() < END {
                w.busy("deadline");
            }
        }
    }
    let mut helper = Sim::<Deadline<1500>>::new(()).unwrap();
    let mut host = Sim::<Deadline<1500>>::new(()).unwrap();
    host.advance(0.0, Clock::Seekable);
    let mut r = reply(&mut host, true);
    for _ in 1..16 {
        if r.quiescent {
            break;
        }
        host.advance(r.settleAt, Clock::Seekable);
        r = reply(&mut host, true);
    }
    assert_eq!(helper.settle(), r.quiescent);
    assert_eq!(helper.world().tick(), host.world().tick());
    // The 15th follow-up lands at 23.1s, tick 1386; its two samples must be still.
    let mut final_round = Sim::<Deadline<1384>>::new(()).unwrap();
    assert!(final_round.settle());
    assert_eq!(final_round.world().tick(), 1386);
}
#[derive(Default, Args)]
struct Scalars {
    x: f32,
    y: f64,
    u: u64,
    i: i64,
}
struct Checked;
impl Game for Checked {
    type Args = Scalars;
    const ID: &'static str = "scalars";
    fn validate(a: &Scalars) -> Result<(), String> {
        assert!(a.x.is_finite() && a.y.is_finite());
        Ok(())
    }
    fn setup(_: &mut World, _: &Scalars) {}
    fn tick(_: &mut World, _: &Input, _: &Scalars) {}
}
#[test]
fn typed_scalars_refuse_before_domain_validation() {
    for a in [
        Scalars {
            x: f32::NAN,
            ..Default::default()
        },
        Scalars {
            x: f32::INFINITY,
            ..Default::default()
        },
        Scalars {
            y: f64::NEG_INFINITY,
            ..Default::default()
        },
        Scalars {
            u: 9_007_199_254_740_992,
            ..Default::default()
        },
        Scalars {
            i: -9_007_199_254_740_992,
            ..Default::default()
        },
    ] {
        assert!(Sim::<Checked>::new(a).is_err());
    }
}
#[test]
fn bound_restore_validates_only_the_arguments_it_uses() {
    #[derive(Default, Args)]
    struct Volume {
        volume: f32,
    }
    struct Old;
    struct New;
    impl Game for Old {
        type Args = Volume;
        const ID: &'static str = "volume";
        fn setup(_: &mut World, _: &Volume) {}
        fn tick(_: &mut World, _: &Input, _: &Volume) {}
    }
    impl Game for New {
        type Args = Volume;
        const ID: &'static str = "volume";
        fn validate(a: &Volume) -> Result<(), String> {
            if a.volume > 1.0 {
                Err("volume".into())
            } else {
                Ok(())
            }
        }
        fn setup(_: &mut World, _: &Volume) {}
        fn tick(_: &mut World, _: &Input, _: &Volume) {}
    }
    let old = Sim::<Old>::new(Volume { volume: 2.0 }).unwrap().save();
    let mut new = Sim::<New>::new(Volume { volume: 0.5 }).unwrap();
    assert!(new.restore(&old).is_err());
    new.restore_bound(&old).unwrap();
    assert_eq!(new.args().volume, 0.5);
    assert!(new.agent(r#"{"op":"state"}"#).contains(&format!(
        "\"restoredFrom\":{}",
        json::to_string(&Volume { volume: 2.0 }).unwrap()
    )));
}
#[test]
fn extreme_tween_deadline_saturates_host_microseconds() {
    #[derive(Default, Component)]
    struct Long(Tween);
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    let mut t = Tween::new(0.0);
    t.to(s.world().now(), 1.0, f32::MAX);
    s.world_mut().spawn(Long(t));
    s.advance(0.0, Clock::Seekable);
    assert_eq!(reply(&mut s, false).settleAt, i64::MAX as f64 / 1000.0);
}
#[test]
fn follow_names_track_lowest_live_index() {
    let mut w = World::new(60, 0);
    let hole = w.spawn(());
    w.spawn_named("player", Transform::at(5.0, 0.0, 0.0));
    let camera = w.spawn((Transform::default(), Follow::new("player")));
    scene::follow(&w);
    w.despawn(hole);
    w.spawn_named("player", Transform::at(9.0, 0.0, 0.0));
    scene::follow(&w);
    assert_eq!(
        w.get::<Transform>(camera).unwrap().position,
        w.get::<Transform>("player").unwrap().position
    );
}
#[test]
fn follow_roll_transports_continuously_through_vertical() {
    let mut w = World::new(60, 0);
    let p = w.spawn(Transform::default());
    let c = w.spawn((Transform::default(), Follow::new(p).offset(0.0, 1.0, 0.02)));
    scene::follow(&w);
    let mut previous = w.get::<Transform>(c).unwrap().rotation;
    for i in -200..=200 {
        w.get_mut::<Follow>(c).unwrap().offset = Vec3::new(0.0, 1.0, -i as f32 * 0.0001);
        scene::follow(&w);
        let rotation = w.get::<Transform>(c).unwrap().rotation;
        assert!(
            previous.angle_between(rotation) < 0.05,
            "step {i}: {}",
            previous.angle_between(rotation)
        );
        previous = rotation;
    }
}
#[test]
#[ignore = "release clock operation cost diagnostic"]
fn clock_200k_cost() {
    struct Large;
    impl Game for Large {
        type Args = ();
        const ID: &'static str = "clock-cost";
        fn setup(w: &mut World, _: &()) {
            for i in 0..200_000 {
                w.spawn(Transform::at(i as f32, 0.0, 0.0));
            }
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut s = Sim::<Large>::new(()).unwrap();
    let mut samples = Vec::new();
    for i in 0..105 {
        let start = std::time::Instant::now();
        s.run(17.0);
        std::hint::black_box(s.agent(r#"{"op":"clock"}"#));
        if i >= 5 {
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "clock 200k median={:.6} ms p95={:.6} ms",
        samples[50], samples[95]
    );
}

#[test]
fn storage_epoch_and_fused_hash_cover_structural_edits_and_messages() {
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    assert!(s.settle());
    let mut epoch = s.world().mutation_epoch();
    let e = s.world_mut().spawn(());
    assert!(s.world().mutation_epoch() > epoch);
    epoch = s.world().mutation_epoch();
    s.world_mut().insert(e, Transform::default());
    assert!(s.world().mutation_epoch() > epoch);
    epoch = s.world().mutation_epoch();
    s.world_mut().remove::<Transform>(e);
    assert!(s.world().mutation_epoch() > epoch);
    epoch = s.world().mutation_epoch();
    s.world_mut().teleport(e, Transform::at(3.0, 2.0, 1.0));
    assert!(s.world().mutation_epoch() > epoch);
    epoch = s.world().mutation_epoch();
    s.world_mut().despawn(e);
    assert!(s.world().mutation_epoch() > epoch);
    s.run(100.0);
    s.world().emit("message");
    let saved = s.world().save();
    let expected = s.world().hash();
    epoch = s.world().mutation_epoch();
    s.world_mut().load(&saved).unwrap();
    assert!(s.world().mutation_epoch() > epoch);
    assert_eq!(s.world().hash(), expected);
    assert_ne!(s.take_messages(), Vec::<String>::new());
    assert_ne!(s.world().hash(), expected);
    s.run(100.0);
    let expected = s.world().hash();
    let saved = s.world().save();
    s.world_mut().load(&saved).unwrap();
    assert_eq!(s.world().hash(), expected);
}

#[test]
fn a_publication_is_a_mutation_and_requires_an_observation() {
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    assert!(s.settle());
    let epoch = s.world().mutation_epoch();
    s.world().publish("score", 1);
    assert_eq!(s.world().published("score").unwrap().as_number(), Some(1.0));
    assert_ne!(
        s.world().mutation_epoch(),
        epoch,
        "a publication is a mutation"
    );
    assert!(
        !s.quiescent(),
        "changed publication answers quiescent without another observation"
    );
    assert!(s.settle());
}
