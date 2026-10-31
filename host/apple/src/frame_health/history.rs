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
/// A time or identity did not fit its 32-bit field and was clamped.
pub const CLAMPED: u64 = 64;

/// One observed apply, with optional owner timing from the same asynchronous tick.
/// Zero owner/target fields mean unavailable, not zero queue or execution time.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Work {
    /// Capture ID at work submission.
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

/// An offset that was not observed.
pub const ABSENT: i32 = i32::MIN;

/// A retained apply: times are microsecond offsets from its hitch's `at_ns`, `ABSENT` when
/// unobserved, so a record stays small and exact within 35 minutes either way.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct HitchWork {
    pub capture: u32,
    pub began_us: i32,
    pub finished_us: i32,
    pub target_us: i32,
    pub queued_us: i32,
    pub owner_began_us: i32,
    pub owner_finished_us: i32,
    pub landed_us: i32,
    pub operations: u32,
}

/// A hitch and bounded adjacent evidence, all on one monotonic timebase. Times other than
/// `at_ns` are microseconds: offsets from `at_ns`, or durations of at most 71 minutes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Hitch {
    /// Callback arrival, or apply end for an asynchronous deadline event.
    pub at_ns: u64,
    /// Evidence bits plus cached host workload/power flags.
    pub flags: u64,
    /// Sequence within this history; interpret with collector/capture identity.
    pub sequence: u32,
    /// Capture ID for this event.
    pub capture: u32,
    /// Core reload/clock generation that observed this event.
    pub generation: u32,
    /// This callback/tick's platform target; never intended content presentation.
    pub target_us: i32,
    /// Scheduled callback interval.
    pub period_us: u32,
    /// Active host request at the event; zero means default/unknown.
    pub requested_period_us: u32,
    /// Observed callback gap, zero without a comparable predecessor.
    pub gap_us: u32,
    /// Synchronous callback duration; zero for an apply event.
    pub callback_us: u32,
    /// Gap preceding this callback, when observed in the same segment.
    pub before_gap_us: u32,
    /// Synchronous duration of the preceding callback.
    pub before_callback_us: u32,
    /// First following callback's arrival, `ABSENT` until observed.
    pub after_at_us: i32,
    /// First following callback's gap.
    pub after_gap_us: u32,
    /// First following callback's synchronous duration.
    pub after_callback_us: u32,
    /// Nearby apply timing; temporal proximity does not establish causation.
    pub work: HitchWork,
}

/// A hitch as observed, before it is stored.
#[derive(Default)]
struct Event {
    at_ns: u64,
    target_ns: u64,
    period_ns: u64,
    gap_ns: u64,
    callback_ns: u64,
    before_gap_ns: u64,
    before_callback_ns: u64,
    flags: u64,
    work: Work,
}

/// Packs a hitch's fields into 32 bits, noting whether any did not fit.
#[derive(Default)]
struct Packer {
    clamped: bool,
}

impl Packer {
    /// Round to the nearest microsecond.
    fn micros(&mut self, duration: u64) -> u32 {
        self.narrow(duration.saturating_add(500) / 1000)
    }

    /// The microseconds from `origin` to `value`, rounded to the nearest; zero means unobserved.
    fn offset(&mut self, value: u64, origin: u64) -> i32 {
        if value == 0 {
            return ABSENT;
        }
        let difference = i128::from(value) - i128::from(origin);
        let rounded = (difference + 500 * difference.signum()) / 1000;
        let fitted = rounded.clamp(i128::from(ABSENT) + 1, i128::from(i32::MAX));
        self.clamped |= fitted != rounded;
        fitted as i32
    }

    fn narrow(&mut self, value: u64) -> u32 {
        self.clamped |= value > u64::from(u32::MAX);
        value.min(u64::from(u32::MAX)) as u32
    }
}

/// Bounded history accounting copied only on an explicit read/control operation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    /// Latest capture ID, retained while paused.
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
    // Add following-callback context to at most two pending events. Leave older
    // events without AFTER_KNOWN to avoid scanning the history on each frame.
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
            let mut pack = Packer::default();
            event.after_at_us = pack.offset(arrival, event.at_ns);
            event.after_gap_us = pack.micros(gap);
            event.after_callback_us = pack.micros(duration);
            event.flags |= AFTER_KNOWN | if pack.clamped { CLAMPED } else { 0 };
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
            self.push(Event {
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
            self.push(Event {
                at_ns: work.finished_ns,
                target_ns: work.target_ns,
                period_ns: self.period,
                flags: APPLY_DEADLINE | WORK_KNOWN | flags,
                work,
                ..Event::default()
            });
        }
    }

    fn push(&mut self, event: Event) {
        self.status.total = self.status.total.saturating_add(1);
        let at = event.at_ns;
        let work = &event.work;
        let mut p = Packer::default();
        let mut event = Hitch {
            at_ns: at,
            flags: event.flags,
            sequence: p.narrow(self.status.total),
            capture: p.narrow(self.status.capture),
            generation: p.narrow(self.generation),
            target_us: p.offset(event.target_ns, at),
            period_us: p.micros(event.period_ns),
            requested_period_us: p.micros(self.requested_period_ns),
            gap_us: p.micros(event.gap_ns),
            callback_us: p.micros(event.callback_ns),
            before_gap_us: p.micros(event.before_gap_ns),
            before_callback_us: p.micros(event.before_callback_ns),
            after_at_us: ABSENT,
            after_gap_us: 0,
            after_callback_us: 0,
            work: HitchWork {
                capture: p.narrow(work.capture),
                began_us: p.offset(work.began_ns, at),
                finished_us: p.offset(work.finished_ns, at),
                target_us: p.offset(work.target_ns, at),
                queued_us: p.offset(work.queued_ns, at),
                owner_began_us: p.offset(work.owner_began_ns, at),
                owner_finished_us: p.offset(work.owner_finished_ns, at),
                landed_us: p.offset(work.landed_ns, at),
                operations: p.narrow(work.operations),
            },
        };
        if p.clamped {
            event.flags |= CLAMPED;
        }
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
