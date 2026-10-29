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
        let previous_untraced = std::mem::take(&mut self.clock.untraced_change);
        let result = run(self);
        let trace = std::mem::replace(&mut self.clock.trace, previous).unwrap_or_default();
        let mut lines = Vec::new();
        for ran in trace {
            lines.push(format!("ran {} {} {}", ran.kind, ran.name, ran.outcome));
            if let Some(reason) = ran.reason {
                lines.push(format!("# {reason}"));
                if reason.starts_with("HandlerArguments") {
                    lines.push("# arguments refused before the action".into());
                }
            }
        }
        if std::mem::replace(&mut self.clock.untraced_change, previous_untraced) {
            lines.push("# a commit the semantics has no branch for".into());
            lines.push("untraced configuration moved".into());
        }
        (result, lines)
    }

    /// Compare the observation's configuration around advance work with no receipt.
    pub(super) fn trace_configuration(&self) -> Option<String> {
        self.clock.trace.as_ref()?;
        let resources: Vec<_> = self
            .plan
            .resources
            .iter()
            .map(|r| self.resource(self.plan.str(r.name)))
            .collect();
        let mut views = Vec::new();
        let mut pending = self.roots();
        while let Some(id) = pending.pop() {
            if let Some(node) = self.kernel.node(id) {
                if let Some(test) = node.props.str(exact_kernel::PropId::TestId) {
                    views.push((
                        test.to_string(),
                        node.props
                            .str(exact_kernel::PropId::Text)
                            .map(str::to_string),
                    ));
                }
                pending.extend(node.children());
            }
        }
        Some(format!(
            "{:?}",
            (
                &self.slots,
                &self.derives,
                resources,
                self.queued(),
                &self.commands,
                views
            )
        ))
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
