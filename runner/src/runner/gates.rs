//! Gated tasks (LLP 1092 D7–D10): a task declared `when cond [key=expr]`
//! has a timer only while its gate holds, as a `when` arm has its nodes;
//! a new key restarts it, as a new `each` key makes a new row. Nothing runs
//! when the gate changes: the change arms or drops a deadline.
//!
//! The step runs inside each commit, right after its settlement succeeds
//! and inside the rollback a settlement failure takes, so a refusal here
//! (a key that is no key, a trap) refuses the whole commit and the
//! checkpoint puts the timers back (D8).

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
