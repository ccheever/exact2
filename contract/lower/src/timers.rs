//! A root task's timer row: its schedule and, for a gated task, its gate
//! and key as plan code (LLP 1092 D9), compiled the way a slot's
//! initializer is and evaluated by the runner at each commit.

use crate::{err_one, LowerError, Lowerer};
use contract_syntax::{Expr, Sequence, Task, TaskKind};
use contract_types::Scope;
use exact_plan::{SequencesRow, Value};

impl Lowerer<'_> {
    /// Every task of the root, in declaration order.
    pub(crate) fn timers(&mut self, tasks: &[Task], scope: &Scope) -> Result<(), Vec<LowerError>> {
        for t in tasks {
            let action = self.actions[self
                .root
                .actions
                .iter()
                .position(|a| a.name == t.timer.1)
                .unwrap()];
            let word = match t.kind {
                TaskKind::Every => "every",
                TaskKind::After => "after",
                TaskKind::Frame => "frame",
            };
            let id = if t.kind == TaskKind::Frame {
                self.b.frame_timer(action)
            } else {
                let Expr::Number(ms, _) = &t.timer.0 else {
                    return Err(err_one(
                        "lower-timer-literal",
                        format!("`{word}` needs a literal number of milliseconds"),
                        t.timer.2,
                    ));
                };
                if !(ms.is_finite() && ms.fract() == 0.0 && *ms >= 1.0 && *ms <= u32::MAX as f64) {
                    // A timer is due when the clock passes it, so 0 would be due
                    // in the commit that armed it; the web's `setTimeout(f, 0)`
                    // runs after the current task, as 1 does here.
                    let soonest = match t.kind {
                        TaskKind::After if *ms < 1.0 => ": the soonest is `after(1, …)`, a commit after the one that armed it, as the web's `setTimeout(f, 0)` runs after the current task",
                        TaskKind::Every if *ms < 1.0 => ": a tick each display frame is `every(frame, …)`",
                        _ => "",
                    };
                    return Err(err_one(
                        "lower-timer-interval",
                        format!(
                            "`{word}` needs a whole number of milliseconds, at least 1; given {ms}{soonest}"
                        ),
                        t.timer.2,
                    ));
                }
                self.b.timer(*ms as u32, action, t.kind == TaskKind::After)
            };
            self.b.set_timer_name(id, &t.name);
            let code = |l: &mut Self, e: &Option<Expr>| {
                e.as_ref()
                    .map(|e| l.expr_code(e, scope, 0))
                    .transpose()
                    .map_err(|e| vec![e])
            };
            let gate = code(self, &t.gate)?;
            // `key=` alone is `when true key=…` (D7).
            let gate = gate.or_else(|| {
                t.key
                    .is_some()
                    .then(|| self.b.constant(&exact_plan::Value::Bool(true)))
            });
            let key = code(self, &t.key)?;
            self.b.set_timer_gate(id, gate, key);
        }
        Ok(())
    }

    /// Every sequence of the root (LLP 1125 D2), in declaration order: its
    /// gate and key as a task's, and each `play` argument as plan code, an
    /// omitted one a constant: `bpm=` 120, the prior that clock in (`clock=`,
    /// D9) plays at until it has heard enough ticks.
    pub(crate) fn sequences(
        &mut self,
        sequences: &[Sequence],
        scope: &Scope,
    ) -> Result<(), Vec<LowerError>> {
        for q in sequences {
            let code = |l: &mut Self, e: Option<&Expr>, default: Value| match e {
                Some(e) => l.expr_code(e, scope, 0).map_err(|e| vec![e]),
                None => Ok(l.b.constant(&default)),
            };
            let always = Value::Bool(true);
            let gated = q.gate.is_some() || q.key.is_some();
            let gate = code(self, q.gate.as_ref(), always.clone())?;
            let key = code(self, q.key.as_ref(), always)?;
            let tracks = code(self, Some(&q.tracks), Value::Number(0.0))?;
            let bpm = code(self, q.option("bpm"), Value::Number(120.0))?;
            let from = code(self, q.option("from"), Value::Number(0.0))?;
            let count_in = code(self, q.option("countIn"), Value::list(Vec::new()))?;
            let clock = code(self, q.option("clock"), Value::str(""))?;
            let clock_out = code(self, q.option("clockOut"), Value::str(""))?;
            let ended = q.ended.as_ref().map(|(name, _)| {
                self.actions[self
                    .root
                    .actions
                    .iter()
                    .position(|a| &a.name == name)
                    .unwrap()]
            });
            let name = self.b.str(&q.name);
            self.b.sequence(SequencesRow {
                name,
                gated,
                gate,
                keyed: q.key.is_some(),
                key,
                tracks,
                bpm,
                from,
                count_in,
                clock,
                clock_out,
                ended,
            });
        }
        Ok(())
    }
}
