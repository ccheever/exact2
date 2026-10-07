//! Gated tasks (LLP 1092 D7–D10): a task declared `when cond [key=expr]`
//! has a timer only while its gate holds, as a `when` arm has its nodes;
//! a new key restarts it, as a new `each` key makes a new row. Nothing runs
//! when the gate changes: the change arms or drops a deadline.
//!
//! The step runs inside each commit, right after its settlement succeeds
//! and inside the rollback a settlement failure takes, so a refusal here
//! (a key that is no key, a trap) refuses the whole commit and the
//! checkpoint puts the timers back (D8).
//!
//! What a host's clock waits for lives here too: whether there are timers,
//! frame tasks, and the soonest deadline of any of them.

use super::*;

impl<D: DataSource> Runner<D> {
    /// D8's table, over the commit's settled state, at the commit's time:
    /// a false gate drops its timer; a gate that turns true arms it from
    /// now; a changed key re-arms it from now; anything else leaves it as
    /// it was (an `every` keeps its phase, a spent `after` stays spent).
    pub(super) fn gate_step(&mut self) -> Result<(), RunnerError> {
        let mut seen = Vec::new();
        for (i, row) in self.plan.timers.iter().enumerate() {
            if !row.gated && !row.keyed {
                continue;
            }
            let on = self.eval(row.gate, &[], &[])? == Value::Bool(true);
            let key = if row.keyed && on {
                let value = self.eval(row.key, &[], &[])?;
                match crate::instance::key_text(&value) {
                    Some(k) => Some(k),
                    None => {
                        return Err(RunnerError::TaskKey {
                            task: self.plan.str(row.name).to_string(),
                        })
                    }
                }
            } else {
                None
            };
            seen.push((i, on, key));
        }
        let now = self.now_ms;
        for (i, on, key) in seen {
            let t = &mut self.timers[i];
            if !on {
                t.armed = false;
                t.next_ms = f64::INFINITY;
                t.key = None;
            } else if !t.armed || t.key != key {
                t.armed = true;
                t.key = key;
                let row = &self.plan.timers[i];
                if row.frame {
                    // While the agent or a test seeks, the task's next
                    // virtual frame; a presenting host fires it at its
                    // next frame (D10).
                    t.base = now;
                    t.k = 1;
                    t.next_ms = super::virtual_frame(now, 1);
                } else {
                    t.next_ms = now + row.interval_ms as f64;
                }
            }
        }
        Ok(())
    }

    /// Whether the plan has timers (a host then drives `advance`): a task, a
    /// `then`, or a queue's `next` (LLP 1092 D3).
    pub fn has_timers(&self) -> bool {
        !self.plan.timers.is_empty()
            || self
                .plan
                .mutations
                .iter()
                .any(|m| m.then.is_some() || m.queue)
    }

    /// Whether the plan has a frame task (LLP 1073 D4): a host keeps its
    /// frame source running and calls [`Runner::frame`] each frame.
    pub fn wants_frames(&self) -> bool {
        // An idle gated frame task keeps no frame source running (LLP 1092 D10).
        self.plan
            .timers
            .iter()
            .zip(&self.timers)
            .any(|(t, s)| t.frame && s.armed)
    }

    /// Soonest timer deadline in this runner's clock domain; no host polling.
    /// A frame task's next virtual frame counts only while the host doesn't
    /// present frames: then its frame source wakes it (LLP 1073 D4).
    /// @ref LLP 1043.000 §3 D8 — hosts wake near the authored timer's due time.
    pub fn timer_due_ms(&self) -> Option<f64> {
        self.timers
            .iter()
            .zip(&self.plan.timers)
            .filter(|(_, row)| !(row.frame && self.presenting))
            .map(|(timer, _)| timer.next_ms)
            .chain(self.then_due.iter().copied())
            // A drop's hold waits a second for its move (LLP 1094 D8).
            .chain(self.reorder_deadline())
            .chain(self.queues.next_due.iter().copied())
            .filter(|ms| ms.is_finite())
            .reduce(f64::min)
    }

    /// Each task's next due time, or `None` while it is idle or spent (the
    /// agent's `state.tasks`, D10).
    pub fn tasks(&self) -> Vec<(String, Option<f64>)> {
        self.plan
            .timers
            .iter()
            .zip(&self.timers)
            .map(|(row, t)| {
                let name = self.plan.str(row.name).to_string();
                (name, Some(t.next_ms).filter(|ms| ms.is_finite()))
            })
            .collect()
    }
}
