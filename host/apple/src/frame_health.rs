//! Bounded display-callback timing. These are scheduling proxies, never presentation evidence.

use std::cell::RefCell;
#[path = "frame_health/history.rs"]
mod history;
pub use history::{Hitch, Status as HistoryStatus, Work, CAPACITY as HITCH_CAPACITY, MAX_AGE_NS};

/// Scalar counters copied only when a reader asks, on the producer's thread. A frame is a
/// callback that ran its work; LLP 1079.000 §4.6 defines each measure.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    /// Collector generation, advanced by reset.
    pub generation: u64,
    /// Callbacks observed, frames and held callbacks alike.
    pub callbacks: u64,
    /// Callbacks that started no frame work because earlier work was still in flight: not frames.
    pub held: u64,
    /// Intervals between consecutive frames' timestamps while the source ran.
    pub intervals: u64,
    /// Sum of those intervals.
    pub active_ns: u64,
    /// Most recent frame's scheduled period.
    pub period_ns: u64,
    /// Longest frame interval since reset.
    pub max_interval_ns: u64,
    /// Scheduled-period changes between callbacks within a run; the frame interval across one has
    /// no refresh count.
    pub cadence_changes: u64,
    /// Frame intervals whose refresh count differs from the previous interval's.
    pub pacing_changes: u64,
    /// Most recent callback arrival.
    pub last_arrival_ns: u64,
    /// Whether an existing app frame clock is running.
    pub running: u64,
    /// Frames whose work has finished, one record each.
    pub worked: u64,
    /// Those that finished after their deadline.
    pub late: u64,
    /// Arrival of the callback that finalized the latest interval.
    pub interval_through_ns: u64,
    /// Finish of the latest work record.
    pub work_through_ns: u64,
    /// Saturated histogram increments invalidate distribution deltas.
    pub histogram_overflows: u64,
    /// Requested slots that have passed while the host requested a rate, each counted once.
    pub requested_slots: u64,
    /// Those slots in which no frame started.
    pub missed_frames: u64,
    /// Frame intervals with an end scheduled slower than the rate requested for that end.
    pub restricted_intervals: u64,
    /// Scheduled slots frame intervals skipped: their refreshes beyond the first.
    pub skipped_refreshes: u64,
    /// Scheduled-period changes between callbacks, across stopped time too, so a reader knows
    /// when a window's refresh counts came from more than one schedule.
    pub schedule_epochs: u64,
    /// Frame intervals across a schedule change, which have no refresh count.
    pub unclassified_intervals: u64,
    /// Callbacks whose frame timestamp did not advance; each ends the run.
    pub out_of_order: u64,
}

/// A callback that ran all its work before returning.
pub const COMPLETE: u32 = 0;
/// A callback whose owner work lands later and reports its finish through `worked`.
pub const PENDING: u32 = 1;
/// A callback that started no frame work because earlier work was still in flight: not a frame.
pub const HELD: u32 = 2;
/// A callback with no frame work due, such as one kept for a timer: not a frame, and it ends the
/// run, so idle time never becomes a frame time.
pub const IDLE: u32 = 3;

/// Saturating counts in half-open bins; a reader receives the first `N` of 64.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Distribution {
    /// Counts in half-open bins; layouts with fewer bins leave the rest zero.
    pub bins: [u32; 64],
}

impl Default for Distribution {
    fn default() -> Self {
        Self { bins: [0; 64] }
    }
}

#[derive(Clone, Copy)]
struct Bins<const N: usize>([u32; N]);

impl<const N: usize> Default for Bins<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> Bins<N> {
    /// Count one value in `index`, returning false when the bin is saturated.
    fn add(&mut self, index: usize) -> bool {
        let bin = &mut self.0[index.min(N - 1)];
        if *bin == u32::MAX {
            return false;
        }
        *bin += 1;
        true
    }

    /// Bins of 0.5 ms up to `linear` (N/2 of them), then doubling tails and an open last bin.
    fn duration(&mut self, duration: u64, linear: u64) -> bool {
        self.add(if duration < linear {
            (duration / 500_000) as usize
        } else {
            (linear / 500_000) as usize + (duration / linear).ilog2() as usize
        })
    }

    fn copy(&self) -> Distribution {
        let mut out = Distribution::default();
        out.bins[..N].copy_from_slice(&self.0);
        out
    }
}

/// The longest frame interval and the slowest work, with its budget (deadline minus start),
/// since the card's reader last took them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Maxima {
    /// Longest interval between frame timestamps.
    pub longest_ns: u64,
    /// Slowest frame's work, from its start to its finish.
    pub slowest_ns: u64,
    /// That frame's deadline minus its start.
    pub budget_ns: u64,
}

