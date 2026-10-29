//! Bounded display-callback timing. These are scheduling proxies, never presentation evidence.

use std::cell::RefCell;
#[path = "frame_health/history.rs"]
mod history;
pub use history::{Hitch, Status as HistoryStatus, Work, CAPACITY as HITCH_CAPACITY, MAX_AGE_NS};

/// Scalar counters copied only when a reader asks, on the producer's thread.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    /// Collector generation, advanced by reset.
    pub generation: u64,
    /// Callbacks observed, including baseline callbacks.
    pub callbacks: u64,
    /// Comparable callback intervals.
    pub intervals: u64,
    /// Sum of comparable intervals; excludes stopped time and cadence transitions.
    pub active_ns: u64,
    /// Most recent platform-predicted interval.
    pub period_ns: u64,
    /// Largest comparable callback gap.
    pub max_gap_ns: u64,
    /// Synchronous callback work completed after its platform target.
    pub work_overruns: u64,
    /// Platform cadence changes, which interrupt the spacing baseline.
    pub cadence_changes: u64,
    /// Most recent callback arrival.
    pub last_arrival_ns: u64,
    /// Whether an existing app frame clock is running.
    pub running: u64,
    /// Completed outermost applies since reset.
    pub applies: u64,
    /// Applies with an applicable frame target.
    pub target_applies: u64,
    /// Applies completing after their own applicable target.
    pub apply_overruns: u64,
    /// Metric-specific finalized-through watermarks.
    pub interval_through_ns: u64,
    /// Last completed apply boundary.
    pub apply_through_ns: u64,
    /// Saturated histogram increments invalidate distribution deltas.
    pub histogram_overflows: u64,
}

/// Fixed duration bins: 48 half-millisecond bins, 15 doubling tails, and overflow.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Distribution {
    /// Saturating counts in half-open duration bins.
    pub bins: [u32; 64],
}

impl Default for Distribution {
    fn default() -> Self {
        Self { bins: [0; 64] }
    }
}

impl Distribution {
    fn record(&mut self, duration: u64) -> bool {
        let index = if duration < 24_000_000 {
            (duration / 500_000) as usize
        } else {
            (48 + (duration / 24_000_000).ilog2() as usize).min(63)
        };
        let bin = &mut self.bins[index];
        if *bin == u32::MAX {
            return false;
        }
        *bin += 1;
        true
    }
}

#[derive(Default)]
struct Collector {
    snapshot: Snapshot,
    previous: u64,
    history: history::History,
    intervals: Distribution,
    applies: Distribution,
}

impl Collector {
    fn record(&mut self, arrival: u64, target: u64, period: u64, finished: u64, flags: u64) {
        if arrival == 0 || finished < arrival || target == 0 || target < period {
            return;
        }
        let s = &mut self.snapshot;
        s.callbacks = s.callbacks.saturating_add(1);
        let changed =
            (s.period_ns != 0 || self.previous != 0) && s.period_ns.abs_diff(period) > 100_000;
        if changed {
            s.cadence_changes = s.cadence_changes.saturating_add(1);
        }
        if period != 0 && self.previous != 0 && arrival > self.previous && !changed {
            let gap = arrival - self.previous;
            s.intervals = s.intervals.saturating_add(1);
            s.active_ns = s.active_ns.saturating_add(gap);
            s.max_gap_ns = s.max_gap_ns.max(gap);
            s.interval_through_ns = arrival;
            s.histogram_overflows = s
                .histogram_overflows
                .saturating_add(u64::from(!self.intervals.record(gap)));
        }
        s.work_overruns = s.work_overruns.saturating_add(u64::from(finished > target));
        s.period_ns = period;
        s.last_arrival_ns = arrival;
        s.running = 1;
        self.history
            .record(arrival, target, period, finished, flags);
        self.previous = arrival;
    }

    fn stop(&mut self) {
        self.previous = 0;
        self.snapshot.running = 0;
        self.history.break_baseline();
    }
}

// A fixed table bounds both storage and lookup. Handles never reuse an old identity.
struct Registry {
    next: u32,
    slots: [Option<(u32, Collector)>; 16],
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            next: 1,
            slots: std::array::from_fn(|_| None),
        }
    }
}

thread_local! {
    static COLLECTORS: RefCell<Registry> = RefCell::new(Registry::default());
}

