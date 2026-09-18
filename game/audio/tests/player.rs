use exact_game::{
    audio::{self, AudioListener, AudioSource, Sounds, Synth},
    Clock, Game, Input, Quat, Sim, Transform, Vec3, World,
};
use exact_game_audio::{spatial_gains, Call, Listener, Player, RecordingOutput};
struct SoundGame;
impl Game for SoundGame {
    type Args = ();
    const ID: &'static str = "audio-test";
    fn setup(w: &mut World, _: &Self::Args) {
        w.register_audio();
        w.resource_mut::<Sounds>()
            .add("chime", Synth::sine(880.0).seconds(1.0));
        w.spawn((AudioListener, Transform::default()));
        w.play("chime").ui().gain(0.8);
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        audio::step(w);
    }
}
#[test]
fn save_mid_chime_restores_offset_and_ends() {
    let mut sim = Sim::<SoundGame>::new(()).unwrap();
    sim.advance(0.0, Clock::Seekable);
    sim.advance(250.0, Clock::Seekable);
    let save = sim.save();
    let before = sim.world().hash();
    let mut restored = Sim::<SoundGame>::new(()).unwrap();
    restored.restore(&save).unwrap();
    assert_eq!(restored.world().hash(), before);
    let mut player = Player::new(RecordingOutput::default(), 48000);
    player.sync(restored.world(), None, Default::default());
    assert_eq!(
        player.output.calls[0],
        Call::Start {
            id: 0,
            samples: 48000,
            rate: 48000,
            looping: false,
            offset: 12000,
            pitch: 1.0
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
    assert!(state.contains(r#""audio":{"voices":[{"sound":"chime","at":"ui","gain":0.8,"began":0,"ends":60}],"sources":[]}"#),"{state}");
    restored.advance(0.0, Clock::Seekable);
    restored.advance(800.0, Clock::Seekable);
    player.sync(restored.world(), None, Default::default());
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
    let mut p = Player::new(RecordingOutput::default(), 48000);
    p.sync(&w, Some(Listener::default()), Default::default());
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
    p.sync(&w, Some(Listener::default()), Default::default());
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
    p.sync(&w, None, Default::default());
    assert_eq!(p.output.calls.last(), Some(&Call::Stop { id: 0 }));
    w.get_mut::<AudioSource>(e).unwrap().playing = true;
    p.sync(&w, None, Default::default());
    assert_eq!(p.cached_sounds(), 1);
    w.despawn(e);
    p.sync(&w, None, Default::default());
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
fn voices_are_ambient_and_late_frames_skip_finished_pcm() {
    let mut sim = Sim::<SoundGame>::new(()).unwrap();
    assert!(!sim.quiescent());
    sim.run(17.0);
    assert!(sim.quiescent(), "finite voice bookkeeping is ambient");
    assert!(!sim
        .world()
        .resource::<exact_game::audio::Voices>()
        .voices
        .is_empty());
    sim.run(983.0);
    let mut player = Player::new(RecordingOutput::default(), 48000);
    player.sync(sim.world(), None, Default::default());
    assert!(player.output.calls.is_empty());
}

fn recording() -> Player<RecordingOutput> {
    Player::new(RecordingOutput::default(), 48000)
}
fn world() -> World {
    let mut w = World::new(60, 0);
    w.register_audio();
    w.resource_mut::<Sounds>()
        .add("wind", Synth::noise().seconds(2.0));
    w
}
fn advance(sim: &mut Sim<SoundGame>, ms: f64) {
    sim.advance(0.0, Clock::Seekable);
    sim.advance(ms, Clock::Seekable);
}

// AU2.1: a click authored in Game::tick must survive the first sync.
#[test]
fn tick_zero_ten_ms_click_starts_at_sample_zero() {
    struct Click;
    impl Game for Click {
        type Args = ();
        const ID: &'static str = "click";
        fn setup(w: &mut World, _: &Self::Args) {
            w.register_audio();
            w.resource_mut::<Sounds>()
                .add("click", Synth::square(500.0).seconds(0.01));
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            if w.tick() == 0 {
                w.play("click");
            }
            audio::step(w);
        }
    }
    let mut sim = Sim::<Click>::new(()).unwrap();
    sim.advance(0.0, Clock::Seekable);
    sim.advance(17.0, Clock::Seekable);
    let mut p = recording();
    p.sync(sim.world(), None, Default::default());
    assert!(matches!(
        p.output.calls.first(),
        Some(Call::Start { offset: 0, .. })
    ));
    assert_eq!(sim.world().resource::<audio::Voices>().voices[0].began, 1);
}

// AU2.2: forward seek, same-tick restore, pause and resume all reuse a Player.
#[test]
fn transport_restarts_at_world_offset_and_silences_pause() {
    use exact_game_audio::Transport;
    let mut sim = Sim::<SoundGame>::new(()).unwrap();
    let mut p = recording();
    p.sync(sim.world(), None, Default::default());
    advance(&mut sim, 250.0);
    p.output.calls.clear();
    let mut transport = Transport {
        generation: 1,
        playing: true,
    };
    p.sync(sim.world(), None, transport);
    assert_eq!(p.output.calls[0], Call::Stop { id: 0 });
    assert!(matches!(
        p.output.calls[1],
        Call::Start { offset: 12000, .. }
    ));
    transport.generation += 1;
    p.output.calls.clear();
    p.sync(sim.world(), None, transport);
    assert!(matches!(p.output.calls[0], Call::Stop { .. }));
    assert!(matches!(
        p.output.calls[1],
        Call::Start { offset: 12000, .. }
    ));
    transport.playing = false;
    p.output.calls.clear();
    p.sync(sim.world(), None, transport);
    assert!(matches!(p.output.calls.as_slice(), [Call::Stop { .. }]));
    p.sync(sim.world(), None, transport);
    transport.playing = true;
    p.output.calls.clear();
    p.sync(sim.world(), None, transport);
    assert!(matches!(
        p.output.calls[0],
        Call::Start { offset: 12000, .. }
    ));
}

// AU2.3: 33 loops contend for 32 slots; the dropped loop returns at current phase.
#[test]
fn thirty_third_loop_returns_when_room_opens() {
    let mut sim = Sim::<SoundGame>::new(()).unwrap();
    let mut entities = Vec::new();
    for i in 0..33 {
        entities.push(sim.world_mut().spawn((
            Transform::default(),
            AudioSource {
                sound: "chime".into(),
                gain: if i == 0 { 0.1 } else { 1.0 },
                playing: true,
            },
        )));
    }
    let mut p = recording();
    p.output.capacity = 32;
    p.sync(sim.world(), Some(Listener::default()), Default::default());
    assert_eq!(
        p.output
            .calls
            .iter()
            .filter(|c| matches!(c, Call::Start { .. }))
            .count(),
        32
    );
    assert!(p
        .output
        .calls
        .iter()
        .filter(|c| matches!(c, Call::Start { .. }))
        .all(|c| matches!(c, Call::Start { looping: true, .. })));
    advance(&mut sim, 250.0);
    sim.world_mut().despawn(entities[32]);
    p.output.calls.clear();
    p.sync(sim.world(), Some(Listener::default()), Default::default());
    assert!(matches!(p.output.calls.first(), Some(Call::Stop { .. })));
    assert_eq!(
        p.output
            .calls
            .iter()
            .filter(|c| matches!(c, Call::Start { .. }))
            .count(),
        1
    );
    assert!(p.output.calls.iter().any(|c| matches!(
        c,
        Call::Start {
            offset: 12000,
            looping: true,
            ..
        }
    )));
}

// AU2.5: corrupt public gains and overflowing layers never reach the executor.
#[test]
fn invalid_gains_are_refused_once_and_pcm_is_finite() {
    let mut w = world();
    let e = w.spawn((
        Transform::default(),
        AudioSource {
            sound: "wind".into(),
            gain: -1.0,
            playing: true,
        },
    ));
    for invalid in [-1.0, f32::NAN, f32::INFINITY] {
        w.get_mut::<AudioSource>(e).unwrap().gain = invalid;
        audio::step(&mut w);
        assert_eq!(w.get::<AudioSource>(e).unwrap().gain, 0.0);
    }
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("refusal: invalid AudioSource"))
            .count(),
        1
    );
    let loud = Synth::square(0.0)
        .attack(0.0)
        .decay(0.0)
        .sustain(4.0)
        .gain(f32::MAX);
    let pcm = audio::render(&loud.clone().layer(loud), 48000);
    assert!(pcm.iter().all(|s| s.is_finite() && s.abs() <= 4.0));
    w.play("wind").gain(-2.0);
    w.get_mut::<AudioSource>(e).unwrap().gain = f32::NAN; // bypass step deliberately
    let mut p = recording();
    p.sync(&w, Some(Listener::default()), Default::default());
    for call in &p.output.calls {
        if let Call::Set { left, right, .. } = call {
            assert_eq!((*left, *right), (0.0, 0.0));
        }
    }
}

// AU2.7: no source may be scheduled before an asynchronous unlock succeeds.
#[test]
fn suspended_output_waits_then_starts_at_current_phase() {
    let mut sim = Sim::<SoundGame>::new(()).unwrap();
    let mut p = recording();
    p.output.ready = false;
    p.sync(sim.world(), None, Default::default());
    assert!(p.output.calls.is_empty());
    advance(&mut sim, 250.0);
    p.output.ready = true;
    p.sync(
        sim.world(),
        None,
        exact_game_audio::Transport {
            generation: 1,
            playing: true,
        },
    );
    assert!(matches!(
        p.output.calls[0],
        Call::Start { offset: 12000, .. }
    ));
}

// AU2.8: restore must preserve the loop journal baseline, including refusals.
#[test]
fn loop_journal_diff_survives_save_and_reports_changes_and_removal() {
    let mut w = world();
    let e = w.spawn_named(
        "breeze",
        (
            Transform::default(),
            AudioSource {
                sound: "wind".into(),
                gain: 0.3,
                playing: true,
            },
        ),
    );
    audio::step(&mut w);
    assert!(w
        .journal()
        .last()
        .unwrap()
        .line
        .ends_with("loop wind on gain 0.30"));
    assert!(audio::state(&w)
        .contains(r#""sources":[{"sound":"wind","entity":"breeze","gain":0.3,"playing":true}]"#));
    let save = w.save();
    let mut restored = world();
    restored.register::<Transform>();
    restored.load(&save).unwrap();
    audio::step(&mut restored);
    assert!(restored.journal().is_empty());
    restored.get_mut::<AudioSource>(e).unwrap().gain = 0.6;
    audio::step(&mut restored);
    assert!(restored
        .journal()
        .last()
        .unwrap()
        .line
        .ends_with("loop wind on gain 0.60"));
    restored.get_mut::<AudioSource>(e).unwrap().playing = false;
    audio::step(&mut restored);
    assert!(restored
        .journal()
        .last()
        .unwrap()
        .line
        .ends_with("loop wind off"));
    restored.get_mut::<AudioSource>(e).unwrap().playing = true;
    audio::step(&mut restored);
    restored.despawn(e);
    audio::step(&mut restored);
    assert!(restored
        .journal()
        .last()
        .unwrap()
        .line
        .ends_with("loop wind off"));
}

// AU2.9: an explosion survives same-tick despawn at its saved last position.
#[test]
fn despawned_projectile_explosion_still_plays_at_saved_position() {
    let mut w = world();
    let e = w.spawn(Transform::at(1.0, 0.0, 0.0));
    w.play("wind").at(e);
    w.despawn(e);
    audio::step(&mut w);
    let save = w.save();
    w.load(&save).unwrap();
    let mut p = recording();
    p.sync(&w, Some(Listener::default()), Default::default());
    assert!(p.output.calls.iter().any(|c| matches!(
        c,
        Call::Set {
            left: 0.0,
            right: 1.0,
            ..
        }
    )));
}

// AU2.10: edits keep the old active shot and only the current named revision.
#[test]
fn editing_a_definition_does_not_accumulate_retired_pcm() {
    let mut w = world();
    let id = w.play("wind").start();
    let mut p = recording();
    for hz in 1..100 {
        w.resource_mut::<Sounds>()
            .add("wind", Synth::noise().hz(hz as f32));
        let loop_entity = w.spawn((
            Transform::default(),
            AudioSource {
                sound: "wind".into(),
                ..Default::default()
            },
        ));
        p.sync(&w, Some(Listener::default()), Default::default());
        assert_eq!(p.cached_sounds(), 2);
        w.despawn(loop_entity);
    }
    audio::stop(&mut w, id);
    p.sync(&w, None, Default::default());
    assert_eq!(p.cached_sounds(), 1);
}

// AU2.11: agent output has no accumulating history, even over a long session.
#[test]
fn null_output_discards_every_call() {
    use exact_game_audio::{NullOutput, Output};
    let mut output = NullOutput;
    assert_eq!(std::mem::size_of_val(&output), 0);
    let pcm = vec![0.0; 10].into();
    for id in 0..100_000 {
        output.start(id, &pcm, 48000, false);
        output.set(id, 1.0, 1.0);
        output.stop(id);
    }
    let mut recorder = RecordingOutput::default();
    recorder.start(0, &pcm, 48000, false);
    assert_eq!(recorder.calls.len(), 1);
}

// AU2.13: overlap joins adjacent tail samples instead of a discontinuous wrap.
#[test]
fn looping_noise_crossfades_seam_without_changing_one_shots() {
    let synth = Synth::noise()
        .attack(0.0)
        .decay(0.0)
        .sustain(1.0)
        .release(0.0)
        .seconds(2.0)
        .lowpass_hz(500.0)
        .highpass_hz(60.0)
        .gain(0.5);
    let original = audio::render(&synth, 48000);
    assert_eq!(exact_game::hash::of(&original), 0x47138b008334dd51);
    let looped = audio::render(&synth.looped(), 48000);
    let n = looped.len();
    assert_eq!(n, original.len() - 480);
    assert_eq!(looped[0], original[n]);
    assert_eq!(looped[n - 1], original[n - 1]);
    assert_eq!(looped[480], original[480]);
    for samples in 0..4 {
        let tiny = audio::render(&Synth::noise().seconds(samples as f32 / 8.0).looped(), 8);
        assert!(tiny.iter().all(|s| s.is_finite()));
    }
}

#[test]
fn pitch_handle_and_saved_master_apply_without_definition_edits() {
    let mut sim = Sim::<SoundGame>::new(()).unwrap();
    let id = sim.world_mut().play("chime").pitch(0.5).start();
    sim.world_mut().resource_mut::<audio::Audio>().master = 0.25;
    advance(&mut sim, 250.0);
    let saved = sim.save();
    sim.restore(&saved).unwrap();
    let mut p = recording();
    p.sync(sim.world(), None, Default::default());
    assert!(p.output.calls.iter().any(|c| matches!(
        c,
        Call::Start {
            pitch: 0.5,
            offset: 6000,
            ..
        }
    )));
    assert!(p.output.calls.iter().any(|c| matches!(
        c,
        Call::Set {
            left: 0.25,
            right: 0.25,
            ..
        }
    )));
    audio::stop(sim.world_mut(), id);
    p.output.calls.clear();
    p.sync(sim.world(), None, Default::default());
    assert!(matches!(p.output.calls.first(), Some(Call::Stop { .. })));
}
