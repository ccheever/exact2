use exact_world::*;

#[derive(Default, Args)]
struct Options {
    #[live]
    fail: bool,
    #[restart]
    restart: bool,
}
#[derive(Default, Component)]
struct Controller {
    ticks: u32,
}
struct Full;
impl Game for Full {
    const ID: &'static str = "returned-tick-error";
    const HZ: u32 = 1000;
    type Args = Options;
    fn register(w: &mut World, _: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
        w.register::<Controller>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &Options) -> Result<(), DataError> {
        // Interleaved ownership, an active controller, and only two spare slots.
        let child = w.spawn_named("child", ())?;
        w.spawn_named("controller", Controller::default())?;
        let parent = w.spawn_named("parent", ())?;
        w.set_parent(child, Some(parent))?;
        for _ in 3..MAX_ENTITIES - 2 {
            w.spawn(())?;
        }
        w.work("assets", Work::Pending)?;
        Ok(())
    }
    fn tick(w: &mut World, _: &Input, args: &Options) -> Result<(), DataError> {
        w.get_mut::<Controller>("controller").unwrap().ticks += 1;
        if w.tick() < 2 {
            w.spawn(())?;
        } else if args.fail {
            w.despawn(w.named("parent").unwrap())?;
            w.spawn_named("partial", ())?; // Recycle the parent without rescuing its child.
            w.spawn(())?; // The kernel's actual entity admission refusal.
        }
        Ok(())
    }
}
fn failure_entries(w: &World, message: &str) -> usize {
    let mut cursor = LogCursor::default();
    let mut count = 0;
    loop {
        let page = w.logs(cursor).unwrap();
        count += page.entries.matches(message).count();
        if page.entries == "[]" {
            return count;
        }
        cursor = page.next;
    }
}
fn failed_tick(mode: Paranoid) {
    let mut s = Sim::<Full>::new(Options {
        fail: true,
        restart: false,
    })
    .unwrap()
    .paranoid(mode);
    s.run(2.).unwrap();
    let saved = s.save().unwrap();
    let hash = s.world().hash().unwrap();
    let mut control = Sim::<Full>::new(Options::default()).unwrap();
    control.run(2.).unwrap();
    assert_eq!(hash, control.world().hash().unwrap());
    let error = s.run(10.).unwrap_err();
    assert_eq!(error.path, "tick 3");
    assert_eq!(error.message, "entity slot limit (200000)");
    assert_eq!(s.world().tick(), 2);
    let child = s.world().named("child").unwrap();
    assert!(
        s.world().get::<Parent>(child).is_some(),
        "failed tick reaped its orphan"
    );
    assert!(s.world().named("partial").is_some());
    let state = s
        .world()
        .state(s.world().named("controller").unwrap())
        .unwrap();
    assert!(state.contains("\"ticks\":3"), "{state}");
    // Tree adapters enumerate bounded entity pages; the core has no tree formatter.
    let tree: Vec<_> = s
        .world()
        .entities()
        .take(512)
        .map(|e| (e, s.world().state(e).unwrap()))
        .collect();
    assert_eq!(tree.len(), 512);
    assert!(tree.iter().any(|(_, value)| value == &state));
    assert!(matches!(s.world().readiness(), Readiness::Pending(_)));
    for refusal in [
        s.run(1.).unwrap_err(),
        s.run(f64::NAN).unwrap_err(),
        s.advance_to(4.).unwrap_err(),
        s.settle(1).unwrap_err(),
        s.save().unwrap_err(),
        s.input(InputEvent::Blur { at_ms: 3. }).unwrap_err(),
        s.reconcile_input(3., &[]).unwrap_err(),
    ] {
        assert_eq!(refusal, error, "lost the first failure");
    }
    assert_eq!(failure_entries(s.world(), &error.to_string()), 1);
    s.restore(&saved).unwrap();
    assert_eq!(s.world().hash().unwrap(), hash);
    assert_eq!(
        s.save().unwrap(),
        saved,
        "session failure entered saved bytes"
    );
    assert_eq!(failure_entries(s.world(), &error.to_string()), 1);
    s.bind(Options::default()).unwrap();
    s.run(1.).unwrap();
    control.run(1.).unwrap();
    assert_eq!(s.world().hash().unwrap(), control.world().hash().unwrap());
    assert_eq!(s.save().unwrap(), control.save().unwrap());
    // Carry and bind-restart independently clear the latched error.
    s.restore(&saved).unwrap();
    assert_eq!(s.run(1.).unwrap_err(), error);
    s.carry(&saved).unwrap();
    assert_eq!(s.save().unwrap(), saved);
    assert_eq!(s.run(1.).unwrap_err(), error);
    s.bind(Options {
        fail: false,
        restart: true,
    })
    .unwrap();
    assert_eq!(s.world().tick(), 0);
    s.run(3.).unwrap();
    assert_eq!(s.world().hash().unwrap(), control.world().hash().unwrap());
}
#[test]
fn returned_tick_error() {
    failed_tick(Paranoid::Off);
}
#[test]
fn returned_tick_error_paranoid_save() {
    failed_tick(Paranoid::Save);
}
#[test]
fn returned_tick_error_paranoid_fresh_game() {
    failed_tick(Paranoid::FreshGame);
}