#[derive(Default)]
struct Collector {
    snapshot: Snapshot,
    /// The previous frame's timestamp in this run, zero before the first.
    previous_frame: u64,
    /// The missed-frame grid's origin; zero re-anchors it at the next callback.
    origin: u64,
    /// The first slot on that grid not yet counted.
    next_slot: u64,
    /// One past a held callback's slot, which stays open until a frame starts in it or the run ends.
    held_slot: u64,
    /// The previous interval's refresh count, zero when it had none.
    previous_refreshes: u64,
    /// The previous frame's scheduled period, and the period requested when it started.
    frame_period: u64,
    frame_requested: u64,
    /// The previous callback's scheduled period in this run, zero at its start.
    run_period: u64,
    /// The schedule changed since the previous frame.
    changed: bool,
    /// The previous callback's scheduled period in any run.
    epoch_period: u64,
    /// The previous callback's frame timestamp.
    last_timestamp: u64,
    maxima: Maxima,
    history: history::History,
    /// Frame intervals: 0.5 ms bins below 24 ms, then doubling.
    intervals: Bins<64>,
    /// Work from each frame's start, in the same bins.
    work: Bins<64>,
    /// Refreshes each frame interval held, 1 to 31 and over.
    refreshes: Bins<32>,
    /// Finish past a late frame's deadline: 0.5 ms bins below 12 ms, then doubling.
    late_by: Bins<32>,
}

impl Collector {
    fn record(
        &mut self,
        arrival: u64,
        target: u64,
        period: u64,
        finished: u64,
        outcome: u32,
        flags: u64,
    ) -> bool {
        if arrival == 0 || finished < arrival || target == 0 || target < period {
            return false;
        }
        let s = &mut self.snapshot;
        s.callbacks = s.callbacks.saturating_add(1);
        s.last_arrival_ns = arrival;
        s.running = 1;
        self.history
            .record(arrival, target, period, finished, flags);
        s.held = s.held.saturating_add(u64::from(outcome == HELD));
        // Schedule changes on every callback, held ones too: an interval spanning one has no
        // single period
        if period != 0 {
            // A timestamp that does not advance bounds no interval: it ends the run, and the next
            // callback starts another
            let timestamp = target - period;
            let backward = self.last_timestamp != 0 && timestamp <= self.last_timestamp;
            self.last_timestamp = timestamp;
            if backward {
                s.out_of_order = s.out_of_order.saturating_add(1);
                self.end_run();
                return false;
            }
            let s = &mut self.snapshot;
            let differs = |previous: u64| previous != 0 && previous.abs_diff(period) > 100_000;
            s.schedule_epochs = s
                .schedule_epochs
                .saturating_add(u64::from(differs(self.epoch_period)));
            if differs(self.run_period) {
                s.cadence_changes = s.cadence_changes.saturating_add(1);
                self.changed = true;
            }
            (self.epoch_period, self.run_period, s.period_ns) = (period, period, period);
        }
        if outcome == IDLE {
            self.end_run();
            return true;
        }
        // A source without a schedule (the native-scroll observer) has work but no frame times
        let start = if period == 0 {
            arrival
        } else {
            let frame = target - period;
            self.slot(frame, outcome == HELD);
            if outcome != HELD {
                self.frame(arrival, frame, period);
            }
            frame
        };
        if outcome == COMPLETE {
            self.worked(start, target, finished);
        }
        true
    }

    /// Count the requested slots this callback's timestamp has passed. Each callback takes its
    /// nearest slot, so timestamp jitter cancels, and a slot counts once however many frames start in
    /// it. Slots no frame started in are missed; a held callback's own slot stays open, since a
    /// later callback may still start a frame in it.
    fn slot(&mut self, at: u64, held: bool) {
        let requested = self.history.requested_period_ns;
        if requested == 0 {
            return;
        }
        if self.origin == 0 {
            (self.origin, self.next_slot, self.held_slot) = (at, 0, 0);
        }
        let slot = at.saturating_sub(self.origin).saturating_add(requested / 2) / requested;
        let s = &mut self.snapshot;
        let passed = slot.saturating_sub(self.next_slot);
        if held {
            s.requested_slots = s.requested_slots.saturating_add(passed);
            s.missed_frames = s.missed_frames.saturating_add(passed);
            self.next_slot = self.next_slot.max(slot);
            self.held_slot = slot.saturating_add(1);
        } else if slot >= self.next_slot {
            s.requested_slots = s.requested_slots.saturating_add(passed.saturating_add(1));
            s.missed_frames = s.missed_frames.saturating_add(passed);
            self.next_slot = slot.saturating_add(1);
            self.held_slot = 0;
        }
    }

