//! Optional, fixed-storage recent hitch evidence. The recorder never writes files.

/// Maximum retained hitch records per producer.
pub const CAPACITY: usize = 16;
/// Maximum age included in an exported report, without an expiration timer.
pub const MAX_AGE_NS: u64 = 120_000_000_000;
/// A callback gap exceeded 1.5 times its stable scheduled interval.
pub const GAP: u64 = 1;
/// Synchronous callback work finished after the platform target.
pub const CALLBACK_DEADLINE: u64 = 2;
/// An observed asynchronous frame tick's apply finished after its own target.
pub const APPLY_DEADLINE: u64 = 4;
/// The next callback was observed in this capture segment.
pub const AFTER_KNOWN: u64 = 8;
/// An apply context was observed before this hitch in this segment.
pub const WORK_KNOWN: u64 = 16;
/// The work context is the largest observed apply in this callback interval.
pub const INTERVAL_MAXIMUM: u64 = 32;

/// One observed apply, with optional owner timing from the same asynchronous tick.
/// Zero owner/target fields mean unavailable, not zero queue or execution time.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Work {
    /// Capture incarnation that submitted the work.
    pub capture: u64,
    /// Main-thread apply start, monotonic nanoseconds.
    pub began_ns: u64,
    /// Main-thread apply end.
    pub finished_ns: u64,
    /// This tick's platform-predicted target; zero for other applies.
    pub target_ns: u64,
    /// Owner submission time.
    pub queued_ns: u64,
    /// Owner execution start.
    pub owner_began_ns: u64,
    /// Owner execution end, including batch decoding.
    pub owner_finished_ns: u64,
    /// Main first began processing the returned batch.
    pub landed_ns: u64,
    /// Number of presenter operations in this batch.
    pub operations: u64,
}

/// A hitch and bounded adjacent evidence, all on one monotonic timebase.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Hitch {
    /// Sequence within this history; interpret with collector/capture identity.
    pub sequence: u64,
    /// Capture segment incarnation that observed this event.
    pub capture: u64,
    /// Core reload/clock generation that observed this event.
    pub generation: u64,
    /// Callback arrival, or apply end for an asynchronous deadline event.
    pub at_ns: u64,
    /// This callback/tick's platform target; never intended content presentation.
    pub target_ns: u64,
    /// Scheduled callback interval.
    pub period_ns: u64,
    /// Active host request at the event; zero means default/unknown.
    pub requested_period_ns: u64,
    /// Observed callback gap, zero without a comparable predecessor.
    pub gap_ns: u64,
    /// Synchronous callback duration; zero for an apply event.
    pub callback_ns: u64,
    /// Gap preceding this callback, when observed in the same segment.
    pub before_gap_ns: u64,
    /// Synchronous duration of the preceding callback.
    pub before_callback_ns: u64,
    /// First following callback's arrival.
    pub after_at_ns: u64,
    /// First following callback's gap.
    pub after_gap_ns: u64,
    /// First following callback's synchronous duration.
    pub after_callback_ns: u64,
    /// Evidence bits plus cached host workload/power flags.
    pub flags: u64,
    /// Most recently completed observed apply, not a claim of causation.
    pub work: Work,
}

/// Bounded history accounting copied only on an explicit read/control operation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    /// Most recent capture incarnation, including while paused.
    pub capture: u64,
    /// Whether detail capture is active.
    pub active: u64,
    /// Observed capture segments; gaps between them are not observed.
    pub segments: u64,
    /// First capture start after the last clear.
    pub started_ns: u64,
    /// Most recent capture start or stop.
    pub changed_ns: u64,
    /// Callback samples observed while detail capture was active.
    pub callbacks: u64,
    /// Total hitch events since the last clear.
    pub total: u64,
    /// Events overwritten because fixed capacity was exhausted.
    pub overwritten: u64,
    /// Retained records before age filtering at export.
    pub retained: u64,
    /// Asynchronous target misses in the observed capture segments only.
    pub apply_deadlines: u64,
}

#[derive(Default)]
pub(super) struct History {
    pub(super) status: Status,
    pub(super) generation: u64,
    pub(super) requested_period_ns: u64,
    entries: [Hitch; CAPACITY],
    next: usize,
    previous: u64,
    period: u64,
    gap: u64,
    callback: u64,
    work: Work,
    maximum_apply: Work,
    // At most two pending events receive post-context. Older pending events
    // remain explicitly without AFTER_KNOWN rather than adding per-frame scans.
    pending: [Option<usize>; 2],
}

