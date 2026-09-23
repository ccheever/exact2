use audio_fixture_logic::AudioFixture;
use exact_game::{audio::Voices, Sim};
use exact_game_audio::{Call, Listener, Player, RecordingOutput};
use std::{collections::BTreeSet, path::Path};

/// Bake `art/<stem>.wav|.ogg` in the test, as the app bake does.
fn baked(name: &str) -> Result<Vec<u8>, String> {
    let stem = name.strip_suffix(".sound").ok_or(name)?;
    let art = Path::new(env!("CARGO_MANIFEST_DIR")).join("../art");
    let path = ["wav", "ogg"]
        .map(|ext| art.join(format!("{stem}.{ext}")))
        .into_iter()
        .find(|path| path.exists())
        .ok_or(name)?;
    exact_game_bake::encode(name, &exact_game_bake::sound(path)?)
}
fn sim() -> Sim<AudioFixture> {
    Sim::<AudioFixture>::with_assets((), baked).unwrap()
}

#[test]
fn baked_samples_reach_the_outputs_at_their_own_rates_and_channels() {
    let mut sim = sim();
    let mut player = Player::new(RecordingOutput::default(), 48_000);
    for _ in 0..90 {
        if sim.world().tick() == 44 {
            sim.tap("Space");
        }
        sim.run(1000.0 / 60.0);
        player.sync(
            sim.world(),
            Listener::from_world(sim.world()),
            Default::default(),
        );
    }
    let started: BTreeSet<_> = player
        .output
        .calls
        .iter()
        .filter_map(|call| match *call {
            Call::Start {
                channels,
                rate,
                looping,
                ..
            } => Some((rate, channels, looping)),
            _ => None,
        })
        .collect();
    let expected = [
        (22_050, 1, false), // blip.wav, 16-bit
        (22_050, 2, true),  // drone.ogg, the voice and the source
        (32_000, 2, false), // chord.wav, 24-bit stereo
        (48_000, 1, false), // whoosh.wav, float; and the synthesized tick
    ];
    assert_eq!(started, expected.into_iter().collect(), "{started:?}");
}

#[test]
fn the_proofs_pinned_endpoint() {
    let mut sim = sim();
    sim.assert_pin(include_str!("../../pins.json"));
    sim.run(1500.0);
    assert_eq!(sim.world().tick(), 90);
    sim.assert_pin(include_str!("../../pins.json"));
}

#[test]
fn restoring_mid_sound_continues_to_the_same_save() {
    let mut continuous = sim();
    continuous.run(750.0);
    continuous.tap("KeyF");
    continuous.run(100.0);
    let midpoint = continuous.save().unwrap();
    continuous.run(650.0);
    let mut restored = sim();
    restored.restore(&midpoint).unwrap();
    assert!(restored
        .world()
        .resource::<Voices>()
        .voices
        .iter()
        .any(|v| v.sound == "drone" && v.fade.is_some()));
    restored.run(650.0);
    assert_eq!(restored.world().hash(), continuous.world().hash());
    assert_eq!(restored.save().unwrap(), continuous.save().unwrap());
    assert!(
        !restored
            .world()
            .resource::<Voices>()
            .voices
            .iter()
            .any(|v| v.sound == "drone"),
        "the faded loop has ended"
    );
}
