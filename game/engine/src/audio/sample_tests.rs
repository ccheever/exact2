use super::*;
use crate::asset::{Content, SoundData};
use crate::audio::{self as audio, Definition, Sounds, Voices};
use crate::{bin, Clock, Game, Input, Sim, World};

struct Samples;
impl Game for Samples {
    const ID: &'static str = "samples";
    const ASSETS: &'static [&'static str] = &["blip.sound", "pad.sound"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.sounds([
            ("blip", Sample::new("blip.sound")),
            ("pad", Sample::new("pad.sound").looped().gain(0.5)),
        ]);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        audio::step(w);
    }
}
/// Half a second of mono at 22,050 Hz, and one second of stereo at 44,100 Hz.
fn blip() -> Vec<u8> {
    let samples: Vec<i16> = (0..11_025).map(|i| (i % 200) as i16 * 100).collect();
    bin::to_vec(&SoundData::encode(22_050, 1, &samples).unwrap())
}
fn pad(seed: i16) -> Vec<u8> {
    let samples: Vec<i16> = (0..88_200).map(|i| (i % 300) as i16 - seed).collect();
    bin::to_vec(&SoundData::encode(44_100, 2, &samples).unwrap())
}
fn loaded(pad_seed: i16) -> Sim<Samples> {
    let mut sim = Sim::<Samples>::new(()).unwrap();
    assert!(sim.is_loading(), "declared sounds gate setup");
    sim.load_assets(|name| match name {
        "blip.sound" => Ok::<_, String>(blip()),
        "pad.sound" => Ok(pad(pad_seed)),
        other => Err(format!("unexpected {other}")),
    })
    .unwrap();
    assert!(!sim.is_loading());
    sim
}

#[test]
fn declared_samples_gate_setup_and_resolve_their_metadata() {
    let sim = loaded(0);
    let sounds = &sim.world().resource::<Sounds>().0;
    let blip = sounds["blip"].sample().unwrap();
    assert_eq!((blip.frames, blip.rate, blip.channels), (11_025, 22_050, 1));
    assert_eq!(sounds["blip"].duration(), 0.5);
    let pad = sounds["pad"].sample().unwrap();
    assert_eq!(
        (pad.frames, pad.rate, pad.channels, pad.gain),
        (44_100, 44_100, 2, 0.5)
    );
    assert!(sounds["pad"].looping());
    let asset = sim.world().sound_asset("pad.sound").unwrap();
    assert_eq!(asset.samples.len(), 88_200);
    assert_eq!(asset.samples[1], 1);
}

