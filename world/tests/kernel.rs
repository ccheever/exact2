use exact_world::*;
use std::{
    cell::Cell,
    panic::{catch_unwind, AssertUnwindSafe},
};
#[derive(Default, Component)]
struct Count(u64);
#[derive(Default, Args)]
struct Options {
    seed: u64,
    #[live]
    paused: bool,
    #[restart]
    restart: bool,
    text: String,
}
struct Counter;
impl Game for Counter {
    const ID: &'static str = "counter";
    const ACTIONS: &'static [Action] = &[
        Action::button("add", &["KeyA", "Space"]),
        Action::axis("horizontal", "KeyL", "KeyR"),
    ];
    type Args = Options;
    fn register(w: &mut World, args: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
        assert!(args.get("paused").is_none());
        assert!(matches!(
            args.get("seed"),
            Some(args::ArgumentRef::Unsigned(_))
        ));
        w.register::<Count>().unwrap();
        Ok(())
    }
    fn setup(w: &mut World, a: &Options) {
        w.reseed(a.seed);
        w.spawn_named("counter", Count(0)).unwrap();
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let random = w.rng().next_u32() as u64;
        let n = {
            let mut c = w.get_mut::<Count>("counter").unwrap();
            c.0 = c.0.wrapping_add(random + u64::from(input.pressed("add")));
            c.0
        };
        w.publish("count", (n % 1_000_000) as u32).unwrap();
        *w.derived::<u64>() = n; // Reconstructible, never a dependency of the next tick.
    }
    fn paused(a: &Options) -> bool {
        a.paused
    }
}
struct Still;
impl Game for Still {
    const ID: &'static str = "still";
    type Args = ();
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
        w.register::<Count>().unwrap();
        Ok(())
    }
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn idempotent_recursive_registration_runs_hook_once() {
    thread_local! { static CALLS: Cell<u32> = const { Cell::new(0) }; }
    #[derive(Default, Data)]
    struct Hook(u32);
    impl Component for Hook {
        const NAME: &'static str = "Hook";
        fn register(w: &mut World) -> Result<(), DataError> {
            CALLS.set(CALLS.get() + 1);
            w.register::<Hook>().unwrap();
            Ok(())
        }
    }
    CALLS.set(0);
    let mut w = World::new(60, 0);
    w.register::<Count>().unwrap();
    w.register::<Hook>().unwrap();
    for n in 0..100 {
        w.spawn(Hook(n)).unwrap();
        w.register::<Hook>().unwrap();
    }
    assert_eq!(CALLS.get(), 1);
    w.load(&w.save().unwrap()).unwrap();
    assert_eq!(CALLS.get(), 1);
    assert_eq!(w.query::<&Hook>().iter().count(), 100);
}
#[test]
fn ownership_validates_cycles_without_any_pose_and_reaps_reverse_chains() {
    let mut w = World::new(60, 0);
    w.register::<Count>().unwrap();
    let consumer = w.subscribe_changes().unwrap();
    let es: Vec<_> = (0..257).map(|_| w.spawn(()).unwrap()).collect();
    for pair in es.windows(2) {
        w.set_parent(pair[0], Some(pair[1])).unwrap();
    }
    let before = w.save().unwrap();
    assert!(w
        .set_parent(es[256], Some(es[0]))
        .unwrap_err()
        .message
        .contains("cycle"));
    assert_eq!(before, w.save().unwrap());
    assert!(w.try_query::<&mut Parent>().is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| w.get_mut::<Parent>(es[0]))).is_err());
    w.despawn(es[256]);
    w.reap_orphans().unwrap();
    assert!(w.is_empty());
    let removed: Vec<_> = w
        .changes(&consumer)
        .unwrap()
        .events
        .filter(|e| e.kind == ChangeKind::Despawn)
        .map(|e| e.entity.index())
        .collect();
    assert_eq!(removed[0], 256);
    assert_eq!(&removed[1..], &(0..256).collect::<Vec<_>>());
}
#[test]
fn decoded_ownership_cycle_refuses_atomically() {
    #[derive(Default, Data)]
    struct Forged(Entity);
    impl Component for Forged {
        const NAME: &'static str = "Parent";
    }
    let mut bad = World::new(60, 0);
    bad.register::<Forged>().unwrap();
    let a = bad.spawn(()).unwrap();
    let b = bad.spawn(()).unwrap();
    bad.insert(a, Forged(b)).unwrap();
    bad.insert(b, Forged(a)).unwrap();
    let mut w = World::new(60, 0);
    w.register::<Count>().unwrap();
    w.register::<Parent>().unwrap();
    w.spawn_named("retained", ()).unwrap();
    let before = w.save().unwrap();
    assert!(w
        .load(&bad.save().unwrap())
        .unwrap_err()
        .message
        .contains("cycle"));
    assert_eq!(w.save().unwrap(), before);
}
#[test]
fn journal_retains_generations_replacements_and_reparent_across_ticks_and_restore() {
    let mut s = Sim::<Still>::new(()).unwrap();
    let w = s.world_mut();
    let consumer = w.subscribe_changes().unwrap();
    let start = w.changes(&consumer).unwrap().next;
    let a = w.spawn(Count(1)).unwrap();
    let owner = w.spawn(()).unwrap();
    w.insert(a, Count(2)).unwrap();
    w.set_parent(a, Some(owner)).unwrap();
    w.despawn(a);
    let b = w.spawn(Count(3)).unwrap();
    assert_eq!(a.index(), b.index());
    assert_ne!(a.generation(), b.generation());
    let cursor = w.changes(&consumer).unwrap().next;
    s.run(1000.).unwrap();
    let retained: Vec<_> = s
        .world()
        .changes(&consumer)
        .unwrap()
        .events
        .cloned()
        .collect();
    assert_eq!(retained.len() as u64, cursor - start);
    assert!(retained
        .iter()
        .any(|e| e.entity == a && matches!(e.kind, ChangeKind::Replace(_))));
    assert!(retained
        .iter()
        .any(|e| e.entity == a && e.kind == ChangeKind::Reparent(Some(owner))));
    assert!(retained
        .iter()
        .any(|e| e.entity == b && e.kind == ChangeKind::Spawn));
    let bytes = s.save().unwrap();
    s.restore(&bytes).unwrap();
    assert_eq!(
        s.world()
            .changes(&consumer)
            .unwrap()
            .events
            .take(retained.len())
            .cloned()
            .collect::<Vec<_>>(),
        retained
    );
    assert_eq!(
        s.world()
            .changes(&consumer)
            .unwrap()
            .events
            .last()
            .unwrap()
            .kind,
        ChangeKind::Reset
    );
    let cursor = s.world().changes(&consumer).unwrap().next;
    s.world_mut()
        .acknowledge_changes(&consumer, cursor)
        .unwrap();
    assert_eq!(s.world().changes(&consumer).unwrap().events.count(), 0);
}
#[test]
fn pending_io_is_distinct_from_saved_simulation_deadlines() {
    let mut s = Sim::<Still>::new(()).unwrap();
    s.world().work("timer", Work::Deadline(7)).unwrap();
    assert_eq!(s.settle(20).unwrap(), 7);
    assert_eq!(s.world().tick(), 7);
    s.world().work("fetch", Work::Pending).unwrap();
    let before = s.world().tick();
    assert!(s
        .settle(20)
        .unwrap_err()
        .message
        .contains("external readiness"));
    assert_eq!(s.world().tick(), before);
    let bytes = s.save().unwrap();
    let mut loaded = Sim::<Still>::from_save(&bytes).unwrap();
    assert_eq!(
        loaded.world().readiness(),
        Readiness::Pending(vec!["fetch".into()])
    );
    loaded
        .world()
        .work("fetch", Work::Failed("missing".into()))
        .unwrap();
    assert!(matches!(loaded.world().readiness(), Readiness::Failed(_)));
    loaded.world().work("fetch", Work::Ready).unwrap();
    loaded.settle(2).unwrap();
    for i in 0..62 {
        loaded.world().work(&i.to_string(), Work::Pending).unwrap();
    }
    assert!(loaded.world().work("too many", Work::Pending).is_err());
    if let Readiness::Pending(reasons) = loaded.world().readiness() {
        assert_eq!(reasons.len(), 8);
    } else {
        panic!("pending vanished");
    }
}
#[test]
fn typed_binary_args_versions_and_truncation_are_atomic() {
    let mut s = Sim::<Counter>::new(Options {
        seed: 9_007_199_254_740_991,
        text: "hello\0🌕".into(),
        ..Options::default()
    })
    .unwrap();
    s.run(200.).unwrap();
    let bytes = s.save().unwrap();
    let mut r = bin::Decoder::new(&bytes[7..]);
    r.begin_seq().unwrap();
    assert!(r.item().unwrap());
    assert_eq!(r.string().unwrap(), "counter");
    assert!(r.item().unwrap());
    let mut args = Options::default();
    args.read(&mut r).unwrap();
    assert_eq!(args.seed, 9_007_199_254_740_991);
    assert_eq!(args.text, "hello\0🌕");
    for version in [0, 1, 2, 3, 4, 5, 6, 7, 8, 255] {
        let mut bad = bytes.clone();
        bad[6] = version;
        assert!(s.restore(&bad).unwrap_err().message.contains("EXSIM v9"));
        assert_eq!(s.save().unwrap(), bytes);
    }
    for end in 0..bytes.len() {
        assert!(s.restore(&bytes[..end]).is_err());
        assert_eq!(s.save().unwrap(), bytes);
    }
    let next = Sim::<Counter>::from_save(&bytes).unwrap();
    assert_eq!(next.save().unwrap(), bytes);
}
#[test]
fn paranoid_modes_match_input_publications_rng_and_continuation_bytes() {
    let run = |mode| {
        let mut s = Sim::<Counter>::new(Options {
            seed: 71,
            ..Options::default()
        })
        .unwrap()
        .paranoid(mode);
        s.input(InputEvent::Key {
            code: "KeyA".into(),
            down: true,
            at_ms: 18.,
        })
        .unwrap();
        s.input(InputEvent::Key {
            code: "KeyA".into(),
            down: false,
            at_ms: 19.,
        })
        .unwrap();
        let mut checkpoints = Vec::new();
        for _ in 0..4 {
            s.run(125.).unwrap();
            checkpoints.push((
                s.world().hash(),
                s.save().unwrap(),
                s.world().publications().clone(),
            ));
        }
        assert!(checkpoints.windows(2).all(|w| w[0].0 != w[1].0));
        checkpoints
    };
    let off = run(Paranoid::Off);
    assert_eq!(off, run(Paranoid::Save));
    assert_eq!(off, run(Paranoid::FreshGame));
}
#[test]
fn restore_never_runs_setup_and_derived_slots_are_unsaved() {
    thread_local! { static SETUPS: Cell<u32> = const { Cell::new(0) }; }
    struct G;
    impl Game for G {
        const ID: &'static str = "no-setup";
        type Args = ();
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<Count>().unwrap();
            Ok(())
        }
        fn setup(w: &mut World, _: &()) {
            SETUPS.set(SETUPS.get() + 1);
            w.spawn(Count(4)).unwrap();
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    SETUPS.set(0);
    let mut s = Sim::<G>::new(()).unwrap();
    let bytes = s.save().unwrap();
    *s.world().derived::<u64>() = 42;
    assert_eq!(s.save().unwrap(), bytes);
    s.restore(&bytes).unwrap();
    assert_eq!(*s.world().derived::<u64>(), 0);
    assert_eq!(SETUPS.get(), 1);
    let _fresh = Sim::<G>::from_save(&bytes).unwrap();
    assert_eq!(SETUPS.get(), 1);
}
#[test]
fn borrowed_queries_and_safe_runs_cover_holes_padding_and_owned_data() {
    #[repr(C)]
    #[derive(Default, Component)]
    struct Padded {
        byte: u8,
        text: String,
        n: u64,
    }
    let mut w = World::new(60, 0);
    w.register::<Count>().unwrap();
    w.register::<Padded>().unwrap();
    let es: Vec<_> = (0..130)
        .map(|n| {
            w.spawn(Padded {
                byte: 1,
                text: format!("{n}"),
                n,
            })
            .unwrap()
        })
        .collect();
    for &i in &[0, 2, 3, 63, 64, 100] {
        w.remove::<Padded>(es[i]);
    }
    let pages = w.pages::<Padded>();
    let rows: Vec<_> = pages
        .iter()
        .flat_map(|p| {
            p.runs()
                .flat_map(|(start, run)| {
                    run.iter()
                        .enumerate()
                        .map(move |(i, v)| (start + i as u32, v.n))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(rows.len(), 124);
    assert!(rows.iter().all(|(i, n)| u64::from(*i) == *n));
    assert!(w.try_query::<&mut Padded>().is_err());
    drop(pages);
    let mut q = w.try_query::<&mut Padded>().unwrap();
    q.get(es[5]).unwrap().n = 900;
    assert!(q.get(es[3]).is_none());
    drop(q);
    assert_eq!(w.get::<Padded>(es[5]).unwrap().n, 900);
    assert_eq!(w.query::<&Padded>().iter().count(), 124);
}
#[test]
fn input_edges_axes_boundaries_and_queue_limits() {
    let mut s = Sim::<Counter>::new(Options::default()).unwrap();
    s.input(InputEvent::Key {
        code: "KeyA".into(),
        down: true,
        at_ms: 100.,
    })
    .unwrap();
    s.input(InputEvent::Key {
        code: "KeyA".into(),
        down: false,
        at_ms: 101.,
    })
    .unwrap();
    s.advance_to(100.).unwrap();
    assert!(!s.input_state().pressed("add"));
    s.run(17.).unwrap();
    assert!(s.input_state().pressed("add"));
    assert!(s.input_state().released("add"));
    assert!(!s.input_state().held("add"));
    s.input(InputEvent::Axis {
        name: "horizontal".into(),
        value: -0.5,
        at_ms: 117.,
    })
    .unwrap();
    s.run(17.).unwrap();
    assert_eq!(s.input_state().axis("horizontal"), -0.5);
    assert_eq!(stick_axis([100., 100.], [130., 70.]).unwrap(), [0.5, 0.5]);
    let axis = stick_axis([0., 0.], [60., -60.]).unwrap();
    assert!((axis[0] * axis[0] + axis[1] * axis[1] - 1.).abs() < 1e-6);
    for i in 0..1024 {
        s.input(InputEvent::Blur {
            at_ms: 10000. + i as f64,
        })
        .unwrap();
    }
    assert!(s.input(InputEvent::Blur { at_ms: 99999. }).is_err());
    let before = s.save().unwrap();
    assert!(s.run(4_000_000.).is_err());
    assert_eq!(s.save().unwrap(), before);
}
#[test]
fn bounded_inspection_refuses_large_values_and_reading_is_passive() {
    let mut w = World::new(60, 0);
    w.register::<Count>().unwrap();
    let e = w.spawn(Count(4)).unwrap();
    let before = w.save().unwrap();
    assert!(w.state(e).unwrap().contains('4'));
    assert_eq!(w.entities().take(512).count(), 1);
    assert!(!w.logs(LogCursor::default()).unwrap().entries.is_empty());
    assert_eq!(w.save().unwrap(), before);
    assert!(json::to_string(&"x".repeat(json::LIMIT + 1)).is_err());
    assert!(w.log(&"x".repeat(4097)).is_err());
    let budget = data::LoadBudget::new(32);
    assert!(
        bin::from_slice_in::<Vec<u8>>(&bin::to_vec(&vec![1u8; 33]).unwrap(), Some(&budget))
            .is_err()
    );
}
#[test]
fn ambient_motion_deadlines_and_same_value_leases_do_not_hide_changes() {
    let mut s = Sim::<Still>::new(()).unwrap();
    #[derive(Default, Component)]
    struct Motion(Tween);
    let mut tween = Tween::new(0.);
    tween.to(Now { tick: 0, hz: 60 }, 1., 1.);
    s.world_mut().register::<Motion>().unwrap();
    s.world_mut().register::<Ambient>().unwrap();
    let e = s.world_mut().spawn(Motion(tween)).unwrap();
    assert_eq!(s.world().settle_tick(), Some(60));
    s.world_mut().insert(e, Ambient).unwrap();
    assert_eq!(s.settle(2).unwrap(), 1);
    s.world_mut().remove::<Ambient>(e);
    assert_eq!(s.settle(100).unwrap(), 59);
    assert!(s.world().quiescent());
    drop(s.world().get_mut::<Motion>(e));
    assert!(!s.world().quiescent());
}

#[test]
#[ignore = "admission ceilings; explicit long workload"]
fn full_entity_and_journal_limits_refuse_without_losing_events() {
    let mut w = World::new(60, 0);
    w.register::<Count>().unwrap();
    for _ in 0..MAX_ENTITIES {
        w.spawn(()).unwrap();
    }
    assert!(w.spawn(()).is_err());
    assert_eq!(w.len(), MAX_ENTITIES);
    let consumer = w.subscribe_changes().unwrap();
    let e = w.resolve("#0").unwrap();
    for n in 0..1_000_000 {
        w.insert(e, Count(n)).unwrap();
    }
    w.insert(e, Count(0)).unwrap();
    assert!(w.changes(&consumer).unwrap().resync);
}

#[test]
fn saved_game_journal_excludes_session_telemetry_and_survives_restore() {
    let mut s = Sim::<Counter>::new(Options::default()).unwrap();
    s.run(17.).unwrap();
    s.world().log("game scored").unwrap();
    let before = s.save().unwrap();
    let cursor = s.world().journal_next();
    s.world().session_log("agent attached").unwrap();
    assert_eq!(s.save().unwrap(), before);
    assert_eq!(s.world().journal_next(), cursor);
    s.restore(&before).unwrap();
    assert_eq!(s.save().unwrap(), before);
    let logs = s.world().logs(LogCursor::default()).unwrap().entries;
    assert!(logs.contains("game scored"));
    assert!(logs.contains("agent attached"));
    assert!(logs.find("game scored") < logs.find("agent attached"));
    s.world().log("game next").unwrap();
    let logs = s.world().logs(LogCursor::default()).unwrap().entries;
    assert!(logs.find("agent attached") < logs.find("game next"));
    let fresh = Sim::<Counter>::from_save(&s.save().unwrap()).unwrap();
    assert!(!fresh
        .world()
        .logs(LogCursor::default())
        .unwrap()
        .entries
        .contains("agent attached"));
    assert!(fresh
        .world()
        .logs(LogCursor::default())
        .unwrap()
        .entries
        .contains("game next"));
}
#[test]
fn carry_keeps_live_args_refuses_setup_changes_and_publication_budget_is_cumulative() {
    let saved = Sim::<Counter>::new(Options::default())
        .unwrap()
        .save()
        .unwrap();
    let mut s = Sim::<Counter>::new(Options {
        paused: true,
        ..Options::default()
    })
    .unwrap();
    s.carry(&saved).unwrap();
    assert!(s.args().paused);
    s.bind(Options {
        seed: 7,
        ..Options::default()
    })
    .unwrap();
    let before = s.save().unwrap();
    assert!(s.carry(&saved).is_err());
    assert_eq!(s.save().unwrap(), before);
    s.world().publish("a", "a".repeat(6000)).unwrap();
    let before = s.save().unwrap();
    assert!(s.world().publish("b", "b".repeat(6000)).is_err());
    assert_eq!(s.save().unwrap(), before);
    s.restore(&before).unwrap();
}

#[test]
fn paranoid_preserves_drained_delivery() {
    struct Delivery;
    impl Game for Delivery {
        const ID: &'static str = "delivery";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.publish("fixed", 1u32).unwrap();
            w.emit("initial");
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.publish("fixed", 1u32).unwrap();
            if w.tick() == 1 {
                w.emit("second");
                w.publish("fixed", 2u32).unwrap();
            }
        }
    }
    let run = |mode| {
        let mut s = Sim::<Delivery>::new(()).unwrap().paranoid(mode);
        let mut deliveries = vec![];
        for _ in 0..4 {
            deliveries.push((s.world().take_published(), s.world().take_messages()));
            s.run(17.).unwrap();
        }
        deliveries
    };
    let off = run(Paranoid::Off);
    assert!(off[0].0.is_some());
    assert!(off[1].0.is_none());
    assert_eq!(off[2].1, ["second"]);
    assert_eq!(off, run(Paranoid::Save));
    assert_eq!(off, run(Paranoid::FreshGame));
}

#[test]
fn input_refusal_preserves_the_entire_tick_boundary() {
    let mut s = Sim::<Counter>::new(Options::default()).unwrap();
    s.world().busy("keep").unwrap();
    for i in 0..65 {
        s.input(InputEvent::Key {
            code: format!("Key{i}"),
            down: true,
            at_ms: 0.,
        })
        .unwrap();
    }
    let before = s.save().unwrap();
    for _ in 0..2 {
        assert!(s.run(17.).unwrap_err().message.contains("held input limit"));
        assert_eq!(s.save().unwrap(), before);
        assert_eq!(s.world().tick(), 0);
    }
}

#[test]
fn driver_refuses_external_world_clock_replacement_before_work() {
    let mut sim = Sim::<Counter>::new(Options::default()).unwrap();
    let before = sim.save().unwrap();
    assert!(sim
        .world_mut()
        .load(&World::new(120, 0).save().unwrap())
        .is_err());
    assert_eq!(sim.save().unwrap(), before);
    assert_eq!(sim.run(17.).unwrap(), 1);
}

#[test]
fn paused_absolute_and_delta_clocks_drop_time_and_edges() {
    let run = |absolute| {
        let mut s = Sim::<Counter>::new(Options::default()).unwrap();
        s.input(InputEvent::Key {
            code: "KeyA".into(),
            down: true,
            at_ms: 0.,
        })
        .unwrap();
        s.run(1000.).unwrap();
        s.bind(Options {
            paused: true,
            ..Options::default()
        })
        .unwrap();
        for (down, at_ms) in [(false, 2000.), (true, 3000.), (false, 4000.)] {
            s.input(InputEvent::Key {
                code: "KeyA".into(),
                down,
                at_ms,
            })
            .unwrap();
        }
        if absolute {
            s.advance_to(10_000.).unwrap();
        } else {
            s.run(9000.).unwrap();
        }
        assert_eq!(s.world().tick(), 60);
        assert!(!s.input_state().held("add"));
        assert!(!s.input_state().pressed("add"));
        assert!(!s.input_state().released("add"));
        s.bind(Options::default()).unwrap();
        if absolute {
            s.advance_to(10_017.).unwrap();
        } else {
            s.run(17.).unwrap();
        }
        assert_eq!(s.world().tick(), 61);
        assert!(!s.input_state().pressed("add"));
        let bytes = s.save().unwrap();
        let mut fresh = Sim::<Counter>::from_save(&bytes).unwrap();
        fresh.run(17.).unwrap();
        s.run(17.).unwrap();
        assert_eq!(fresh.save().unwrap(), s.save().unwrap());
        bytes
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn reconcile_input_is_atomic_and_never_simulates() {
    let mut s = Sim::<Counter>::new(Options::default()).unwrap();
    s.input(InputEvent::Key {
        code: "KeyA".into(),
        down: true,
        at_ms: 0.,
    })
    .unwrap();
    s.run(17.).unwrap();
    s.input(InputEvent::Key {
        code: "KeyA".into(),
        down: true,
        at_ms: 99_999.,
    })
    .unwrap();
    let before = s.save().unwrap();
    let bad: Vec<_> = (0..65)
        .map(|i| InputEvent::Key {
            code: format!("{i}"),
            down: true,
            at_ms: 0.,
        })
        .collect();
    assert!(s.reconcile_input(5000., &bad).is_err());
    assert_eq!(s.save().unwrap(), before);
    s.reconcile_input(5000., &[]).unwrap();
    assert_eq!(s.world().tick(), 1);
    assert!(!s.input_state().held("add"));
    assert!(!s.input_state().pressed("add"));
    assert!(!s.input_state().released("add"));
    s.advance_to(5017.).unwrap();
    assert_eq!(s.world().tick(), 2);
    assert!(!s.input_state().held("add"));
}

#[test]
fn restoring_argument_selected_types_does_not_inherit_the_live_registry() {
    #[derive(Default, Data)]
    struct Old(u32);
    #[derive(Default, Data)]
    struct New(u32);
    impl Component for Old {
        const NAME: &'static str = "Actor";
    }
    impl Component for New {
        const NAME: &'static str = "Actor";
    }
    #[derive(Default, Args)]
    struct Mode {
        new: bool,
    }
    struct G;
    impl Game for G {
        const ID: &'static str = "selected-registration";
        type Args = Mode;
        fn register(w: &mut World, a: args::SetupArgs<'_, Mode>) -> Result<(), DataError> {
            if matches!(a.get("new"), Some(args::ArgumentRef::Bool(true))) {
                w.register::<New>().unwrap();
            } else {
                w.register::<Old>().unwrap();
            }
            Ok(())
        }
        fn setup(w: &mut World, a: &Mode) {
            if a.new {
                w.spawn(New(7)).unwrap();
            } else {
                w.spawn(Old(4)).unwrap();
            }
        }
        fn tick(_: &mut World, _: &Input, _: &Mode) {}
    }
    let saved = Sim::<G>::new(Mode { new: true }).unwrap().save().unwrap();
    let mut old = Sim::<G>::new(Mode::default()).unwrap();
    let before = old.save().unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| old.carry(&saved)))
        .unwrap()
        .is_err());
    assert_eq!(old.save().unwrap(), before);
    old.restore(&saved).unwrap();
    assert_eq!(old.world().get::<New>("#0").unwrap().0, 7);
    assert_eq!(old.save().unwrap(), saved);
}

#[test]
fn lagging_consumer_cannot_refuse_orphan_reaping() {
    thread_local! { static TICKS: Cell<u32> = const { Cell::new(0) }; }
    struct G;
    impl Game for G {
        const ID: &'static str = "reap-refusal";
        type Args = ();
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<Count>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &()) {
            let parent = w.spawn_named("parent", Count(0)).unwrap();
            let child = w.spawn(()).unwrap();
            w.set_parent(child, Some(parent)).unwrap();
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            TICKS.set(TICKS.get() + 1);
            w.despawn(w.named("parent").unwrap());
        }
    }
    TICKS.set(0);
    let mut s = Sim::<G>::new(()).unwrap();
    let consumer = s.world_mut().subscribe_changes().unwrap();
    let parent = s.world().named("parent").unwrap();
    while s.world().changes(&consumer).unwrap().next < 5000 {
        s.world_mut().insert(parent, Count(0)).unwrap();
    }
    assert_eq!(s.run(17.).unwrap(), 1);
    assert_eq!(TICKS.get(), 1);
    assert!(s.world().is_empty());
    assert!(s.world().changes(&consumer).unwrap().resync);
    assert!(s.save().is_ok());
}

#[test]
fn save_refuses_strings_that_cannot_decode() {
    assert!(Sim::<Counter>::new(Options {
        text: "x".repeat(1_048_577),
        ..Options::default()
    })
    .is_err());
}

#[test]
fn sim_exact_restore_refuses_renamed_fields_and_carry_reports_adaptation() {
    #[derive(Default, Data)]
    struct Before {
        points: u32,
    }
    #[derive(Default, Data)]
    struct After {
        score: u32,
    }
    impl Component for Before {
        const NAME: &'static str = "SavedCounter";
    }
    impl Component for After {
        const NAME: &'static str = "SavedCounter";
    }
    struct OldGame;
    struct NewGame;
    impl Game for OldGame {
        const ID: &'static str = "same-id";
        type Args = ();
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<Before>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &()) {
            w.spawn(Before { points: 123 }).unwrap();
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    impl Game for NewGame {
        const ID: &'static str = "same-id";
        type Args = ();
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<After>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &()) {
            w.spawn(After { score: 7 }).unwrap();
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let saved = Sim::<OldGame>::new(()).unwrap().save().unwrap();
    let mut new = Sim::<NewGame>::new(()).unwrap();
    let before = new.save().unwrap();
    assert!(Sim::<NewGame>::from_save(&saved).is_err());
    assert!(new.restore(&saved).is_err());
    assert_eq!(new.save().unwrap(), before);
    assert!(new.carry(&saved).unwrap());
    assert_eq!(new.world().get::<After>("#0").unwrap().score, 0);
    assert!(!new.carry(&new.save().unwrap()).unwrap());
}

#[test]
fn subscriptions_retain_the_minimum_ack_and_suffix_reads_have_exact_size() {
    let mut w = World::new(60, 0);
    let slow = w.subscribe_changes().unwrap();
    let fast = w.subscribe_changes().unwrap();
    for _ in 0..4000 {
        w.spawn(()).unwrap();
    }
    let end = w.changes(&fast).unwrap().next;
    w.acknowledge_changes(&fast, end - 1).unwrap();
    assert_eq!(w.changes(&fast).unwrap().events.len(), 1);
    assert_eq!(w.changes(&slow).unwrap().events.len(), 4000);
    assert!(w.acknowledge_changes(&fast, end + 1).is_err());
    let mut foreign = World::new(60, 0);
    assert!(foreign.changes(&slow).is_err());
    assert!(foreign.acknowledge_changes(&slow, 0).is_err());
    for _ in 0..200 {
        w.spawn(()).unwrap();
    }
    let old = w.changes(&slow).unwrap();
    assert!(old.resync);
    assert_eq!(old.events.len(), 0);
    let end = old.next;
    w.acknowledge_changes(&slow, end).unwrap();
    assert!(!w.changes(&slow).unwrap().resync);
    assert_eq!(w.changes(&fast).unwrap().events.len(), 201);
    let mut consumers = vec![slow, fast];
    for _ in 2..64 {
        consumers.push(w.subscribe_changes().unwrap());
    }
    assert!(w.subscribe_changes().is_err());
    consumers.pop();
    assert!(w.subscribe_changes().is_ok());
}

#[test]
fn merged_logs_page_session_only_history_and_report_truncation_and_reset() {
    let mut s = Sim::<Still>::new(()).unwrap();
    let saved = s.save().unwrap();
    let game_next = s.world().journal_next();
    for i in 0..1200 {
        s.world().session_log(&format!("session-{i:04}")).unwrap();
    }
    assert_eq!(s.world().journal_next(), game_next);
    assert_eq!(s.save().unwrap(), saved);
    let mut cursor = LogCursor::default();
    let mut history = String::new();
    let mut pages = 0;
    loop {
        let page = s.world().logs(cursor).unwrap();
        assert!(!page.reset && !page.truncated);
        if page.entries == "[]" {
            break;
        }
        assert_ne!(page.next, cursor);
        cursor = page.next;
        history.push_str(&page.entries);
        pages += 1;
    }
    assert!(pages > 2);
    for i in 0..1200 {
        assert_eq!(history.matches(&format!("session-{i:04}")).count(), 1);
    }
    for i in 0..5000 {
        s.world().session_log(&format!("overflow-{i:04}")).unwrap();
    }
    let page = s.world().logs(cursor).unwrap();
    assert!(page.truncated && !page.reset);
    assert!(!page.entries.contains("overflow-0000"));
    assert!(page.entries.contains("overflow-0904"));
    s.restore(&saved).unwrap();
    let page = s.world().logs(page.next).unwrap();
    assert!(page.reset);
    assert_eq!(s.world().journal_next(), game_next);
    assert_eq!(s.save().unwrap(), saved);
}

#[test]
fn merged_log_pages_admit_escaped_text_before_formatting() {
    let w = World::new(60, 0);
    for _ in 0..600 {
        w.session_log(&"\0".repeat(4096)).unwrap();
    }
    let mut cursor = LogCursor::default();
    let mut entries = 0;
    loop {
        let page = w.logs(cursor).unwrap();
        assert!(page.entries.len() <= json::LIMIT);
        if page.entries == "[]" {
            break;
        }
        entries += page.entries.matches("\"Message\"").count();
        assert_ne!(cursor, page.next);
        cursor = page.next;
    }
    assert_eq!(entries, 600);
}

#[test]
fn clock_deltas_round_individually_to_microseconds() {
    let mut sim = Sim::<Still>::new(()).unwrap();
    for _ in 0..1000 {
        assert_eq!(sim.run(0.0004).unwrap(), 0);
    }
    assert_eq!(sim.alpha_inputs(), (0, 0, 1_000_000));
    sim.run(0.0005).unwrap();
    assert_eq!(sim.alpha_inputs(), (0, 60, 1_000_000));
    assert!(sim.advance_to(0.0004).is_err());
    sim.run(0.0005).unwrap();
    assert_eq!(sim.alpha_inputs(), (0, 120, 1_000_000));
}

#[test]
fn live_bindings_and_reconciled_input_invalidate_an_old_rest_observation() {
    #[derive(Default, Args)]
    struct A {
        #[live]
        increment: u32,
    }
    struct G;
    impl Game for G {
        const ID: &'static str = "rest-invalidation";
        type Args = A;
        fn register(w: &mut World, _: args::SetupArgs<'_, A>) -> Result<(), DataError> {
            w.register::<Count>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &A) {
            w.spawn_named("value", Count(0)).unwrap();
        }
        fn tick(w: &mut World, input: &Input, args: &A) {
            w.get_mut::<Count>("value").unwrap().0 +=
                u64::from(args.increment) + u64::from(input.key("KeyW"));
        }
    }
    let mut s = Sim::<G>::new(A::default()).unwrap();
    assert_eq!(s.settle(1).unwrap(), 1);
    s.bind(A { increment: 1 }).unwrap();
    assert_eq!(s.world().observation(), None);
    assert!(s.settle(1).is_err());
    assert_eq!(s.world().get::<Count>("value").unwrap().0, 1);
    s.bind(A::default()).unwrap();
    assert_eq!(s.settle(1).unwrap(), 1);
    s.reconcile_input(
        50.,
        &[InputEvent::Key {
            code: "KeyW".into(),
            down: true,
            at_ms: 50.,
        }],
    )
    .unwrap();
    assert_eq!(s.world().observation(), None);
    assert!(s.settle(1).is_err());
    assert_eq!(s.world().get::<Count>("value").unwrap().0, 2);
}
