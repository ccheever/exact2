//! The shell's text menu uses the same editing operations as keyboard shortcuts.
use super::*;
use crate::surfaces::controls::disabled_control;
use exact_kernel::NodeKey;
use exact_runner::{FieldSelection, InputDetails};

/// Standard commands in a desktop text editor's context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EditAction {
    /// Restore the preceding edit.
    Undo = 1,
    /// Reapply the last undone edit.
    Redo,
    /// Copy the selection and remove it.
    Cut,
    /// Copy the selection.
    Copy,
    /// Insert clipboard text at the selection.
    Paste,
    /// Remove the selection.
    Delete,
    /// Select the whole value.
    SelectAll,
}

/// A menu for one live field and selection. Its commands cannot migrate to
/// another allocation, value, or selection while the shell opens the menu.
#[derive(Clone, Debug)]
pub struct EditMenu {
    key: NodeKey,
    value: String,
    selection: FieldSelection,
    enabled: u16,
    /// Menu anchor in logical viewport coordinates.
    pub point: (f32, f32),
}
impl EditMenu {
    /// Whether the field permits this command. The shell also checks whether
    /// the clipboard offers text before enabling Paste.
    pub fn enabled(&self, action: EditAction) -> bool {
        self.enabled & (1 << action as u32) != 0
    }
}

impl<D: DataSource> Presenter<D> {
    /// A secondary click's default menu. Authored context menus and handlers
    /// run first; disabled, covered and hosted fields have no painted-field menu.
    pub fn edit_menu_at(&mut self, point: (f32, f32)) -> Option<EditMenu> {
        if self.top_open() {
            return None;
        }
        let id = self.hit(point.0, point.1)?;
        if !self.menu_field(id) {
            self.long_press(point.0, point.1, self.host.now());
            return None;
        }
        self.open_edit_menu(id, point, true)
    }

    /// Handle the context-menu key or Shift+F10, including key handlers and
    /// their cancellation. `false` leaves the key to the ordinary input path.
    pub fn edit_menu_key(
        &mut self,
        code: &str,
        key: &str,
        repeat: bool,
    ) -> (bool, Option<EditMenu>) {
        let held = self.modifiers();
        let asks = key == "ContextMenu" || (key == "F10" && held.shift);
        if !asks || held.ctrl || held.alt || held.meta || self.top_open() {
            return (false, None);
        }
        let Some(id) = self.focus.filter(|&id| self.menu_field(id)) else {
            return (false, None);
        };
        let now = self.host.now();
        let mark = self.hatch_key_mark();
        self.hold_input(self.focus);
        self.key_shows_ring(key);
        if self.shortcut(key, repeat, now) {
            self.hatch_key(mark, key, code, true, repeat);
            return (true, None);
        }
        let (error, prevented) = self.key_event(EventKind::Key, key, code, repeat, now);
        if let Some(error) = error {
            self.host.log(error);
        }
        let menu = if prevented || repeat || self.focus != Some(id) {
            None
        } else {
            let rect = self.caret_frame().or_else(|| self.rect_of(id));
            rect.and_then(|r| self.open_edit_menu(id, (r.0, r.1 + r.3), false))
        };
        self.hatch_key(mark, key, code, true, repeat);
        (true, menu)
    }

    fn menu_field(&self, id: ViewId) -> bool {
        self.host.kernel().node(id).is_some_and(|n| {
            n.node_type == NodeType::TextInput
                && !crate::fields::hosts(&n)
                && !disabled_control(&n)
                && self.focus_refusal(id).is_none()
        })
    }

