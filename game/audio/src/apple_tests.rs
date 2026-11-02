use super::*;
use crate::Pcm;
fn f(samples: &Arc<[f32]>) -> Pcm {
    Pcm::F32(samples.clone())
}
fn start(p: &mut Pending, id: u64, pcm: &Arc<[f32]>, rate: u32) {
    p.start(id, &f(pcm), rate, true, 0, 1.0);
    p.set(id, 1.0, 1.0);
}

// AU2.4: no callback for hundreds of frames, then draining resumes.
#[test]
fn stopped_callback_coalesces_sets_and_retries_every_start_stop() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.25].into();
    for id in 0..32 {
        start(&mut p, id, &pcm, 48000);
    }
    for frame in 0..2000 {
        for id in 0..32 {
            p.set(id, frame as f32 / 2000.0, 0.0);
        }
        p.flush();
    }
    assert_eq!(p.sets.len(), 32);
    for id in 0..32 {
        p.stop(id);
    }
    for id in 32..1100 {
        start(&mut p, id, &pcm, 48000);
        p.stop(id);
    }
    start(&mut p, 1100, &pcm, 48000);
    p.set(1100, 0.7, 0.2);
    for _ in 0..10 {
        p.flush();
        m.commands();
    }
    p.flush();
    assert!(p.controls.is_empty() && p.sets.is_empty());
    let voices: Vec<_> = m.voices.iter().flatten().collect();
    assert_eq!(voices.len(), 1);
    assert_eq!(voices[0].id, 1100);
    assert_eq!((voices[0].target_left, voices[0].target_right), (0.7, 0.2));
}

// AU2.5/6: NaN/Inf silence, full scale and quiet samples retain linear gain.
#[test]
fn mixer_is_linear_below_one_and_firewalls_nonfinite_samples() {
    let (mut p, mut m) = Pending::new();
    m.rate = 4.0; // one-sample ramp for the resampling oracle
    let pcm: Arc<[f32]> = vec![0.0, 1.0, 0.0, -1.0].into();
    p.start(7, &f(&pcm), 2, true, 1, 1.0);
    p.set(7, 1.0, 0.0);
    p.flush();
    m.commands();
    for expected in [1.0, 0.5, 0.0, -0.5, -1.0, -0.5, 0.0, 0.5, 1.0] {
        assert_eq!(m.frame(), (expected, 0.0));
    }
    p.stop(7);
    let pcm: Arc<[f32]> = vec![f32::NAN, f32::INFINITY, 0.1, 4.0].into();
    start(&mut p, 8, &pcm, 4);
    p.flush();
    m.commands();
    assert_eq!(m.frame(), (0.0, 0.0));
    assert_eq!(m.frame(), (0.0, 0.0));
    assert_eq!(m.frame(), (0.1, 0.1));
    assert_eq!(m.frame(), (1.0, 1.0));
}

// AU2.10: retired raw pointers stay owned until the return-ring acknowledgement.
#[test]
fn pcm_is_freed_only_on_producer_after_stop_acknowledgement() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.25].into();
    let weak = Arc::downgrade(&pcm);
    start(&mut p, 0, &pcm, 48000);
    drop(pcm);
    p.flush();
    m.commands();
    p.stop(0);
    p.flush();
    assert!(weak.upgrade().is_some());
    m.commands();
    assert!(weak.upgrade().is_some()); // the callback never releases ownership
    assert_eq!(m.frame(), (0.0, 0.0));
    p.flush();
    assert!(weak.upgrade().is_none());
}

// AU2.12: a frame-rate gain change ramps over 480 samples, including retargeting.
#[test]
fn gains_ramp_per_sample_over_ten_ms() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![1.0].into();
    start(&mut p, 0, &pcm, 48000);
    p.flush();
    m.commands();
    let first = m.frame().0;
    assert!((first - 1.0 / 480.0).abs() < 1e-7);
    for _ in 1..240 {
        m.frame();
    }
    assert!((m.voices[0].unwrap().left - 0.5).abs() < 1e-5);
    p.set(0, 0.0, 0.0);
    p.flush();
    m.commands();
    let retarget = m.frame().0;
    assert!(retarget < 0.5 && retarget > 0.49);
    for _ in 1..480 {
        m.frame();
    }
    assert_eq!(m.frame(), (0.0, 0.0));
}

