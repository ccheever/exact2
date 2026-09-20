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
    fn register(w: &mut World, args: args::SetupArgs<'_, Options>) {
        assert!(args.get("paused").is_none());
        assert!(matches!(
            args.get("seed"),
            Some(args::ArgumentRef::Unsigned(_))
        ));
        w.register::<Count>();
    }
    fn setup(w: &mut World, a: &Options) {
        w.reseed(a.seed);
        w.spawn_named("counter", Count(0));
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let random = w.rng().next_u32() as u64;
        let n = {
            let mut c = w.require_mut::<Count>("counter");
            c.0 = c.0.wrapping_add(random + u64::from(input.pressed("add")));
            c.0
        };
        w.publish("count", (n % 1_000_000) as u32);
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
        fn register(w: &mut World) {
            CALLS.set(CALLS.get() + 1);
            w.register::<Hook>();
        }
    }
    CALLS.set(0);
    let mut w = World::new(60, 0);
    for n in 0..100 {
        w.spawn(Hook(n));
        w.register::<Hook>();
    }
    assert_eq!(CALLS.get(), 1);
    w.load(&w.save()).unwrap();
    assert_eq!(CALLS.get(), 1);
    assert_eq!(w.query::<&Hook>().iter().count(), 100);
}
#[test]
fn ownership_validates_cycles_without_any_pose_and_reaps_reverse_chains() {
    let mut w = World::new(60, 0);
    let es: Vec<_> = (0..257).map(|_| w.spawn(())).collect();
    for pair in es.windows(2) {
        w.set_parent(pair[0], Some(pair[1])).unwrap();
    }
    let before = w.save();
    assert!(w
        .set_parent(es[256], Some(es[0]))
        .unwrap_err()
        .message
        .contains("cycle"));
    assert_eq!(before, w.save());
    assert!(w.try_query::<&mut Parent>().is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| w.get_mut::<Parent>(es[0]))).is_err());
    w.despawn(es[256]);
    w.reap_orphans().unwrap();
    assert!(w.is_empty());
    let removed: Vec<_> = w
        .changes(0)
        .unwrap()
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
    let a = bad.spawn(());
    let b = bad.spawn(());
    bad.insert(a, Forged(b));
    bad.insert(b, Forged(a));
    let mut w = World::new(60, 0);
    w.register::<Parent>();
    w.spawn_named("retained", ());
    let before = w.save();
    assert!(w.load(&bad.save()).unwrap_err().message.contains("cycle"));
    assert_eq!(w.save(), before);
}
#[test]
fn journal_retains_generations_replacements_and_reparent_across_ticks_and_restore() {
    let mut s = Sim::<Still>::new(()).unwrap();
    let w = s.world_mut();
    let a = w.spawn(Count(1));
    let owner = w.spawn(());
    w.insert(a, Count(2));
    w.set_parent(a, Some(owner)).unwrap();
    w.despawn(a);
    let b = w.spawn(Count(3));
    assert_eq!(a.index(), b.index());
    assert_ne!(a.generation(), b.generation());
    let cursor = w.change_cursor();
    s.run(1000.).unwrap();
    let retained: Vec<_> = s.world().changes(0).unwrap().cloned().collect();
    assert_eq!(retained.len() as u64, cursor);
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
            .changes(0)
            .unwrap()
            .take(retained.len())
            .cloned()
            .collect::<Vec<_>>(),
        retained
    );
    assert_eq!(
        s.world().changes(cursor).unwrap().last().unwrap().kind,
        ChangeKind::Reset
    );
    let cursor = s.world().change_cursor();
    s.world_mut().consume_changes(cursor).unwrap();
    assert!(s.world().changes(0).is_err());
    assert_eq!(s.world().changes(cursor).unwrap().count(), 0);
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
    for version in [0, 1, 2, 3, 4, 5, 6, 7, 9, 255] {
        let mut bad = bytes.clone();
        bad[6] = version;
        assert!(s.restore(&bad).unwrap_err().message.contains("EXSIM v8"));
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
                s.world().publications().unwrap(),
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
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) {
            w.register::<Count>();
        }
        fn setup(w: &mut World, _: &()) {
            SETUPS.set(SETUPS.get() + 1);
            w.spawn(Count(4));
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
    let es: Vec<_> = (0..130)
        .map(|n| {
            w.spawn(Padded {
                byte: 1,
                text: format!("{n}"),
                n,
            })
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
    assert_eq!(QueryBorrow::<&Padded>::matching_count(&w).unwrap().0, 124);
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
    let e = w.spawn(Count(4));
    let before = w.save();
    assert!(w.state(e).unwrap().contains('4'));
    assert_eq!(w.tree().0.len(), 1);
    assert!(!w.logs(0).unwrap().is_empty());
    assert_eq!(w.save(), before);
    assert!(json::to_string(&"x".repeat(json::LIMIT + 1)).is_err());
    assert!(w.log(&"x".repeat(4097)).is_err());
    let budget = data::LoadBudget::new(32);
    assert!(bin::from_slice_in::<Vec<u8>>(&bin::to_vec(&vec![1u8; 33]), Some(&budget)).is_err());
}
#[test]
fn ambient_motion_deadlines_and_same_value_leases_do_not_hide_changes() {
    let mut s = Sim::<Still>::new(()).unwrap();
    #[derive(Default, Component)]
    struct Motion(Tween);
    let mut tween = Tween::new(0.);
    tween.to(Now { tick: 0, hz: 60 }, 1., 1.);
    let e = s.world_mut().spawn(Motion(tween));
    assert_eq!(s.world().settle_tick(), Some(60));
    s.world_mut().insert(e, Ambient);
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
    for _ in 0..MAX_ENTITIES {
        w.spawn(());
    }
    assert!(catch_unwind(AssertUnwindSafe(|| w.spawn(()))).is_err());
    assert_eq!(w.len(), MAX_ENTITIES);
    let cursor = w.change_cursor();
    w.consume_changes(cursor).unwrap();
    let e = w.resolve("#0").unwrap();
    for n in 0..999_999 {
        w.insert(e, Count(n));
    }
    // insert reserves two events so ownership can always journal its edge too.
    let cursor = w.change_cursor();
    assert!(catch_unwind(AssertUnwindSafe(|| w.insert(e, Count(0)))).is_err());
    assert_eq!(w.change_cursor(), cursor);
    assert_eq!(w.changes(cursor - 1).unwrap().count(), 1);
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
    let logs = s.world().logs(0).unwrap();
    assert!(logs.contains("game scored"));
    assert!(logs.contains("agent attached"));
    assert!(logs.find("game scored") < logs.find("agent attached"));
    s.world().log("game next").unwrap();
    let logs = s.world().logs(0).unwrap();
    assert!(logs.find("agent attached") < logs.find("game next"));
    let fresh = Sim::<Counter>::from_save(&s.save().unwrap()).unwrap();
    assert!(!fresh.world().logs(0).unwrap().contains("agent attached"));
    assert!(fresh.world().logs(0).unwrap().contains("game next"));
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
    s.world().publish("a", "a".repeat(6000));
    let before = s.save().unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| s
        .world()
        .publish("b", "b".repeat(6000))))
    .is_err());
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
            w.publish("fixed", 1u32);
            w.emit("initial");
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            w.publish("fixed", 1u32);
            if w.tick() == 1 {
                w.emit("second");
                w.publish("fixed", 2u32);
            }
        }
    }
    let run = |mode| {
        let mut s = Sim::<Delivery>::new(()).unwrap().paranoid(mode);
        let mut deliveries = vec![];
        for _ in 0..4 {
            deliveries.push((
                s.world().take_published().unwrap(),
                s.world().take_messages(),
            ));
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
