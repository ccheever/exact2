use exact_game::{
    audio::{self, AudioListener, AudioSource, Sounds, Synth},
    Args, Clock, Game, Input, Quat, Sim, Transform, Vec3, World,
};
use exact_game_audio::{spatial_gains, Call, Listener, NullOutput, Player};
struct SoundGame;
impl Game for SoundGame {
    const ID: &'static str = "audio-test";
    fn setup(w: &mut World, _: &Args) -> Result<(), String> {
        w.register_audio();
        w.resource_mut::<Sounds>()
            .add("chime", Synth::sine(880.0).seconds(1.0));
        w.spawn((AudioListener, Transform::default()));
        w.play("chime").ui().gain(0.8);
        Ok(())
    }
    fn tick(w: &mut World, _: &Input) {
        audio::step(w);
    }
}
#[test]
fn save_mid_chime_restores_offset_and_ends() {
    let mut sim = Sim::<SoundGame>::new(&[]).unwrap();
    sim.advance(0.0, Clock::Seekable);
    sim.advance(250.0, Clock::Seekable);
    let save = sim.save();
    let before = sim.world().hash();
    let mut restored = Sim::<SoundGame>::new(&[]).unwrap();
    restored.restore(&save).unwrap();
    assert_eq!(restored.world().hash(), before);
    let mut player = Player::new(NullOutput::default(), 48000);
    player.sync(restored.world(), None);
    assert_eq!(
        player.output.calls[0],
        Call::Start {
            id: 0,
            samples: 48000,
            rate: 48000,
            looping: false,
            offset: 12000
        }
    );
    assert_eq!(
        player.output.calls[1],
        Call::Set {
            id: 0,
            left: 0.8,
            right: 0.8
        }
    );
    let state = restored.agent(r#"{"op":"state"}"#);
    assert!(state.contains(r#""audio":{"voices":[{"sound":"chime","at":"ui","gain":0.8,"began":0,"ends":60}],"sources":0}"#),"{state}");
    restored.advance(0.0, Clock::Seekable);
    restored.advance(800.0, Clock::Seekable);
    player.sync(restored.world(), None);
    assert_eq!(player.output.calls.last(), Some(&Call::Stop { id: 0 }));
    assert!(restored
        .world()
        .resource::<audio::Voices>()
        .voices
        .is_empty());
}
#[test]
fn loops_follow_sources_and_cache_once() {
    let mut w = World::new(60, 0);
    w.register_audio();
    w.resource_mut::<Sounds>().add("wind", Synth::noise());
    let e = w.spawn((
        Transform::at(1.0, 0.0, 0.0),
        AudioSource {
            sound: "wind".into(),
            ..AudioSource::default()
        },
    ));
    w.propagate();
    let before = w.hash();
    let mut p = Player::new(NullOutput::default(), 48000);
    p.sync(&w, Some(Listener::default()));
    assert_eq!(w.hash(), before);
    assert!(matches!(
        p.output.calls[0],
        Call::Start { looping: true, .. }
    ));
    assert_eq!(
        p.output.calls[1],
        Call::Set {
            id: 0,
            left: 0.0,
            right: 1.0
        }
    );
    p.sync(&w, Some(Listener::default()));
    assert_eq!(p.cached_sounds(), 1);
    assert_eq!(
        p.output
            .calls
            .iter()
            .filter(|c| matches!(c, Call::Start { .. }))
            .count(),
        1
    );
    w.get_mut::<AudioSource>(e).unwrap().playing = false;
    p.sync(&w, None);
    assert_eq!(p.output.calls.last(), Some(&Call::Stop { id: 0 }));
    w.get_mut::<AudioSource>(e).unwrap().playing = true;
    p.sync(&w, None);
    assert_eq!(p.cached_sounds(), 1);
    w.despawn(e);
    p.sync(&w, None);
    assert_eq!(p.output.calls.last(), Some(&Call::Stop { id: 1 }));
}
#[test]
fn poses_pan_and_attenuate() {
    let l = Listener::default();
    let (a, b) = spatial_gains(l, Vec3::NEG_Z, 1.0);
    assert!((a - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert_eq!(a, b);
    assert_eq!(spatial_gains(l, Vec3::X, 1.0), (0.0, 1.0));
    assert_eq!(spatial_gains(l, Vec3::NEG_X, 1.0), (1.0, 0.0));
    assert_eq!(spatial_gains(l, Vec3::X * 40.0, 1.0), (0.0, 0.0));
    let near = spatial_gains(l, Vec3::X * 2.0, 1.0).1;
    assert!(near > 0.49 && near < 0.5);
    let turned = Listener {
        position: Vec3::new(2.0, 0.0, 0.0),
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
    };
    let (a, b) = spatial_gains(turned, Vec3::new(2.0, 0.0, -1.0), 1.0);
    assert!(a < 0.001 && b > 0.999);
}

#[test]
fn settle_waits_for_voice_and_late_frames_skip_finished_pcm() {
    let mut sim = Sim::<SoundGame>::new(&[]).unwrap();
    assert!(!sim.quiescent());
    sim.advance(0.0, Clock::Seekable);
    sim.advance(1000.0, Clock::Seekable);
    assert!(sim.quiescent());
    let mut player = Player::new(NullOutput::default(), 48000);
    player.sync(sim.world(), None);
    assert!(player.output.calls.is_empty());
}
