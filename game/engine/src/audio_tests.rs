use super::*;
#[test]
fn journal_lifetime_state_and_listener_refusal() {
    let mut w = World::new(60, 0);
    w.sounds([("chime", Synth::sine(880.0).seconds(0.25))]);
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
    w.sounds([("chime", Synth::sine(880.0))]);
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

#[test]
#[should_panic(expected = "AudioSource `orphan` requires sounds")]
fn r14_audio_source_requires_sound_definitions() {
    let mut w = World::new(60, 0);
    w.spawn_named("orphan", AudioSource::new("wind"));
    step(&mut w);
}

#[test]
fn listener_refusals_keep_entity_order_and_do_not_repeat() {
    let mut w = World::new(60, 0);
    w.sounds([("wind", Synth::noise().looped())]);
    let first = w.spawn_named("first", AudioListener);
    w.spawn_named("second", AudioListener);
    w.spawn_named("third", AudioListener);
    let before = w.journal().len();
    step(&mut w);
    let lines: Vec<_> = w
        .journal()
        .iter()
        .skip(before)
        .map(|e| e.line.clone())
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("refusal: second AudioListener at second"));
    assert!(lines[1].ends_with("refusal: second AudioListener at third"));
    assert_eq!(w.query::<&AudioListener>().iter().next().unwrap().0, first);
    step(&mut w);
    assert_eq!(w.journal().len(), before + lines.len());
    w.despawn(first);
    let replacement = w.spawn_named("replacement", AudioListener);
    w.spawn_named("extra", AudioListener);
    step(&mut w);
    assert_eq!(w.query::<&AudioListener>().iter().count(), 1);
    assert_eq!(
        w.query::<&AudioListener>().iter().next().unwrap().0,
        replacement
    );
    assert!(w
        .journal()
        .last()
        .unwrap()
        .line
        .ends_with("AudioListener at extra"));
}

#[test]
fn source_changes_preserve_order_across_holes_and_recycled_entities() {
    let mut w = World::new(60, 0);
    w.sounds([
        ("wind", Synth::noise().looped()),
        ("tone", Synth::sine(440.)),
    ]);
    let first = w.spawn_named("first", AudioSource::new("wind"));
    let second = w.spawn_named("second", AudioSource::new("wind"));
    let third = w.spawn_named("third", AudioSource::new("wind"));
    step(&mut w);
    w.remove::<AudioSource>(first);
    w.despawn(second);
    let replacement = w.spawn_named("replacement", AudioSource::new("tone").gain(0.5));
    assert_eq!(replacement.index(), second.index());
    w.get_mut::<AudioSource>(third).unwrap().gain = f32::NAN;
    let before = w.journal().len();
    step(&mut w);
    let lines: Vec<_> = w
        .journal()
        .iter()
        .skip(before)
        .map(|e| e.line.clone())
        .collect();
    assert_eq!(
        lines,
        [
            "t=0 tick=0 loop wind off",
            "t=0 tick=0 loop wind off",
            "t=0 tick=0 loop tone on gain 0.50",
            "t=0 tick=0 refusal: invalid AudioSource gain at third",
            "t=0 tick=0 loop wind on gain 0.00",
        ]
    );
    w.insert(first, AudioSource::new("wind").gain(0.2));
    let before = w.journal().len();
    step(&mut w);
    assert_eq!(w.journal().len(), before + 1);
    assert!(w
        .journal()
        .last()
        .unwrap()
        .line
        .ends_with("loop wind on gain 0.20"));
    for entity in [first, replacement, third] {
        w.remove::<AudioSource>(entity);
    }
    let before = w.journal().len();
    step(&mut w);
    let lines: Vec<_> = w
        .journal()
        .iter()
        .skip(before)
        .map(|e| e.line.clone())
        .collect();
    assert_eq!(
        lines,
        [
            "t=0 tick=0 loop wind off",
            "t=0 tick=0 loop tone off",
            "t=0 tick=0 loop wind off"
        ]
    );
    let before = w.journal().len();
    step(&mut w);
    assert_eq!(w.journal().len(), before);
    w.insert(third, AudioSource::new("wind").gain(f32::NAN));
    step(&mut w);
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("invalid AudioSource gain at third"))
            .count(),
        2
    );
    w.despawn(third);
    let recycled = w.spawn_named("third", AudioSource::new("wind").gain(f32::NAN));
    assert_eq!(recycled.index(), third.index());
    step(&mut w);
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("invalid AudioSource gain at third"))
            .count(),
        3
    );
}

// Synth-only worlds save and hash exactly as before sampled sounds existed.
#[test]
fn synth_definitions_and_voices_keep_their_encoding() {
    let mut w = World::new(60, 3);
    w.sounds([(
        "chime",
        Synth::sine(880.0)
            .decay(0.6)
            .seconds(0.8)
            .layer(Synth::sine(1320.0).gain(0.4)),
    )]);
    let e = w.spawn_named("lantern", crate::Transform::at(1.0, 0.0, 0.0));
    w.play("chime").at(e).pitch(1.03).gain(0.7).start();
    w.play("chime").ui().start();
    let definition = crate::hash::of(&w.resource::<Sounds>().0["chime"]);
    let voices = crate::hash::of(&*w.resource::<Voices>());
    let save = crate::hash::of(&w.save());
    let pins: BTreeMap<String, String> =
        crate::json::from_str(include_str!("../tests/pins.json")).unwrap();
    for (name, got) in [
        ("definition", definition),
        ("voices", voices),
        ("save", save),
        ("world", w.hash()),
    ] {
        let pin = &pins[&format!("synth-only-{name}")];
        assert_eq!(&format!("0x{got:016x}"), pin, "synth-only {name} encoding changed");
    }
}
