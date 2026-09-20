use exact_world::*;
#[derive(Default, Component)]
struct Counter {
    n: u32,
}
struct Board;
impl Game for Board {
    const ID: &'static str = "k1d";
    type Args = ();
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
        w.register::<Counter>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("counter", Counter { n: 7 }).unwrap();
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.get_mut::<Counter>("counter").unwrap().n += 1;
    }
}
#[test]
fn untouched_and_edited_candidates_preserve_complete_continuation() {
    for drained in [false, true] {
        let mut s = Sim::<Board>::new(()).unwrap();
        s.run(17.).unwrap();
        s.world().publish("score", 7u32);
        s.world().log("saved game history").unwrap();
        s.world().emit("pending delivery");
        s.world().session_log("session history").unwrap();
        if drained {
            s.world().take_published();
        }
        let before = s.save().unwrap();
        s.world()
            .candidate()
            .unwrap()
            .commit(s.world_mut())
            .unwrap();
        assert_eq!(s.save().unwrap(), before);
        assert_eq!(s.world().take_published().is_some(), !drained);
        assert!(s
            .world()
            .logs(LogCursor::default())
            .unwrap()
            .entries
            .contains("session history"));
        let e = s.world().named("counter").unwrap();
        let mut c = s.world().candidate().unwrap();
        c.edit(
            Some(e),
            "Counter",
            &bin::to_vec(&Counter { n: 99 }).unwrap(),
        )
        .unwrap();
        c.commit(s.world_mut()).unwrap();
        assert_eq!(s.world().get::<Counter>(e).unwrap().n, 99);
        assert_eq!(s.world().publications().len(), 1);
        assert!(s
            .world()
            .logs(LogCursor::default())
            .unwrap()
            .entries
            .contains("saved game history"));
        assert_eq!(s.world().take_messages(), ["pending delivery"]);
    }
}
