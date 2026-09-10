//! Deterministic root and instance timers.
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

    /// Move the clock to `now_ms`, firing every timer due, in order, each at
    /// its own due time. A refusal stops the advance there: the commits so
    /// far are returned with their times, the clock stays at the refusing
    /// timer's due time, and the refusal rides along.
    pub fn advance_timed(&mut self, now_ms: f64) -> Advanced {
        let mut receipts = Vec::new();
        if !now_ms.is_finite() {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::NonFiniteClock),
            };
        }
        if now_ms < self.now_ms {
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
        loop {
            // Current frames are reconstructed from the one live tree at
            // each fire, so reorders retain registration and update captures.
            let mut due: Vec<(usize, f64, u64, Vec<Frame>)> = self
                .timers
                .iter()
                .enumerate()
                .filter(|(_, t)| t.next_ms <= now_ms)
                .map(|(i, t)| (i, t.next_ms, 0, Vec::new()))
                .collect();
            for frames in self.scope_frames() {
                let cell = frames.last().unwrap().scope.as_ref().unwrap().borrow();
                for (i, at) in &cell.timers {
                    if *at <= now_ms {
                        due.push((*i, *at, cell.lifetime, frames.clone()));
                    }
                }
            }
            let Some((i, at, lifetime, frames)) = due
                .into_iter()
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)).then(a.2.cmp(&b.2)))
            else {
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
            let interval = self.plan.timers[i].interval_ms as f64;
            let next_ms = at + interval;
            if !next_ms.is_finite() || next_ms <= at {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::ClockDidNotAdvance { timer: i }),
                };
            }
            self.now_ms = at;
            if lifetime == 0 {
                self.timers[i].next_ms = next_ms;
            } else {
                frames
                    .last()
                    .unwrap()
                    .scope
                    .as_ref()
                    .unwrap()
                    .borrow_mut()
                    .timers
                    .insert(i, next_ms);
            }
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            let args = self.plan.timers[i]
                .args
                .iter()
                .map(|a| self.eval(self.plan.arg(a).expr, &[], &frames))
                .collect::<Result<Vec<_>, _>>();
            let result = args.and_then(|args| self.run_action(action, args, &frames));
            match result {
                Ok(receipt) => {
                    if lifetime != 0 {
                        self.log(format!("timer {i}@{lifetime} fired"));
                    }
                    receipts.push(Timed { at_ms: at, receipt });
                }
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
        }
        self.now_ms = now_ms;
        if !receipts.is_empty() {
            let line = format!(
                "advance → {} timer{} fired, epoch {}",
                receipts.len(),
                if receipts.len() == 1 { "" } else { "s" },
                receipts.last().map_or(0, |t| t.receipt.epoch)
            );
            self.log(line);
        }
        Advanced {
            receipts,
            now_ms,
            error: None,
        }
    }
}
