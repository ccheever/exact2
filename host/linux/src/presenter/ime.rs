//! Platform composition owns its displayed text until it commits or cancels.
use super::*;
use exact_kernel::NodeKey;
use exact_runner::{FieldSelection, InputDetails, SelectionDirection};
use std::rc::Rc;

/// Identifies a field in this runtime, including replacement of the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    key: NodeKey,
    id: ViewId,
    runtime: u64,
    epoch: u64,
}

pub(super) struct Composition {
    target: Target,
    before: String,
    selection: FieldSelection,
    paint: Rc<crate::paint::field::Composition>,
    changed: bool,
}

impl<D: DataSource> Presenter<D> {
    /// The focused painted editor that can accept platform composition.
    pub fn ime_target(&self) -> Option<Target> {
        let node = self.host.kernel().node(self.focus?)?;
        (self.window_active
            && !self.shape_surface_open()
            && self.input_live(node.key)
            && node.node_type == NodeType::TextInput
            && !crate::fields::hosts(&node)
            && node.props.bool(PropId::Editable) != Some(false)
            && node.props.str(PropId::Type) != Some("password")
            && node.props.str(PropId::InputMode) != Some("none"))
        .then_some(Target {
            key: node.key,
            id: node.id,
            runtime: self.hosts,
            epoch: self.ime_epoch,
        })
    }

    /// Begin one composition at the selection the platform is replacing.
    /// Returns false when a stale target must not receive further events.
    pub fn ime_begin(&mut self, target: Target) -> bool {
        if self.composing_field().is_none() {
            self.ime = None;
        }
        if let Some(composition) = &self.ime {
            return composition.target == target && self.composing_field().is_some();
        }
        if self.ime_target() != Some(target) {
            return false;
        }
        let id = self.focus.unwrap();
        let before = self.field_text(id);
        let selection = self.field_selection(id);
        self.ime = Some(Composition {
            target,
            before: before.clone(),
            selection,
            paint: Rc::new(crate::paint::field::Composition {
                key: target.key,
                value: before,
                range: FieldSelection::caret(selection.start),
                cursor: true,
                unconverted: Vec::new(),
            }),
            changed: false,
        });
        self.hold_input(Some(id));
        true
    }

    fn composing_field(&self) -> Option<ViewId> {
        let composition = self.ime.as_ref()?;
        let node = self.host.kernel().node_by_key(composition.target.key)?;
        (composition.target.runtime == self.hosts
            && composition.target.epoch == self.ime_epoch
            && node.node_type == NodeType::TextInput
            && node.props.bool(PropId::Editable) != Some(false)
            && node.props.bool(PropId::Disabled) != Some(true))
        .then_some(node.id)
    }

    pub(super) fn ime_owns(&self, id: ViewId) -> bool {
        self.composing_field() == Some(id)
    }

    pub(super) fn ime_value(&self, id: ViewId) -> Option<&str> {
        self.ime_owns(id)
            .then(|| self.ime.as_ref().unwrap().paint.value.as_str())
    }

    pub(super) fn ime_paint(&self) -> Option<Rc<crate::paint::field::Composition>> {
        self.composing_field()?;
        Some(self.ime.as_ref()?.paint.clone())
    }

    pub(super) fn ime_edited(&mut self, id: ViewId, value: &str) {
        if self.ime_owns(id) {
            let composition = self.ime.as_mut().unwrap();
            value.clone_into(&mut Rc::make_mut(&mut composition.paint).value);
            composition.changed = true;
        }
    }

    pub(super) fn ime_forget(&mut self, id: ViewId) {
        if self.ime.as_ref().is_some_and(|c| c.target.id == id) {
            self.ime = None;
            self.ime_epoch += 1;
        } else if self.focus == Some(id) {
            self.ime_epoch += 1;
        }
    }

