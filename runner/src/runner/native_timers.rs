//! Independent platform timer delivery; virtual clocks retain the ordinary scheduler.
use super::*;

/// An ordinary timer whose native callback owns its live firing.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeTimer {
    /// Plan row to deliver.
    pub index: u32,
    /// Arming generation; a cancelled callback cannot fire a replacement.
    pub generation: u64,
    /// Nominal next firing in the runner's clock domain.
    pub due_ms: f64,
    /// Authored interval, including a one-shot's initial delay.
    pub interval_ms: u32,
    /// Authored tolerance, passed unchanged to the platform.
    pub tolerance_ms: u32,
    /// Whether the timer fires once.
    pub once: bool,
}

impl<D: DataSource> Runner<D> {
    /// Give live ordinary millisecond timers to independent native timers.
    /// Frame tasks, admission waits and internal deadlines keep their own paths.
    pub fn use_native_timers(&mut self, enabled: bool) {
        self.clock.native_timers = enabled;
    }

    /// Whether independent native timer delivery is configured.
    pub fn uses_native_timers(&self) -> bool {
        self.clock.native_timers
    }

    pub(super) fn native_timer(&self, i: usize) -> bool {
        self.clock.native_timers
            && self.clock_live()
            && !self.plan.timers[i].frame
            && self.plan.timers[i].wait == 0
    }

    /// Current native timer instances. Repeats retain their arming generation.
    pub fn native_timers(&self) -> Vec<NativeTimer> {
        if self.poisoned || self.wake_stops_timers() {
            return Vec::new();
        }
        self.timers
            .iter()
            .enumerate()
            .filter(|(i, t)| self.native_timer(*i) && t.next_ms.is_finite())
            .map(|(i, t)| NativeTimer {
                index: i as u32,
                generation: t.generation,
                due_ms: t.next_ms,
                interval_ms: self.plan.timers[i].interval_ms,
                tolerance_ms: self.plan.timers[i].tolerance_ms,
                once: self.plan.timers[i].once,
            })
            .collect()
    }

    /// Deliver only the native timer whose callback ran. Other due native
    /// timers remain the platform's; inputs and frame callbacks do not pull them in.
    pub fn fire_native_timer(&mut self, index: u32, generation: u64, cutoff: f64) -> Advanced {
        let mut result = Advanced {
            receipts: Vec::new(),
            now_ms: self.now_ms,
            error: None,
            stopped_on_request: false,
        };
        let i = index as usize;
        if self.poisoned
            || self.wake_stops_timers()
            || self
                .timers
                .get(i)
                .is_none_or(|t| !t.next_ms.is_finite() || t.generation != generation)
            || !self.native_timer(i)
        {
            return result;
        }
        if !cutoff.is_finite() || cutoff > MAX_CLOCK_MS {
            result.error = Some(if cutoff.is_finite() {
                RunnerError::ClockOutOfRange
            } else {
                RunnerError::NonFiniteClock
            });
            return result;
        }
        let nominal = self.timers[i].next_ms;
        if cutoff < nominal {
            return result;
        }
        self.now_ms = self.now_ms.max(cutoff);
        match self.fire_timer(i, nominal, self.now_ms, cutoff, false) {
            Ok(receipt) => result.receipts.push(Timed {
                at_ms: self.now_ms,
                receipt,
            }),
            Err(e) => {
                let what = format!(
                    "native timer {} ({})",
                    i,
                    self.plan
                        .str(self.plan.action(self.plan.timers[i].action).name)
                );
                let failed = Err(e);
                self.log_outcome(&what, &failed, false);
                result.error = failed.err();
            }
        }
        result.now_ms = self.now_ms;
        result
    }
}

#[derive(Clone)]
pub(super) struct Timer {
    /// The next due time; infinite once a one-shot timer has fired, and
    /// while a gated task is idle (LLP 1092 D8); a frame task's next
    /// virtual frame (LLP 1073 D3), `virtual_frame(base, k)`.
    pub(super) next_ms: f64,
    /// Whether the task's gate holds (always, for `mount`).
    pub(super) armed: bool,
    pub(super) generation: u64,
    /// The key it was armed with (`key=`), as `for` compares keys.
    pub(super) key: Option<String>,
    /// A frame task's last presented frame (or mount) …
    pub(super) base: f64,
    /// … and which virtual frame after it is next.
    pub(super) k: u32,
}