#[test]
fn full_return_ring_retries_latest_watermark_without_freeing_early() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.5].into();
    // Do not consume acknowledgements while the callback keeps sending them.
    for sequence in 1..=CAPACITY as u64 + 2 {
        p.commands
            .push(Packet {
                sequence,
                command: Command::Stop(sequence),
            })
            .ok()
            .unwrap();
        m.commands();
    }
    assert!(m.pending_ack.is_some());
    while p.acknowledgements.pop().is_some() {}
    m.commands();
    assert_eq!(p.acknowledgements.pop(), Some(CAPACITY as u64 + 2));
    start(&mut p, 0, &pcm, 48000);
}

#[test]
fn callback_accepts_null_interleaved_planar_short_and_mono() {
    use std::{ffi::c_void, ptr};
    #[repr(C)]
    struct List {
        count: u32,
        buffers: [Buffer; 2],
    }
    for case in 0..7 {
        let (mut p, mut mixer) = Pending::new();
        mixer.rate = 4.0;
        let pcm: Arc<[f32]> = vec![0.25].into();
        start(&mut p, 1, &pcm, 4);
        p.set(1, 1.0, 0.5);
        p.flush();
        let mut left = [9.0f32; 8];
        let mut right = [9.0f32; 8];
        let mut list = List {
            count: if case == 3 { 2 } else { 1 },
            buffers: [
                Buffer {
                    channels: if case == 3 || case == 5 { 1 } else { 2 },
                    bytes: if case == 4 { 4 } else { 16 },
                    data: left.as_mut_ptr().cast(),
                },
                Buffer {
                    channels: 1,
                    bytes: 8,
                    data: right.as_mut_ptr().cast(),
                },
            ],
        };
        if case == 6 {
            list.buffers[0].data = ptr::null_mut();
        }
        let context = if case == 0 {
            ptr::null_mut()
        } else {
            (&mut mixer as *mut Mixer).cast::<c_void>()
        };
        let data = if case == 1 {
            ptr::null_mut()
        } else {
            (&mut list as *mut List).cast::<Buffers>()
        };
        // SAFETY: the list has its advertised buffer records and live sample storage.
        assert_eq!(
            unsafe { render(context, ptr::null_mut(), ptr::null(), 0, 2, data) },
            0
        );
        assert_eq!(
            mixer.voices.iter().flatten().count(),
            usize::from(case != 0)
        );
        match case {
            0 | 1 | 6 => assert_eq!(left, [9.0; 8]),
            2 => assert_eq!(&left[..4], &[0.25, 0.125, 0.25, 0.125]),
            3 => {
                assert_eq!(&left[..2], &[0.25; 2]);
                assert_eq!(&right[..2], &[0.125; 2]);
            }
            4 => {
                assert_eq!(left[0], 0.0);
                assert_eq!(left[1], 9.0);
            }
            5 => assert_eq!(&left[..4], &[0.0; 4]),
            _ => unreachable!(),
        }
        assert_eq!(left[4], 9.0);
    }
}

#[test]
fn stalled_device_bounds_controls_and_pcm_during_voice_churn() {
    let (mut p, mut m) = Pending::new();
    for id in 0..100_000 {
        let pcm: Arc<[f32]> = vec![0.25].into();
        start(&mut p, id, &pcm, 48000);
        p.flush();
        p.stop(id);
        assert!(p.controls.len() <= 64);
        assert!(p.retained.len() <= CAPACITY + 32);
    }
    for _ in 0..4 {
        p.flush();
        m.commands();
    }
    p.flush();
    assert!(p.controls.is_empty());
    assert!(p.retained.is_empty());
    assert!(m.voices.iter().all(Option::is_none));
}

#[test]
fn cancelling_unpublished_shared_pcm_does_not_release_a_pending_stop() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.25].into();
    let weak = Arc::downgrade(&pcm);
    start(&mut p, 0, &pcm, 48000);
    p.flush();
    m.commands();
    p.flush();
    p.stop(0); // published voice still references PCM; stop is not published yet
    start(&mut p, 1, &pcm, 48000);
    p.stop(1); // cancelling this unpublished start must preserve voice 0's ownership
    drop(pcm);
    assert!(weak.upgrade().is_some());
    assert!(m.frame().0 > 0.0);
    p.flush();
    m.commands();
    p.flush();
    assert!(weak.upgrade().is_none());
}

