use exact_game::*;

#[derive(Default, Args)]
struct Options {
    #[live]
    paused: bool,
}
struct Still;
impl Game for Still {
    type Args = Options;
    const ID: &'static str = "g1d";
    fn setup(_: &mut World, _: &Options) {}
    fn paused(args: &Options) -> bool {
        args.paused
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
fn reply(s: &mut Sim<Still>) -> Reply {
    json::from_str(&s.agent(r#"{"op":"clock"}"#)).unwrap()
}
#[test]
fn logging_refusal_preserves_rest_and_hash() {
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    assert!(s.settle());
    let hash = s.world().hash();
    let epoch = s.world().mutation_epoch();
    let cursor = s.world().journal_next();
    assert!(s.agent("malformed").contains("error"));
    assert_eq!(s.world().journal_next(), cursor + 1);
    assert_eq!(s.world().mutation_epoch(), epoch);
    assert!(s.quiescent());
    assert_eq!(s.world().hash(), hash);
}
#[derive(Default, Component)]
struct Motion(Tween, Spring);
fn ambient_world() -> Sim<Still> {
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    let mut tween = Tween::new(0.0);
    tween.to(s.world().now(), 1.0, 10.0);
    s.world_mut()
        .spawn_named("weather", (Ambient, Motion(tween, Spring::default())));
    s.run(100.0);
    s
}
#[test]
fn ambient_tween_does_not_block_rest() {
    assert!(ambient_world().quiescent());
}
#[test]
fn ambient_motion_is_absent_from_changing() {
    let mut s = ambient_world();
    s.world().publish("score", 1);
    assert_eq!(reply(&mut s).changing, ["not observed"]);
}
#[test]
fn ambient_tween_does_not_drive_settle_at() {
    let mut s = ambient_world();
    s.world().publish("score", 1);
    assert_eq!(reply(&mut s).settleAt, 200.0);
}
#[test]
fn paused_queued_input_explains_why_it_is_not_quiescent() {
    let mut s = Sim::<Still>::new(Options::default()).unwrap();
    s.advance(0.0, Clock::Seekable);
    s.input(InputEvent::Key {
        code: "KeyW".into(),
        down: true,
        at_ms: 1000.0,
    });
    s.bind(&Options { paused: true }.values(), None).unwrap();
    let r = reply(&mut s);
    assert!(!r.quiescent);
    assert_eq!(r.changing, ["paused", "input queued"]);
}
