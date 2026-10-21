//! Timers — pure, ungated (LLP 0059.000 §3.2).
//!
//! The wheel is Rust's; the callbacks stay in the engine. A JavaScript function
//! is not something that can cross the boundary — §1.1 admits primitives and
//! handles — so JavaScript keeps its closures in a map keyed by the integer
//! handle this module mints, and the pump asks which handle is due.
//!
//! That split is the same one `fetch` uses: Rust owns *when*, the engine owns
//! *what*.
//!
//! Time is a parameter rather than something this module reads, so ordering is
//! tested deterministically instead of by sleeping.
//!
//! Out of v1: `setImmediate`, `requestIdleCallback`.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

/// A monotonic instant, in milliseconds since the runtime started.
///
/// A plain number rather than `Instant` because it must cross the boundary and
/// be comparable with the frame clock's base (LLP 0059.000 §2).
pub type Millis = f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    /// Ordered first by deadline...
    deadline_micros: u64,
    /// ...then by insertion, which is what makes same-delay timers fire in the
    /// order they were set. The HTML spec requires that, and a naive heap keyed
    /// on the deadline alone gets it wrong whenever two deadlines tie.
    sequence: u64,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deadline_micros
            .cmp(&other.deadline_micros)
            .then(self.sequence.cmp(&other.sequence))
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    handle: u64,
    /// `Some` for `setInterval`, which reschedules after firing.
    interval: Option<Duration>,
    /// Occurrences of this interval delivered so far (HTML's nesting level).
    runs: u32,
}

/// The timer wheel for one runtime.
#[derive(Debug, Default)]
pub struct Timers {
    scheduled: BTreeMap<Key, Entry>,
    by_handle: HashMap<u64, Key>,
    /// Intervals whose occurrence has been taken for admission and not yet
    /// delivered (LLP 0071 D6): rescheduled by `delivered`, so one interval
    /// never has two occurrences queued, and dropped by `clear`.
    queued: HashMap<u64, (Duration, u32)>,
    next_handle: u64,
    next_sequence: u64,
}

/// HTML's nesting clamp, as an interval meets it (LLP 0071 D6): after its
/// fifth delivered occurrence, an interval repeats no sooner than this, so a
/// `setInterval(f, 0)` at a held clock runs five times and then waits.
pub const INTERVAL_FLOOR: Duration = Duration::from_millis(4);
/// Delivered occurrences before [`INTERVAL_FLOOR`] applies (HTML's "nesting
/// level greater than 5").
pub const INTERVAL_FLOOR_AFTER: u32 = 5;

/// Milliseconds as the wheel's integer microseconds (floor). Every comparison
/// the wheel makes is on this integer, so "due" and "how long until due"
/// agree (LLP 0071 D3).
pub fn micros(ms: Millis) -> u64 {
    (ms * 1000.0) as u64
}

/// The HTML spec's minimum for a nested timer. Applied unconditionally in v1:
/// tracking nesting level is a refinement, and clamping everything to 0 makes a
/// `setTimeout(f, 0)` loop starve the rest of the turn.
pub const MIN_DELAY: Duration = Duration::from_millis(0);