/// Register on the UI thread; zero means capacity exhausted. All calls use that thread.
pub fn create() -> u32 {
    COLLECTORS.with_borrow_mut(|r| {
        let Some(next) = r.next.checked_add(1) else {
            return 0;
        };
        let Some(slot) = r.slots.iter_mut().find(|s| s.is_none()) else {
            return 0;
        };
        let id = r.next;
        r.next = next;
        *slot = Some((id, Collector::default()));
        id
    })
}

fn with<T: Default>(id: u32, f: impl FnOnce(&mut Collector) -> T) -> T {
    COLLECTORS.with_borrow_mut(|r| {
        r.slots
            .iter_mut()
            .flatten()
            .find(|(key, _)| *key == id)
            .map(|(_, collector)| f(collector))
            .unwrap_or_default()
    })
}

/// Record one existing callback, without allocation, clock reads, I/O or locks.
pub fn record(id: u32, arrival: u64, target: u64, period: u64, finished: u64, flags: u64) {
    with(id, |c| c.record(arrival, target, period, finished, flags));
}

/// Start, pause or clear bounded detail capture (actions 1, 0 and 2).
pub fn capture(id: u32, action: u32, now: u64) {
    with(id, |c| c.history.control(action, now));
}

/// Read capture/loss accounting on the producer thread.
pub fn history_status(id: u32) -> HistoryStatus {
    with(id, |c| c.history.status)
}

/// Copy one retained hitch, oldest first. An out-of-range read returns zero sequence.
pub fn hitch(id: u32, index: u32) -> Hitch {
    with(id, |c| c.history.get(index as usize))
}

/// Record a completed main-thread apply, optionally correlated with its owner tick.
pub fn applied(id: u32, work: Work, flags: u64) {
    with(id, |c| {
        if work.began_ns == 0 || work.finished_ns < work.began_ns {
            return;
        }
        let s = &mut c.snapshot;
        s.applies = s.applies.saturating_add(1);
        s.target_applies = s
            .target_applies
            .saturating_add(u64::from(work.target_ns != 0));
        s.apply_overruns = s.apply_overruns.saturating_add(u64::from(
            work.target_ns != 0 && work.finished_ns > work.target_ns,
        ));
        s.apply_through_ns = work.finished_ns;
        s.histogram_overflows = s.histogram_overflows.saturating_add(u64::from(
            !c.applies.record(work.finished_ns - work.began_ns),
        ));
        c.history.applied(work, flags);
    });
}

/// Copy a histogram on the producer thread: 0 callback intervals, 1 apply durations.
pub fn distribution(id: u32, kind: u32) -> Distribution {
    with(id, |c| if kind == 0 { c.intervals } else { c.applies })
}

/// Retain temporal work context without adding another source's apply/deadline population.
pub fn context(id: u32, work: Work) {
    with(id, |c| {
        c.history.context(work);
    });
}

/// Cache a changed host request instead of querying platform policy in the hook.
pub fn requested_period(id: u32, period: u64) {
    with(id, |c| c.history.requested_period_ns = period);
}

/// Read counters; an unknown handle returns an empty snapshot.
pub fn snapshot(id: u32) -> Snapshot {
    with(id, |c| c.snapshot)
}

/// Break the interval baseline when the source stops, excluding idle time from gaps.
pub fn stop(id: u32) {
    with(id, Collector::stop);
}

/// Start a new generation after a reload or clock-domain change.
pub fn reset(id: u32) {
    with(id, |c| {
        let generation = c.snapshot.generation.saturating_add(1);
        c.stop();
        c.snapshot = Snapshot {
            generation,
            ..Snapshot::default()
        };
        c.history.generation = generation;
        c.history.status.active = 0;
        c.history.requested_period_ns = 0;
        c.intervals = Distribution::default();
        c.applies = Distribution::default();
    });
}

/// Release a registration on its producer thread.
pub fn destroy(id: u32) {
    COLLECTORS.with_borrow_mut(|r| {
        if let Some(slot) = r
            .slots
            .iter_mut()
            .find(|s| s.as_ref().is_some_and(|(key, _)| *key == id))
        {
            *slot = None;
        }
    });
}