    fn open_edit_menu(&mut self, id: ViewId, point: (f32, f32), mouse: bool) -> Option<EditMenu> {
        if !self.menu_field(id) {
            return None;
        }
        let key = self.host.kernel().node(id)?.key;
        let now = self.host.now();
        // Snapshot the context path before any handler changes the tree.
        let mut path = Vec::new();
        let mut at = Some(id);
        let mut popover = None;
        while let Some(node) = at.and_then(|id| self.host.kernel().node(id)) {
            if popover.is_none() {
                popover = node
                    .props
                    .str(PropId::ContextPopover)
                    .filter(|s| !s.is_empty())
                    .map(|s| (node.id, s.to_owned()));
            }
            if self
                .host
                .runner()
                .handlers_of(node.id)
                .contains(&EventKind::Contextmenu)
            {
                path.push(node.key);
            }
            at = node.parent;
        }
        let mut prevented = false;
        for key in path {
            let Some(node) = self.host.kernel().node_by_key(key) else {
                continue;
            };
            let error = self.host.dispatch_at(node.id, Event::Contextmenu, now);
            if let Some(error) = error.or(self.after_commit()) {
                self.host.log(error);
            }
            let mut stopped = false;
            self.commands.retain(|c| match c.name.as_str() {
                "preventDefault" => {
                    prevented = true;
                    false
                }
                "stopPropagation" => {
                    stopped = true;
                    false
                }
                _ => true,
            });
            if stopped {
                break;
            }
        }
        if prevented || !self.input_live(key) || !self.menu_field(id) || self.top_open() {
            return None;
        }
        if let Some((invoker, name)) = popover {
            self.present(
                self.element(&name),
                &name,
                Some(invoker),
                Some(point),
                false,
                now,
            );
            return None;
        }
        if let Some(error) = self.set_focus(Some(id), now) {
            self.host.log(error);
        }
        if self.focus != Some(id) || !self.input_live(key) || !self.menu_field(id) {
            return None;
        }
        if mouse {
            let value = self.field_text(id);
            let selection = self.field_selection(id);
            let field_point = self
                .boxes
                .iter()
                .find(|b| b.id == id)
                .map_or(point, |b| b.field_point(point.0, point.1));
            let hit = self
                .current_field(id, &value)
                .and_then(|f| f.hit(field_point.0, field_point.1));
            if selection.start == selection.end
                || hit.is_none_or(|c| c.offset < selection.start || c.offset > selection.end)
            {
                self.press_field(id, point.0, point.1);
            }
        }
        Some(self.make_edit_menu(id, point))
    }

    fn make_edit_menu(&self, id: ViewId, point: (f32, f32)) -> EditMenu {
        let node = self.host.kernel().node(id).expect("a checked live field");
        let value = self.field_text(id);
        let selection = self.field_selection(id);
        let editable = node.props.bool(PropId::Editable) != Some(false);
        let selected = selection.start != selection.end;
        let copy = selected
            && node.props.str(PropId::Type) != Some("password")
            && self.clipboard.is_some();
        let mut enabled = 0;
        for (action, yes) in [
            (
                EditAction::Undo,
                editable && self.field_history.available(id, &value, false),
            ),
            (
                EditAction::Redo,
                editable && self.field_history.available(id, &value, true),
            ),
            (EditAction::Cut, editable && copy),
            (EditAction::Copy, copy),
            (EditAction::Paste, editable && self.clipboard.is_some()),
            (EditAction::Delete, editable && selected),
            (
                EditAction::SelectAll,
                !value.is_empty()
                    && (selection.start != 0 || selection.end != field::utf16_len(&value)),
            ),
        ] {
            if yes {
                enabled |= 1 << action as u32;
            }
        }
        EditMenu {
            key: node.key,
            value,
            selection,
            enabled,
            point,
        }
    }

    /// Whether the menu still describes the focused text field and selection.
    pub fn edit_menu_valid(&self, menu: &EditMenu) -> bool {
        if self.top_open() || !self.input_live(menu.key) {
            return false;
        }
        self.host.kernel().node_by_key(menu.key).is_some_and(|n| {
            self.focus == Some(n.id)
                && self.menu_field(n.id)
                && self.field_text(n.id) == menu.value
                && self.field_selection(n.id) == menu.selection
        })
    }