#[test]
fn unique_pcm_budget_refuses_without_queueing_and_releases_after_ack() {
    let (mut p, mut m) = Pending::new();
    p.byte_budget = 32;
    let a: Arc<[f32]> = vec![0.25; 8].into();
    let b: Arc<[f32]> = vec![0.5; 8].into();
    assert!(p.start(1, &f(&a), 48000, true, 0, 1.0));
    assert!(p.start(2, &f(&a), 48000, true, 0, 1.0)); // shared allocation counts once
    let queued = p.controls.len();
    assert!(!p.start(3, &f(&b), 48000, true, 0, 1.0));
    assert_eq!(p.controls.len(), queued);
    assert!(!p.live.contains_key(&3));
    assert!(!p.start(1, &f(&b), 48000, true, 0, 1.0));
    assert_eq!(p.controls.len(), queued);
    assert_eq!(p.live[&1], a.as_ptr() as usize);
    p.flush();
    p.stop(1);
    p.stop(2);
    p.flush();
    assert!(!p.start(3, &f(&b), 48000, true, 0, 1.0)); // callback still owns A
    m.commands();
    p.flush();
    assert!(p.start(3, &f(&b), 48000, true, 0, 1.0));
    assert_eq!(p.retained.len(), 1);
    p.stop(3); // unpublished start releases its bytes immediately
    assert!(p.retained.is_empty());
    let oversized: Arc<[f32]> = vec![0.0; 9].into();
    assert!(!p.start(4, &f(&oversized), 48000, true, 0, 1.0));
}

#[test]
fn unsupported_callback_layouts_advance_phase_and_finish_voices() {
    use std::ptr;
    for case in 0..5 {
        let (mut p, mut m) = Pending::new();
        m.rate = 4.0;
        let pcm: Arc<[f32]> = vec![0.1, 0.2, 0.3, 0.4].into();
        p.start(1, &f(&pcm), 4, false, 0, 1.0);
        p.set(1, 1.0, 1.0);
        p.flush();
        let mut samples = [9.0f32; 4];
        let mut buffers = Buffers {
            count: if case == 4 { 0 } else { 1 },
            first: Buffer {
                channels: if case == 1 { 1 } else { 2 },
                bytes: if case == 2 { 4 } else { 16 },
                data: if case == 3 {
                    ptr::null_mut()
                } else {
                    samples.as_mut_ptr().cast()
                },
            },
        };
        let output = if case == 0 {
            ptr::null_mut()
        } else {
            &mut buffers
        };
        // SAFETY: live mixer and buffers with the advertised storage.
        unsafe {
            render(
                (&mut m as *mut Mixer).cast(),
                ptr::null_mut(),
                ptr::null(),
                0,
                2,
                output,
            );
        }
        assert_eq!(m.frame(), (0.3, 0.3), "layout {case} lost phase");
        // SAFETY: same live callback context, discarding the next quantum.
        unsafe {
            render(
                (&mut m as *mut Mixer).cast(),
                ptr::null_mut(),
                ptr::null(),
                0,
                1,
                ptr::null_mut(),
            );
        }
        assert!(m.voices.iter().all(Option::is_none));
    }
}

#[test]
fn full_mixer_does_not_ack_or_lose_a_start() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.25; 2].into();
    for id in 0..32 {
        start(&mut p, id, &pcm, 48000);
    }
    p.flush();
    m.commands();
    while p.acknowledgements.pop().is_some() {}
    p.commands
        .push(Packet {
            sequence: 9999,
            command: Command::Start {
                id: 99,
                pcm: Samples::of(&f(&pcm)),
                rate: 48000,
                looping: true,
                offset: 0,
                pitch: 1.0,
            },
        })
        .ok()
        .unwrap();
    m.commands();
    assert_eq!(
        p.acknowledgements.pop(),
        None,
        "unaccepted start was acknowledged"
    );
    m.voices[0] = None;
    m.commands();
    assert!(m.voices.iter().flatten().any(|v| v.id == 99));
    assert_eq!(p.acknowledgements.pop(), Some(9999));
}

