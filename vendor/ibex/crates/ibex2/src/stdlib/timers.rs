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
}

/// The timer wheel for one runtime.
#[derive(Debug, Default)]
pub struct Timers {
    scheduled: BTreeMap<Key, Entry>,
    by_handle: HashMap<u64, Key>,
    next_handle: u64,
    next_sequence: u64,
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
        self.next_handle += 1;
        let handle = self.next_handle;
        let delay = delay.max(MIN_DELAY);
        self.schedule(handle, now, delay, repeating.then_some(delay));
        handle
    }

    fn schedule(&mut self, handle: u64, now: Millis, delay: Duration, interval: Option<Duration>) {
        self.next_sequence += 1;
        let deadline_micros = ((now * 1000.0) as u64).saturating_add(delay.as_micros() as u64);
        let key = Key {
            deadline_micros,
            sequence: self.next_sequence,
        };
        self.scheduled.insert(key, Entry { handle, interval });
        self.by_handle.insert(handle, key);
    }

    /// `clearTimeout` / `clearInterval`. Unknown handles are a no-op, as in a
    /// browser.
    pub fn clear(&mut self, handle: u64) {
        if let Some(key) = self.by_handle.remove(&handle) {
            self.scheduled.remove(&key);
        }
    }

    /// Take the next timer due at `now`, rescheduling it if it repeats.
    ///
    /// One at a time, because each fired timer is a separate task and the
    /// engine owes a microtask checkpoint between them.
    pub fn take_due(&mut self, now: Millis) -> Option<u64> {
        let now_micros = (now * 1000.0) as u64;
        let (&key, &entry) = self.scheduled.iter().next()?;
        if key.deadline_micros > now_micros {
            return None;
        }
        self.scheduled.remove(&key);
        self.by_handle.remove(&entry.handle);

        if let Some(interval) = entry.interval {
            // Rescheduled from NOW rather than from the missed deadline, so a
            // slow turn cannot leave an interval owing a burst of catch-up
            // firings — the behaviour browsers settled on for the same reason.
            self.schedule(entry.handle, now, interval, Some(interval));
        }
        Some(entry.handle)
    }

    /// When the next timer is due, for an embedder that wants to sleep rather
    /// than spin.
    pub fn next_deadline(&self) -> Option<Millis> {
        self.scheduled
            .keys()
            .next()
            .map(|key| key.deadline_micros as f64 / 1000.0)
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
    fn an_interval_reschedules_itself() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(10), true);
        assert_eq!(timers.take_due(10.0), Some(h));
        assert_eq!(timers.take_due(10.0), None, "not immediately due again");
        assert_eq!(timers.take_due(20.0), Some(h));
        assert_eq!(timers.take_due(30.0), Some(h));
        timers.clear(h);
        assert_eq!(timers.take_due(100.0), None);
    }

    /// A slow turn must not leave an interval owing a burst of catch-up
    /// firings — it reschedules from now, not from the deadline it missed.
    #[test]
    fn a_late_interval_does_not_fire_a_backlog() {
        let mut timers = Timers::new();
        let h = timers.set(0.0, Duration::from_millis(10), true);
        // 500ms late: fifty intervals' worth of missed deadlines.
        assert_eq!(timers.take_due(500.0), Some(h));
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
