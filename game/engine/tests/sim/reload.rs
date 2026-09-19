use super::*;
#[derive(Default, Component)]
struct ReloadProbe {
    authored: u32,
    running: u32,
}
struct ReloadGame<const EDITED: bool>;
impl<const EDITED: bool> Game for ReloadGame<EDITED> {
    const ID: &'static str = "reload-boundary";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "probe",
            ReloadProbe {
                authored: if EDITED { 2 } else { 1 },
                running: if EDITED { 20 } else { 10 },
            },
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.get_mut::<ReloadProbe>("probe").unwrap().running += 1;
    }
}
#[derive(Default, Data)]
struct ReloadItem {
    entity: String,
    component: String,
    field: String,
}
#[derive(Default, Data)]
struct ReloadReport {
    applied: Vec<ReloadItem>,
    kept: Vec<ReloadItem>,
}
#[derive(Default, Data)]
struct ReloadState {
    reload: ReloadReport,
}
#[derive(Default, Data)]
struct ReloadEnvelope {
    world: ReloadState,
}
#[test]
fn reload_report_survives_ticks_clears_on_restore_and_refusal_is_atomic() {
    let mut old = Sim::<ReloadGame<false>>::new(()).unwrap();
    old.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    old.agent(r#"{"op":"clock","ticks":1}"#);
    let saved = old.save().unwrap();
    for (bound, mode) in [false, true].into_iter().flat_map(|bound| {
        [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame].map(|mode| (bound, mode))
    }) {
        let mut next = Sim::<ReloadGame<true>>::new(()).unwrap().paranoid(mode);
        next.agent(r#"{"op":"clock","owner":"agent","now":9000}"#);
        if bound {
            next.restore_bound(&saved)
        } else {
            next.restore(&saved)
        }
        .unwrap();
        let state: ReloadEnvelope = json::from_str(&next.agent(r#"{"op":"state"}"#)).unwrap();
        assert_eq!(state.world.reload.applied.len(), 1);
        assert_eq!(state.world.reload.kept.len(), 1);
        let item = &state.world.reload.applied[0];
        assert_eq!(
            (&*item.entity, &*item.component, &*item.field),
            ("probe", "ReloadProbe", "authored")
        );
        let probe = next.world().get::<ReloadProbe>("probe").unwrap();
        assert_eq!((probe.authored, probe.running), (2, 11));
        drop(probe);
        let current = next.save().unwrap();
        let mut v5 = current.clone();
        v5[6] = 5;
        let error = next.restore_bound(&v5).unwrap_err();
        assert!(error.message.contains("EXSIM v7"));
        assert_eq!(next.save().unwrap(), current);
        next.agent(r#"{"op":"clock","ticks":2}"#);
        let state: ReloadEnvelope = json::from_str(&next.agent(r#"{"op":"state"}"#)).unwrap();
        assert_eq!(state.world.reload.applied.len(), 1);
        let saved = next.save().unwrap();
        next.restore_bound(&saved).unwrap();
        assert_eq!(next.save().unwrap(), saved);
        let state: ReloadEnvelope = json::from_str(&next.agent(r#"{"op":"state"}"#)).unwrap();
        assert!(state.world.reload.applied.is_empty() && state.world.reload.kept.is_empty());
    }
}

#[derive(Default, Args)]
struct BindingArgs {
    authored: u32,
    #[live]
    live: u32,
}
struct BindingGame;
impl Game for BindingGame {
    const ID: &'static str = "reload-current-bindings";
    type Args = BindingArgs;
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, exact_game::Value>) {
        w.register::<ReloadProbe>();
    }
    fn setup(w: &mut World, args: &BindingArgs) {
        w.spawn_named(
            "probe",
            ReloadProbe {
                authored: args.authored,
                running: args.live,
            },
        );
    }
    fn tick(w: &mut World, _: &Input, _: &BindingArgs) {
        w.get_mut::<ReloadProbe>("probe").unwrap().running += 1;
    }
}
#[test]
fn bound_carry_uses_destination_initializer_and_all_current_bindings() {
    let mut old = Sim::<BindingGame>::new(BindingArgs {
        authored: 1,
        live: 10,
    })
    .unwrap();
    old.run(17.);
    let saved = old.save().unwrap();
    for open in [false, true] {
        let mut next = Sim::<BindingGame>::new(BindingArgs {
            authored: 2,
            live: 20,
        })
        .unwrap();
        next.bind(&[Value::Number(2.), Value::Number(30.)], None)
            .unwrap();
        if open {
            next.open_bound(&saved)
        } else {
            next.restore_bound(&saved)
        }
        .unwrap();
        assert_eq!((next.args().authored, next.args().live), (2, 30));
        let p = next.world().get::<ReloadProbe>("probe").unwrap();
        assert_eq!((p.authored, p.running), (if open { 1 } else { 2 }, 11));
        drop(p);
        let again = next.save().unwrap();
        if open {
            next.open_bound(&again)
        } else {
            next.restore_bound(&again)
        }
        .unwrap();
        assert_eq!(next.save().unwrap(), again);
    }
}
#[test]
fn unbound_restore_keeps_live_bindings_without_reauthoring_the_saved_base() {
    let mut source = Sim::<BindingGame>::new(BindingArgs {
        authored: 1,
        live: 10,
    })
    .unwrap();
    source
        .bind(&[Value::Number(1.), Value::Number(20.)], None)
        .unwrap();
    let saved = source.save().unwrap();
    let mut restored = Sim::<BindingGame>::new(BindingArgs::default()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!((restored.args().authored, restored.args().live), (1, 20));
    assert_eq!(
        restored
            .world()
            .get::<ReloadProbe>("probe")
            .unwrap()
            .running,
        10
    );
    assert_eq!(restored.save().unwrap(), saved);
    assert_eq!(
        Sim::<BindingGame>::from_save(&saved)
            .unwrap()
            .save()
            .unwrap(),
        saved
    );
}

struct HierarchyEdit<const EDITED: bool>;
impl<const EDITED: bool> Game for HierarchyEdit<EDITED> {
    const ID: &'static str = "reload-cycle";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let root = w.spawn_named("root", Transform::default());
        let a = w.spawn_named("a", (Transform::default(), Parent(root)));
        w.spawn_named(
            "b",
            (Transform::default(), Parent(if EDITED { a } else { root })),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn independently_valid_parent_edits_refuse_a_combined_cycle_atomically() {
    let mut source = Sim::<HierarchyEdit<false>>::new(()).unwrap();
    let valid = source.save().unwrap();
    let b = source.world().resolve("b").unwrap();
    source.world().get_mut::<Parent>("a").unwrap().0 = b;
    source.world_mut().propagate();
    let cyclic_when_merged = source.save().unwrap();
    let mut target = Sim::<HierarchyEdit<true>>::new(()).unwrap();
    let before = target.save().unwrap();
    let state = target.agent(r#"{"op":"state"}"#);
    let error = target.restore_bound(&cyclic_when_merged).unwrap_err();
    assert!(error.message.contains("Parent cycle"), "{error}");
    assert_eq!(target.save().unwrap(), before);
    assert_eq!(target.agent(r#"{"op":"state"}"#), state);
    target.restore_bound(&valid).unwrap();
    assert_eq!(
        target.world().get::<Parent>("b").unwrap().0,
        target.world().resolve("a").unwrap()
    );
}

#[test]
fn argument_selected_variant_uses_its_own_typed_decoder_before_setup() {
    thread_local! { static SETUPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    #[derive(Default, Args)]
    struct VariantArgs {
        variant: bool,
    }
    #[derive(Default, Data)]
    struct A {
        value: u32,
    }
    #[derive(Default, Data)]
    struct B {
        value: u32,
    }
    impl Component for A {
        const NAME: &'static str = "VariantValue";
    }
    impl Component for B {
        const NAME: &'static str = "VariantValue";
    }
    struct Variant;
    impl Game for Variant {
        const ID: &'static str = "argument-selected-decoder";
        type Args = VariantArgs;
        fn register(w: &mut World, args: &std::collections::BTreeMap<&str, exact_game::Value>) {
            if args["variant"].as_bool() == Some(true) {
                w.register::<B>();
            } else {
                w.register::<A>();
            }
        }
        fn setup(w: &mut World, args: &VariantArgs) {
            SETUPS.with(|n| n.set(n.get() + 1));
            if args.variant {
                w.spawn_named("value", B { value: 29 });
            } else {
                w.spawn_named("value", A { value: 17 });
            }
        }
        fn tick(_: &mut World, _: &Input, _: &VariantArgs) {}
    }
    let source = Sim::<Variant>::new(VariantArgs { variant: true }).unwrap();
    let bytes = source.save().unwrap();
    let mut target = Sim::<Variant>::new(VariantArgs::default()).unwrap();
    SETUPS.with(|n| n.set(0));
    target.restore(&bytes).unwrap();
    assert_eq!(SETUPS.with(|n| n.replace(0)), 1);
    assert_eq!(target.world().require::<B>("value").value, 29);
    assert!(target.world().get::<A>("value").is_none());
    assert_eq!(target.save().unwrap(), bytes);
    let fresh = Sim::<Variant>::from_save(&bytes).unwrap();
    assert_eq!(SETUPS.with(|n| n.replace(0)), 0);
    assert_eq!(fresh.world().require::<B>("value").value, 29);
    let mut bad = bytes.clone();
    let world = bad.windows(8).position(|v| v == b"EXGAME\0\x03").unwrap();
    bad[world + 8] = 0xff;
    assert!(target.restore(&bad).is_err());
    assert!(Sim::<Variant>::from_save(&bad).is_err());
    assert_eq!(SETUPS.with(|n| n.get()), 0);
    assert_eq!(target.save().unwrap(), bytes);
}
