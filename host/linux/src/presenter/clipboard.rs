//! Desktop text editing through the shell's clipboard.
use super::*;
use crate::surfaces::controls::disabled_control;
use exact_runner::{FieldSelection, InputDetails, SelectionDirection};

/// The clipboard owned by the window's platform adapter.
pub trait Clipboard {
    /// Read Unicode text, or `None` when the clipboard contains another format.
    fn read_text(&mut self) -> Result<Option<String>, String>;
    /// Replace the clipboard's contents with Unicode text.
    fn write_text(&mut self, text: &str) -> Result<(), String>;
}

impl<D: DataSource> Presenter<D> {
    /// Enable desktop clipboard shortcuts for this window's text fields.
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = Some(clipboard);
    }

    pub(super) fn clipboard_key(&mut self, id: ViewId, name: &str, now: f64) -> bool {
        let modifiers = self.modifiers();
        if self.clipboard.is_none() || modifiers.alt || modifiers.meta {
            return false;
        }
        let key = name.to_ascii_lowercase();
        let action = match (key.as_str(), modifiers.ctrl, modifiers.shift) {
            ("insert", true, false) => "c",
            ("insert", false, true) => "v",
            ("delete", false, true) => "x",
            (key @ ("a" | "c" | "x" | "v"), true, _) => key,
            _ => return false,
        };
        self.clipboard_action(id, action, now)
    }

    pub(super) fn clipboard_action(&mut self, id: ViewId, key: &str, now: f64) -> bool {
        if self.clipboard.is_none() {
            return false;
        }
        if key == "a" {
            let value = self.field_text(id);
            let selection = FieldSelection {
                start: 0,
                end: field::utf16_len(&value),
                direction: SelectionDirection::None,
            };
            self.mark_field(id, &value, selection, false);
            self.field_select(id, &value, selection, now);
            self.dirty = true;
            return true;
        }
        let Some(node) = self.host.kernel().node(id) else {
            return true;
        };
        let readonly = node.props.bool(PropId::Editable) == Some(false);
        let password = node.props.str(PropId::Type) == Some("password");
        let multiline = node.props.str(PropId::SemanticTag) == Some("textarea");
        if (readonly && key != "c") || (password && key != "v") {
            return true;
        }
        let kind = match key {
            "c" => EventKind::Copy,
            "x" => EventKind::Cut,
            _ => EventKind::Paste,
        };
        let text = if kind == EventKind::Paste {
            match self.clipboard.as_mut().unwrap().read_text() {
                Ok(Some(text)) => text.replace("\r\n", "\n").replace('\r', "\n"),
                Ok(None) => return true,
                Err(error) => {
                    self.host.log(error);
                    return true;
                }
            }
        } else {
            String::new()
        };
        let mut at = Some(id);
        while let Some(view) = at {
            if self.host.runner().handlers_of(view).contains(&kind) {
                let error = self.host.dispatch_at(
                    view,
                    Event::Clipboard(kind, text.clone(), Vec::new()),
                    now,
                );
                if let Some(error) = error.or(self.after_commit()) {
                    self.host.log(error);
                    return true;
                }
                break;
            }
            at = self.host.kernel().node(view).and_then(|n| n.parent);
        }
        let queued = self.commands.len();
        self.commands.retain(|c| c.name != "preventDefault");
        if queued != self.commands.len()
            || self.focus != Some(id)
            || self
                .host
                .kernel()
                .node(id)
                .is_none_or(|n| disabled_control(&n))
        {
            return true;
        }
        if kind == EventKind::Paste {
            self.paste_field(
                id,
                &if multiline {
                    text
                } else {
                    text.replace('\n', "")
                },
                now,
            );
            return true;
        }
        let value = self.field_text(id);
        let selection = self.field_selection(id);
        let selected =
            &value[field::byte_at(&value, selection.start)..field::byte_at(&value, selection.end)];
        if selected.is_empty() {
            return true;
        }
        if let Err(error) = self.clipboard.as_mut().unwrap().write_text(selected) {
            self.host.log(error);
            return true;
        }
        if kind == EventKind::Cut {
            let (next, after) = field::replace(&value, selection, "");
            if let Some(error) = self.edit_field(
                id,
                next,
                after,
                now,
                InputDetails {
                    input_type: "deleteByCut".into(),
                    data: None,
                    cancelable: true,
                    is_composing: false,
                },
            ) {
                self.host.log(error);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    struct Memory(Rc<RefCell<String>>);
    impl Clipboard for Memory {
        fn read_text(&mut self) -> Result<Option<String>, String> {
            Ok(Some(self.0.borrow().clone()))
        }
        fn write_text(&mut self, text: &str) -> Result<(), String> {
            *self.0.borrow_mut() = text.into();
            Ok(())
        }
    }
    fn fixture(extra: &str, attributes: &str) -> (Presenter<()>, ViewId, Rc<RefCell<String>>) {
        let tag = if attributes.contains("type=") {
            "input"
        } else {
            "textarea"
        };
        let source = format!("component App\n{extra}  view\n    {tag} value=\"seed\" testId=\"field\" width=300 height=100 {attributes}\n");
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 400.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let id = p
            .host
            .kernel()
            .node_by_key(p.host.kernel().find_by_test_id("field")[0])
            .unwrap()
            .id;
        p.set_focus(Some(id), 0.);
        let text = Rc::new(RefCell::new("café\r\n🦀".into()));
        p.set_clipboard(Box::new(Memory(text.clone())));
        (p, id, text)
    }
    fn chord(p: &mut Presenter<()>, key: &str) {
        let code = format!("Key{}", key.to_ascii_uppercase());
        p.hardware_key("ControlLeft", "Control", true, false);
        p.hardware_key(&code, key, true, false);
        p.hardware_key(&code, key, false, false);
        p.hardware_key("ControlLeft", "Control", false, false);
    }

    fn insert_chord(p: &mut Presenter<()>, modifier: &str, key: &str) {
        let code = format!("{modifier}Left");
        p.hardware_key(&code, modifier, true, false);
        p.hardware_key(key, key, true, false);
        p.hardware_key(key, key, false, false);
        p.hardware_key(&code, modifier, false, false);
    }

    #[test]
    fn insert_delete_clipboard_chords_preserve_unicode_and_undo() {
        let (mut p, id, text) = fixture("", "");
        chord(&mut p, "a");
        insert_chord(&mut p, "Shift", "Insert");
        let value = "café\n🦀";
        assert_eq!(p.field_text(id), value);
        assert_eq!(p.field_selection(id), FieldSelection::caret(7));
        chord(&mut p, "a");
        insert_chord(&mut p, "Control", "Insert");
        assert_eq!(&*text.borrow(), value);
        *text.borrow_mut() = "different".into();
        insert_chord(&mut p, "Shift", "Delete");
        assert_eq!(p.field_text(id), "");
        assert_eq!(&*text.borrow(), value);
        chord(&mut p, "z");
        assert_eq!(p.field_text(id), value);
        chord(&mut p, "z");
        assert_eq!(p.field_text(id), "seed");
        assert!(!p.modifiers().ctrl && !p.modifiers().shift);
    }

    #[test]
    fn insert_delete_clipboard_chords_obey_field_and_handler_restrictions() {
        for (extra, attrs, modifier, key) in [
            ("", "readonly=true", "Shift", "Insert"),
            ("", "readonly=true", "Shift", "Delete"),
            ("", "type=\"password\"", "Control", "Insert"),
            ("", "type=\"password\"", "Shift", "Delete"),
            (
                "  action block(k: string)\n    if k == \"Insert\"\n      preventDefault()\n",
                "key=block",
                "Shift",
                "Insert",
            ),
            (
                "  action block\n    preventDefault()\n",
                "paste=block",
                "Shift",
                "Insert",
            ),
            (
                "  action block\n    preventDefault()\n",
                "cut=block",
                "Shift",
                "Delete",
            ),
            (
                "  action block\n    preventDefault()\n",
                "beforeinput=block",
                "Shift",
                "Insert",
            ),
        ] {
            let (mut p, id, text) = fixture(extra, attrs);
            chord(&mut p, "a");
            insert_chord(&mut p, modifier, key);
            assert_eq!(p.field_text(id), "seed", "{attrs}: {modifier}+{key}");
            assert_eq!(&*text.borrow(), "café\r\n🦀", "{attrs}: {modifier}+{key}");
        }
    }
    #[test]
    fn desktop_clipboard_replaces_selection_and_round_trips_unicode() {
        let (mut p, id, text) = fixture("", "");
        chord(&mut p, "a");
        chord(&mut p, "v");
        assert_eq!(p.field_text(id), "café\n🦀");
        assert_eq!(p.field_selection(id), FieldSelection::caret(7));
        chord(&mut p, "a");
        chord(&mut p, "c");
        assert_eq!(&*text.borrow(), "café\n🦀");
        chord(&mut p, "x");
        assert_eq!(p.field_text(id), "");
        chord(&mut p, "v");
        assert_eq!(p.field_text(id), "café\n🦀");
        assert!(!p.modifiers().ctrl);
    }
    #[test]
    fn desktop_document_selection_crosses_lines_and_readonly_text() {
        for attributes in ["", "readonly=true"] {
            let (mut p, id, _) = fixture("", attributes);
            let value = "first line\nsecond 🦀 line";
            p.field_edit(
                id,
                value.into(),
                FieldSelection::at_end(value),
                "seed".into(),
                0.,
            );
            p.hardware_key("ControlLeft", "Control", true, false);
            p.hardware_key("Home", "Home", true, false);
            p.hardware_key("Home", "Home", false, false);
            assert_eq!(p.field_selection(id), FieldSelection::caret(0));
            p.hardware_key("ShiftLeft", "Shift", true, false);
            p.hardware_key("End", "End", true, false);
            p.hardware_key("End", "End", false, false);
            let selection = p.field_selection(id);
            assert_eq!(
                (selection.start, selection.end),
                (0, field::utf16_len(value))
            );
            assert_eq!(selection.direction, SelectionDirection::Forward);
            p.hardware_key("ShiftLeft", "Shift", false, false);
            p.hardware_key("End", "End", true, false);
            p.hardware_key("End", "End", false, false);
            assert_eq!(p.field_selection(id), FieldSelection::at_end(value));
            p.hardware_key("ControlLeft", "Control", false, false);
            p.hardware_key("Home", "Home", true, false);
            p.hardware_key("Home", "Home", false, false);
            assert_eq!(p.field_selection(id), FieldSelection::caret(11));
            if !attributes.is_empty() {
                p.hardware_key("Backspace", "Backspace", true, false);
                p.hardware_key("Backspace", "Backspace", false, false);
                assert_eq!(p.field_text(id), value);
            }
        }
    }
    #[test]
    fn desktop_clipboard_respects_handlers_readonly_and_password_fields() {
        for (extra, attrs, key) in [
            (
                "  action block(k: string)\n    if k == \"v\"\n      preventDefault()\n",
                "key=block",
                "v",
            ),
            ("  action block\n    preventDefault()\n", "paste=block", "v"),
            (
                "  action block\n    preventDefault()\n",
                "beforeinput=block",
                "x",
            ),
            ("", "readonly=true", "x"),
            ("", "readonly=true", "v"),
            ("", "type=\"password\"", "c"),
        ] {
            let (mut p, id, text) = fixture(extra, attrs);
            chord(&mut p, "a");
            chord(&mut p, key);
            assert_eq!(p.field_text(id), "seed", "{attrs}: {key}");
            if key != "x" {
                assert_eq!(&*text.borrow(), "café\r\n🦀", "{attrs}");
            }
        }
    }
}
