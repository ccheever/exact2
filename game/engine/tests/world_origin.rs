//! A world's message carries what caused it (LLP 1133.022 D6, D10 item 6):
//! once a tool's message is posted into a world, every message the world
//! emits carries the least privileged origin posted since its fresh start,
//! through a person's input, the clock and a save restored into a new
//! world; only a restart clears it. An unmarked world's save is unchanged.
use exact_game::*;

/// Every message posted in is emitted back on the next tick.
struct Echo;
#[derive(Default, exact_game::Args)]
struct EchoArgs {
    /// A fresh start when it flips.
    #[restart]
    pub again: bool,
}
impl Game for Echo {
    const ID: &'static str = "Echo";
    type Args = EchoArgs;

    fn setup(w: &mut World, _: &Self::Args) {
        w.spawn_named("echo", (Transform::default(), Ambient));
    }
    fn actions() -> Actions {
        Actions::new().button("act", &["KeyE"])
    }
    fn tick(w: &mut World, i: &Input, _: &Self::Args) {
        for m in i.messages() {
            w.emit(format!("heard {m}"));
        }
        if i.pressed("act") {
            w.emit("pressed");
        }
    }
}

fn sim() -> Sim<Echo> {
    let mut s = Sim::new(EchoArgs::default()).unwrap();
    s.advance(0.0, Clock::Seekable);
    s
}

fn post(s: &mut Sim<Echo>, text: &str, at_ms: f64) {
    s.input(InputEvent::Message {
        text: text.into(),
        at_ms,
        player: 0,
    });
}

const TOOL: &str = "\u{1}origin:tool\u{1}";

#[test]
fn an_unmarked_world_emits_unmarked_and_saves_as_before() {
    let mut s = sim();
    post(&mut s, "hi", 0.0);
    s.advance(100.0, Clock::Seekable);
    assert_eq!(s.take_messages(), ["heard hi"]);
    assert_eq!(s.origin(), "app");
    let save = s.save().unwrap();
    assert!(
        !save.windows(6).any(|w| w == b"origin"),
        "an unmarked save names no origin"
    );
}

#[test]
fn a_tool_message_marks_every_later_emission_until_a_restart() {
    let mut s = sim();
    post(&mut s, &format!("{TOOL}buzz"), 0.0);
    s.advance(100.0, Clock::Seekable);
    // The text reaches the world without its mark; the emission carries it.
    assert_eq!(s.take_messages(), [format!("{TOOL}heard buzz")]);
    assert_eq!(s.origin(), "tool");
    // A person's input, then a minute of clock: still marked.
    s.input(InputEvent::Key {
        code: "KeyE".into(),
        down: true,
        at_ms: 120.0,
        player: 0,
    });
    s.advance(200.0, Clock::Seekable);
    s.advance(60_200.0, Clock::Seekable);
    assert_eq!(s.take_messages(), [format!("{TOOL}pressed")]);
    // A save restored into a new world is marked.
    let save = s.save().unwrap();
    let mut restored = sim();
    restored.restore(&save).unwrap();
    assert_eq!(restored.origin(), "tool");
    restored.advance(60_300.0, Clock::Seekable);
    post(&mut restored, "plain", 60_300.0);
    restored.advance(60_400.0, Clock::Seekable);
    assert_eq!(restored.take_messages(), [format!("{TOOL}heard plain")]);
    // A fresh start from the arguments clears it.
    restored
        .bind(&EchoArgs { again: true }.values(), None)
        .unwrap();
    assert_eq!(restored.origin(), "app");
    post(&mut restored, "after", 60_500.0);
    restored.advance(60_600.0, Clock::Seekable);
    assert_eq!(restored.take_messages(), ["heard after"]);
}

#[test]
fn the_least_privileged_origin_wins_and_a_mark_cannot_raise_one() {
    let mut s = sim();
    post(&mut s, "\u{1}origin:device\u{1}a", 0.0);
    post(&mut s, &format!("{TOOL}b"), 0.0);
    post(&mut s, "\u{1}origin:app\u{1}c", 0.0);
    s.advance(100.0, Clock::Seekable);
    assert_eq!(s.origin(), "device");
    let mark = "\u{1}origin:device\u{1}";
    assert_eq!(
        s.take_messages(),
        [
            format!("{mark}heard a"),
            format!("{mark}heard b"),
            format!("{mark}heard c")
        ]
    );
    // An unknown word is the least privileged.
    let mut t = sim();
    post(&mut t, "\u{1}origin:root\u{1}x", 0.0);
    assert_eq!(t.origin(), "device");
}
