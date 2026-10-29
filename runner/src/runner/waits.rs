//! Timer admission and virtual execution times (LLP 1147).
use super::*;

/// Wait kinds a host implements or currently admits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WaitKinds(u8);
impl WaitKinds {
    /// No intentional waits.
    pub const NONE: Self = Self(0);
    /// Admit at a display callback.
    pub const FRAME: Self = Self(1 << 1);
    /// Join other frame work, or run without demand.
    pub const UPDATE: Self = Self(1 << 2);
    /// Wait for observed scrolling to end.
    pub const SCROLLEND: Self = Self(1 << 3);
    /// Admit one firing after sampling the idle condition.
    pub const IDLE: Self = Self(1 << 4);
    /// Every defined wait kind.
    pub const ALL: Self = Self(30);
    /// Combine observations before selecting by deadline and plan row.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    /// Decode the host ABI's bit mask, discarding undefined bits.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL.0)
    }
    fn contains(self, kind: u8) -> bool {
        kind != 0 && self.0 & (1 << kind) != 0
    }
}

/// One kind's next observation deadline and whether it has due work.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wait {
    /// An outstanding firing has reached its nominal deadline.
    pub held: bool,
    /// The earliest nominal deadline not reached yet.
    pub due_ms: Option<f64>,
}
/// Waiting work reported to the host independently of ordinary deadlines.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Waits {
    /// Display-frame waits.
    pub frame: Wait,
    /// Rendering-update waits.
    pub update: Wait,
    /// Scroll-completion waits.
    pub scrollend: Wait,
    /// Idle waits.
    pub idle: Wait,
}
/// One task's scheduling details for inspection.
#[derive(Clone, Debug)]
pub struct TaskTiming {
    /// Authored task name.
    pub name: String,
    /// Admission kind, zero for precise scheduling.
    pub wait: u8,
    /// Outstanding nominal deadline.
    pub due_ms: Option<f64>,
    /// The live host is holding this firing.
    pub held: bool,
    /// Absolute waiting cap, absent for an uncapped or spent firing.
    pub cap_ms: Option<f64>,
    /// Allowed native wake coalescing delay.
    pub tolerance_ms: u32,
}

/// A batch to apply before resampling admission conditions.
pub struct Admitted {
    /// Commits and any recoverable refusal.
    pub advanced: Advanced,
    /// Resume this turn with the same cutoff after applying its batch.
    pub continuing: bool,
}

impl<D: DataSource> Runner<D> {
    /// Declare supported waits before installing the live clock.
    pub fn hold_waits(&mut self, kinds: WaitKinds) {
        self.clock.waits = kinds;
    }

    /// Read wait observations; caps remain visible through `timer_due_ms`.
    pub fn waits(&self) -> Waits {
        let mut result = Waits::default();
        if !self.clock_live() || self.poisoned || self.wake_stops_timers() {
            return result;
        }
        for (i, t) in self.timers.iter().enumerate() {
            let kind = self.plan.timers[i].wait;
            if !t.next_ms.is_finite() || !self.clock.waits.contains(kind) {
                continue;
            }
            let wait = match kind {
                1 => &mut result.frame,
                2 => &mut result.update,
                3 => &mut result.scrollend,
                4 => &mut result.idle,
                _ => continue,
            };
            if t.next_ms <= self.now_ms {
                wait.held = true;
            } else {
                wait.due_ms = Some(wait.due_ms.map_or(t.next_ms, |old| old.min(t.next_ms)));
            }
        }
        result
    }

