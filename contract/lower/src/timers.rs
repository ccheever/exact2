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
                        format!("`{word}` needs a literal number of milliseconds (`{word}(1000, …)`). For a computed wait, gate an `every(frame, check)` task on waiting and have `check` compare `performanceNow()` with the wait; one that follows the tempo follows the transport (docs/contract-for-agents.md, \"Following the transport\")"),
                        t.timer.2,
                    ));
                };
                if !(ms.is_finite() && ms.fract() == 0.0 && *ms >= 1.0 && *ms <= u32::MAX as f64) {
                    // A timer is due when the clock passes it, so 0 would be due
                    // in the commit that armed it; the web's `setTimeout(f, 0)`
                    // runs after the current task, as 1 does here.
                    let soonest = match t.kind {
                        TaskKind::After if *ms < 1.0 => ": write `after(1, …)`, the soonest, which runs a commit after the one that armed it, as the web's `setTimeout(f, 0)` runs after the current task",
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
            let mut wait = 0;
            let mut maximum = 0;
            let mut tolerance = 0;
            let mut seen = std::collections::HashSet::new();
            for (name, value, span) in &t.options {
                let invalid = || {
                    err_one("lower-timer-wait", "Use one `wait=frame|update|scrollend|idle` with optional `max=1..4294967295` for scrollend or idle, or `tolerance=0..4294967295`. Values must be literal whole milliseconds; specify each option once", *span)
                };
                if !seen.insert(name) {
                    return Err(invalid());
                }
                match (name.as_str(), value) {
                    ("wait", Expr::Ident(word, _)) => {
                        wait = match word.as_str() {
                            "frame" => 1,
                            "update" => 2,
                            "scrollend" => 3,
                            "idle" => 4,
                            _ => return Err(invalid()),
                        };
                    }
                    ("max", Expr::Number(ms, _))
                        if ms.is_finite()
                            && ms.fract() == 0.0
                            && *ms >= 1.0
                            && *ms <= u32::MAX as f64 =>
                    {
                        maximum = *ms as u32
                    }
                    ("tolerance", Expr::Number(ms, _))
                        if ms.is_finite()
                            && ms.fract() == 0.0
                            && *ms >= 0.0
                            && *ms <= u32::MAX as f64 =>
                    {
                        tolerance = *ms as u32
                    }
                    _ => return Err(invalid()),
                }
            }
            if (tolerance != 0 && (wait != 0 || t.kind == TaskKind::Frame))
                || (wait != 0 && t.kind == TaskKind::Frame)
                || (maximum != 0 && wait < 3)
            {
                return Err(err_one(
                    "lower-timer-wait",
                    "Use `wait=` on a millisecond timer and `max=` only with scrollend or idle. Positive `tolerance=` is an alternative to `wait=`, and cannot be used on a frame task",
                    t.timer.2,
                ));
            }
            self.b.set_timer_wait(id, wait, maximum);
            self.b.set_timer_tolerance(id, tolerance);
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
