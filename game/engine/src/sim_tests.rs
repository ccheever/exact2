use super::*;
struct Ticker<const HZ: u32>;
impl<const HZ: u32> Game for Ticker<HZ> {
    const ID: &'static str = "alpha-guard";
    const HZ: u32 = HZ;
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn steady<const HZ: u32>() {
    for frame_hz in [60.0, 59.94, 120.0, 144.0, 240.0] {
        for epoch in [0.0, 1234.567, 1_000_000.123] {
            for phase in [0.0, 0.1, 0.25, 0.4] {
                let mut s = Sim::<Ticker<HZ>>::new(()).unwrap();
                s.frame_period(1000.0 / frame_hz);
                s.advance(epoch, Clock::Live);
                for frame in 1..=600 {
                    let now = epoch + frame as f64 * 1000.0 / frame_hz + phase * 1000.0 / HZ as f64;
                    let due = s.ticks_due(now, Clock::Live);
                    assert_eq!(s.advance(now, Clock::Live), due);
                    if HZ == 120 && frame_hz == 120.0 && phase == 0.0 {
                        assert_eq!(due, 1, "120/120 frame {frame}");
                    }
                    // Inspect the signed numerator BEFORE alpha's guard.
                    let raw = s.alpha_numerator();
                    assert!((0..=1_000_000).contains(&raw),
                        "world {HZ}, display {frame_hz}, epoch {epoch}, phase {phase}, frame {frame}: {raw}");
                    if s.world.tick() > 0 {
                        assert_eq!(s.alpha(), raw as f32 / 1_000_000.0);
                    }
                }
                // Seekable must never reuse the preceding live lookahead.
                s.advance(epoch + 602.0 * 1000.0 / frame_hz, Clock::Seekable);
                assert_eq!(s.lookahead_us_hz, 0);
                assert!(s.live_time.is_none());
            }
        }
    }
}
#[test]
fn alpha_guard_never_fires_on_600_steady_frames_at_both_world_rates() {
    steady::<60>();
    steady::<120>();
}
#[test]
fn integer_phase_survives_the_old_eighteen_hour_precision_limit() {
    let mut s = Sim::<Ticker<120>>::new(()).unwrap();
    // Just beyond 2^26 milliseconds, without running eight million ticks.
    let tick = 8_100_000;
    for _ in 0..tick {
        s.world.step_clock();
    }
    assert_eq!(s.world.tick(), tick);
    s.world_us = tick as i64 * 1_000_000 / 120;
    s.frame_period(1000.0 / 120.0);
    s.advance(0.0, Clock::Live);
    for frame in 1..=3600 {
        let now = frame as f64 * 1000.0 / 120.0;
        assert_eq!(s.ticks_due(now, Clock::Live), 1);
        assert_eq!(s.advance(now, Clock::Live), 1);
        let live = s.live_time.unwrap();
        assert!(live.remainder.abs() <= 0.5);
        assert_eq!(live.phase, (tick as i128 + frame as i128) * 1_000_000);
        assert_eq!(s.alpha(), 1.0);
    }
}
#[test]
fn live_restore_preserves_a_pending_future_stamp_on_the_epoch_sample() {
    let mut s = Sim::<Ticker<60>>::new(()).unwrap();
    s.advance(10.0, Clock::Live);
    s.queue.push_back(Queued {
        host_us: 12_000,
        ..Default::default()
    });
    let save = s.save().unwrap();
    s.restore(&save).unwrap();
    s.advance(100.0, Clock::Live);
    assert_eq!(s.queue.len(), 1);
    assert_eq!(s.queue[0].host_us, 102_000);
    assert!(s.queue[0].world_us.is_none());
}
#[test]
fn restore_refuses_a_one_tick_ahead_clock() {
    let mut s = Sim::<Ticker<60>>::new(()).unwrap();
    s.advance(0.0, Clock::Seekable);
    s.advance(17.0, Clock::Seekable);
    let good = s.save().unwrap();
    let mut saved: Saved = bin::from_slice(&good[7..]).unwrap();
    saved.world_us = 10_000;
    let mut bad = b"EXSIM\0\x05".to_vec();
    bad.extend(bin::to_vec(&saved));
    assert!(s
        .restore(&bad)
        .unwrap_err()
        .to_string()
        .contains("world and clock disagree"));
    assert_eq!(s.save().unwrap(), good);
}
