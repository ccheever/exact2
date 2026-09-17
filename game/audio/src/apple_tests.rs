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