#[test]
fn setup_refusal_drops_the_candidate_and_preserves_the_running_sim() {
    use std::cell::Cell;
    thread_local! {
        static LIVE: Cell<usize> = const { Cell::new(0) };
        static REFUSE: Cell<bool> = const { Cell::new(false) };
        static SETUPS: Cell<usize> = const { Cell::new(0) };
    }
    #[derive(Component)]
    struct Owned {}
    impl Default for Owned {
        fn default() -> Self {
            LIVE.set(LIVE.get() + 1);
            Self {}
        }
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            LIVE.set(LIVE.get() - 1);
        }
    }
    #[derive(Default, Component)]
    struct Unregistered;
    struct Setup;
    impl Game for Setup {
        const ID: &'static str = "setup-refusal";
        type Args = Options;
        fn register(w: &mut World, _: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
            w.register::<Owned>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &Options) -> Result<(), DataError> {
            SETUPS.set(SETUPS.get() + 1);
            w.spawn(Owned::default())?;
            if REFUSE.get() {
                w.spawn(Unregistered)?;
            }
            Ok(())
        }
        fn tick(_: &mut World, _: &Input, _: &Options) -> Result<(), DataError> {
            Ok(())
        }
    }
    REFUSE.set(true);
    let error = Sim::<Setup>::new(Options::default()).err().unwrap();
    assert_eq!(error.path, "setup.Unregistered");
    assert!(error.message.contains("unregistered storage"));
    assert_eq!(LIVE.get(), 0);
    REFUSE.set(false);
    let mut s = Sim::<Setup>::new(Options::default()).unwrap();
    let saved = s.save().unwrap();
    REFUSE.set(true);
    assert_eq!(
        s.bind(Options {
            fail: false,
            restart: true
        })
        .unwrap_err(),
        error
    );
    assert_eq!(LIVE.get(), 1, "refused restart leaked its partial world");
    assert_eq!(s.save().unwrap(), saved);
    // Exact from_save/FreshGame never invokes setup, including a now-failing setup.
    let setups = SETUPS.get();
    let fresh = Sim::<Setup>::from_save(&saved).unwrap();
    assert_eq!(fresh.save().unwrap(), saved);
    drop(fresh);
    s = s.paranoid(Paranoid::FreshGame);
    s.run(17.).unwrap();
    assert_eq!(SETUPS.get(), setups);
    assert_eq!(LIVE.get(), 1);
    drop(s);
    assert_eq!(LIVE.get(), 0);
}
