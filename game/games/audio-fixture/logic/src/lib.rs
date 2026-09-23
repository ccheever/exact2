//! Sampled sounds beside a synthesized one. `art/` holds a 16-bit mono WAV, a
//! 24-bit stereo WAV, a float WAV and an Ogg Vorbis loop; the bake turns each into a
//! `.sound` asset. One-shot and looping voices use pan, pitch, offset and fades; a
//! looping source is heard from the listener's right.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use exact_game::audio::{self, AudioListener, AudioSource, Sample, Synth, VoiceId, Voices};
use exact_game::{Actions, Data, Game, Input, Resource, Transform, World};

#[derive(Default, Data)]
struct Hud {
    voices: u32,
    loops: u32,
}

/// The looping music voice, until the player fades it.
#[derive(Resource, Default, Clone)]
pub struct Music {
    /// Its handle while it plays.
    pub voice: Option<VoiceId>,
}

/// Logic behind `canvas surface=world()`.
pub struct AudioFixture;
impl Game for AudioFixture {
    const ID: &'static str = "audio-fixture";
    const ASSETS: &'static [&'static str] =
        &["blip.sound", "chord.sound", "whoosh.sound", "drone.sound"];
    type Args = ();

    fn actions() -> Actions {
        Actions::new()
            .button("chord", &["Space"])
            .button("fade", &["KeyF"])
    }
    fn setup(world: &mut World, _: &()) {
        world.sounds([
            // blip.wav: 16-bit mono, 22,050 Hz.
            ("blip", Sample::new("blip.sound")),
            // chord.wav: 24-bit stereo, 32,000 Hz.
            ("chord", Sample::new("chord.sound").gain(0.8)),
            // whoosh.wav: 32-bit float mono, 48,000 Hz.
            ("whoosh", Sample::new("whoosh.sound")),
            // drone.ogg: Vorbis stereo, 22,050 Hz, one second of whole periods.
            ("drone", Sample::new("drone.sound").looped().gain(0.6)),
        ]);
        world.sounds([(
            "tick",
            Synth::square(660.0).seconds(0.05).release(0.03).gain(0.3),
        )]);
        world.spawn_named("ears", (Transform::default(), AudioListener));
        world.spawn_named(
            "speaker",
            (
                Transform::at(2.0, 0.0, -1.0),
                AudioSource::new("drone").gain(0.5),
            ),
        );
        let voice = world.play("drone").offset(0.5).fade_in(1.0).start();
        world.insert_resource(Music { voice: Some(voice) });
        world.publish_record(&Hud::default());
    }
    fn tick(world: &mut World, input: &Input, _: &()) {
        let tick = world.tick();
        if tick % 30 == 0 {
            let side = if tick % 60 == 0 { -0.6 } else { 0.6 };
            let pitch = 1.0 + (tick / 30 % 3) as f32 * 0.25;
            world.play("blip").pan(side).pitch(pitch).start();
        }
        if tick % 45 == 15 {
            world.play("tick").start();
        }
        if tick == 20 {
            world.play("whoosh").at("speaker").start();
        }
        if input.pressed("chord") {
            world.play("chord").offset(0.02).start();
        }
        if input.pressed("fade") {
            let voice = world.resource_mut::<Music>().voice.take();
            if let Some(voice) = voice {
                audio::fade(world, voice, 0.5);
            }
        }
        audio::step(world);
        let hud = {
            let voices = world.resource::<Voices>();
            Hud {
                voices: voices.voices.len() as u32,
                loops: voices.voices.iter().filter(|v| v.looping()).count() as u32
                    + world.count::<AudioSource>(|s| s.playing),
            }
        };
        world.publish_record(&hud);
    }
}