impl Timers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule, returning the handle JavaScript holds.
    ///
    /// Handles start at 1 so that 0 is never a valid timer — `clearTimeout(0)`
    /// and `clearTimeout(undefined)` are both no-ops in a browser, and a
    /// zero-valued handle would make one of them cancel a real timer.
    pub fn set(&mut self, now: Millis, delay: Duration, repeating: bool) -> u64 {
        self.set_micros(micros(now), delay, repeating)
    }

    /// [`Self::set`] at an integer-microsecond `now`.
    pub fn set_micros(&mut self, now_micros: u64, delay: Duration, repeating: bool) -> u64 {
        self.next_handle += 1;
        let handle = self.next_handle;
        let delay = delay.max(MIN_DELAY);
        self.schedule(handle, now_micros, delay, repeating.then_some(delay), 0);
        handle
    }

    fn schedule(
        &mut self,
        handle: u64,
        now_micros: u64,
        delay: Duration,
        interval: Option<Duration>,
        runs: u32,
    ) {
        self.next_sequence += 1;
        let deadline_micros =
            now_micros.saturating_add(u64::try_from(delay.as_micros()).unwrap_or(u64::MAX));
        let key = Key {
            deadline_micros,
            sequence: self.next_sequence,
        };
        self.scheduled.insert(
            key,
            Entry {
                handle,
                interval,
                runs,
            },
        );
        self.by_handle.insert(handle, key);
    }

    /// `clearTimeout` / `clearInterval`. Unknown handles are a no-op, as in a
    /// browser. An interval whose occurrence is already queued is not
    /// rescheduled when that occurrence is delivered.
    pub fn clear(&mut self, handle: u64) {
        if let Some(key) = self.by_handle.remove(&handle) {
            self.scheduled.remove(&key);
        }
        self.queued.remove(&handle);
    }

    /// Take the next timer due at `now`.
    ///
    /// One at a time, because each fired timer is a separate task and the
    /// engine owes a microtask checkpoint between them. An interval leaves
    /// the wheel until its occurrence is delivered ([`Self::delivered`]).
    pub fn take_due(&mut self, now: Millis) -> Option<u64> {
        self.take_due_micros(micros(now))
    }

    /// [`Self::take_due`] at an integer-microsecond `now`.
    pub fn take_due_micros(&mut self, now_micros: u64) -> Option<u64> {
        let (&key, &entry) = self.scheduled.iter().next()?;
        if key.deadline_micros > now_micros {
            return None;
        }
        self.scheduled.remove(&key);
        self.by_handle.remove(&entry.handle);
        if let Some(interval) = entry.interval {
            self.queued
                .insert(entry.handle, (interval, entry.runs.saturating_add(1)));
        }
        Some(entry.handle)
    }

    /// The occurrence of `handle` taken by [`Self::take_due`] is being
    /// delivered at `now_micros`: an interval not cleared since is
    /// rescheduled from now (LLP 0058.000.000 §8, rescheduling on delivery),
    /// rather than from the deadline it missed, so a slow turn cannot leave it
    /// owing a burst of catch-up firings — the behaviour browsers settled on
    /// for the same reason.
    pub fn delivered(&mut self, handle: u64, now_micros: u64) {
        if let Some((interval, runs)) = self.queued.remove(&handle) {
            let period = if runs >= INTERVAL_FLOOR_AFTER {
                interval.max(INTERVAL_FLOOR)
            } else {
                interval
            };
            self.schedule(handle, now_micros, period, Some(interval), runs);
        }
    }

    /// When the next timer is due, for an embedder that wants to sleep rather
    /// than spin.
    pub fn next_deadline(&self) -> Option<Millis> {
        self.next_deadline_micros()
            .map(|micros| micros as f64 / 1000.0)
    }

    /// [`Self::next_deadline`] in the wheel's integer microseconds.
    pub fn next_deadline_micros(&self) -> Option<u64> {
        self.scheduled.keys().next().map(|key| key.deadline_micros)
    }

    pub fn len(&self) -> usize {
        self.scheduled.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scheduled.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timer_is_not_due_before_its_deadline() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(10), false);
        assert_eq!(timers.take_due(0.0), None);
        assert_eq!(timers.take_due(9.9), None);
        assert_eq!(timers.take_due(10.0), Some(h));
        assert_eq!(timers.take_due(10.0), None, "fired once, not twice");
    }

    /// The HTML rule a deadline-only ordering gets wrong.
    #[test]
    fn same_delay_timers_fire_in_insertion_order() {
        let mut timers = Timers::new();
        let first = timers.set(0.0, Duration::from_millis(5), false);
        let second = timers.set(0.0, Duration::from_millis(5), false);
        let third = timers.set(0.0, Duration::from_millis(5), false);
        assert_eq!(timers.take_due(5.0), Some(first));
        assert_eq!(timers.take_due(5.0), Some(second));
        assert_eq!(timers.take_due(5.0), Some(third));
        assert_eq!(timers.take_due(5.0), None);
    }

    #[test]
    fn a_shorter_delay_set_later_still_fires_first() {
        let mut timers = Timers::new();
        let slow = timers.set(0.0, Duration::from_millis(50), false);
        let quick = timers.set(0.0, Duration::from_millis(5), false);
        assert_eq!(timers.take_due(50.0), Some(quick));
        assert_eq!(timers.take_due(50.0), Some(slow));
    }

    #[test]
    fn clear_cancels_and_unknown_handles_are_harmless() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(5), false);
        timers.clear(h);
        assert_eq!(timers.take_due(100.0), None);
        timers.clear(h);
        timers.clear(0);
        timers.clear(9999);
    }

    #[test]
    fn handles_are_never_zero() {
        let mut timers = Timers::new();
        for _ in 0..5 {
            assert_ne!(timers.set(0.0, Duration::from_millis(1), false), 0);
        }
    }

    #[test]
    fn an_interval_reschedules_itself_when_delivered() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(10), true);
        assert_eq!(timers.take_due(10.0), Some(h));
        assert_eq!(timers.take_due(10.0), None, "not immediately due again");
        assert_eq!(
            timers.take_due(100.0),
            None,
            "one occurrence queued, not two"
        );
        timers.delivered(h, micros(10.0));
        assert_eq!(timers.take_due(20.0), Some(h));
        timers.delivered(h, micros(20.0));
        assert_eq!(timers.take_due(30.0), Some(h));
        timers.clear(h);
        timers.delivered(h, micros(30.0));
        assert_eq!(
            timers.take_due(100.0),
            None,
            "a cleared interval is not rescheduled"
        );
    }

    /// LLP 0071 D6: a zero interval runs five times at one instant and then
    /// waits HTML's 4 ms, so a held clock settles.
    #[test]
    fn a_zero_interval_meets_the_nesting_clamp_after_five_runs() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(0), true);
        for run in 1..=5 {
            assert_eq!(
                timers.take_due(0.0),
                Some(h),
                "run {run} is at the interval given"
            );
            timers.delivered(h, 0);
        }
        assert_eq!(timers.take_due(3.999), None);
        assert_eq!(timers.take_due(4.0), Some(h));
    }

    /// A 1 ms interval keeps its period for five runs, then waits 4 ms.
    #[test]
    fn a_short_interval_meets_the_clamp_after_five_runs() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(1), true);
        let mut at = 1.0;
        for _ in 1..=5 {
            assert_eq!(timers.take_due(at), Some(h));
            timers.delivered(h, micros(at));
            at += 1.0;
        }
        assert_eq!(
            timers.next_deadline(),
            Some(9.0),
            "the sixth waits 4 ms after the fifth at 5"
        );
    }

    /// The run count saturates: only whether it reached five matters.
    #[test]
    fn the_run_count_saturates() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(0), true);
        let key = timers.by_handle[&h];
        timers.scheduled.get_mut(&key).unwrap().runs = u32::MAX;
        assert_eq!(timers.take_due(0.0), Some(h));
        assert_eq!(timers.queued[&h].1, u32::MAX);
        timers.delivered(h, 0);
        assert_eq!(timers.next_deadline(), Some(4.0), "still clamped");
    }

    /// A delay whose microseconds overflow a `u64` saturates rather than
    /// wrapping to a nearer deadline.
    #[test]
    fn a_huge_delay_saturates() {
        let mut timers = Timers::new();
        timers.set(1.0, Duration::from_secs(u64::MAX), false);
        assert_eq!(timers.next_deadline_micros(), Some(u64::MAX));
    }

    /// An occurrence taken at 25 and left queued until 60 repeats from 60.
    #[test]
    fn an_interval_repeats_from_its_delivery_not_its_admission() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(20), true);
        assert_eq!(timers.take_due(25.0), Some(h));
        timers.delivered(h, micros(60.0));
        assert_eq!(timers.next_deadline(), Some(80.0));
    }

    #[test]
    fn delivering_a_one_shot_or_an_unknown_handle_schedules_nothing() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(1), false);
        assert_eq!(timers.take_due(1.0), Some(h));
        timers.delivered(h, micros(1.0));
        timers.delivered(9999, micros(1.0));
        assert!(timers.is_empty());
    }

    /// A slow turn must not leave an interval owing a burst of catch-up
    /// firings — it reschedules from now, not from the deadline it missed.
    #[test]
    fn a_late_interval_does_not_fire_a_backlog() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(10), true);
        // 500ms late: fifty intervals' worth of missed deadlines.
        assert_eq!(timers.take_due(500.0), Some(h));
        timers.delivered(h, micros(500.0));
        assert_eq!(timers.take_due(500.0), None, "no backlog");
        assert_eq!(timers.take_due(510.0), Some(h));
    }

    #[test]
    fn a_negative_or_zero_delay_is_due_immediately_but_still_ordered() {
        let mut timers = Timers::new();
        let a = timers.set(0.0, Duration::from_millis(0), false);
        let b = timers.set(0.0, Duration::from_millis(0), false);
        assert_eq!(timers.take_due(0.0), Some(a));
        assert_eq!(timers.take_due(0.0), Some(b));
    }

    #[test]
    fn next_deadline_reports_the_earliest() {
        let mut timers = Timers::new();
        assert_eq!(timers.next_deadline(), None);
        timers.set(0.0, Duration::from_millis(50), false);
        timers.set(0.0, Duration::from_millis(5), false);
        assert_eq!(timers.next_deadline(), Some(5.0));
        assert_eq!(timers.len(), 2);
    }
}