#[test]
fn sample_voices_end_by_frames_offset_and_pitch_and_loops_never_end() {
    let mut sim = loaded(0);
    let w = sim.world_mut();
    let plain = w.play("blip").start();
    let late = w.play("blip").offset(0.2).start();
    let fast = w.play("blip").pitch(2.0).pan(-0.5).start();
    let looping = w.play("pad").offset(1.25).fade_in(0.5).start();
    let voices = &w.resource::<Voices>().voices;
    let ends = |id| voices.iter().find(|v| v.id == id).unwrap().ends;
    assert_eq!(ends(plain), 30);
    assert_eq!(ends(late), 18);
    assert_eq!(ends(fast), 15);
    assert_eq!(ends(looping), u64::MAX);
    assert_eq!(voices[3].offset, 0.25, "loop offsets wrap");
    let journal: Vec<_> = w.journal().iter().map(|e| e.line.clone()).collect();
    assert!(journal.contains(&"t=0 tick=0 sfx blip at ui gain 1.00 pan -0.50".into()));
    assert!(journal
        .contains(&"t=0 tick=0 sfx pad at ui gain 1.00 offset 0.25 fade-in 0.50 loop".into()));
    let state = audio::state(w);
    assert!(
        state.contains(
            r#""sound":"pad","at":"ui","gain":1,"began":0,"ends":null,"offset":0.25,"fade":0}"#
        ),
        "{state}"
    );
    assert!(state.contains(r#""ends":15,"pan":-0.5}"#), "{state}");
}

#[test]
fn fade_ramps_to_silence_then_stops_and_stop_journals() {
    let mut sim = loaded(0);
    let id = sim.world_mut().play("pad").start();
    let blip = sim.world_mut().play("blip").start();
    sim.advance(0.0, Clock::Seekable);
    sim.advance(100.0, Clock::Seekable);
    let now = sim.world().tick();
    audio::fade(sim.world_mut(), id, 0.5);
    audio::stop(sim.world_mut(), blip);
    let voice = sim.world().resource::<Voices>().voices[0].clone();
    assert_eq!(voice.ends, now + 30);
    let state = audio::state(sim.world());
    assert!(state.contains(&format!(r#""ends":{}"#, now + 30)), "a faded loop reports its end: {state}");
    assert_eq!(voice.fade_at(now), 1.0);
    assert_eq!(voice.fade_at(now + 15), 0.5);
    let lines: Vec<_> = sim
        .world()
        .journal()
        .iter()
        .map(|e| e.line.clone())
        .collect();
    assert!(
        lines.iter().any(|l| l.ends_with("sfx pad fade 0.50")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.ends_with("sfx blip stop")),
        "{lines:?}"
    );
    sim.advance(100.0 + 520.0, Clock::Seekable);
    assert!(sim.world().resource::<Voices>().voices.is_empty());
}

#[test]
fn save_mid_sample_restores_and_a_changed_asset_refuses_by_name() {
    let mut sim = loaded(0);
    sim.world_mut()
        .play("blip")
        .at_point(crate::Vec3::X)
        .start();
    sim.world_mut().play("pad").pan(0.25).start();
    sim.advance(0.0, Clock::Seekable);
    sim.advance(250.0, Clock::Seekable);
    let save = sim.save().unwrap();
    let hash = sim.world().hash();
    let mut restored = loaded(0);
    restored.restore(&save).unwrap();
    assert_eq!(restored.world().hash(), hash);
    let voices = &restored.world().resource::<Voices>().voices;
    assert_eq!(voices.len(), 2);
    assert_eq!(voices[1].pan, 0.25);
    assert!(voices[1].definition.sample().is_some());
    // PCM is delivery, not simulation: same-shaped sounds hash alike...
    let mut other = loaded(1);
    other
        .world_mut()
        .play("blip")
        .at_point(crate::Vec3::X)
        .start();
    other.world_mut().play("pad").pan(0.25).start();
    other.advance(0.0, Clock::Seekable);
    other.advance(250.0, Clock::Seekable);
    assert_eq!(other.world().hash(), hash);
    // ...but a save names the content it was made with.
    let error = other.restore(&save).unwrap_err().to_string();
    assert!(
        error.contains("asset `pad.sound` identity differs"),
        "{error}"
    );
}

#[test]
fn undeclared_and_over_budget_sounds_are_refused_by_name() {
    let mut sim = Sim::<Samples>::new(()).unwrap();
    let error = sim.asset("other.sound", Some(&blip())).unwrap_err();
    assert!(
        error.contains("sound `other.sound` is not declared by Game::ASSETS"),
        "{error}"
    );
    struct Heavy;
    impl Game for Heavy {
        const ID: &'static str = "heavy";
        const ASSETS: &'static [&'static str] = &["a.sound", "b.sound"];
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    // Two sounds of 20 MiB each: the second would exceed 32 MiB resident.
    let big = SoundData::encode(48_000, 1, &vec![0; 10 * 1024 * 1024]).unwrap();
    let mut heavy = Sim::<Heavy>::new(()).unwrap();
    heavy
        .deliver_asset("a.sound", Ok(Content::Sound(big.clone())))
        .unwrap();
    let error = heavy
        .deliver_asset("b.sound", Ok(Content::Sound(big)))
        .unwrap_err();
    assert!(
        error.contains("20.0 MiB of 16-bit PCM with 20.0 MiB already resident exceeds the 32.0 MiB sound residency budget"),
        "{error}"
    );
    assert!(
        heavy.is_loading(),
        "a refused declared sound keeps setup waiting"
    );
    let state = heavy.world().assets.state_json();
    assert!(
        state.contains(r#""name":"b.sound","state":"Failed""#),
        "{state}"
    );
}

#[test]
#[should_panic(expected = "sound `x`: asset `x.sound` has not arrived; declare it in Game::ASSETS")]
fn registering_an_undelivered_sample_names_the_declaration() {
    World::new(60, 0).sounds([("x", Sample::new("x.sound"))]);
}

#[test]
fn malformed_saved_samples_are_refused() {
    let sim = loaded(0);
    let good = sim.world().resource::<Sounds>().0["blip"]
        .sample()
        .unwrap()
        .clone();
    for (field, value) in [("channels", 3u64), ("rate", 7_999), ("frames", 0)] {
        let mut sample = good.clone();
        match field {
            "channels" => sample.channels = value as u32,
            "rate" => sample.rate = value as u32,
            _ => sample.frames = value,
        }
        let mut world = World::new(60, 0);
        world.register_audio();
        world.resource_mut::<Sounds>().0.insert(
            "blip".into(),
            Definition {
                sound: std::sync::Arc::new(Sound::Sample(sample)),
                revision: 0,
            },
        );
        let mut fresh = World::new(60, 0);
        fresh.register_audio();
        let error = fresh.load(&world.save()).unwrap_err().to_string();
        assert!(error.contains(field), "{field}: {error}");
    }
}