    /// Run ordinary work first, then one eligible firing per batch. A host
    /// applies each batch and resamples its observations before continuing.
    pub fn release_waits(&mut self, kinds: WaitKinds, cutoff: f64) -> Admitted {
        if self.poisoned {
            return Admitted {
                advanced: Advanced {
                    receipts: Vec::new(),
                    now_ms: self.now_ms,
                    error: Some(RunnerError::Poisoned),
                    stopped_on_request: false,
                },
                continuing: false,
            };
        }
        if !self.clock_live() || !cutoff.is_finite() || cutoff > MAX_CLOCK_MS {
            return Admitted {
                advanced: self.advance_timed(cutoff),
                continuing: false,
            };
        }
        let resumed = self.clock.admission == Some(cutoff) && !self.wake_stops_timers();
        if !resumed {
            let advanced = self.advance_timed(cutoff);
            if advanced.error.is_some() || !advanced.receipts.is_empty() {
                if advanced.error.is_none() {
                    self.clock.admission = Some(cutoff);
                }
                return Admitted {
                    advanced,
                    continuing: true,
                };
            }
        }
        self.clock.admission = Some(cutoff);
        let stopped = self.wake_stops_timers();
        let candidate = self
            .timers
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                !stopped
                    && t.next_ms <= cutoff
                    && self.clock.waits.contains(self.plan.timers[*i].wait)
                    && kinds.contains(self.plan.timers[*i].wait)
                    && (self.plan.timers[*i].wait != 4 || kinds == WaitKinds::IDLE)
            })
            .min_by(|(ia, a), (ib, b)| a.next_ms.total_cmp(&b.next_ms).then(ia.cmp(ib)))
            .map(|(i, t)| (i, t.next_ms));
        let mut advanced = Advanced {
            receipts: Vec::new(),
            now_ms: self.now_ms,
            error: None,
            stopped_on_request: false,
        };
        if let Some((i, nominal)) = candidate {
            self.now_ms = self.now_ms.max(cutoff);
            match self.fire_timer(i, nominal, self.now_ms, cutoff, true) {
                Ok(receipt) => advanced.receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Err(e) => advanced.error = Some(e),
            }
        }
        advanced.now_ms = self.now_ms;
        let continuing = candidate.is_some() && kinds != WaitKinds::IDLE && !self.poisoned;
        if !continuing {
            self.clock.admission = None;
        }
        Admitted {
            advanced,
            continuing,
        }
    }

    /// Scheduling details, including held state and absolute caps.
    pub fn task_timing(&self) -> Vec<TaskTiming> {
        self.plan
            .timers
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let due_ms = Some(self.timers[i].next_ms).filter(|at| at.is_finite());
                TaskTiming {
                    name: self.plan.str(row.name).to_string(),
                    wait: row.wait,
                    due_ms,
                    held: !self.poisoned
                        && self.clock_live()
                        && self.clock.waits.contains(row.wait)
                        && due_ms.is_some_and(|at| at <= self.now_ms),
                    cap_ms: due_ms
                        .filter(|_| {
                            !self.poisoned
                                && self.clock_live()
                                && self.clock.waits.contains(row.wait)
                        })
                        .and_then(|_| (row.wait != 0).then(|| self.timer_cap(i)))
                        .filter(|at| at.is_finite()),
                    tolerance_ms: row.tolerance_ms,
                }
            })
            .collect()
    }

    pub(super) fn timer_cap(&self, i: usize) -> f64 {
        let row = &self.plan.timers[i];
        let bound = match row.wait {
            1 | 2 => 100.0,
            3 if row.max_ms == 0 => f64::INFINITY,
            4 if row.max_ms == 0 => row.interval_ms.max(1000) as f64,
            _ => row.max_ms as f64,
        };
        self.timers[i].next_ms + bound
    }

    pub(super) fn timer_execution(&self, i: usize) -> f64 {
        let t = &self.timers[i];
        let row = &self.plan.timers[i];
        if !t.next_ms.is_finite() {
            return t.next_ms;
        }
        if self.clock_live() {
            if self.native_timer(i) {
                f64::INFINITY
            } else if self.clock.waits.contains(row.wait) {
                self.timer_cap(i)
            } else {
                t.next_ms
            }
        } else if let Some(at) = self.clock.takeover.filter(|at| t.next_ms <= *at) {
            at
        } else if row.wait == 1 {
            frame_at_or_after(t.next_ms)
        } else {
            t.next_ms
        }
    }

    pub(super) fn fire_timer(
        &mut self,
        i: usize,
        nominal: f64,
        execution: f64,
        cutoff: f64,
        admitted: bool,
    ) -> Result<CommitReceipt, RunnerError> {
        let row = &self.plan.timers[i];
        let (once, frame, interval, waiting) =
            (row.once, row.frame, row.interval_ms as f64, row.wait != 0);
        let live = self.clock_live();
        let coalesce = (live || waiting) && !once;
        let next_ms = if once {
            f64::INFINITY
        } else if coalesce && frame {
            self.timers[i].base = self.now_ms.max(cutoff);
            self.timers[i].k = 1;
            virtual_frame(self.timers[i].base, 1)
        } else if coalesce {
            advance::beat_after(nominal, interval, execution.max(cutoff))
        } else if frame {
            self.timers[i].k = self.timers[i].k.saturating_add(1);
            virtual_frame(self.timers[i].base, self.timers[i].k)
        } else {
            nominal + interval
        };
        if !once && (!next_ms.is_finite() || next_ms <= nominal) {
            return Err(RunnerError::ClockDidNotAdvance { timer: i });
        }
        self.now_ms = if live {
            self.now_ms.max(execution)
        } else {
            execution
        };
        self.timers[i].next_ms = next_ms;
        let kind = if frame {
            6
        } else if admitted || (!live && waiting) {
            row.wait as usize
        } else if live && self.clock.waits.contains(row.wait) {
            5
        } else {
            0
        };
        if !self.ids.work.sites.is_empty() {
            self.ids.work.timer_firings[kind] += 1;
        }
        let instance = self.timers[i].key.clone();
        let action = self.plan.timers[i].action;
        let fire = |r: &mut Self| {
            let result = r.traced_action("task", action, Vec::new(), &[]);
            if !r.ids.work.sites.is_empty() {
                r.journal_only(format!(
                    "timer row={i} kind={kind} nominal_ms={nominal:.3} cutoff_ms={cutoff:.3} commit_ms={:.3}",
                    r.now_ms
                ));
            }
            result
        };
        let result = if admitted || (live && waiting && self.clock.waits.contains(row.wait)) {
            // Sample before choosing the scheduling instant for a held firing
            self.clock.admitting = true;
            let result = self.scheduled(self.now_ms, fire);
            self.clock.admitting = false;
            result
        } else {
            self.scheduled(execution, fire)
        };
        let done = self.now_ms;
        let t = &mut self.timers[i];
        if coalesce && t.armed {
            let same = t.key == instance && t.next_ms == next_ms;
            t.next_ms = if frame {
                (t.base, t.k) = (done, 1);
                virtual_frame(done, 1)
            } else if same {
                advance::beat_after(
                    nominal,
                    interval,
                    if live { done.max(cutoff) } else { done },
                )
            } else {
                done + interval
            };
        }
        result
    }
}

