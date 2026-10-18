//! Moving the clock: every timer, `then`, queue `next` and drop deadline due
//! by a time runs, each its own commit, on a seek clock at its own due time
//! and on a live clock as the host's clock reads when it runs (the timers
//! RFC, D1–D4).

use super::*;

/// How the runner's clock moves: whether the host presents frames, and the
/// host's live clock when one owns it (D1; none is a seek clock).
#[derive(Default)]
pub(super) struct Clock {
    /// The host presents frames: frame tasks fire only at `frame` (LLP 1073 D4)
    pub(super) presenting: bool,
    sample: Option<Box<dyn FnMut() -> f64>>,
    /// The scheduling instant of the callback running, which the `then`s and
    /// queue `next`s it arms inherit (D2)
    instant: Option<f64>,
    /// When the commit running started, or the input or reply it serves
    /// arrived
    started: f64,
    /// An input or reply is running: its commits keep its arrival
    arrived: bool,
}

impl<D: DataSource> Runner<D> {
    /// Move the clock to `now_ms`, firing every timer due, in order — the
    /// commits alone, or the refusal that stopped it. Tests use this; a host
    /// uses [`Runner::advance_timed`], which keeps the commits before a
    /// refusal and the time each was made.
    pub fn advance(&mut self, now_ms: f64) -> Result<Vec<CommitReceipt>, RunnerError> {
        let a = self.advance_timed(now_ms);
        match a.error {
            Some(e) => Err(e),
            None => Ok(a.receipts.into_iter().map(|t| t.receipt).collect()),
        }
    }

    /// An input at the host's time `now_ms`: the clock moves there first,
    /// firing every timer due by then, so the action's `performanceNow()` — and a sound
    /// it schedules (LLP 1096 D3) — is the event's time, as on the JS
    /// target, not the last timer's. The commits in order, the event's last,
    /// at `now_ms`; a timer's refusal rides along and the event still runs.
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> Advanced {
        let mut a = self.advance_timed(now_ms);
        match self.dispatch(view, event) {
            Ok(receipt) => a.receipts.push(Timed {
                at_ms: now_ms.max(self.now_ms),
                receipt,
            }),
            Err(e) => a.error = Some(e),
        }
        a.now_ms = self.now_ms;
        a
    }

    /// Move the clock to `now_ms`, firing every timer due, in order, each at
    /// its own due time. A refusal stops the advance there: the commits so
    /// far are returned with their times, the clock stays at the refusing
    /// timer's due time, and the refusal rides along. Frame tasks fire their
    /// virtual frames here unless the host presents frames
    /// ([`Runner::present_frames`]; LLP 1073 D3). That is a seek clock; a
    /// live one is [`Runner::set_live_clock`]'s.
    pub fn advance_timed(&mut self, now_ms: f64) -> Advanced {
        self.advance_within(now_ms, false, true)
    }

    /// Give the runner the host's live clock, or `None` (the default) for a
    /// seek clock: the agent's, a test's, a replay's (the timers RFC, D1). On
    /// a live clock every commit observes `sample()`, the clock as it starts,
    /// never earlier than the last, and ends with another sample. An advance
    /// runs what is due by its cutoff in the order it was scheduled, a `then`
    /// or queue `next` a callback arms taking the callback's place (D2); a
    /// repeating timer that missed beats fires once, and its next beat is the
    /// first of its phase after its callback completes, while one its
    /// callback cancelled stays cancelled and one it rekeyed starts from that
    /// completion (D3). Virtual frames restart from the completion too.
    pub fn set_live_clock(&mut self, sample: Option<Box<dyn FnMut() -> f64>>) {
        self.clock.sample = sample;
    }

    /// Whether a host's live clock owns the runner's (none is a seek clock).
    pub(super) fn clock_live(&self) -> bool {
        self.clock.sample.is_some()
    }

    /// On a live clock, move the clock to a fresh sample; a sample that is no
    /// clock time (not finite, or past [`MAX_CLOCK_MS`]) moves nothing.
    pub(super) fn sample(&mut self) {
        if let Some(t) = self.clock.sample.as_mut().map(|sample| sample()) {
            if t.is_finite() && t <= MAX_CLOCK_MS {
                self.now_ms = self.now_ms.max(t);
            }
        }
    }

