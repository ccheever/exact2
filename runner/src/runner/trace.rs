//! Commit attribution for the differential observation (LLP 1148 D3).

use super::*;

pub(super) struct Ran {
    kind: &'static str,
    name: String,
    outcome: &'static str,
    reason: Option<String>,
}

impl<D: DataSource> Runner<D> {
    /// Run an operation, recording each handler, task, mutation `then` and
    /// queued send it attempts. Lines use the differential observation's
    /// `ran <kind> <name> <outcome>` format, with `# <Debug error>` on failure.
    /// Recording is confined to this call; ordinary host operations keep no trace.
    pub fn trace_commits<T>(&mut self, run: impl FnOnce(&mut Self) -> T) -> (T, Vec<String>) {
        let previous = self.clock.trace.replace(Vec::new());
        let result = run(self);
        let trace = std::mem::replace(&mut self.clock.trace, previous).unwrap_or_default();
        let mut lines = Vec::new();
        for ran in trace {
            lines.push(format!("ran {} {} {}", ran.kind, ran.name, ran.outcome));
            if let Some(reason) = ran.reason {
                lines.push(format!("# {reason}"));
            }
        }
        (result, lines)
    }

    pub(super) fn traced_action(
        &mut self,
        kind: &'static str,
        action: ActionsId,
        args: Vec<Value>,
        frames: &[Frame],
    ) -> Result<CommitReceipt, RunnerError> {
        let was = self.poisoned;
        let result = self.run_action(action, args, frames);
        self.record_ran(kind, self.plan.action(action).name, was, &result);
        result
    }

    pub(super) fn record_ran(
        &mut self,
        kind: &'static str,
        name: exact_plan::StrId,
        was_poisoned: bool,
        result: &Result<CommitReceipt, RunnerError>,
    ) {
        if was_poisoned {
            return;
        }
        if let Some(trace) = &mut self.clock.trace {
            trace.push(Ran {
                kind,
                name: self.plan.str(name).to_string(),
                outcome: if self.poisoned && !was_poisoned {
                    "poisoned"
                } else if result.is_err() {
                    "refused"
                } else {
                    "ok"
                },
                reason: result.as_ref().err().map(|e| format!("{e:?}")),
            });
        }
    }
}
