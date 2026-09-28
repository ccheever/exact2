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
            // A range reports a number, clamped and snapped as HTML does,
            // whatever a host sent (LLP 1069.001 D4).
            (Some(ControlKind::Range), ControlValue::Text(text)) => {
                let node = self.kernel.node(view).expect("a control");
                let number = exact_num::parse_f64(text.trim())
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| invalid(format!("{text:?} is not a number")))?;
                Ok(Value::Number(
                    exact_kernel::Range::of(node.props).sanitize(number),
                ))
            }
            // A date control reports HTML's value format, within its `min`
            // and `max` (which a picker never passes), or empty when cleared.
            (
                Some(kind @ (ControlKind::Date | ControlKind::Time | ControlKind::DateTimeLocal)),
                ControlValue::Text(text),
            ) => {
                let node = self.kernel.node(view).expect("a control");
                if !kind.valid_value(text) {
                    return Err(invalid(format!(
                        "{text:?} is not a {} value (HTML's format: {})",
                        node.props.str(exact_kernel::PropId::Type).unwrap_or(""),
                        match kind {
                            ControlKind::Date => "2026-09-27",
                            ControlKind::Time => "14:30",
                            _ => "2026-09-27T14:30",
                        }
                    )));
                }
                let bound = |id| {
                    node.props
                        .str(id)
                        .filter(|b| kind.valid_value(b) && !b.is_empty())
                };
                if !text.is_empty() {
                    if let Some(min) =
                        bound(exact_kernel::PropId::Min).filter(|m| text.as_str() < *m)
                    {
                        return Err(invalid(format!("{text:?} is before min {min:?}")));
                    }
                    if let Some(max) =
                        bound(exact_kernel::PropId::Max).filter(|m| text.as_str() > *m)
                    {
                        return Err(invalid(format!("{text:?} is after max {max:?}")));
                    }
                }
                Ok(value.value())
            }
            _ => Err(mismatch()),
        }
    }
}
