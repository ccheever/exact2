//! Deliver windowless-editor changes outside its callbacks.
//! @ref LLP 1115.004 D7.1.6.
use super::*;
use exact_ink::device::d3d11::fields;
use exact_runner::{FieldSelection, InputDetails, SelectionDirection};

impl<D: DataSource> Presenter<D> {
    /// Apply editor changes after the native message has returned.
    pub fn windows_field_edits(&mut self, now: f64) {
        if !fields::enabled() {
            return;
        }
        for edit in fields::take_edits() {
            let Some(node) = self.host.kernel().node(edit.id) else {
                continue;
            };
            if node.key.generation != edit.generation
                || node.node_type != NodeType::TextInput
                || node.props.bool(PropId::Disabled) == Some(true)
                || self.host.route_visibility(edit.id).1
            {
                continue;
            }
            let readonly = node.props.bool(PropId::Editable) == Some(false);
            if edit.focused && self.focusable(edit.id) {
                let _ = self.set_focus(Some(edit.id), now);
            }
            let selection = FieldSelection {
                start: edit.selection[0],
                end: edit.selection[1],
                direction: SelectionDirection::None,
            };
            if self.field_text(edit.id) != edit.value {
                if readonly {
                    self.repaint();
                    continue;
                }
                let details = InputDetails {
                    input_type: if edit.composing {
                        "insertCompositionText"
                    } else {
                        "insertText"
                    }
                    .into(),
                    data: None,
                    cancelable: false,
                    is_composing: edit.composing,
                };
                let bound = self.bound_text(edit.id);
                if let Some(error) =
                    self.apply_field_edit(edit.id, edit.value, selection, bound, now, details)
                {
                    self.host.log(error);
                }
            } else {
                self.mark_field(edit.id, &edit.value, selection, false);
            }
            self.repaint();
        }
    }
}
