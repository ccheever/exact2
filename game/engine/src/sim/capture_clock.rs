use super::*;

// Presentation scheduling belongs to the development capture, never EXSIM.
// Fixed-size state: no history, allocation or tick work is triggered by sampling it.
#[derive(Clone, Default, Data)]
pub(crate) struct CaptureClock {
    world_us: i64,
    last_us: Option<i64>,
    last_ms: Option<f64>,
    period_ms: f64,
    paused: bool,
    lookahead: i64,
    live: Option<CapturedLive>,
}
#[derive(Clone, Default, Data)]
struct CapturedLive {
    phase: [u64; 2],
    remainder: f64,
    period_ms: f64,
    slew_left: Option<f64>,
    lookahead: f64,
}
impl<G: Game> Sim<G> {
    pub(crate) fn last_ms_for_capture(&self) -> f64 {
        self.last_ms.unwrap_or(0.0)
    }
    pub(crate) fn capture_clock(&self) -> CaptureClock {
        CaptureClock {
            world_us: self.world_us,
            last_us: self.last_us,
            last_ms: self.last_ms,
            period_ms: self.period_ms,
            paused: self.paused_clock,
            lookahead: self.lookahead_us_hz as i64,
            live: self.live_time.map(|v| CapturedLive {
                phase: [v.phase as u64, (v.phase >> 64) as u64],
                remainder: v.remainder,
                period_ms: v.period_ms,
                slew_left: v.slew_left,
                lookahead: v.lookahead,
            }),
        }
    }
    pub(crate) fn restore_capture_clock(&mut self, clock: &CaptureClock) -> Result<(), DataError> {
        let invalid = || DataError::new("invalid captured scheduling clock");
        let at = clock.last_ms.ok_or_else(invalid)?;
        if !at.is_finite()
            || !(0.0..=86_400_000.0).contains(&at)
            || clock.last_us != Some(micros(at))
            || clock.world_us < 0
            || !clock.period_ms.is_finite()
            || !(0.0..=1000.0).contains(&clock.period_ms)
            || clock.lookahead < 0
            || clock.lookahead as i128 > G::HZ as i128 * 1_000_000
        {
            return Err(invalid());
        }
        let live = clock.live.as_ref().map(|v| LiveTime {
            phase: ((v.phase[1] as i128) << 64) | v.phase[0] as i128,
            remainder: v.remainder,
            period_ms: v.period_ms,
            slew_left: v.slew_left,
            lookahead: v.lookahead,
        });
        if live.is_some_and(|v| {
            v.phase < 0
                || v.phase / G::HZ as i128 > i64::MAX as i128
                || !v.remainder.is_finite()
                || v.remainder.abs() > 1.0
                || !v.period_ms.is_finite()
                || !(0.0..=1000.0).contains(&v.period_ms)
                || !v.lookahead.is_finite()
                || !(0.0..=1_000_000.0 * G::HZ as f64).contains(&v.lookahead)
                || v.slew_left
                    .is_some_and(|n| !n.is_finite() || n.abs() > 1_000_000.0 * G::HZ as f64)
        }) {
            return Err(invalid());
        }
        self.rebase(at, false).map_err(DataError::new)?;
        self.world_us = clock.world_us;
        self.last_us = clock.last_us;
        self.last_ms = clock.last_ms;
        self.period_ms = clock.period_ms;
        self.paused_clock = clock.paused;
        self.lookahead_us_hz = clock.lookahead as i128;
        self.live_time = live;
        Ok(())
    }
}
