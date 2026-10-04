//! A root task's timer row: its schedule and, for a gated task, its gate
//! and key as plan code (LLP 1092 D9), compiled the way a slot's
//! initializer is and evaluated by the runner at each commit.

use crate::{err_one, LowerError, Lowerer};
use contract_syntax::{Expr, Task, TaskKind};
use contract_types::Scope;

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
                    return Err(err_one(
                        "lower-timer-interval",
                        format!(
                            "`{word}` needs a whole number of milliseconds, at least 1; given {ms}"
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
}
