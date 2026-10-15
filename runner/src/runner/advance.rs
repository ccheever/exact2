//! Moving the clock: every timer, `then`, queue `next` and drop deadline due
//! by a time runs, each its own commit, on a seek clock at its own due time
//! and on a live clock as the host's clock reads when it runs (the timers
//! RFC, D1–D4).

use super::*;

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
    /// ([`Runner::present_frames`]; LLP 1073 D3).
    pub fn advance_timed(&mut self, now_ms: f64) -> Advanced {
        self.advance_within(now_ms, false, true, None)
    }

    /// [`Runner::advance_timed`] on a live clock, the host's own rather than
    /// a seek's (the timers RFC, D1–D3): what is due by `cutoff_ms` runs in
    /// the order it was scheduled, but a repeating timer that missed beats
    /// fires once, and each callback observes `sample()`, the clock as it
    /// runs, never earlier than the last. A repeat's next beat is the first
    /// of its phase after its callback completes; one its callback cancelled
    /// stays cancelled, and one it rekeyed starts from that completion.
    pub fn advance_live(&mut self, cutoff_ms: f64, sample: &mut dyn FnMut() -> f64) -> Advanced {
        // A sample that is no clock time (not finite, or past the clock's
        // range) moves nothing.
        let mut checked = || {
            let t = sample();
            if t.is_finite() && t <= MAX_CLOCK_MS {
                t
            } else {
                f64::NEG_INFINITY
            }
        };
        self.advance_within(cutoff_ms, false, true, Some(&mut checked))
    }

    /// Whether the host presents frames (LLP 1073 D4): while it does, frame
    /// tasks fire only at [`Runner::frame`]; while it doesn't (the default:
    /// tests, and the agent's seekable clock), every advance fires their
    /// virtual frames. A host turns it on when its display drives the clock
    /// and off when the agent's clock takes over.
    pub fn present_frames(&mut self, on: bool) {
        self.presenting = on;
    }

    /// A presented frame at `now_ms` (LLP 1073 D2, D4): timers due by then
    /// fire as [`Runner::advance_timed`] fires them, then every frame task
    /// once, at `now_ms`, in plan order; a frame missed is never caught up.
    /// Each task's virtual frames restart from `now_ms` (`super::virtual_frame`).
    pub fn frame(&mut self, now_ms: f64) -> Advanced {
        let presenting = std::mem::replace(&mut self.presenting, true);
        let mut a = self.advance_timed(now_ms);
        self.presenting = presenting;
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
            match self.run_action(action, Vec::new(), &[]) {
                Ok(receipt) => a.receipts.push(Timed { at_ms: at, receipt }),
                Err(e) => {
                    let what = format!(
                        "frame {} ({})",
                        i,
                        self.plan.str(self.plan.action(action).name)
                    );
                    let failed = Err(e);
                    self.log_outcome(&what, &failed, was_poisoned);
                    a.error = failed.err();
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
        self.advance_within(now_ms, true, true, None)
    }

    /// The `then` of every answer already landed, each its own commit, with
    /// the clock where it is and no timer fired: how an agent's input ends
    /// (LLP 1012 §2), so the next operation sees what the input settled —
    /// a wall-clock host runs them at once by itself (`timer_due_ms`).
    /// trivia F3, kanban F19.
    pub fn land_then(&mut self) -> Advanced {
        self.advance_within(self.now_ms, false, false, None)
    }

    /// `live`: the live clock's sampler, `None` for a seek clock (D4).
    fn advance_within(
        &mut self,
        now_ms: f64,
        until_request: bool,
        timers: bool,
        mut live: Option<&mut dyn FnMut() -> f64>,
    ) -> Advanced {
        // A host that presents frames fires frame tasks at `frame`; else
        // their virtual frames are timers (LLP 1073 D3).
        let frames = !self.presenting;
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
        if now_ms < self.now_ms && live.is_none() {
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
            // An answer's `then` goes before a timer due at the same time: the
            // answer landed first. A queue's `next` goes after the `then`s and
            // before the timers (LLP 1092 D3).
            let then = self
                .then_due
                .iter()
                .enumerate()
                .filter(|(_, at)| **at <= now_ms)
                .min_by(|(ia, a), (ib, b)| a.partial_cmp(b).unwrap().then(ia.cmp(ib)))
                .map(|(m, at)| (m, *at));
            let next = self
                .due_next(now_ms)
                .filter(|(_, at)| due.is_none_or(|(_, timer)| *at <= timer))
                .filter(|(_, at)| then.is_none_or(|(_, then)| *at < then));
            let then = then.filter(|(_, at)| due.is_none_or(|(_, timer)| *at <= timer));
            let then = then.filter(|_| next.is_none());
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
                self.now_ms = observe(self.now_ms.max(at), &mut live);
                let ticket = self.next_ticket;
                let result = self.run_next(m);
                self.now_ms = observe(self.now_ms, &mut live);
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
                self.now_ms = observe(self.now_ms.max(at), &mut live);
                self.then_due[m] = f64::INFINITY;
                let action = self.plan.mutations[m].then.expect("armed only with a then");
                let was_poisoned = self.poisoned;
                let ticket = self.next_ticket;
                let result = self.run_action(action, Vec::new(), &[]);
                self.now_ms = observe(self.now_ms, &mut live);
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
                .filter(|at| timers && *at <= now_ms && due.is_none_or(|(_, t)| *at <= t))
            {
                self.now_ms = observe(self.now_ms.max(at), &mut live);
                let result = self.reorder_timeout();
                self.now_ms = observe(self.now_ms, &mut live);
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
            let Some((i, at)) = due else { break };
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
            // A one-shot timer is spent: an infinite deadline is never due
            // and never reported (`timer_due_ms`), so it keeps no host awake.
            let next_ms = if row.once {
                f64::INFINITY
            } else if row.frame {
                self.timers[i].k = self.timers[i].k.saturating_add(1);
                super::virtual_frame(self.timers[i].base, self.timers[i].k)
            } else {
                at + row.interval_ms as f64
            };
            if !row.once && (!next_ms.is_finite() || next_ms <= at) {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::ClockDidNotAdvance { timer: i }),
                };
            }
            // On a live clock a timer observes when it runs, never earlier
            // than the last callback did (D2). A repeat fires once for every
            // beat it missed: its firing is consumed before its callback, so a
            // refused commit (whose checkpoint keeps this) still consumes it.
            let repeat =
                (live.is_some() && !row.once && !row.frame).then_some(row.interval_ms as f64);
            let start = match live.is_some() && !row.frame {
                true => observe(self.now_ms.max(at), &mut live),
                false => at,
            };
            let next_ms = repeat.map_or(next_ms, |interval| {
                beat_after(at, interval, start.max(now_ms))
            });
            self.now_ms = start;
            self.timers[i].next_ms = next_ms;
            let instance = self.timers[i].key.clone();
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            let ticket = self.next_ticket;
            let result = self.run_action(action, Vec::new(), &[]);
            // Every live callback ends with a sample of the clock (D2).
            let done = observe(self.now_ms, &mut live);
            self.now_ms = done;
            // D3: its next beat follows its callback's completion, unless the
            // callback cancelled it; one it rekeyed starts from there.
            if let Some(interval) = repeat {
                let t = &mut self.timers[i];
                if t.armed && t.key == instance && t.next_ms == next_ms {
                    t.next_ms = beat_after(at, interval, done.max(now_ms));
                } else if t.armed {
                    t.next_ms = done + interval;
                }
            }
            match result {
                Ok(receipt) => receipts.push(Timed {
                    at_ms: start,
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
                landed = start;
                break;
            }
        }
        self.now_ms = if live.is_some() {
            self.now_ms.max(landed)
        } else {
            landed
        };
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
}

/// The live clock's time now, never earlier than `at`; `at` on a seek clock.
fn observe(at: f64, live: &mut Option<&mut dyn FnMut() -> f64>) -> f64 {
    live.as_mut().map_or(at, |sample| at.max(sample()))
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