    /// When a `then` or queue `next` armed now is due: with its parent
    /// callback's place, or the arrival of the commit that armed it (D2).
    pub(super) fn armed_at(&self) -> f64 {
        self.clock.instant.unwrap_or(self.clock.started)
    }

    /// Before a commit: sample the clock, and keep when it started.
    pub(super) fn start_commit(&mut self) {
        self.sample();
        if !self.clock.arrived {
            self.clock.started = self.now_ms;
        }
    }

    /// Run an input or a reply that arrives now: what its commits arm takes
    /// its arrival's place (D2), and its handler's arguments observe it.
    pub(super) fn arrived<T>(&mut self, run: impl FnOnce(&mut Self) -> T) -> T {
        self.start_commit();
        let arrived = std::mem::replace(&mut self.clock.arrived, true);
        let result = run(self);
        self.clock.arrived = arrived;
        result
    }

    /// Whether the host presents frames (LLP 1073 D4): while it does, frame
    /// tasks fire only at [`Runner::frame`]; while it doesn't (the default:
    /// tests, and the agent's seekable clock), every advance fires their
    /// virtual frames. A host turns it on when its display drives the clock
    /// and off when the agent's clock takes over.
    pub fn present_frames(&mut self, on: bool) {
        self.clock.presenting = on;
    }

    /// A presented frame at `now_ms` (LLP 1073 D2, D4): timers due by then
    /// fire as [`Runner::advance_timed`] fires them, then every frame task
    /// once, at `now_ms`, in plan order; a frame missed is never caught up.
    /// Each task's virtual frames restart from `now_ms` (`super::virtual_frame`).
    /// On a live clock, timers due by the clock's sample fire, not by the
    /// presentation target `now_ms`, and frame tasks observe the clock.
    pub fn frame(&mut self, now_ms: f64) -> Advanced {
        let cutoff = if self.clock.sample.is_some() {
            self.sample();
            self.now_ms
        } else {
            now_ms
        };
        let presenting = std::mem::replace(&mut self.clock.presenting, true);
        let mut a = self.advance_timed(cutoff);
        self.clock.presenting = presenting;
        // A refused `next` is journaled and the frame goes on: its frame
        // tasks fire, and the `next` waits for a later wake (LLP 1092 D3).
        let next_refused = std::mem::take(&mut self.queues.stopped_on_next);
        if (a.error.is_some() && !next_refused) || !self.wants_frames() {
            return a;
        }
        let at = self.now_ms;
        // The frame tasks armed as the frame starts fire, once each: one a
        // task's commit arms keeps the virtual frame its gate step set, and
        // one it drops is skipped (LLP 1092 D10), as the web's `paint`
        // snapshots them (b6 review A3).
        let armed: Vec<usize> = (0..self.plan.timers.len())
            .filter(|&i| self.plan.timers[i].frame && self.timers[i].armed)
            .collect();
        for i in armed {
            if !self.timers[i].armed {
                continue;
            }
            self.timers[i].base = at;
            self.timers[i].k = 1;
            self.timers[i].next_ms = super::virtual_frame(at, 1);
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            match self.scheduled(at, |r| r.run_action(action, Vec::new(), &[])) {
                Ok(receipt) => a.receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Err(e) => {
                    let what = format!(
                        "frame {} ({})",
                        i,
                        self.plan.str(self.plan.action(action).name)
                    );
                    let failed = Err(e);
                    self.log_outcome(&what, &failed, was_poisoned);
                    a.error = failed.err();
                    a.now_ms = self.now_ms;
                    return a;
                }
            }
        }
        a.now_ms = self.now_ms;
        a
    }

    /// [`Runner::advance_timed`], stopping after the first timer whose
    /// commit hands the host a request: the clock stays at that timer's due
    /// time, for the host to land the reply before advancing again. The
    /// runner keeps one request per target (LLP 1016 D5), so the next tick's
    /// send would drop it. An agent's clock jump advances this way (LLP
    /// 1012); on the wall clock, replies land between ticks by themselves.
    pub fn advance_until_request(&mut self, now_ms: f64) -> Advanced {
        self.advance_within(now_ms, true, true)
    }