    /// Execute a selected command through its normal cancelable editing path.
    pub fn edit_menu_action(&mut self, menu: &EditMenu, action: EditAction) {
        if !self.edit_menu_valid(menu) {
            return;
        }
        let id = self.host.kernel().node_by_key(menu.key).unwrap().id;
        if !self.make_edit_menu(id, menu.point).enabled(action) {
            return;
        }
        let now = self.host.now();
        match action {
            EditAction::Undo | EditAction::Redo => {
                self.history_action(id, action == EditAction::Redo, now);
            }
            EditAction::Cut | EditAction::Copy | EditAction::Paste | EditAction::SelectAll => {
                let key = match action {
                    EditAction::Cut => "x",
                    EditAction::Copy => "c",
                    EditAction::Paste => "v",
                    _ => "a",
                };
                self.clipboard_action(id, key, now);
            }
            EditAction::Delete => {
                let (next, selection) = field::replace(&menu.value, menu.selection, "");
                if let Some(error) = self.edit_field(
                    id,
                    next,
                    selection,
                    now,
                    InputDetails {
                        input_type: "deleteContentForward".into(),
                        data: None,
                        cancelable: true,
                        is_composing: false,
                    },
                ) {
                    self.host.log(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    struct Memory(Rc<RefCell<String>>);
    impl clipboard::Clipboard for Memory {
        fn read_text(&mut self) -> Result<Option<String>, String> {
            Ok(Some(self.0.borrow().clone()))
        }
        fn write_text(&mut self, text: &str) -> Result<(), String> {
            *self.0.borrow_mut() = text.into();
            Ok(())
        }
    }
    fn fixture(extra: &str, attrs: &str) -> (Presenter<()>, ViewId, Rc<RefCell<String>>) {
        let source = format!("component App\n{extra}  view\n    input value=\"café 界 🦀\" testId=\"editor\" width=300 height=48 appearance=\"none\" {attrs}\n");
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 200.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let k = p.host.kernel();
        let id = k.node_by_key(k.find_by_test_id("editor")[0]).unwrap().id;
        let clipboard = Rc::new(RefCell::new("paste 🦀".into()));
        p.set_clipboard(Box::new(Memory(clipboard.clone())));
        p.set_focus(Some(id), 0.);
        p.frame();
        (p, id, clipboard)
    }
    fn menu(p: &mut Presenter<()>) -> EditMenu {
        let (handled, menu) = p.edit_menu_key("ContextMenu", "ContextMenu", false);
        assert!(handled);
        menu.expect("an enabled field has an editing menu")
    }
    fn command(p: &mut Presenter<()>, action: EditAction) {
        let menu = menu(p);
        assert!(menu.enabled(action), "{action:?}");
        p.edit_menu_action(&menu, action);
    }

    #[test]
    fn menu_edits_preserve_unicode_and_share_clipboard_and_undo() {
        let (mut p, id, clipboard) = fixture("", "");
        let original = p.field_text(id);
        let first = menu(&mut p);
        assert!(!first.enabled(EditAction::Copy));
        assert!(!first.enabled(EditAction::Delete));
        assert!(!first.enabled(EditAction::Undo));
        command(&mut p, EditAction::SelectAll);
        command(&mut p, EditAction::Copy);
        assert_eq!(*clipboard.borrow(), original);
        command(&mut p, EditAction::Cut);
        assert_eq!(p.field_text(id), "");
        command(&mut p, EditAction::Undo);
        assert_eq!(p.field_text(id), original);
        assert_eq!(p.field_selection(id).end, field::utf16_len(&original));
        command(&mut p, EditAction::Redo);
        assert_eq!(p.field_text(id), "");
        *clipboard.borrow_mut() = "paste 🦀".into();
        command(&mut p, EditAction::Paste);
        assert_eq!(p.field_text(id), "paste 🦀");
        command(&mut p, EditAction::SelectAll);
        command(&mut p, EditAction::Delete);
        assert_eq!(p.field_text(id), "");
        assert_eq!(
            *clipboard.borrow(),
            "paste 🦀",
            "Delete must not change the clipboard"
        );
        command(&mut p, EditAction::Undo);
        assert_eq!(p.field_text(id), "paste 🦀");
    }

    #[test]
    fn menus_respect_readonly_password_disabled_and_stale_selections() {
        for attrs in ["readonly=true", "type=\"password\"", "disabled=true", ""] {
            let (mut p, id, clipboard) = fixture("", attrs);
            if attrs == "disabled=true" {
                assert!(p.edit_menu_at((20., 20.)).is_none());
                continue;
            }
            command(&mut p, EditAction::SelectAll);
            let menu = menu(&mut p);
            let readonly = attrs == "readonly=true";
            let password = attrs.starts_with("type=");
            assert_eq!(menu.enabled(EditAction::Copy), !password);
            assert_eq!(menu.enabled(EditAction::Cut), !readonly && !password);
            assert_eq!(menu.enabled(EditAction::Paste), !readonly);
            assert_eq!(menu.enabled(EditAction::Delete), !readonly);
            let before = p.field_text(id);
            p.mark_field(id, &before, FieldSelection::caret(0), false);
            p.edit_menu_action(&menu, EditAction::Paste);
            assert_eq!(
                p.field_text(id),
                before,
                "a stale selection cannot receive a menu command"
            );
            assert_eq!(*clipboard.borrow(), "paste 🦀");
            let fresh = p.make_edit_menu(id, (0., 0.));
            p.set_focus(None, 1.);
            assert!(!p.edit_menu_valid(&fresh));
        }
    }

    #[test]
    fn context_and_key_handlers_cancel_without_swallowing_later_keys() {
        for (blocked, attrs) in [
            (
                "  action block\n    preventDefault()\n",
                "contextmenu=block",
            ),
            (
                "  action block(key: string)\n    preventDefault()\n",
                "key=block",
            ),
        ] {
            let (mut p, id, _) = fixture(blocked, attrs);
            p.hold_modifier("ShiftLeft", true);
            let (handled, menu) = p.edit_menu_key("F10", "F10", false);
            assert!(handled);
            assert!(menu.is_none());
            p.hold_modifier("ShiftLeft", false);
            assert!(!p.edit_menu_key("KeyA", "a", false).0);
            assert_eq!(p.focus, Some(id));
        }
        let (mut p, _, _) = fixture("", "");
        p.hold_modifier("ShiftLeft", true);
        assert!(p.edit_menu_key("F10", "F10", false).1.is_some());
        assert!(p.edit_menu_key("F10", "F10", true).1.is_none());
        p.hold_modifier("ShiftLeft", false);
        assert!(!p.edit_menu_key("F10", "F10", false).0);
    }

    #[test]
    fn right_click_preserves_a_selection_inside_it_and_moves_outside_it() {
        let (mut p, id, _) = fixture("", "");
        command(&mut p, EditAction::SelectAll);
        let selection = p.field_selection(id);
        p.frame();
        let field = p.current_field(id, &p.field_text(id)).unwrap();
        let first = field.caret_frame(crate::text::field::Cursor {
            offset: 1,
            upstream: false,
        });
        let at = (first.0 + 0.1, first.1 + first.3 / 2.);
        assert!(p.edit_menu_at(at).is_some());
        assert_eq!(p.field_selection(id), selection);
        p.mark_field(
            id,
            &p.field_text(id),
            FieldSelection {
                start: 0,
                end: 1,
                ..FieldSelection::caret(0)
            },
            false,
        );
        let outside = p.edit_menu_at((250., 20.)).unwrap();
        assert!(!outside.enabled(EditAction::Copy));
        assert_eq!(
            p.field_selection(id),
            FieldSelection::at_end(&p.field_text(id))
        );
    }

    #[test]
    fn menu_edit_default_obeys_clipboard_and_beforeinput_cancellation() {
        for attrs in ["paste=block", "beforeinput=block"] {
            let (mut p, id, _) = fixture("  action block\n    preventDefault()\n", attrs);
            let before = p.field_text(id);
            command(&mut p, EditAction::SelectAll);
            command(&mut p, EditAction::Paste);
            assert_eq!(p.field_text(id), before, "{attrs}");
            assert!(!menu(&mut p).enabled(EditAction::Undo));
        }
    }
}
