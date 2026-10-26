//! Undo transactions for fields drawn by the desktop presenter.
use super::*;
use exact_runner::{FieldSelection, InputDetails};
use std::collections::VecDeque;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_FIELDS: usize = 64;
const MAX_EDITS: usize = 128;

#[derive(Clone)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    before: FieldSelection,
    after: FieldSelection,
    kind: String,
    at: f64,
}

impl Edit {
    fn between(
        before: &str,
        after: &str,
        selection: FieldSelection,
        next: FieldSelection,
        kind: &str,
        at: f64,
    ) -> Self {
        let start = before
            .chars()
            .zip(after.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let suffix = before[start..]
            .chars()
            .rev()
            .zip(after[start..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        Self {
            start,
            removed: before[start..before.len() - suffix].into(),
            inserted: after[start..after.len() - suffix].into(),
            before: selection,
            after: next,
            kind: kind.into(),
            at,
        }
    }

    fn merge(&mut self, next: &Self) -> bool {
        if self.kind != next.kind
            || self.after != next.before
            || next.at < self.at
            || next.at - self.at > 1000.
        {
            return false;
        }
        match self.kind.as_str() {
            "insertText"
                if self.removed.is_empty()
                    && next.removed.is_empty()
                    && next.start == self.start + self.inserted.len() =>
            {
                self.inserted.push_str(&next.inserted)
            }
            "deleteContentBackward"
                if self.inserted.is_empty()
                    && next.inserted.is_empty()
                    && self.start == next.start + next.removed.len() =>
            {
                self.start = next.start;
                self.removed.insert_str(0, &next.removed);
            }
            "deleteContentForward"
                if self.inserted.is_empty()
                    && next.inserted.is_empty()
                    && self.start == next.start =>
            {
                self.removed.push_str(&next.removed)
            }
            _ => return false,
        }
        self.after = next.after;
        self.at = next.at;
        true
    }
}

#[derive(Default)]
struct History {
    value: String,
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    used: u64,
}
impl History {
    fn bytes(&self) -> usize {
        self.value.capacity()
            + (self.undo.capacity() + self.redo.capacity()) * std::mem::size_of::<Edit>()
            + self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|e| e.removed.capacity() + e.inserted.capacity() + e.kind.capacity())
                .sum::<usize>()
    }
}

#[derive(Default)]
pub(super) struct Histories {
    fields: BTreeMap<ViewId, History>,
    serial: u64,
}
impl Histories {
    pub(super) fn available(&self, id: ViewId, value: &str, redo: bool) -> bool {
        self.fields.get(&id).is_some_and(|h| {
            h.value == value
                && if redo {
                    !h.redo.is_empty()
                } else {
                    !h.undo.is_empty()
                }
        })
    }
    pub(super) fn invalidate(&mut self, id: ViewId) {
        self.fields.remove(&id);
    }
    pub(super) fn retain(&mut self, mut live: impl FnMut(ViewId) -> bool) {
        self.fields.retain(|&id, _| live(id));
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        id: ViewId,
        value: &str,
        before: FieldSelection,
        next: &str,
        after: FieldSelection,
        kind: &str,
        at: f64,
    ) {
        if value == next {
            return;
        }
        if value.len() + next.len() > MAX_BYTES {
            self.fields.remove(&id);
            return;
        }
        self.serial += 1;
        let history = self.fields.entry(id).or_default();
        if history.value != value {
            *history = History::default();
        }
        let edit = Edit::between(value, next, before, after, kind, at);
        let merge = history.redo.is_empty()
            && history
                .undo
                .back_mut()
                .is_some_and(|last| last.merge(&edit));
        if !merge {
            history.undo.push_back(edit);
        }
        history.redo.clear();
        history.value = next.into();
        history.used = self.serial;
        while history.undo.len() > MAX_EDITS {
            history.undo.pop_front();
        }
        self.trim();
    }

    fn trim(&mut self) {
        while self.fields.len() > MAX_FIELDS
            || self.fields.values().map(History::bytes).sum::<usize>() > MAX_BYTES
        {
            let oldest = *self.fields.iter().min_by_key(|(_, h)| h.used).unwrap().0;
            self.fields.remove(&oldest);
        }
    }

    fn prepare(&mut self, id: ViewId, value: &str, redo: bool) -> Option<(String, FieldSelection)> {
        if self.fields.get(&id).is_some_and(|h| h.value != value) {
            self.fields.remove(&id);
            return None;
        }
        let history = self.fields.get(&id)?;
        let edit = if redo {
            history.redo.last()?
        } else {
            history.undo.back()?
        };
        let (remove, insert, selection) = if redo {
            (&edit.removed, &edit.inserted, edit.after)
        } else {
            (&edit.inserted, &edit.removed, edit.before)
        };
        let mut next = value.to_owned();
        next.replace_range(edit.start..edit.start + remove.len(), insert);
        Some((next, selection))
    }

    fn applied(&mut self, id: ViewId, next: &str, redo: bool) {
        let Some(history) = self.fields.get_mut(&id) else {
            return;
        };
        if redo {
            if let Some(edit) = history.redo.pop() {
                history.undo.push_back(edit);
            }
        } else if let Some(edit) = history.undo.pop_back() {
            history.redo.push(edit);
        }
        history.value = next.into();
        self.serial += 1;
        history.used = self.serial;
        self.trim();
    }
}

impl<D: DataSource> Presenter<D> {
    pub(super) fn history_key(&mut self, id: ViewId, key: &str, now: f64) -> bool {
        let modifiers = self.modifiers();
        if !modifiers.ctrl || modifiers.alt || modifiers.meta {
            return false;
        }
        let redo = match key.to_ascii_lowercase().as_str() {
            "z" => modifiers.shift,
            "y" => true,
            _ => return false,
        };
        self.history_action(id, redo, now)
    }

    pub(super) fn history_action(&mut self, id: ViewId, redo: bool, now: f64) -> bool {
        let Some(node) = self.host.kernel().node(id) else {
            return false;
        };
        if node.node_type != NodeType::TextInput || crate::fields::hosts(&node) {
            return false;
        }
        if node.props.bool(PropId::Editable) == Some(false)
            || crate::surfaces::controls::disabled_control(&node)
        {
            return true;
        }
        self.forget_replaced_choices();
        let value = self.field_text(id);
        let Some((next, selection)) = self.field_history.prepare(id, &value, redo) else {
            return true;
        };
        let mut input = InputDetails {
            input_type: if redo { "historyRedo" } else { "historyUndo" }.into(),
            data: None,
            cancelable: true,
            is_composing: false,
        };
        match self.before_input(id, &input, now) {
            Ok(true) => {
                input.cancelable = false;
                let bound = self.bound_text(id);
                self.field_history.applied(id, &next, redo);
                if let Some(error) = self.apply_field_edit(id, next, selection, bound, now, input) {
                    self.host.log(error);
                }
            }
            Ok(false) => {}
            Err(error) => self.host.log(error),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Presenter<()>, ViewId) {
        let source = r#"component App
  state value = "seed"
  state blocked = false
  state locked = false
  state heard = ""
  action before(e: InputEvent)
    if blocked and (e.inputType == "historyUndo" or e.inputType == "historyRedo")
      preventDefault()
  action edited(v: string, e: InputEvent)
    value = v
    heard = e.inputType
  action block
    blocked = not blocked
  action lock
    locked = not locked
  action reset
    value = "reset"
  view
    column
      textarea testId="editor" value=value input=edited beforeinput=before readonly=locked
      button "Block" testId="block" press=block
      button "Lock" testId="lock" press=lock
      button "Reset" testId="reset" press=reset
      text heard testId="heard"
"#;
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (400., 400.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let editor = id(&p, "editor");
        p.set_focus(Some(editor), 0.);
        (p, editor)
    }
    fn id(p: &Presenter<()>, test: &str) -> ViewId {
        let kernel = p.host.kernel();
        kernel
            .node_by_key(kernel.find_by_test_id(test)[0])
            .unwrap()
            .id
    }
    fn chord(p: &mut Presenter<()>, key: &str, shift: bool) {
        p.hardware_key("ControlLeft", "Control", true, false);
        if shift {
            p.hardware_key("ShiftLeft", "Shift", true, false);
        }
        let code = format!("Key{}", key.to_ascii_uppercase());
        p.hardware_key(&code, key, true, false);
        p.hardware_key(&code, key, false, false);
        if shift {
            p.hardware_key("ShiftLeft", "Shift", false, false);
        }
        p.hardware_key("ControlLeft", "Control", false, false);
    }
    fn press(p: &mut Presenter<()>, name: &str, editor: ViewId) {
        p.tap(id(p, name)).unwrap();
        p.set_focus(Some(editor), p.host.now());
    }
    #[test]
    fn undo_redo_restore_unicode_selection_and_dispatch_history_input() {
        let (mut p, editor) = fixture();
        p.type_text(editor, "café\n界 🦀").unwrap();
        let end = p.field_selection(editor);
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "seed");
        assert_eq!(
            (
                p.field_selection(editor).start,
                p.field_selection(editor).end
            ),
            (0, 4)
        );
        let heard = id(&p, "heard");
        assert_eq!(
            p.host.kernel().node(heard).unwrap().props.str(PropId::Text),
            Some("historyUndo")
        );
        chord(&mut p, "y", false);
        assert_eq!(p.field_text(editor), "café\n界 🦀");
        assert_eq!(p.field_selection(editor), end);
        chord(&mut p, "z", false);
        chord(&mut p, "z", true);
        assert_eq!(p.field_text(editor), "café\n界 🦀");
        assert_eq!(
            p.host.kernel().node(heard).unwrap().props.str(PropId::Text),
            Some("historyRedo")
        );
        chord(&mut p, "z", false);
        p.type_text(editor, "replacement").unwrap();
        chord(&mut p, "y", false);
        assert_eq!(
            p.field_text(editor),
            "replacement",
            "new input discards redo"
        );
    }
    #[test]
    fn canceled_history_readonly_and_scripted_values_do_not_consume_undo() {
        let (mut p, editor) = fixture();
        p.type_text(editor, "typed").unwrap();
        press(&mut p, "block", editor);
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "typed");
        press(&mut p, "block", editor);
        press(&mut p, "lock", editor);
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "typed");
        press(&mut p, "lock", editor);
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "seed");
        chord(&mut p, "y", false);
        press(&mut p, "reset", editor);
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "reset");
        p.type_text(editor, "next").unwrap();
        p.replaced();
        chord(&mut p, "z", false);
        assert_eq!(
            p.field_text(editor),
            "next",
            "a replacement runtime has no old undo history"
        );
    }
    #[test]
    fn adjacent_typing_and_deletion_coalesce_and_moving_breaks_a_group() {
        let (mut p, editor) = fixture();
        p.type_text(editor, "").unwrap();
        for c in ["a", "界", "🦀"] {
            p.field_key(editor, c, 10.);
        }
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "");
        chord(&mut p, "y", false);
        p.field_key(editor, "Backspace", 20.);
        p.field_key(editor, "Backspace", 30.);
        assert_eq!(p.field_text(editor), "a");
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "a界🦀");
        p.field_key(editor, "Home", 40.);
        p.field_key(editor, "x", 50.);
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "a界🦀");
        assert_eq!(p.field_selection(editor), FieldSelection::caret(0));
    }
    #[test]
    fn history_memory_is_bounded_and_unmounted_fields_release_it() {
        let mut h = Histories::default();
        for id in 1..=80 {
            let value = "界".repeat(30_000);
            h.record(
                id,
                "",
                FieldSelection::caret(0),
                &value,
                FieldSelection::at_end(&value),
                "insertFromPaste",
                0.,
            );
            assert!(h.fields.values().map(History::bytes).sum::<usize>() <= MAX_BYTES);
            assert!(h.fields.len() <= MAX_FIELDS);
        }
        assert!(h.fields.contains_key(&80));
        h.retain(|_| false);
        assert!(h.fields.is_empty());
    }
}