/// The least binary64 virtual frame at or after a nominal deadline.
pub fn frame_at_or_after(deadline: f64) -> f64 {
    if !deadline.is_finite() {
        return deadline;
    }
    let mut k = (deadline * 60.0 / 1000.0).ceil() as u64;
    let frame = |k: u64| k as f64 * 1000.0 / 60.0;
    while k > 0 && frame(k - 1) >= deadline {
        k -= 1;
    }
    while frame(k) < deadline {
        k += 1;
    }
    frame(k)
}

impl Waits {
    /// Next nominal check, excluding held firings whose observation must change.
    pub fn next_check(&self) -> Option<f64> {
        [self.frame, self.update, self.scrollend, self.idle]
            .into_iter()
            .filter_map(|w| w.due_ms)
            .reduce(f64::min)
    }
}

/// The observations sampled immediately before idle admission.
#[derive(Clone, Copy, Debug, Default)]
pub struct IdleObservation {
    /// Observed scroll activity.
    pub scrolling: bool,
    /// Frames or document animations are running.
    pub demand: bool,
    /// A batch or publication is unfinished.
    pub applying: bool,
    /// A non-idle firing is held or due.
    pub ordinary: bool,
    /// The next known runner deadline or frame, if any.
    pub next_ms: Option<f64>,
}
impl IdleObservation {
    /// Each term is sampled together; the horizon includes its endpoint.
    pub fn allows(self, now_ms: f64) -> bool {
        !self.scrolling
            && !self.demand
            && !self.applying
            && !self.ordinary
            && self.next_ms.is_none_or(|at| at > now_ms + 1000.0 / 60.0)
    }
}

