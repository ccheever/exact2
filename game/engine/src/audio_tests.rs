use super::*;
#[test]
fn journal_lifetime_state_and_listener_refusal() {
    let mut w = World::new(60, 0);
    w.register_audio();
    w.resource_mut::<Sounds>()
        .add("chime", Synth::sine(880.0).seconds(0.25));
    let e = w.spawn_named("lantern-3", ());
    w.play("chime").at(e).gain(0.8).start();
    let j = w.journal();
    assert_eq!(
        j.last().unwrap().line,
        "t=0 tick=0 sfx chime at lantern-3 gain 0.80"
    );
    assert_eq!(w.resource::<Voices>().voices[0].ends, 15);
    assert!(state(&w).contains("\"at\":\"lantern-3\""));
    w.spawn(AudioListener);
    w.spawn_named("second", AudioListener);
    step(&mut w);
    step(&mut w);
    assert_eq!(w.query::<&AudioListener>().iter().count(), 1);
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("second AudioListener"))
            .count(),
        1
    );
    for _ in 0..14 {
        w.step_clock();
    }
    step(&mut w);
    assert_eq!(w.resource::<Voices>().voices.len(), 1);
    w.step_clock();
    step(&mut w);
    assert!(w.resource::<Voices>().voices.is_empty());
}
#[test]
#[should_panic(expected = "unknown sound `missing`")]
fn unknown_sound_names_itself() {
    World::new(60, 0).play("missing").start();
}

#[test]
fn discarding_play_never_commits_a_voice_or_journal_line() {
    let mut w = World::new(60, 0);
    w.register_audio();
    w.resource_mut::<Sounds>().add("chime", Synth::sine(880.0));
    let before = w.journal();
    {
        let _pending = w.play("chime").gain(0.5);
    }
    assert_eq!(w.journal().len(), before.len());
    assert!(w.resource::<Voices>().voices.is_empty());
    let id = w.play("chime").start();
    assert_eq!(w.resource::<Voices>().voices.len(), 1);
    assert_eq!(w.resource::<Voices>().voices[0].id, id);
}