/// C exports instantiated by the app's host macro.
#[macro_export]
macro_rules! frame_health_exports {
    () => {
        /// Create bounded UI-thread callback counters.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_create() -> u32 {
            $crate::frame_health::create()
        }
        /// Record timing from an existing app frame callback.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_record(
            id: u32,
            arrival: u64,
            target: u64,
            period: u64,
            finished: u64,
            flags: u64,
        ) {
            $crate::frame_health::record(id, arrival, target, period, finished, flags);
        }
        /// Control optional bounded detail capture.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_capture(id: u32, action: u32, now: u64) {
            $crate::frame_health::capture(id, action, now);
        }
        /// Read detail capture accounting.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_history_status(
            id: u32,
        ) -> $crate::frame_health::HistoryStatus {
            $crate::frame_health::history_status(id)
        }
        /// Copy one retained hitch, oldest first.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_hitch(
            id: u32,
            index: u32,
        ) -> $crate::frame_health::Hitch {
            $crate::frame_health::hitch(id, index)
        }
        /// Record one completed apply and its optional owner timeline.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_applied(
            id: u32,
            work: $crate::frame_health::Work,
            flags: u64,
        ) {
            $crate::frame_health::applied(id, work, flags);
        }
        /// Cache the active host rate request.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_requested_period(id: u32, period: u64) {
            $crate::frame_health::requested_period(id, period);
        }
        /// Forward context only; this does not count another apply or deadline event.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_context(id: u32, work: $crate::frame_health::Work) {
            $crate::frame_health::context(id, work);
        }
        /// Copy one fixed histogram on the producer thread.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_distribution(
            id: u32,
            kind: u32,
        ) -> $crate::frame_health::Distribution {
            $crate::frame_health::distribution(id, kind)
        }
        /// Copy scalar counters on demand.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_snapshot(id: u32) -> $crate::frame_health::Snapshot {
            $crate::frame_health::snapshot(id)
        }
        /// Stop the spacing baseline.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_stop(id: u32) {
            $crate::frame_health::stop(id);
        }
        /// Clear counters and advance generation.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_reset(id: u32) {
            $crate::frame_health::reset(id);
        }
        /// Release a collector.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_destroy(id: u32) {
            $crate::frame_health::destroy(id);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_gaps_exclude_idle_and_cadence_changes() {
        let mut c = Collector::default();
        c.record(1_000_000_000, 1_008_000_000, 8_000_000, 1_001_000_000, 0);
        c.record(1_008_000_000, 1_016_000_000, 8_000_000, 1_017_000_000, 0);
        c.stop();
        c.record(9_000_000_000, 9_008_000_000, 8_000_000, 9_001_000_000, 0);
        c.record(9_016_000_000, 9_032_000_000, 16_000_000, 9_017_000_000, 0);
        c.record(9_032_000_000, 9_048_000_000, 16_000_000, 9_033_000_000, 0);
        assert_eq!(c.snapshot.callbacks, 5);
        assert_eq!(c.snapshot.intervals, 2);
        assert_eq!(c.snapshot.active_ns, 24_000_000);
        assert_eq!(c.snapshot.max_gap_ns, 16_000_000);
        assert_eq!(c.snapshot.work_overruns, 1);
        assert_eq!(c.snapshot.cadence_changes, 1);
        assert!(std::mem::size_of::<Collector>() <= 4096);
    }

    #[test]
    fn capacity_lifetime_and_generation_are_bounded() {
        let ids: Vec<_> = (0..16).map(|_| create()).collect();
        assert!(ids.iter().all(|id| *id != 0));
        assert_eq!(create(), 0);
        record(ids[0], 100, 200, 100, 150, 0);
        reset(ids[0]);
        assert_eq!(snapshot(ids[0]).callbacks, 0);
        assert_eq!(snapshot(ids[0]).generation, 1);
        destroy(ids[0]);
        let replacement = create();
        assert_ne!(replacement, ids[0]);
        assert_eq!(snapshot(ids[0]).generation, 0);
        destroy(replacement);
        for id in ids.into_iter().skip(1) {
            destroy(id);
        }
    }

    #[test]
    fn distributions_have_explicit_finalization_reset_and_saturation() {
        let id = create();
        record(
            id,
            1_000_000_000,
            1_008_333_333,
            8_333_333,
            1_001_000_000,
            0,
        );
        assert_eq!(
            snapshot(id).interval_through_ns,
            0,
            "a baseline finalizes no interval"
        );
        record(
            id,
            1_008_333_333,
            1_016_666_666,
            8_333_333,
            1_009_000_000,
            0,
        );
        assert_eq!(distribution(id, 0).bins[16], 1);
        assert_eq!(snapshot(id).interval_through_ns, 1_008_333_333);
        applied(
            id,
            Work {
                began_ns: 1_010_000_000,
                finished_ns: 1_012_000_000,
                target_ns: 1_011_000_000,
                ..Work::default()
            },
            0,
        );
        assert_eq!(
            snapshot(id).applies,
            1,
            "core apply timing is independent of detail capture"
        );
        assert_eq!(snapshot(id).apply_overruns, 1);
        assert_eq!(snapshot(id).apply_through_ns, 1_012_000_000);
        assert_eq!(distribution(id, 1).bins[4], 1);
        stop(id);
        record(
            id,
            9_000_000_000,
            9_008_333_333,
            8_333_333,
            9_001_000_000,
            0,
        );
        assert_eq!(
            snapshot(id).intervals,
            1,
            "idle time contributes no interval"
        );
        reset(id);
        assert_eq!(distribution(id, 0).bins.iter().sum::<u32>(), 0);
        record(id, 10_000_000_000, 10_008_000_000, 0, 10_001_000_000, 0);
        record(id, 10_030_000_000, 10_038_000_000, 0, 10_031_000_000, 0);
        assert_eq!(
            distribution(id, 0).bins.iter().sum::<u32>(),
            0,
            "passive updates with unknown cadence do not define intervals"
        );
        assert_eq!(snapshot(id).intervals, 0);
        let mut histogram = Distribution::default();
        histogram.bins[0] = u32::MAX;
        assert!(!histogram.record(0));
        assert_eq!(histogram.bins[0], u32::MAX);
        assert!(histogram.record(u64::MAX));
        assert_eq!(histogram.bins[63], 1);
        destroy(id);
    }

    #[test]
    fn forwarded_apply_context_does_not_add_a_second_apply_or_deadline() {
        let id = create();
        capture(id, 1, 1_000_000_000);
        let work = Work {
            capture: history_status(id).capture,
            began_ns: 1_001_000_000,
            finished_ns: 1_003_000_000,
            target_ns: 1_002_000_000,
            operations: 10,
            ..Work::default()
        };
        context(id, work);
        assert_eq!(snapshot(id).applies, 0);
        assert_eq!(history_status(id).total, 0);
        record(id, 1_004_000_000, 1_005_000_000, 0, 1_006_000_000, 0);
        let event = hitch(id, 0);
        assert_eq!(event.flags & history::APPLY_DEADLINE, 0);
        assert_ne!(event.flags & history::CALLBACK_DEADLINE, 0);
        assert_ne!(event.flags & history::WORK_KNOWN, 0);
        assert_eq!(event.work.operations, 10);
        assert_eq!(snapshot(id).intervals, 0);
        assert_eq!(snapshot(id).max_gap_ns, 0);
        destroy(id);
    }

    #[test]
    #[ignore = "manual optimized microbenchmark; whole Apple hooks still need device qualification"]
    fn recorder_cost() {
        use std::hint::black_box;
        use std::time::Instant;

        let ids: Vec<_> = (0..16).map(|_| create()).collect();
        for (id, detail, storm) in [
            (ids[0], false, false),
            (ids[15], false, false),
            (ids[0], true, false),
            (ids[15], true, false),
            (ids[0], true, true),
            (ids[15], true, true),
        ] {
            if detail {
                capture(id, 1, 1);
            }
            let before = snapshot(id).callbacks;
            let mut samples = Vec::new();
            for batch in 0..31_u64 {
                let began = Instant::now();
                for i in 0..100_000 {
                    let arrival = black_box(
                        1_000_000_000
                            + (batch * 100_000 + i) * if storm { 24_999_999 } else { 8_333_333 },
                    );
                    record(
                        black_box(id),
                        arrival,
                        arrival + 8_333_333,
                        8_333_333,
                        arrival + 1_000,
                        0,
                    );
                }
                samples.push(began.elapsed().as_nanos() as f64 / 100_000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!("slot {}, detail {detail}, hitch storm {storm}: median {:.2} ns/record, batch maximum {:.2} ns/record; collector {} B, registry {} B", if id == ids[0] { 1 } else { 16 }, samples[15], samples[30], std::mem::size_of::<Collector>(), std::mem::size_of::<Registry>());
            assert_eq!(snapshot(id).callbacks - before, 3_100_000);
        }
        for id in ids {
            destroy(id);
        }
    }

    #[test]
    fn recent_hitches_have_bounded_storage_loss_and_adjacent_work_evidence() {
        let id = create();
        let start = 1_000_000_000;
        capture(id, 1, start);
        let incarnation = history_status(id).capture;
        for i in 0..40 {
            let at = start + i * 24_000_000;
            record(id, at, at + 8_000_000, 8_000_000, at + 1_000_000, 1 << 16);
        }
        let status = history_status(id);
        assert_eq!(status.total, 39);
        assert_eq!(status.retained, HITCH_CAPACITY as u64);
        assert_eq!(status.overwritten, 23);
        assert_eq!(hitch(id, 0).sequence, 24);
        let event = hitch(id, 14);
        assert_eq!(event.gap_ns, 24_000_000);
        assert_eq!(event.before_callback_ns, 1_000_000);
        assert_eq!(event.after_gap_ns, 24_000_000);
        assert_ne!(event.flags & history::AFTER_KNOWN, 0);
        let work = Work {
            capture: incarnation,
            began_ns: start + 960_000_000,
            finished_ns: start + 970_000_000,
            target_ns: start + 965_000_000,
            operations: 45,
            ..Work::default()
        };
        applied(id, work, 0);
        let last = hitch(id, 15);
        assert_ne!(last.flags & history::APPLY_DEADLINE, 0);
        assert_eq!(last.work.operations, 45);
        capture(id, 0, start + 1_000_000_000);
        record(
            id,
            start + 4_000_000_000,
            start + 4_008_000_000,
            8_000_000,
            start + 4_001_000_000,
            0,
        );
        assert_eq!(
            history_status(id).total,
            40,
            "paused capture records no new detail"
        );
        capture(id, 1, start + 5_000_000_000);
        applied(id, work, 0);
        assert_eq!(
            history_status(id).total,
            40,
            "old capture work cannot join the new segment"
        );
        record(
            id,
            start + 5_000_000_000,
            start + 5_008_000_000,
            8_000_000,
            start + 5_001_000_000,
            0,
        );
        assert_eq!(
            history_status(id).total,
            40,
            "resuming capture does not count paused time as a gap"
        );
        let old_event = hitch(id, 0);
        reset(id);
        assert_eq!(
            history_status(id).retained,
            16,
            "reload keeps recent evidence"
        );
        assert_eq!(hitch(id, 0).generation, old_event.generation);
        capture(id, 1, start + 6_000_000_000);
        let next = history_status(id).capture;
        assert!(next > incarnation);
        applied(
            id,
            Work {
                capture: next,
                ..work
            },
            0,
        );
        assert_eq!(hitch(id, 15).generation, 1);
        capture(id, 2, start + 6_000_000_000);
        assert_eq!(history_status(id).retained, 0);
        assert_eq!(history_status(id).active, 1);
        assert!(std::mem::size_of::<Collector>() <= 4096);
        assert!(std::mem::size_of::<Registry>() <= 256 * 1024);
        destroy(id);
    }

    #[test]
    fn a_short_followup_does_not_hide_the_long_apply_before_a_callback_gap() {
        let id = create();
        capture(id, 1, 1_000_000_000);
        let current = history_status(id).capture;
        record(
            id,
            1_000_000_000,
            1_008_000_000,
            8_000_000,
            1_001_000_000,
            0,
        );
        applied(
            id,
            Work {
                capture: current,
                began_ns: 1_001_000_000,
                finished_ns: 1_041_000_000,
                operations: 100,
                ..Work::default()
            },
            0,
        );
        applied(
            id,
            Work {
                capture: current,
                began_ns: 1_041_000_000,
                finished_ns: 1_042_000_000,
                operations: 1,
                ..Work::default()
            },
            0,
        );
        record(
            id,
            1_048_000_000,
            1_056_000_000,
            8_000_000,
            1_049_000_000,
            0,
        );
        let event = hitch(id, 0);
        assert_ne!(event.flags & history::INTERVAL_MAXIMUM, 0);
        assert_eq!(event.work.operations, 100);
        assert_eq!(event.work.finished_ns - event.work.began_ns, 40_000_000);
        record(
            id,
            1_072_000_000,
            1_080_000_000,
            8_000_000,
            1_073_000_000,
            0,
        );
        assert_eq!(
            hitch(id, 1).flags & history::WORK_KNOWN,
            0,
            "stale apply evidence is unavailable"
        );
        destroy(id);
    }
}