/// A frame task's `k`th virtual frame after `base`: `base + k·1000/60`, the
/// product first, so sixty frames are exactly a second on every host.
pub fn virtual_frame(base: f64, k: u32) -> f64 {
    base + f64::from(k) * 1000.0 / 60.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_rechecks_hidden_wakes_started_and_completed_by_ordinary_work() {
        use std::{cell::Cell, rc::Rc};
        struct PendingStorage(Rc<Cell<bool>>);
        impl DataSource for PendingStorage {
            fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
                Err(DataError::UnknownSource(source.into()))
            }
            fn background_state(&self) -> Option<crate::BackgroundState> {
                Some(crate::BackgroundState {
                    queued: u64::from(self.0.get()),
                    ..Default::default()
                })
            }
        }
        let source = "component App\n  state count = 0\n  action tick\n    count = count + 1\n  task held mount\n    after(10, tick, wait=scrollend)\n  view\n    text \"timer\"\n";
        let pending = Rc::new(Cell::new(true));
        let mut runner = Runner::boot(
            contract::compile(source).unwrap(),
            PendingStorage(pending.clone()),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        runner.hold_waits(WaitKinds::ALL);
        runner.set_live_clock(Some(Box::new(|| 10.0)));
        runner
            .raise_wake(
                "unheard",
                "event",
                "[]",
                crate::WakeHow {
                    id: 1,
                    cold: false,
                    hidden: true,
                },
            )
            .unwrap();
        let stopped = runner.release_waits(WaitKinds::SCROLLEND, 10.0);
        assert!(stopped.advanced.error.is_none());
        assert!(stopped.advanced.receipts.is_empty());
        assert!(!stopped.continuing);
        assert!(runner.page().hidden);
        assert_eq!(runner.slot("count"), Some(&Value::Number(0.0)));
        assert_eq!(runner.waits(), Waits::default());
        pending.set(false);
        let mut continuing = true;
        for _ in 0..3 {
            let resumed = runner.release_waits(WaitKinds::SCROLLEND, 10.0);
            assert!(resumed.advanced.error.is_none());
            continuing = resumed.continuing;
            if !continuing {
                break;
            }
        }
        assert!(!continuing);
        assert!(!runner.page().hidden);
        assert_eq!(runner.slot("count"), Some(&Value::Number(1.0)));
    }

    #[test]
    fn poison_removes_timer_wakes_and_admission_without_spending_more_firings() {
        let source = "component App\n  state count = 0\n  action tick\n    count = count + 1\n  task held mount\n    after(10, tick, wait=frame)\n  task frames mount\n    every(frame, tick)\n  view\n    text \"timer\"\n";
        let mut runner = Runner::boot(
            contract::compile(source).unwrap(),
            (),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        runner.hold_waits(WaitKinds::ALL);
        runner.set_live_clock(Some(Box::new(|| 10.0)));
        runner.present_frames(true);
        runner.advance(10.0).unwrap();
        let deadlines = runner.tasks();
        for invalid in [f64::NAN, f64::INFINITY, MAX_CLOCK_MS.next_up()] {
            assert!(runner.frame_tasks(invalid).error.is_some());
            assert_eq!(runner.now_ms(), 10.0);
            assert_eq!(runner.tasks(), deadlines);
        }
        runner.poisoned = true;
        assert_eq!(runner.timer_due_ms(), None);
        assert_eq!(runner.waits(), Waits::default());
        assert!(!runner.wants_frames());
        let stopped = runner.release_waits(WaitKinds::FRAME, 10.0);
        assert!(matches!(
            stopped.advanced.error,
            Some(RunnerError::Poisoned)
        ));
        assert!(!stopped.continuing);
        assert!(runner.advance_timed(100.0).receipts.is_empty());
        assert!(matches!(
            runner.frame_tasks(100.0).error,
            Some(RunnerError::Poisoned)
        ));
        assert_eq!(runner.tasks(), deadlines);
        assert_eq!(runner.now_ms(), 10.0);
        runner.set_live_clock(None);
        assert!(runner.frame_tasks(200.0).error.is_some());
        assert_eq!(runner.now_ms(), 10.0);
    }
}