    /// Count the interval ending at this frame's timestamp.
    fn frame(&mut self, arrival: u64, frame: u64, period: u64) {
        let s = &mut self.snapshot;
        let changed = std::mem::take(&mut self.changed);
        let previous = self.previous_frame;
        if previous != 0 && frame > previous {
            let interval = frame - previous;
            s.intervals = s.intervals.saturating_add(1);
            s.active_ns = s.active_ns.saturating_add(interval);
            s.max_interval_ns = s.max_interval_ns.max(interval);
            s.interval_through_ns = arrival;
            self.maxima.longest_ns = self.maxima.longest_ns.max(interval);
            let mut saturated = !self.intervals.duration(interval, 24_000_000);
            // Refreshes need one period; across a schedule change there is none
            if changed {
                s.unclassified_intervals = s.unclassified_intervals.saturating_add(1);
                self.previous_refreshes = 0;
            } else {
                let refreshes = (interval.saturating_add(period / 2) / period).max(1);
                s.skipped_refreshes = s.skipped_refreshes.saturating_add(refreshes - 1);
                s.pacing_changes = s.pacing_changes.saturating_add(u64::from(
                    self.previous_refreshes != 0 && refreshes != self.previous_refreshes,
                ));
                self.previous_refreshes = refreshes;
                saturated |= !self.refreshes.add(refreshes.min(32) as usize - 1);
            }
            // Each end of an interval ran on its own schedule under its own request, which a schedule
            // or request change between them makes differ: one end slower than its request restricts it
            let slower = |period: u64, requested: u64| {
                requested != 0 && period > requested.saturating_add(100_000)
            };
            s.restricted_intervals = s.restricted_intervals.saturating_add(u64::from(
                slower(self.frame_period, self.frame_requested)
                    || slower(period, self.history.requested_period_ns),
            ));
            s.histogram_overflows = s.histogram_overflows.saturating_add(u64::from(saturated));
        }
        self.frame_period = period;
        self.frame_requested = self.history.requested_period_ns;
        self.previous_frame = frame;
    }

    /// Record one frame's work, from its start to its finish, against its deadline.
    fn worked(&mut self, start: u64, deadline: u64, finished: u64) {
        if start == 0 || finished < start || deadline < start {
            return;
        }
        let s = &mut self.snapshot;
        let duration = finished - start;
        s.worked = s.worked.saturating_add(1);
        s.work_through_ns = finished;
        let mut saturated = !self.work.duration(duration, 24_000_000);
        if finished > deadline {
            s.late = s.late.saturating_add(1);
            saturated |= !self.late_by.duration(finished - deadline, 12_000_000);
        }
        s.histogram_overflows = s.histogram_overflows.saturating_add(u64::from(saturated));
        if duration > self.maxima.slowest_ns {
            self.maxima.slowest_ns = duration;
            self.maxima.budget_ns = deadline - start;
        }
    }

    /// Close the requested-slot grid. A held callback's slot is final now: no frame can start in it.
    fn close_grid(&mut self) {
        let open = self.held_slot.saturating_sub(self.next_slot);
        let s = &mut self.snapshot;
        s.requested_slots = s.requested_slots.saturating_add(open);
        s.missed_frames = s.missed_frames.saturating_add(open);
        self.origin = 0;
        self.held_slot = 0;
    }

    /// End the run.
    fn end_run(&mut self) {
        self.close_grid();
        self.previous_frame = 0;
        self.previous_refreshes = 0;
        self.run_period = 0;
        self.changed = false;
    }