#[test]
fn full_producer_reports_rejection_and_stops_pass_unpublished_starts() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.25; 48000].into();
    for id in 0..32 {
        assert!(p.start(id, &f(&pcm), 48000, true, 0, 1.0));
    }
    assert!(!p.start(32, &f(&pcm), 48000, true, 0, 1.0));
    p.flush();
    m.commands();
    p.flush();
    p.stop(0);
    assert!(p.start(32, &f(&pcm), 48000, true, 0, 1.0));
    p.flush(); // Stop(0) is published; Start(32) waits for acknowledgement.
    p.stop(1); // Must pass that blocked start so it can release another slot.
    assert!(p.start(33, &f(&pcm), 48000, true, 0, 1.0));
    p.flush();
    m.commands();
    p.flush();
    m.commands();
    p.flush();
    let ids: Vec<_> = m.voices.iter().flatten().map(|v| v.id).collect();
    assert_eq!(ids.len(), 32);
    assert!(ids.contains(&32) && ids.contains(&33));
    assert!(!ids.contains(&0) && !ids.contains(&1));
    assert!(p.controls.is_empty() && p.stopping.is_empty());
}

#[test]
fn unpublished_same_id_replacement_subtracts_released_bytes() {
    let (mut p, _) = Pending::new();
    p.byte_budget = 32;
    let a: Arc<[f32]> = vec![0.25; 8].into();
    let b: Arc<[f32]> = vec![0.5; 8].into();
    assert!(p.start(1, &f(&a), 48000, true, 0, 1.0));
    assert!(p.start(1, &f(&b), 48000, true, 0, 1.0));
    assert_eq!(p.retained.len(), 1);
    assert_eq!(p.live[&1], b.as_ptr() as usize);
}

#[test]
fn player_reserves_pcm_before_synthesis_and_walks_past_refused_voices() {
    use crate::{Listener, Output, Player};
    use exact_game::{
        audio::{AudioSource, Sounds, Synth},
        Transform, World,
    };
    struct Device(Pending);
    impl Output for Device {
        fn capacity(&self) -> usize {
            32
        }
        fn flush(&mut self) {
            self.0.flush();
        }
        fn owns_pcm(&self, pcm: &Pcm) -> bool {
            self.0.retained.contains_key(&pcm.address())
        }
        fn start(
            &mut self,
            id: u64,
            pcm: &Pcm,
            rate: u32,
            looping: bool,
            offset: usize,
            pitch: f32,
        ) -> bool {
            self.0.start(id, pcm, rate, looping, offset, pitch)
        }
        fn set(&mut self, id: u64, l: f32, r: f32) {
            self.0.set(id, l, r);
        }
        fn stop(&mut self, id: u64) {
            self.0.stop(id);
        }
    }
    let (pending, mut mixer) = Pending::new();
    let mut player = Player::new(Device(pending), 48000);
    let mut world = World::new(60, 0);
    world.sounds([("small", Synth::square(2.).seconds(0.01).looped())]);
    world.spawn((
        Transform::default(),
        AudioSource {
            sound: "small".into(),
            gain: 0.1,
            playing: true,
        },
    ));
    for i in 0..32 {
        let name = format!("large-{i}");
        world
            .resource_mut::<Sounds>()
            .add(&name, Synth::square(i as f32).seconds(60.).looped());
        world.spawn((
            Transform::default(),
            AudioSource {
                sound: name,
                gain: 1.,
                playing: true,
            },
        ));
    }
    player.sync(&world, Some(Listener::default()), Default::default());
    assert!(player.cache.values().map(|p| p.len() * 4).sum::<usize>() <= PCM_BYTE_BUDGET);
    assert_eq!(
        player.active.len(),
        3,
        "two long loops plus the small lower-priority loop"
    );
    let ids: Vec<_> = player.active.values().map(|a| a.output_id).collect();
    mixer.commands();
    player.sync(&world, Some(Listener::default()), Default::default());
    assert_eq!(
        ids,
        player
            .active
            .values()
            .map(|a| a.output_id)
            .collect::<Vec<_>>(),
        "a selected small fallback must not restart every frame"
    );
    mixer.commands();
    player.sync(&world, None, Default::default());
    assert!(
        !player.cache.is_empty(),
        "published PCM lives until stop acknowledgement"
    );
    mixer.commands();
    player.sync(&world, None, Default::default());
    assert!(
        player.cache.is_empty(),
        "acknowledgement releases the reservation"
    );
}

