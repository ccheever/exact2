//! What a form control's `input` and `change` may carry (LLP 1069.001 D4):
//! the payload its kind reports, held to HTML's rules on every host, so an
//! action's parameter is never a string to parse and an agent's `type`
//! fails the same way wherever it runs.

use super::{ControlValue, DataSource, Runner, RunnerError};
use exact_kernel::{ControlKind, ViewId};
use exact_plan::Value;

impl<D: DataSource> Runner<D> {
    /// The action's payload for a control event at `view`, or the refusal:
    /// a kind mismatch by the event's name, a value the control could never
    /// report with the reason.
    pub(super) fn control_payload(
        &self,
        view: ViewId,
        event: &'static str,
        value: &ControlValue,
    ) -> Result<Value, RunnerError> {
        let kind = self
            .kernel
            .node(view)
            .and_then(|n| ControlKind::of(n.node_type, n.props));
        let mismatch = || RunnerError::InvalidEvent { event };
        let invalid = |reason: String| RunnerError::InvalidValue { event, reason };
        match (kind, value) {
            (None, ControlValue::Text(_))
            | (Some(ControlKind::Checkbox | ControlKind::Switch), ControlValue::Checked(_))
            | (Some(ControlKind::File), ControlValue::Files(_)) => Ok(value.value()),
            // @ref LLP 1069.001 D4 — a select reports the chosen option's
            // value: one of its enabled options, as a menu could choose.
            (Some(ControlKind::Select), ControlValue::Text(chosen)) => {
                let choices = self.kernel.select_choices(view);
                match choices.iter().find(|c| &c.value == chosen) {
                    Some(c) if !c.disabled => Ok(value.value()),
                    Some(_) => Err(invalid(format!("option {chosen:?} is disabled"))),
                    None => Err(invalid(format!(
                        "no option has the value {chosen:?} (options: {})",
                        choices
                            .iter()
                            .map(|c| format!("{:?}", c.value))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))),
                }
            }
            _ => Err(mismatch()),
        }
    }
}
