use super::*;
fn start(p: &mut Pending, id: u64, pcm: &Arc<[f32]>, rate: u32) {
    p.start(id, pcm, rate, true, 0, 1.0);
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
    p.start(7, &pcm, 2, true, 1, 1.0);
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