#[test]
fn preferred_source_waits_for_stop_ack_without_restarting_the_loser() {
    use crate::{Listener, Output, Player};
    use exact_game::{
        audio::{AudioSource, Synth},
        Transform, World,
    };
    struct Device(Pending);
    impl Output for Device {
        fn capacity(&self) -> usize {
            32
        }
        fn flush(&mut self) {
            self.0.flush();
        }
        fn owns_pcm(&self, pcm: &Pcm) -> bool {
            self.0.retained.contains_key(&pcm.address())
        }
        fn start(
            &mut self,
            id: u64,
            pcm: &Pcm,
            rate: u32,
            looping: bool,
            offset: usize,
            pitch: f32,
        ) -> bool {
            self.0.start(id, pcm, rate, looping, offset, pitch)
        }
        fn set(&mut self, id: u64, l: f32, r: f32) {
            self.0.set(id, l, r);
        }
        fn stop(&mut self, id: u64) {
            self.0.stop(id);
        }
    }
    let (pending, mut mixer) = Pending::new();
    let mut player = Player::new(Device(pending), 48000);
    let mut world = World::new(60, 0);
    let tones = [("A", 100., 0.1), ("B", 200., 0.8), ("C", 300., 0.7)];
    world.sounds(tones.map(|(name, hz, _)| (name, Synth::square(hz).seconds(60.).looped())));
    let mut entities = Vec::new();
    for (name, _, gain) in tones {
        entities.push(world.spawn((Transform::default(), AudioSource::new(name).gain(gain))));
    }
    player.sync(&world, Some(Listener::default()), Default::default());
    mixer.commands();
    world.get_mut::<AudioSource>(entities[0]).unwrap().gain = 1.;
    for _ in 0..4 {
        player.sync(&world, Some(Listener::default()), Default::default());
        assert!(
            !player.active.contains_key(&crate::Key::Source(entities[2])),
            "C must stay stopped while A waits for its PCM"
        );
    }
    mixer.commands();
    player.sync(&world, Some(Listener::default()), Default::default());
    assert!(player.active.contains_key(&crate::Key::Source(entities[0])));
    assert!(player.active.contains_key(&crate::Key::Source(entities[1])));
    assert!(!player.active.contains_key(&crate::Key::Source(entities[2])));
}

fn stereo(samples: &[i16]) -> Pcm {
    Pcm::I16 {
        samples: samples.to_vec().into(),
        channels: 2,
    }
}

// AU4: 16-bit stereo frames resample per channel; each gain scales its own channel.
#[test]
fn stereo_sixteen_bit_frames_resample_per_channel() {
    let (mut p, mut m) = Pending::new();
    m.rate = 4.0; // one-sample ramp; a 2 Hz source steps half a frame per sample
    let pcm = stereo(&[16384, -16384, 0, 8192, -16384, 0]);
    assert!(p.start(1, &pcm, 2, true, 0, 1.0));
    p.set(1, 1.0, 0.5);
    p.flush();
    m.commands();
    let frames: Vec<_> = (0..6).map(|_| m.frame()).collect();
    assert_eq!(
        frames,
        [
            (0.5, -0.25),
            (0.25, -0.0625),
            (0.0, 0.125),
            (-0.25, 0.0625),
            (-0.5, 0.0),
            (0.0, -0.125), // the loop interpolates back toward frame zero
        ]
    );
}

// AU4: a synthesized mono voice and a sampled stereo voice sum linearly, each at
// its own rate; a 44.1 kHz source advances 44.1/48 frames per 48 kHz sample.
#[test]
fn synth_and_sample_voices_mix_at_their_own_rates() {
    let (mut p, mut m) = Pending::new();
    m.rate = 4.0;
    let tone: Arc<[f32]> = vec![0.25; 4].into();
    start(&mut p, 1, &tone, 4);
    assert!(p.start(2, &stereo(&[8192, -8192, 8192, -8192]), 2, true, 0, 1.0));
    p.set(2, 1.0, 1.0);
    p.flush();
    m.commands();
    assert_eq!(m.frame(), (0.5, 0.0));
    let steps: Vec<_> = m.voices.iter().flatten().map(|v| v.step).collect();
    assert_eq!(steps, [1.0, 0.5]);
    let (mut p, mut m) = Pending::new();
    m.rate = 48_000.0;
    assert!(p.start(3, &stereo(&[0; 8]), 44_100, true, 0, 1.0));
    p.flush();
    m.commands();
    let step = m.voices.iter().flatten().next().unwrap().step;
    assert!((step - 44_100.0 / 48_000.0).abs() < 1e-12);
}

// AU4: retained 16-bit PCM counts two bytes per sample against the byte budget.
#[test]
fn sixteen_bit_pcm_counts_its_own_bytes() {
    let (mut p, _) = Pending::new();
    p.byte_budget = 16;
    assert!(p.start(1, &stereo(&[0; 8]), 48_000, true, 0, 1.0)); // 16 bytes
    assert!(!p.start(2, &stereo(&[0; 2]), 48_000, true, 0, 1.0));
}