    /// The `then` of every answer already landed, each its own commit, with
    /// the clock where it is and no timer fired: how an agent's input ends
    /// (LLP 1012 §2), so the next operation sees what the input settled —
    /// a wall-clock host runs them at once by itself (`timer_due_ms`).
    /// trivia F3, kanban F19.
    pub fn land_then(&mut self) -> Advanced {
        self.advance_within(self.now_ms, false, false)
    }

    fn advance_within(&mut self, now_ms: f64, until_request: bool, timers: bool) -> Advanced {
        let live = self.clock.sample.is_some();
        // A host that presents frames fires frame tasks at `frame`; else
        // their virtual frames are timers (LLP 1073 D3).
        let frames = !self.clock.presenting;
        let mut receipts = Vec::new();
        if !now_ms.is_finite() {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::NonFiniteClock),
            };
        }
        // A live clock's cutoff may be behind what its last callback
        // observed: the work due by the cutoff still runs (D2).
        if now_ms < self.now_ms && !live {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: None,
            };
        }
        if now_ms > MAX_CLOCK_MS {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::ClockOutOfRange),
            };
        }
        let mut landed = now_ms;
        self.queues.stopped_on_next = false;
        loop {
            // The earliest due timer, deterministic by index on ties.
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|_| timers)
                .filter(|(i, t)| t.next_ms <= now_ms && (frames || !self.plan.timers[*i].frame))
                .min_by(|(ia, a), (ib, b)| {
                    a.next_ms.partial_cmp(&b.next_ms).unwrap().then(ia.cmp(ib))
                })
                .map(|(i, t)| (i, t.next_ms));
            // A sequence's end is a timer after every task (LLP 1125 D2): its
            // index is the last; its own wake (a fill, the count-in's end)
            // comes after everything due at the same time (D3).
            let wake = Some(self.transport.wake_due()).filter(|at| timers && *at <= now_ms);
            let before_wake = |at: f64| wake.is_none_or(|w| at <= w);
            let ended = Some(self.transport.ended_due())
                .filter(|at| timers && *at <= now_ms && before_wake(*at))
                .filter(|at| due.is_none_or(|(_, timer)| *at < timer));
            let due = due.filter(|(_, at)| ended.is_none() && before_wake(*at));
            let timer_at = due.map(|(_, at)| at).or(ended);
            let ready = |at: f64| timer_at.is_none_or(|timer| at <= timer) && before_wake(at);
            // An answer's `then` goes before a timer due at the same time: the
            // answer landed first. A queue's `next` goes after the `then`s and
            // before the timers (LLP 1092 D3). `exactTransport`'s answer is a
            // `then` after every mutation's (LLP 1125 D3).
            let then = self
                .then_due
                .iter()
                .enumerate()
                .filter(|(_, at)| **at <= now_ms)
                .min_by(|(ia, a), (ib, b)| a.partial_cmp(b).unwrap().then(ia.cmp(ib)))
                .map(|(m, at)| (m, *at));
            let answer = Some(self.transport.answer_due.min(self.midi.answer_due))
                .filter(|at| *at <= now_ms && then.is_none_or(|(_, then)| *at < then));
            let then_at = answer.or(then.map(|(_, at)| at));
            let next = self
                .due_next(now_ms)
                .filter(|(_, at)| ready(*at))
                .filter(|(_, at)| then_at.is_none_or(|then| *at < then));
            let answer = answer.filter(|at| ready(*at) && next.is_none());
            let then = then.filter(|(_, at)| ready(*at) && next.is_none() && answer.is_none());
            if let Some(at) = answer {
                if receipts.len() == TIMER_FIRE_LIMIT {
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: Some(RunnerError::TimerFireLimit {
                            limit: TIMER_FIRE_LIMIT,
                        }),
                    };
                }
                self.now_ms = self.now_ms.max(at);
                let ticket = self.next_ticket;
                // The transport's first at one time, then `exactMIDI`'s (LLP 1125 D6).
                let answered = if self.transport.answer_due <= at {
                    self.transport.answer_due = f64::INFINITY;
                    let readers = self.transport.readers.clone();
                    self.scheduled(at, |r| r.commit_again(readers, "transport"))
                } else {
                    self.scheduled(at, |r| r.midi_answered())
                };
                match answered {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: Some(e),
                        }
                    }
                }
                if until_request && self.next_ticket != ticket {
                    landed = self.now_ms;
                    break;
                }
                continue;
            }
            if let Some((m, at)) = next {
                if receipts.len() == TIMER_FIRE_LIMIT {
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: Some(RunnerError::TimerFireLimit {
                            limit: TIMER_FIRE_LIMIT,
                        }),
                    };
                }
                self.now_ms = self.now_ms.max(at);
                let ticket = self.next_ticket;
                let result = self.scheduled(at, |r| r.run_next(m));
                match result {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        self.queues.stopped_on_next = true;
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: Some(e),
                        };
                    }
                }
                if until_request && self.next_ticket != ticket {
                    landed = self.now_ms;
                    break;
                }
                continue;
            }
            if let Some((m, at)) = then {
                if receipts.len() == TIMER_FIRE_LIMIT {
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: Some(RunnerError::TimerFireLimit {
                            limit: TIMER_FIRE_LIMIT,
                        }),
                    };
                }
                self.now_ms = self.now_ms.max(at);
                self.then_due[m] = f64::INFINITY;
                let action = self.plan.mutations[m].then.expect("armed only with a then");
                let was_poisoned = self.poisoned;
                let ticket = self.next_ticket;
                let result = self.scheduled(at, |r| r.run_action(action, Vec::new(), &[]));
                match result {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        let what = format!(
                            "{} then {}",
                            self.plan.str(self.plan.mutations[m].name),
                            self.plan.str(self.plan.action(action).name)
                        );
                        let failed = Err(e);
                        self.log_outcome(&what, &failed, was_poisoned);
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: failed.err(),
                        };
                    }
                }
                if until_request && self.next_ticket != ticket {
                    landed = self.now_ms;
                    break;
                }
                continue;
            }
            // A drop's hold ends at its deadline (LLP 1094 D8), before a
            // timer due at the same time: the drop came first.
            if let Some(at) = self
                .reorder_deadline()
                .filter(|at| timers && *at <= now_ms && ready(*at))
            {
                self.now_ms = self.now_ms.max(at);
                // Its commit takes no checkpoint, so it samples for itself
                self.sample();
                let result = self.reorder_timeout();
                self.sample();
                match result {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: Some(e),
                        }
                    }
                }
                continue;
            }
            if let Some(at) = ended {
                // An end that runs `ended=` is a timer's commit: past the limit it
                // stays due, as a task's timer does.
                if self.transport.ended_action(&self.plan).is_some()
                    && receipts.len() == TIMER_FIRE_LIMIT
                {
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: Some(RunnerError::TimerFireLimit {
                            limit: TIMER_FIRE_LIMIT,
                        }),
                    };
                }
                self.now_ms = if live { self.now_ms.max(at) } else { at };
                let Some(action) = self.transport_end() else {
                    continue;
                };
                let was_poisoned = self.poisoned;
                let ticket = self.next_ticket;
                match self.scheduled(at, |r| r.run_action(action, Vec::new(), &[])) {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        let what =
                            format!("ended ({})", self.plan.str(self.plan.action(action).name));
                        let failed = Err(e);
                        self.log_outcome(&what, &failed, was_poisoned);
                        return Advanced {
                            receipts,
                            now_ms: self.now_ms,
                            error: failed.err(),
                        };
                    }
                }
                if until_request && self.next_ticket != ticket {
                    landed = self.now_ms;
                    break;
                }
                continue;
            }
            let Some((i, at)) = due else {
                // Nothing else is due by the transport's own wake: no action,
                // no commit, no receipt.
                if let Some(at) = wake {
                    self.now_ms = if live { self.now_ms.max(at) } else { at };
                    self.sample();
                    self.transport_wake(self.now_ms);
                    continue;
                }
                break;
            };
            if receipts.len() == TIMER_FIRE_LIMIT {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::TimerFireLimit {
                        limit: TIMER_FIRE_LIMIT,
                    }),
                };
            }
            let row = &self.plan.timers[i];
            let (once, frame, interval) = (row.once, row.frame, row.interval_ms as f64);
            // On a live clock a repeat fires once for every beat it missed,
            // and a virtual frame once for every frame (D2): its firing is
            // consumed before its callback, so a refused commit (whose
            // checkpoint keeps this) still consumes it.
            let coalesce = live && !once;
            // A one-shot timer is spent: an infinite deadline is never due
            // and never reported (`timer_due_ms`), so it keeps no host awake.
            let next_ms = if once {
                f64::INFINITY
            } else if coalesce && frame {
                self.timers[i].base = self.now_ms.max(now_ms);
                self.timers[i].k = 1;
                super::virtual_frame(self.timers[i].base, 1)
            } else if coalesce {
                beat_after(at, interval, self.now_ms.max(now_ms))
            } else if frame {
                self.timers[i].k = self.timers[i].k.saturating_add(1);
                super::virtual_frame(self.timers[i].base, self.timers[i].k)
            } else {
                at + interval
            };
            if !once && (!next_ms.is_finite() || next_ms <= at) {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::ClockDidNotAdvance { timer: i }),
                };
            }
            self.now_ms = if live { self.now_ms.max(at) } else { at };
            self.timers[i].next_ms = next_ms;
            let instance = self.timers[i].key.clone();
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            let ticket = self.next_ticket;
            let result = self.scheduled(at, |r| r.run_action(action, Vec::new(), &[]));
            // D3: its next beat follows its callback's completion, unless the
            // callback cancelled it; one it rekeyed starts from there.
            let done = self.now_ms;
            let t = &mut self.timers[i];
            if coalesce && t.armed {
                let same = t.key == instance && t.next_ms == next_ms;
                t.next_ms = if frame {
                    (t.base, t.k) = (done, 1);
                    super::virtual_frame(done, 1)
                } else if same {
                    beat_after(at, interval, done.max(now_ms))
                } else {
                    done + interval
                };
            }
            match result {
                Ok(receipt) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Err(e) => {
                    let what = format!(
                        "timer {} ({})",
                        i,
                        self.plan.str(self.plan.action(action).name)
                    );
                    let failed = Err(e);
                    self.log_outcome(&what, &failed, was_poisoned);
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: failed.err(),
                    };
                }
            }
            if until_request && self.next_ticket != ticket {
                landed = self.now_ms;
                break;
            }
        }
        self.now_ms = if live {
            self.now_ms.max(landed)
        } else {
            landed
        };
        if self.live_times.due().is_some_and(|due| due <= self.now_ms) {
            match self.tick_live_times() {
                Ok(None) => {}
                Ok(Some(receipt)) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Err(error) => {
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: Some(error),
                    }
                }
            }
        }
        if !receipts.is_empty() {
            let line = super::lines::advanced(
                receipts.len(),
                receipts.last().map_or(0, |t| t.receipt.epoch),
            );
            self.log(line);
        }
        Advanced {
            receipts,
            now_ms: self.now_ms,
            error: None,
        }
    }

    /// Run a callback scheduled at `at`; on a live clock what it arms takes
    /// its place (D2).
    fn scheduled<T>(&mut self, at: f64, run: impl FnOnce(&mut Self) -> T) -> T {
        self.clock.instant = self.clock.sample.is_some().then_some(at);
        let result = run(self);
        self.clock.instant = None;
        result
    }
}