    /// Replace the composing region. Cursor offsets are UTF-8 byte offsets,
    /// as the window's IME protocol reports them; field offsets stay UTF-16.
    pub fn ime_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        let Some(id) = self.composing_field() else {
            return;
        };
        // Winit clears preedit immediately before Commit too. Remove its ink
        // now, but wait for Commit or Disabled before reporting cancellation.
        if text.is_empty() {
            let composition = self.ime.as_mut().unwrap();
            let paint = Rc::make_mut(&mut composition.paint);
            paint.value.clone_from(&composition.before);
            paint.range = FieldSelection::caret(composition.selection.start);
            paint.unconverted.clear();
            paint.cursor = false;
            let (value, selection) = (paint.value.clone(), composition.selection);
            self.mark_field(id, &value, selection, false);
            self.dirty = true;
            return;
        }
        let composition = self.ime.as_ref().unwrap();
        let (next, end) = field::replace(&composition.before, composition.selection, text);
        let start = end.start - field::utf16_len(text);
        let offset = |mut byte: usize| {
            byte = byte.min(text.len());
            while !text.is_char_boundary(byte) {
                byte -= 1;
            }
            start + field::utf16_len(&text[..byte])
        };
        let selection = cursor.map_or(end, |(a, b)| {
            FieldSelection::clamped(
                offset(a).into(),
                offset(b).into(),
                if a == b {
                    SelectionDirection::None
                } else {
                    SelectionDirection::Forward
                },
                field::utf16_len(&next),
            )
        });
        let paint = Rc::make_mut(&mut self.ime.as_mut().unwrap().paint);
        paint.range = FieldSelection {
            start,
            end: end.start,
            direction: SelectionDirection::None,
        };
        paint.cursor = cursor.is_some();
        paint.unconverted.clear();
        self.ime_edit(id, next, selection, text, true);
    }

    /// Mark unconverted preedit ranges for dotted underlines, in relative UTF-16 offsets.
    pub fn ime_unconverted(&mut self, ranges: &[(u32, u32)]) {
        if self.composing_field().is_none() {
            return;
        }
        let paint = Rc::make_mut(&mut self.ime.as_mut().unwrap().paint);
        let length = paint.range.end - paint.range.start;
        let mut ranges: Vec<_> = ranges
            .iter()
            .filter_map(|&(start, end)| {
                let end = end.min(length);
                (start < end).then(|| (paint.range.start + start, paint.range.start + end))
            })
            .collect();
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::new();
        for (start, end) in ranges {
            if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        if paint.unconverted != merged {
            paint.unconverted = merged;
            self.dirty = true;
        }
    }

    /// Commit the platform's result as one undo transaction.
    pub fn ime_commit(&mut self, text: &str) {
        let Some(id) = self.composing_field() else {
            self.ime = None;
            return;
        };
        let node = self.host.kernel().node(id).unwrap();
        let limit = exact_kernel::control::text_maxlength(node.props);
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let text = if node.props.str(PropId::SemanticTag) == Some("textarea") {
            text
        } else {
            text.replace('\n', "")
        };
        let composition = self.ime.as_ref().unwrap();
        let text = field::fit(&text, &composition.before, composition.selection, limit);
        let (next, selection) = field::replace(&composition.before, composition.selection, text);
        self.ime_edit(id, next, selection, text, false);
        self.finish_ime();
    }

    /// The platform ended without a result: remove the unfinished composition.
    pub fn ime_end(&mut self) {
        if let Some(id) = self.composing_field() {
            let composition = self.ime.as_ref().unwrap();
            if composition.changed {
                self.ime_edit(
                    id,
                    composition.before.clone(),
                    composition.selection,
                    "",
                    true,
                );
            }
        }
        self.finish_ime();
    }

    fn ime_edit(
        &mut self,
        id: ViewId,
        next: String,
        selection: FieldSelection,
        data: &str,
        composing: bool,
    ) {
        let details = InputDetails {
            input_type: if composing {
                "insertCompositionText"
            } else {
                "insertFromComposition"
            }
            .into(),
            data: Some(data.into()),
            cancelable: false,
            is_composing: composing,
        };
        if let Some(error) = self.edit_field(id, next, selection, self.host.now(), details) {
            self.host.log(error);
        }
    }

    fn finish_ime(&mut self) {
        let id = self.composing_field();
        let kept = id.map(|id| self.field_selection(id));
        let Some(composition) = self.ime.take() else {
            return;
        };
        self.dirty = true;
        let Some(id) = id else { return };
        let value = self.field_text(id);
        let selection = if value == composition.paint.value {
            kept.unwrap_or(composition.selection)
        } else {
            FieldSelection::at_end(&value)
        };
        self.mark_field(id, &value, selection, false);
        if composition.changed {
            self.field_history.record(
                id,
                &composition.before,
                composition.selection,
                &value,
                selection,
                "insertFromComposition",
                self.host.now(),
            );
        }
        if self.focus != Some(id) {
            // A native result can arrive after Windows has moved the focus.
            let edited = self.edited.replace(id);
            if let Some(error) = self
                .commit_text(id, self.host.now())
                .flatten()
                .or(self.after_commit())
            {
                self.host.log(error);
            }
            self.edited = edited.filter(|old| *old != id);
        }
    }
}

#[cfg(test)]
#[path = "ime_tests.rs"]
mod tests;