    /// The session stopped wanting frames.
    fn stop(&mut self) {
        self.end_run();
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

/// Record one existing callback, without allocation, clock reads, I/O or locks. `outcome` is
/// `COMPLETE`, `PENDING`, `HELD` or `IDLE`. Returns false for an observation the collector rejected,
/// whose pending work the host must then drop.
pub fn record(
    id: u32,
    arrival: u64,
    target: u64,
    period: u64,
    finished: u64,
    outcome: u32,
    flags: u64,
) -> bool {
    with(id, |c| {
        c.record(arrival, target, period, finished, outcome, flags)
    })
}

/// Record the finish of a `PENDING` frame's work, once the owner work it sent has landed.
pub fn worked(id: u32, start: u64, deadline: u64, finished: u64) {
    with(id, |c| c.worked(start, deadline, finished));
}

/// Return and reset the per-read maxima. Only the card's sampler takes them.
pub fn take_maxima(id: u32) -> Maxima {
    with(id, |c| std::mem::take(&mut c.maxima))
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

/// Retain a completed main-thread apply as hitch evidence; frame work is recorded per frame.
pub fn applied(id: u32, work: Work, flags: u64) {
    with(id, |c| c.history.applied(work, flags));
}

/// Copy a histogram on the producer thread: 0 frame intervals, 1 work, 2 refreshes and 3 lateness.
pub fn distribution(id: u32, kind: u32) -> Distribution {
    with(id, |c| match kind {
        0 => c.intervals.copy(),
        1 => c.work.copy(),
        2 => c.refreshes.copy(),
        _ => c.late_by.copy(),
    })
}

/// Retain temporal work context without adding another source's apply/deadline population.
pub fn context(id: u32, work: Work) {
    with(id, |c| {
        c.history.context(work);
    });
}

/// Cache a changed host request instead of querying platform policy in the hook. A new request
/// re-anchors the missed-frame grid at the next callback; the interval across it counts no slot.
pub fn requested_period(id: u32, period: u64) {
    with(id, |c| {
        if c.history.requested_period_ns != period {
            // Slots already passed on the old grid stay counted
            c.close_grid();
            c.history.requested_period_ns = period;
        }
    });
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
        // A new generation may bring a new clock domain
        c.last_timestamp = 0;
        c.epoch_period = 0;
        c.maxima = Maxima::default();
        c.intervals = Bins::default();
        c.work = Bins::default();
        c.refreshes = Bins::default();
        c.late_by = Bins::default();
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
            outcome: u32,
            flags: u64,
        ) -> u32 {
            u32::from($crate::frame_health::record(
                id, arrival, target, period, finished, outcome, flags,
            ))
        }
        /// Record the finish of a pending frame's work.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_worked(
            id: u32,
            start: u64,
            deadline: u64,
            finished: u64,
        ) {
            $crate::frame_health::worked(id, start, deadline, finished);
        }
        /// Return and reset the per-read maxima; only the card's sampler calls this.
        #[no_mangle]
        pub extern "C" fn exact_frame_health_take_maxima(id: u32) -> $crate::frame_health::Maxima {
            $crate::frame_health::take_maxima(id)
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
        /// Retain one completed apply and its optional owner timeline as hitch evidence.
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

    const P: u64 = 8_333_333;

    /// A callback for the frame starting at `frame`, arriving `late` after it.
    fn callback(c: &mut Collector, frame: u64, late: u64, period: u64, outcome: u32) {
        c.record(
            frame + late,
            frame + period,
            period,
            frame + late + 500_000,
            outcome,
            0,
        );
    }

    #[test]
    fn frame_intervals_exclude_idle_held_callbacks_and_nothing_else() {
        let mut c = Collector::default();
        callback(&mut c, 1_000_000_000, 0, P, COMPLETE);
        // Arrival jitter does not move a frame's timestamp
        callback(&mut c, 1_000_000_000 + P, 6_000_000, P, COMPLETE);
        // A held callback is not a frame: the next interval spans it
        callback(&mut c, 1_000_000_000 + 2 * P, 0, P, HELD);
        callback(&mut c, 1_000_000_000 + 3 * P, 0, P, COMPLETE);
        // A callback with no frame work due is not a frame either
        callback(&mut c, 1_000_000_000 + 4 * P, 0, P, IDLE);
        c.stop();
        callback(&mut c, 9_000_000_000, 0, P, COMPLETE);
        // A schedule change keeps the interval: its time is well defined
        callback(&mut c, 9_000_000_000 + 2 * P, 0, 2 * P, COMPLETE);
        let s = c.snapshot;
        assert_eq!((s.callbacks, s.held, s.worked), (7, 1, 5));
        assert_eq!(s.intervals, 3);
        assert_eq!(s.active_ns, 5 * P);
        assert_eq!(s.max_interval_ns, 2 * P);
        assert_eq!(
            s.cadence_changes, 1,
            "the change across stopped time is not counted"
        );
        assert_eq!(c.intervals.0[16] + c.intervals.0[33], 3);
        assert_eq!(
            c.refreshes.0[..2],
            [1, 1],
            "the interval across the change counts no refreshes"
        );
    }

    #[test]
    fn missed_frames_are_requested_slots_without_a_frame() {
        let mut c = Collector::default();
        let base = 1_000_000_000;
        callback(&mut c, base, 0, P, COMPLETE);
        callback(&mut c, base + P, 0, P, COMPLETE);
        assert_eq!(c.snapshot.requested_slots, 0, "no rate requested");
        c.history.requested_period_ns = P;
        // A skipped frame, then a held callback, then two skipped
        for (n, outcome) in [
            (2, COMPLETE),
            (4, COMPLETE),
            (5, HELD),
            (6, COMPLETE),
            (9, COMPLETE),
        ] {
            callback(&mut c, base + n * P, 0, P, outcome);
        }
        assert_eq!(
            (c.snapshot.requested_slots, c.snapshot.missed_frames),
            (8, 4)
        );
        assert_eq!(c.snapshot.restricted_intervals, 0);
        // An 80 Hz schedule under the 120 FPS request misses 40 a second, though every frame is
        // even at its own rate; nearest slots keep timestamp jitter from adding any
        let p80 = 12_500_000;
        let start = base + 9 * P;
        for n in 1..=80 {
            let jitter = if n % 2 == 0 { 30_000 } else { 0 };
            callback(&mut c, start + n * p80 - jitter, 0, p80, COMPLETE);
        }
        assert_eq!(c.snapshot.missed_frames - 4, 40);
        assert_eq!(
            c.snapshot.restricted_intervals, 80,
            "each interval ends on the slower schedule"
        );
        assert_eq!(
            c.refreshes.0[0],
            2 + 79,
            "each 80 Hz frame held one of its own refreshes"
        );
    }

    #[test]
    fn each_end_of_an_interval_meets_its_own_request() {
        let mut c = Collector::default();
        let base = 1_000_000_000;
        // A 60 Hz frame under a 60 FPS request, then a 120 Hz frame under a 120 FPS request
        c.history.requested_period_ns = 2 * P;
        callback(&mut c, base, 0, 2 * P, COMPLETE);
        c.history.requested_period_ns = P;
        callback(&mut c, base + 2 * P, 0, P, COMPLETE);
        assert_eq!(
            c.snapshot.restricted_intervals, 0,
            "each schedule met its own request"
        );
        // A 120 Hz frame, then a 60 Hz one under the same 120 FPS request: its end is restricted
        callback(&mut c, base + 4 * P, 0, 2 * P, COMPLETE);
        assert_eq!(c.snapshot.restricted_intervals, 1);
    }

    #[test]
    fn a_new_request_or_a_stop_restarts_the_grid() {
        let id = create();
        let base = 1_000_000_000;
        requested_period(id, 2 * P);
        let call = |n: u64, period: u64| {
            record(
                id,
                base + n,
                base + n + period,
                period,
                base + n + 1,
                COMPLETE,
                0,
            )
        };
        call(0, 2 * P);
        call(2 * P, 2 * P);
        // The request rises to 120 FPS; the interval across the change counts nothing
        requested_period(id, P);
        call(5 * P, P);
        assert_eq!(
            (snapshot(id).requested_slots, snapshot(id).missed_frames),
            (3, 0)
        );
        call(6 * P, P);
        call(8 * P, P);
        assert_eq!(
            (snapshot(id).requested_slots, snapshot(id).missed_frames),
            (6, 1)
        );
        // A repeated request changes nothing; stopped time is not missed
        requested_period(id, P);
        call(9 * P, P);
        stop(id);
        call(1_000 * P, P);
        call(1_001 * P, P);
        assert_eq!(
            (snapshot(id).requested_slots, snapshot(id).missed_frames),
            (9, 1)
        );
        // Timestamps that go backwards count nothing and never panic
        call(10 * P, P);
        call(11 * P, P);
        assert_eq!(snapshot(id).missed_frames, 1);
        destroy(id);
    }

    #[test]
    fn held_callbacks_miss_their_slots_even_when_the_run_ends() {
        let mut c = Collector::default();
        c.history.requested_period_ns = P;
        let base = 1_000_000_000;
        // A frame whose work stays in flight, then two held callbacks, then the run ends
        callback(&mut c, base, 0, P, PENDING);
        callback(&mut c, base + P, 0, P, HELD);
        callback(&mut c, base + 2 * P, 0, P, HELD);
        assert_eq!(
            (c.snapshot.requested_slots, c.snapshot.missed_frames),
            (2, 1),
            "the newest held slot is open"
        );
        c.stop();
        assert_eq!(
            (c.snapshot.requested_slots, c.snapshot.missed_frames),
            (3, 2)
        );
        assert_eq!(c.snapshot.held, 2);
    }

    #[test]
    fn a_slot_counts_once_when_callbacks_outpace_the_request() {
        let mut c = Collector::default();
        // 60 FPS requested, 120 Hz callbacks: frames share slots, and none is missed
        c.history.requested_period_ns = 2 * P;
        for n in 0..5 {
            callback(&mut c, 1_000_000_000 + n * P, 0, P, COMPLETE);
        }
        assert_eq!(
            (c.snapshot.requested_slots, c.snapshot.missed_frames),
            (3, 0)
        );
        // A held callback in a slot a later frame fills misses nothing
        callback(&mut c, 1_000_000_000 + 5 * P, 0, P, HELD);
        callback(&mut c, 1_000_000_000 + 6 * P, 0, P, COMPLETE);
        c.stop();
        assert_eq!(
            (c.snapshot.requested_slots, c.snapshot.missed_frames),
            (4, 0)
        );
    }

    #[test]
    fn an_idle_spell_ends_the_run_and_a_held_callback_can_change_the_schedule() {
        let mut c = Collector::default();
        callback(&mut c, 1_000_000_000, 0, P, COMPLETE);
        // A timer keeps the link running with no frame work; the next frame starts a new run
        callback(&mut c, 1_000_000_000 + P, 0, P, IDLE);
        callback(&mut c, 2_000_000_000, 0, P, COMPLETE);
        assert_eq!(c.snapshot.intervals, 0, "idle time is never a frame time");
        assert_eq!(c.snapshot.running, 1);
        // A held callback on a 60 Hz schedule between two 120 Hz frames: the interval has no single period
        callback(&mut c, 2_000_000_000 + P, 0, 2 * P, HELD);
        callback(&mut c, 2_000_000_000 + 3 * P, 0, P, COMPLETE);
        assert_eq!(c.snapshot.intervals, 1);
        assert_eq!(c.refreshes.0.iter().sum::<u32>(), 0);
        assert_eq!(c.snapshot.cadence_changes, 2);
        // A schedule change across stopped time interrupts no interval but still separates schedules
        c.stop();
        callback(&mut c, 9_000_000_000, 0, 2 * P, COMPLETE);
        assert_eq!(c.snapshot.cadence_changes, 2);
        assert_eq!(c.snapshot.schedule_epochs, 3);
    }

    #[test]
    fn backward_time_ends_the_run_and_a_new_request_keeps_old_misses() {
        let mut c = Collector::default();
        // Timestamps 1.000, 1.008, 0.500, 1.016 s: no interval spans the step back
        for at in [1_000_000_000, 1_008_000_000, 500_000_000, 1_016_000_000] {
            c.record(at, at + 8_000_000, 8_000_000, at + 1_000_000, COMPLETE, 0);
        }
        assert_eq!(c.snapshot.out_of_order, 1);
        assert_eq!(c.snapshot.intervals, 1);
        assert_eq!(
            c.snapshot.max_interval_ns, 8_000_000,
            "no interval spans the step back"
        );
        let id = create();
        requested_period(id, P);
        let call = |n: u64, outcome| {
            record(
                id,
                2_000_000_000 + n * P,
                2_000_000_000 + (n + 1) * P,
                P,
                2_000_000_000 + n * P + 1,
                outcome,
                0,
            )
        };
        call(0, COMPLETE);
        call(1, HELD);
        // The request changes before a frame starts in the held slot: that slot was missed
        requested_period(id, 2 * P);
        assert_eq!(
            (snapshot(id).requested_slots, snapshot(id).missed_frames),
            (2, 1)
        );
        destroy(id);
    }

    #[test]
    fn an_interval_across_a_schedule_change_is_counted_unclassified() {
        let mut c = Collector::default();
        callback(&mut c, 1_000_000_000, 0, P, COMPLETE);
        callback(&mut c, 1_000_000_000 + P, 0, 2 * P, HELD);
        callback(&mut c, 1_000_000_000 + 3 * P, 0, P, COMPLETE);
        assert_eq!(
            (c.snapshot.intervals, c.snapshot.unclassified_intervals),
            (1, 1)
        );
    }

    #[test]
    fn extreme_observations_saturate_and_never_panic() {
        let mut c = Collector::default();
        c.history.requested_period_ns = u64::MAX - 1;
        for at in [1, u64::MAX / 2, u64::MAX - 2] {
            c.record(at, at.saturating_add(1), 1, at, COMPLETE, 0);
            c.record(at, at, at, at, HELD, 0);
        }
        c.worked(1, u64::MAX, u64::MAX);
        c.stop();
        assert!(c.snapshot.callbacks >= 5);
    }

    #[test]
    fn pacing_counts_refreshes_and_their_changes() {
        let mut c = Collector::default();
        let mut at = 1_000_000_000;
        // One, two, one, one, three refreshes: three changes
        for refreshes in [0, 1, 2, 1, 1, 3] {
            at += refreshes * P;
            callback(&mut c, at, 0, P, COMPLETE);
        }
        assert_eq!(c.refreshes.0[..3], [3, 1, 1]);
        assert_eq!(c.snapshot.pacing_changes, 3);
        // An interval over 31 refreshes falls in the open last bin
        callback(&mut c, at + 40 * P, 0, P, COMPLETE);
        assert_eq!(c.refreshes.0[31], 1);
        assert_eq!(
            c.snapshot.skipped_refreshes,
            1 + 2 + 39,
            "exact past the open bin"
        );
        // Intervals of 40 and 50 refreshes share the open bin but are a change
        callback(&mut c, at + 90 * P, 0, P, COMPLETE);
        assert_eq!(c.snapshot.pacing_changes, 5);
    }

    #[test]
    fn work_is_recorded_once_per_frame_against_its_deadline() {
        let id = create();
        let frame = 1_000_000_000;
        // A complete frame finishes when its callback returns
        record(
            id,
            frame + 100_000,
            frame + P,
            P,
            frame + 4_100_000,
            COMPLETE,
            0,
        );
        // A pending frame finishes when its owner work lands, here late by 1.7 ms
        record(
            id,
            frame + P + 100_000,
            frame + 2 * P,
            P,
            frame + P + 600_000,
            PENDING,
            0,
        );
        assert_eq!(snapshot(id).worked, 1);
        worked(id, frame + P, frame + 2 * P, frame + P + 10_000_000);
        // A held callback adds no work record
        record(
            id,
            frame + 2 * P + 100_000,
            frame + 3 * P,
            P,
            frame + 2 * P + 200_000,
            HELD,
            0,
        );
        let s = snapshot(id);
        assert_eq!((s.worked, s.late), (2, 1));
        assert_eq!(s.work_through_ns, frame + P + 10_000_000);
        let work = distribution(id, 1).bins;
        assert_eq!((work[8], work[20]), (1, 1));
        assert_eq!(distribution(id, 3).bins[3], 1, "late by 1.67 ms");
        // Applies are hitch evidence only
        applied(
            id,
            Work {
                began_ns: frame,
                finished_ns: frame + 50_000_000,
                target_ns: frame + P,
                ..Work::default()
            },
            0,
        );
        assert_eq!(snapshot(id).worked, 2);
        destroy(id);
    }

    #[test]
    fn maxima_are_exact_and_taken_only_by_their_reader() {
        let id = create();
        let frame = 1_000_000_000;
        record(id, frame, frame + P, P, frame + 3_000_000, COMPLETE, 0);
        record(
            id,
            frame + 3 * P,
            frame + 4 * P,
            P,
            frame + 3 * P + 9_000_000,
            COMPLETE,
            0,
        );
        // Snapshot reads leave them for the card's sampler
        let _ = snapshot(id);
        let maxima = take_maxima(id);
        assert_eq!(maxima.longest_ns, 3 * P);
        assert_eq!((maxima.slowest_ns, maxima.budget_ns), (9_000_000, P));
        assert_eq!(take_maxima(id).longest_ns, 0, "taking resets them");
        record(
            id,
            frame + 4 * P,
            frame + 5 * P,
            P,
            frame + 4 * P + 1_000_000,
            COMPLETE,
            0,
        );
        assert_eq!(take_maxima(id).longest_ns, P);
        destroy(id);
    }

    #[test]
    fn capacity_lifetime_and_generation_are_bounded() {
        let ids: Vec<_> = (0..16).map(|_| create()).collect();
        assert!(ids.iter().all(|id| *id != 0));
        assert_eq!(create(), 0);
        record(ids[0], 100, 200, 100, 150, COMPLETE, 0);
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
        assert_eq!(std::mem::size_of::<history::Hitch>(), 104);
        assert_eq!(std::mem::size_of::<Collector>(), 3040);
    }

    #[test]
    fn distributions_have_explicit_finalization_reset_and_saturation() {
        let id = create();
        record(
            id,
            1_000_000_000,
            1_000_000_000 + P,
            P,
            1_001_000_000,
            COMPLETE,
            0,
        );
        assert_eq!(
            snapshot(id).interval_through_ns,
            0,
            "a baseline finalizes no interval"
        );
        record(
            id,
            1_000_000_000 + P,
            1_000_000_000 + 2 * P,
            P,
            1_009_000_000,
            COMPLETE,
            0,
        );
        assert_eq!(distribution(id, 0).bins[16], 1);
        assert_eq!(snapshot(id).interval_through_ns, 1_000_000_000 + P);
        stop(id);
        record(
            id,
            9_000_000_000,
            9_000_000_000 + P,
            P,
            9_001_000_000,
            COMPLETE,
            0,
        );
        assert_eq!(
            snapshot(id).intervals,
            1,
            "idle time contributes no interval"
        );
        reset(id);
        assert_eq!(distribution(id, 0).bins.iter().sum::<u32>(), 0);
        assert_eq!(distribution(id, 1).bins.iter().sum::<u32>(), 0);
        // Passive updates with unknown cadence have work but no intervals
        record(
            id,
            10_000_000_000,
            10_008_000_000,
            0,
            10_001_000_000,
            COMPLETE,
            0,
        );
        record(
            id,
            10_030_000_000,
            10_038_000_000,
            0,
            10_039_000_000,
            COMPLETE,
            0,
        );
        assert_eq!(snapshot(id).intervals, 0);
        assert_eq!((snapshot(id).worked, snapshot(id).late), (2, 1));
        let mut bins = Bins::<64>::default();
        bins.0[0] = u32::MAX;
        assert!(!bins.duration(0, 24_000_000));
        assert_eq!(bins.0[0], u32::MAX);
        assert!(bins.duration(u64::MAX, 24_000_000));
        assert_eq!(bins.0[63], 1);
        let mut late = Bins::<32>::default();
        for duration in [
            11_999_999,
            12_000_000,
            1_535_999_999,
            1_536_000_000,
            u64::MAX,
        ] {
            late.duration(duration, 12_000_000);
        }
        assert_eq!(
            (late.0[23], late.0[24], late.0[30], late.0[31]),
            (1, 1, 1, 2)
        );
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
        assert_eq!(history_status(id).total, 0);
        record(
            id,
            1_004_000_000,
            1_005_000_000,
            0,
            1_006_000_000,
            COMPLETE,
            0,
        );
        let event = hitch(id, 0);
        assert_eq!(event.flags & history::APPLY_DEADLINE, 0);
        assert_ne!(event.flags & history::CALLBACK_DEADLINE, 0);
        assert_ne!(event.flags & history::WORK_KNOWN, 0);
        assert_eq!(event.work.operations, 10);
        assert_eq!(event.work.finished_us, -1_000);
        assert_eq!(event.flags & history::CLAMPED, 0);
        // A boundary over 35 minutes from its event does not fit, and the hitch says so
        context(
            id,
            Work {
                began_ns: 1,
                finished_ns: 2,
                ..work
            },
        );
        record(
            id,
            3_000_000_000_000,
            3_000_001_000_000,
            0,
            3_000_002_000_000,
            COMPLETE,
            0,
        );
        assert_ne!(hitch(id, 1).flags & history::CLAMPED, 0);
        assert_eq!(snapshot(id).intervals, 0);
        assert_eq!(snapshot(id).max_interval_ns, 0);
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
            requested_period(id, 8_333_333);
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
                        COMPLETE,
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
            record(
                id,
                at,
                at + 8_000_000,
                8_000_000,
                at + 1_000_000,
                COMPLETE,
                1 << 16,
            );
        }
        let status = history_status(id);
        assert_eq!(status.total, 39);
        assert_eq!(status.retained, HITCH_CAPACITY as u64);
        assert_eq!(status.overwritten, 23);
        assert_eq!(hitch(id, 0).sequence, 24);
        let event = hitch(id, 14);
        assert_eq!(event.gap_us, 24_000);
        assert_eq!(event.before_callback_us, 1_000);
        assert_eq!(event.after_gap_us, 24_000);
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
            COMPLETE,
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
            COMPLETE,
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
            COMPLETE,
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
            COMPLETE,
            0,
        );
        let event = hitch(id, 0);
        assert_ne!(event.flags & history::INTERVAL_MAXIMUM, 0);
        assert_eq!(event.work.operations, 100);
        assert_eq!(event.work.finished_us - event.work.began_us, 40_000);
        record(
            id,
            1_072_000_000,
            1_080_000_000,
            8_000_000,
            1_073_000_000,
            COMPLETE,
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