/// The first beat of a timer's phase (beats at `beat + k·interval`) strictly
/// after `t`, and after `beat`. The division can round the count either way,
/// so it is corrected until beat k is after `t` and beat k − 1 is not.
fn beat_after(beat: f64, interval: f64, t: f64) -> f64 {
    let mut k = (((t - beat) / interval).floor() + 1.0).max(1.0);
    while k > 1.0 && beat + (k - 1.0) * interval > t {
        k -= 1.0;
    }
    while beat + k * interval <= t {
        k += 1.0;
    }
    beat + k * interval
}

#[cfg(test)]
mod tests {
    use super::beat_after;

    #[test]
    fn the_next_beat_is_strictly_after_the_time_whatever_the_rounding() {
        assert!(beat_after(1.1, 1.0, 4.1) > 4.1);
        assert_eq!(
            beat_after(74.19, 3.0, 884.1899999999999),
            884.19,
            "the first after, not one past it"
        );
        assert_eq!(beat_after(1000.0, 1000.0, 4500.0), 5000.0);
        assert_eq!(
            beat_after(1000.0, 1000.0, 5000.0),
            6000.0,
            "a beat at the time is not after it"
        );
        assert_eq!(
            beat_after(1000.0, 1000.0, 10.0),
            2000.0,
            "and never the beat itself"
        );
    }
}