// AU4: the callback writes interleaved stereo from a 16-bit stereo voice.
#[test]
fn callback_renders_sixteen_bit_stereo_into_the_device_buffer() {
    use std::ffi::c_void;
    let (mut p, mut mixer) = Pending::new();
    mixer.rate = 4.0;
    assert!(p.start(1, &stereo(&[16384, -8192, -16384, 8192]), 4, true, 0, 1.0));
    p.set(1, 1.0, 1.0);
    p.flush();
    let mut out = [9.0f32; 8];
    let mut list = Buffers {
        count: 1,
        first: Buffer {
            channels: 2,
            bytes: 32,
            data: out.as_mut_ptr().cast(),
        },
    };
    // SAFETY: the list has one advertised interleaved buffer with 4 stereo frames.
    let status = unsafe {
        render(
            (&mut mixer as *mut Mixer).cast::<c_void>(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            4,
            &mut list,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(out, [0.5, -0.25, -0.5, 0.25, 0.5, -0.25, -0.5, 0.25]);
}

#[test]
fn lifecycle_discard_clears_voices_and_full_start_ring_without_dropping_pcm() {
    let (mut p, mut m) = Pending::new();
    let pcm: Arc<[f32]> = vec![0.25].into();
    for id in 0..32 {
        start(&mut p, id, &pcm, 48000);
    }
    p.flush();
    m.commands();
    assert!(m.voices.iter().all(Option::is_some));
    // Fill the ring directly to reproduce the full mixer/full queue boundary.
    for sequence in 100..100 + CAPACITY as u64 {
        assert!(p
            .commands
            .push(Packet {
                sequence,
                command: Command::Start {
                    id: sequence,
                    pcm: Samples::of(&f(&pcm)),
                    rate: 48000,
                    looping: true,
                    offset: 0,
                    pitch: 1.,
                }
            })
            .is_ok());
    }
    m.discard();
    assert!(m.voices.iter().all(Option::is_none));
    assert!(m.commands.pop().is_none());
    assert_eq!(m.frame(), (0., 0.));
    let mut last = 0;
    while let Some(n) = p.acknowledgements.pop() {
        last = n;
    }
    assert_eq!(last, 99 + CAPACITY as u64);
    assert!(p.retained.contains_key(&(pcm.as_ptr() as usize)));
}

#[test]
fn malformed_pcm_refuses_without_mutating_pending_ownership() {
    let invalid = [
        Pcm::F32(Arc::from([])),
        Pcm::I16 {
            samples: Arc::from([7i16]),
            channels: 0,
        },
        Pcm::I16 {
            samples: Arc::from([7i16; 3]),
            channels: 3,
        },
        Pcm::I16 {
            samples: Arc::from([]),
            channels: 1,
        },
        Pcm::I16 {
            samples: Arc::from([]),
            channels: 2,
        },
        Pcm::I16 {
            samples: Arc::from([7i16]),
            channels: 2,
        },
    ];
    let (mut pending, mut mixer) = Pending::new();
    for pcm in &invalid {
        assert!(!pending.start(7, pcm, 48000, true, 0, 1.));
        assert!(pending.live.is_empty());
        assert!(pending.retained.is_empty());
        assert!(pending.controls.is_empty());
        assert!(mixer.commands.pop().is_none());
    }
    let valid = Pcm::F32(Arc::from([0.25f32]));
    assert!(pending.start(7, &valid, 48000, true, 0, 1.));
    pending.set(7, 1., 1.);
    pending.flush();
    let live = pending.live.clone();
    let sent = pending.sent.clone();
    let sequence = pending.sequence;
    for pcm in &invalid {
        assert!(!pending.start(7, pcm, 48000, true, 0, 1.));
        assert_eq!(pending.live, live);
        assert_eq!(pending.sent, sent);
        assert_eq!(pending.sequence, sequence);
        assert_eq!(pending.retained.len(), 1);
        assert!(pending.retained.contains_key(&valid.address()));
        assert!(pending.controls.is_empty() && pending.stopping.is_empty());
    }
    mixer.commands();
    for _ in 0..500 {
        mixer.frame();
    }
    assert_eq!(mixer.frame(), (0.25, 0.25));
}
