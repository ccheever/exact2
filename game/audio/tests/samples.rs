//! Sampled voices through the Player: their own rates and channels, offsets,
//! pan, fades, loops, restore, and mixing with synthesized voices.
use exact_game::{
    asset::SoundData,
    audio::{self, AudioListener, AudioSource, Sample, Synth},
    bin, Clock, Game, Input, Sim, Transform, World,
};
use exact_game_audio::{Call, Listener, Player, RecordingOutput};

struct Mix;
impl Game for Mix {
    const ID: &'static str = "audio-samples";
    const ASSETS: &'static [&'static str] = &["blip.sound", "pad.sound"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.sounds([
            ("blip", Sample::new("blip.sound")),
            ("pad", Sample::new("pad.sound").looped().gain(0.5)),
        ]);
        w.sounds([("chime", Synth::sine(880.0).seconds(1.0))]);
        w.spawn((AudioListener, Transform::default()));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        audio::step(w);
    }
}
fn sim() -> Sim<Mix> {
    let mut sim = Sim::<Mix>::new(()).unwrap();
    sim.load_assets(|name| {
        let (rate, channels, frames) = match name {
            "blip.sound" => (22_050, 1, 11_025), // 0.5 s mono
            _ => (44_100, 2, 44_100),            // 1 s stereo loop
        };
        let samples: Vec<i16> = (0..frames * channels).map(|i| (i % 97) as i16).collect();
        Ok::<_, String>(bin::to_vec(
            &SoundData::encode(rate, channels as u32, &samples).unwrap(),
        ))
    })
    .unwrap();
    sim
}
fn advance(sim: &mut Sim<Mix>, ms: f64) {
    sim.advance(0.0, Clock::Seekable);
    sim.advance(ms, Clock::Seekable);
}
fn starts(p: &Player<RecordingOutput>) -> Vec<(u64, usize, usize, u32, bool, usize, f32)> {
    p.output
        .calls
        .iter()
        .filter_map(|c| match *c {
            Call::Start {
                id,
                samples,
                channels,
                rate,
                looping,
                offset,
                pitch,
            } => Some((id, samples, channels, rate, looping, offset, pitch)),
            _ => None,
        })
        .collect()
}
fn set(p: &Player<RecordingOutput>, id: u64) -> (f32, f32) {
    p.output
        .calls
        .iter()
        .rev()
        .find_map(|c| match *c {
            Call::Set { id: i, left, right } if i == id => Some((left, right)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn samples_start_at_their_own_rates_and_channels_beside_synth_voices() {
    let mut sim = sim();
    let w = sim.world_mut();
    w.play("blip").gain(0.8).start();
    w.play("pad").start();
    w.play("chime").start();
    let before = sim.world().hash();
    let mut p = Player::new(RecordingOutput::default(), 48_000);
    p.sync(
        sim.world(),
        Listener::from_world(sim.world()),
        Default::default(),
    );
    assert_eq!(
        sim.world().hash(),
        before,
        "playback never changes the world"
    );
    let mut calls = starts(&p);
    calls.sort_by_key(|c| c.1);
    // Loops first, then louder: output ids follow priority, not play order.
    assert_eq!(
        calls,
        [
            (2, 11_025, 1, 22_050, false, 0, 1.0),
            (0, 44_100, 2, 44_100, true, 0, 1.0),
            (1, 48_000, 1, 48_000, false, 0, 1.0),
        ]
    );
    assert_eq!(
        set(&p, 0),
        (0.5, 0.5),
        "a sample definition's gain applies at playback"
    );
    assert_eq!(set(&p, 1), (1.0, 1.0));
    assert_eq!(set(&p, 2), (0.8, 0.8));
    assert_eq!(
        p.cached_sounds(),
        1,
        "only the synth is rendered; samples stay shared"
    );
}

#[test]
fn restore_mid_sample_resumes_at_world_offsets_including_start_offset_and_pitch() {
    let mut sim = sim();
    sim.world_mut().play("blip").pitch(0.5).start();
    sim.world_mut().play("pad").offset(0.75).start();
    advance(&mut sim, 500.0);
    let save = sim.save().unwrap();
    let mut restored = self::sim();
    restored.restore(&save).unwrap();
    let mut p = Player::new(RecordingOutput::default(), 48_000);
    p.sync(restored.world(), None, Default::default());
    let calls = starts(&p);
    // 30 ticks: blip at half speed is 0.25 s in; the loop is 0.75 + 0.5 = 1.25 s → 0.25 s.
    assert!(
        calls.contains(&(1, 11_025, 1, 22_050, false, 5_512, 0.5)),
        "{calls:?}"
    );
    assert!(
        calls.contains(&(0, 44_100, 2, 44_100, true, 11_025, 1.0)),
        "{calls:?}"
    );
    advance(&mut restored, 5_000.0);
    p.sync(restored.world(), None, Default::default());
    assert!(
        p.output.calls.contains(&Call::Stop { id: 1 }),
        "the blip ended"
    );
    assert_eq!(
        restored.world().resource::<audio::Voices>().voices.len(),
        1,
        "the loop continues until stopped"
    );
}

#[test]
fn pan_fade_in_and_fade_out_shape_the_gains_frame_by_frame() {
    let mut sim = sim();
    let pad = sim.world_mut().play("pad").pan(-0.5).fade_in(0.5).start();
    let mut p = Player::new(RecordingOutput::default(), 48_000);
    let mut gains = Vec::new();
    advance(&mut sim, 0.0);
    p.sync(sim.world(), None, Default::default());
    assert!(
        p.output.calls.is_empty(),
        "silent at the start of its fade-in"
    );
    for ms in [250.0, 500.0] {
        advance(&mut sim, ms);
        p.sync(sim.world(), None, Default::default());
        gains.push(set(&p, 0));
    }
    assert_eq!(
        gains[0],
        (0.25, 0.125),
        "half-way in, balance halves the right"
    );
    assert_eq!(gains[1], (0.5, 0.25));
    audio::fade(sim.world_mut(), pad, 1.0);
    advance(&mut sim, 1_000.0);
    p.sync(sim.world(), None, Default::default());
    assert_eq!(set(&p, 0), (0.25, 0.125), "half-way out");
    advance(&mut sim, 1_600.0);
    p.sync(sim.world(), None, Default::default());
    assert_eq!(p.output.calls.last(), Some(&Call::Stop { id: 0 }));
    assert!(sim.world().resource::<audio::Voices>().voices.is_empty());
}

#[test]
fn a_looping_sample_attached_to_an_entity_is_spatial_and_keeps_world_phase() {
    let mut sim = sim();
    sim.world_mut().spawn((
        Transform::at(1.0, 0.0, 0.0),
        AudioSource::new("pad").gain(0.8),
    ));
    advance(&mut sim, 1_500.0);
    let mut p = Player::new(RecordingOutput::default(), 48_000);
    p.sync(
        sim.world(),
        Listener::from_world(sim.world()),
        Default::default(),
    );
    // Tick 90 of a one-second loop at 44.1 kHz: half-way through the second pass.
    assert_eq!(starts(&p), [(0, 44_100, 2, 44_100, true, 22_050, 1.0)]);
    assert_eq!(
        set(&p, 0),
        (0.0, 0.4),
        "one metre to the right, at the definition's gain"
    );
}