impl History {
    pub(super) fn control(&mut self, action: u32, now: u64) {
        if action == 2 {
            let capture = self.status.capture;
            let requested = self.requested_period_ns;
            let generation = self.generation;
            let active = self.status.active != 0;
            *self = Self::default();
            self.status.capture = capture;
            self.generation = generation;
            self.requested_period_ns = requested;
            if active {
                self.control(1, now);
            }
            return;
        }
        let active = u64::from(action == 1);
        if self.status.active == active {
            return;
        }
        self.status.active = active;
        self.status.changed_ns = now;
        self.break_baseline();
        if active != 0 {
            self.status.capture = self.status.capture.saturating_add(1);
            if self.status.started_ns == 0 {
                self.status.started_ns = now;
            }
        }
    }

    pub(super) fn break_baseline(&mut self) {
        self.previous = 0;
        self.period = 0;
        self.gap = 0;
        self.callback = 0;
        self.work = Work::default();
        self.maximum_apply = Work::default();
        self.pending = [None; 2];
    }

    pub(super) fn record(
        &mut self,
        arrival: u64,
        target: u64,
        period: u64,
        finished: u64,
        flags: u64,
    ) {
        if self.status.active == 0 {
            return;
        }
        self.status.callbacks = self.status.callbacks.saturating_add(1);
        if self.previous == 0 {
            self.status.segments = self.status.segments.saturating_add(1);
        }
        let comparable = period != 0
            && self.period != 0
            && self.previous != 0
            && arrival > self.previous
            && self.period.abs_diff(period) <= 100_000;
        let gap = if comparable {
            arrival - self.previous
        } else {
            0
        };
        let duration = finished - arrival;
        for index in self.pending.iter_mut().filter_map(Option::take) {
            let event = &mut self.entries[index];
            event.after_at_ns = arrival;
            event.after_gap_ns = gap;
            event.after_callback_ns = duration;
            event.flags |= AFTER_KNOWN;
        }
        let reasons = (u64::from(
            comparable && self.period != 0 && gap > self.period.saturating_add(self.period / 2),
        ) * GAP)
            | (u64::from(finished > target) * CALLBACK_DEADLINE);
        if reasons != 0 {
            let maximum = self.maximum_apply.finished_ns != 0;
            let work = if maximum {
                self.maximum_apply
            } else if self.work.finished_ns != 0
                && arrival.saturating_sub(self.work.finished_ns) <= period.saturating_mul(2)
            {
                self.work
            } else {
                Work::default()
            };
            self.push(Hitch {
                at_ns: arrival,
                target_ns: target,
                period_ns: period,
                gap_ns: gap,
                callback_ns: duration,
                before_gap_ns: self.gap,
                before_callback_ns: self.callback,
                flags: reasons
                    | flags
                    | if work.finished_ns != 0 { WORK_KNOWN } else { 0 }
                    | if maximum { INTERVAL_MAXIMUM } else { 0 },
                work,
                ..Hitch::default()
            });
        }
        self.previous = arrival;
        self.period = period;
        self.gap = gap;
        self.callback = duration;
        self.maximum_apply.finished_ns = 0;
    }

    pub(super) fn context(&mut self, work: Work) -> bool {
        if self.status.active == 0
            || work.capture != self.status.capture
            || work.began_ns == 0
            || work.finished_ns < work.began_ns
        {
            return false;
        }
        self.work = work;
        if self.maximum_apply.finished_ns == 0
            || work.finished_ns - work.began_ns
                >= self.maximum_apply.finished_ns - self.maximum_apply.began_ns
        {
            self.maximum_apply = work;
        }
        true
    }

    pub(super) fn applied(&mut self, work: Work, flags: u64) {
        if !self.context(work) {
            return;
        }
        if work.target_ns != 0 && work.finished_ns > work.target_ns {
            self.status.apply_deadlines = self.status.apply_deadlines.saturating_add(1);
            self.push(Hitch {
                at_ns: work.finished_ns,
                target_ns: work.target_ns,
                period_ns: self.period,
                flags: APPLY_DEADLINE | WORK_KNOWN | flags,
                work,
                ..Hitch::default()
            });
        }
    }

    fn push(&mut self, mut event: Hitch) {
        self.status.total = self.status.total.saturating_add(1);
        event.sequence = self.status.total;
        event.capture = self.status.capture;
        event.generation = self.generation;
        event.requested_period_ns = self.requested_period_ns;
        if self.status.retained == CAPACITY as u64 {
            self.status.overwritten = self.status.overwritten.saturating_add(1);
        } else {
            self.status.retained += 1;
        }
        let index = self.next;
        self.entries[index] = event;
        self.next = (index + 1) % CAPACITY;
        self.pending = [self.pending[1].filter(|old| *old != index), Some(index)];
    }

    pub(super) fn get(&self, index: usize) -> Hitch {
        if index >= self.status.retained as usize {
            return Hitch::default();
        }
        let first = if self.status.retained == CAPACITY as u64 {
            self.next
        } else {
            0
        };
        self.entries[(first + index) % CAPACITY]
    }
}
