//! Text and keyboard input, sharing the presenter's focus owner.
use super::*;
use exact_runner::{ControlValue, FieldSelection, InputDetails, SelectionDirection};
use std::collections::BTreeSet;

impl<D: DataSource> Presenter<D> {
    /// Type into a focused field. `input` edits it now; `change` waits
    /// for Enter or blur, as for hardware typing (LLP 1069.001 D4).
    pub fn type_text(&mut self, id: ViewId, text: &str) -> Result<String, String> {
        if !self.alert_allows(id) || self.host.route_visibility(id).1 {
            return Err(format!("view {id} is hidden or inert"));
        }
        let kernel = self.host.kernel();
        let node = kernel.node(id).ok_or_else(|| format!("no view {id}"))?;
        // @ref LLP 1038 D11 — the agent's root text is a location.
        if node.props.str(PropId::NavigationBack).is_some() {
            if node.props.bool(PropId::Disabled) == Some(true) {
                return Err(format!("view {id} is disabled"));
            }
            let error =
                self.host
                    .dispatch_at(id, Event::Navigate(text.to_owned()), self.host.now());
            let after = self.after_commit();
            if let Some(error) = error.or(after) {
                return Err(error);
            }
            return Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"));
        }
        // @ref LLP 1069.001 D9 — a select's value, as a choice sets it.
        if node.node_type == NodeType::Control {
            if node.props.str(PropId::Type) == Some("button") {
                return Err(format!(
                    "view {id} is a button: it takes a press, not a value"
                ));
            }
            // An indeterminate progress has no value (LLP 1069.001, amended
            // 2026-10-07).
            if node.props.str(PropId::Type) == Some("progress") {
                return Err(format!("view {id} is not an input"));
            }
            return self.set_control_value(id, text);
        }
        if node.node_type != NodeType::TextInput {
            return Err(format!("view {id} is not an input"));
        }
        if node.props.bool(PropId::Disabled) == Some(true) {
            return Err(format!("view {id} is disabled"));
        }
        if node.props.bool(PropId::Editable) == Some(false) {
            return Err(format!("view {id} is readonly"));
        }
        if node.props.bool(PropId::EmojiPicker) == Some(true) {
            return Err("emoji selection is not supported on the Linux host".into());
        }
        let now = self.host.now();
        if let Some(e) = self.set_focus(Some(id), now) {
            return Err(e);
        }
        // The web's `focus()` runs its handler's commands before it returns.
        self.run_commands_now();
        self.forget_replaced_choices();
        let value = self.field_text(id);
        let selection = FieldSelection {
            start: 0,
            end: field::utf16_len(&value),
            direction: SelectionDirection::None,
        };
        let moved = self.field_selection(id) != selection;
        self.mark_field(id, &value, selection, true);
        // `select()` then focuses the field it selected (Chromium's
        // `TextControlElement::select`): one a focus handler sent the focus
        // away from takes it back, its handler hearing `focus` again, unless
        // that handler left it unable to take the focus.
        if self.focus() != Some(id) && self.focusable(id) && !self.host.route_visibility(id).1 {
            if let Some(e) = self.set_focus(Some(id), now) {
                return Err(e);
            }
            self.run_commands_now();
        }
        let (value, selection) = (self.field_text(id), self.field_selection(id));
        // Select all, then insert, as a person's select-all and typing: a
        // changed selection's `select` is heard before the insertion (the
        // web driver waits for it), and its handler and the commands it
        // runs may rewrite the field or move its selection.
        let (value, selection) = if moved {
            self.field_select(id, &value, selection, now);
            self.run_commands_now();
            (self.field_text(id), self.field_selection(id))
        } else {
            (value, selection)
        };
        // The insertion goes where the handlers left the focus, as the
        // browser's does: into an enabled, editable text field (cut to the
        // room its maxlength leaves), the one typed into or another, else
        // nowhere.
        let target = self.focus().filter(|&f| {
            self.host.kernel().node(f).is_some_and(|n| {
                n.node_type == NodeType::TextInput
                    && n.props.bool(PropId::Disabled) != Some(true)
                    && n.props.bool(PropId::Editable) != Some(false)
            })
        });
        let Some(target) = target else {
            return Ok(format!("{{\"typed\":{id}}}"));
        };
        let (value, selection) = if target == id {
            (value, selection)
        } else {
            (self.field_text(target), self.field_selection(target))
        };
        let limit = self
            .host
            .kernel()
            .node(target)
            .and_then(|n| exact_kernel::control::text_maxlength(n.props));
        // `beforeinput` carries the whole text; what is inserted, and what
        // `input` carries, is cut to the room left. An empty insertion still
        // deletes a selected range (Chrome's insertText).
        let bound = self.bound_text(target);
        let mut edit = InputDetails {
            input_type: "insertText".into(),
            data: Some(text.to_owned()),
            cancelable: true,
            is_composing: false,
        };
        if self.before_input(target, &edit, now)? {
            // Chromium advances an endpoint inside a surrogate pair past
            // the character before fitting or replacing the selection.
            let boundary = |offset| {
                let mut at = 0;
                for ch in value.chars() {
                    if at >= offset {
                        break;
                    }
                    at += ch.len_utf16() as u32;
                }
                at
            };
            let selection = FieldSelection {
                start: boundary(selection.start),
                end: boundary(selection.end),
                ..selection
            };
            let fitted = field::fit(text, &value, selection, limit);
            let removes =
                field::byte_at(&value, selection.start) != field::byte_at(&value, selection.end);
            let (next, _) = field::replace(&value, selection, fitted);
            // Even a rejected insertion advances by the original text's
            // UTF-16 length, clamped to the remaining value.
            let caret = FieldSelection::caret(
                selection
                    .start
                    .saturating_add(field::utf16_len(text))
                    .min(field::utf16_len(&next)),
            );
            if !fitted.is_empty() || text.is_empty() || removes {
                edit.data = Some(fitted.to_owned());
                edit.cancelable = false;
                if let Some(e) = self.apply_field_edit(target, next, caret, bound, now, edit) {
                    return Err(e);
                }
            } else if caret != self.field_selection(target) {
                self.mark_field(target, &value, caret, false);
            }
        }
        let value = self.field_text(target);
        let mut s = format!("{{\"typed\":{target},\"value\":");
        // A password's value is never agent output (#134).
        let props = self.host.kernel().node(target).map(|n| n.props);
        quote(
            props.map_or(&value[..], |p| exact_runner::agent::shown_value(p, &value)),
            &mut s,
        );
        s.push('}');
        Ok(s)
    }

    /// The commands a handler's commits issue, run as the web runs them,
    /// at once, and theirs in turn; 16 rounds bound a handler that loops.
    fn run_commands_now(&mut self) {
        for _ in 0..16 {
            self.run_queued_commands();
            if self.commands.is_empty() {
                break;
            }
        }
    }

    /// `type <id> copy|cut|paste [text]` (spreadsheet F6): the clipboard's
    /// event with the focus at `id`, heard by the nearest node with a
    /// handler — itself or an ancestor — as on the web; a paste carries
    /// `text` as the clipboard's. This host has no clipboard of its own.
    pub fn clipboard(&mut self, id: ViewId, edit: &str, text: &str) -> Result<String, String> {
        let r = self.clipboard_event(id, edit, text);
        // The paste's chord comes up through the `keyup` handlers last,
        // whatever took its down, as the web driver's (#140).
        if edit == "paste" {
            self.hold_modifier("ControlLeft", true);
            self.key_up("v", "KeyV", self.host.now());
            self.hold_modifier("ControlLeft", false);
            // Control's own keyup after it, without its own bit (Charlie, 2026-10-07).
            self.key_up("Control", "ControlLeft", self.host.now());
        }
        r
    }

    fn clipboard_event(&mut self, id: ViewId, edit: &str, text: &str) -> Result<String, String> {
        let kind = match edit {
            "copy" => EventKind::Copy,
            "cut" => EventKind::Cut,
            "paste" => EventKind::Paste,
            _ => return Err(format!("type: {edit} is not copy, cut or paste")),
        };
        if !self.alert_allows(id) || self.host.route_visibility(id).1 {
            return Err(format!("view {id} is hidden or inert"));
        }
        // A field's own paste inserts with no handler to hear it.
        let field = kind == EventKind::Paste
            && self
                .host
                .kernel()
                .node(id)
                .is_some_and(|n| n.node_type == NodeType::TextInput);
        let mut at = Some(id);
        let target = loop {
            let Some(node) = at.and_then(|n| self.host.kernel().node(n)) else {
                if field {
                    break None;
                }
                return Err(format!("no {edit} handler at view {id} or above it"));
            };
            if self.host.runner().handlers_of(node.id).contains(&kind)
                && node.props.bool(PropId::Disabled) != Some(true)
            {
                break Some(node.id);
            }
            at = node.parent;
        };
        let now = self.host.now();
        if self.focusable(id) {
            if let Some(e) = self.set_focus(Some(id), now) {
                return Err(e);
            }
        }
        // A paste is Ctrl+V first. A `key` handler that preventDefault()s
        // that chord keeps the clipboard event from landing (drums: the
        // driver's paste skipped the key and hid that bug). Copy and cut
        // stay the clipboard event alone.
        if kind == EventKind::Paste {
            self.hold_modifier("ControlLeft", true);
            // Control's own keydown first, as a keyboard's (Charlie, 2026-10-07).
            let (error, _) = self.key_event(EventKind::Key, "Control", "ControlLeft", false, now);
            if let Some(e) = error {
                self.hold_modifier("ControlLeft", false);
                return Err(e);
            }
            // The page's shortcuts hear the chord first, as the web's capture
            // listener does: a button declaring Control+V takes it.
            let (error, prevented) = if self.shortcut("v", false, now) {
                (None, true)
            } else {
                self.key_event(EventKind::Key, "v", "KeyV", false, now)
            };
            self.hold_modifier("ControlLeft", false);
            if let Some(e) = error {
                return Err(e);
            }
            if prevented {
                return Ok(format!(
                    "{{\"typed\":{id},\"clipboard\":\"{edit}\",\"delivery\":\"recognized\"}}"
                ));
            }
        }
        let text = if kind == EventKind::Paste { text } else { "" };
        if let Some(target) = target {
            let error = self.host.dispatch_at(
                target,
                Event::Clipboard(kind, text.to_owned(), Vec::new()),
                now,
            );
            if let Some(e) = error.or(self.after_commit()) {
                return Err(e);
            }
        }
        // The paste's default action: the text in at the field's selection
        // (x2apps codeedit #2), after its listeners, as the web's.
        let queued = self.commands.len();
        self.commands.retain(|c| c.name != "preventDefault");
        if field && queued == self.commands.len() {
            self.paste_field(id, text, now);
        }
        Ok(format!(
            "{{\"typed\":{id},\"clipboard\":\"{edit}\",\"delivery\":\"recognized\"}}"
        ))
    }

    /// Targeted keyboard input for both the agent and device adapters.
    pub fn type_key(
        &mut self,
        id: ViewId,
        code: &str,
        key: &str,
        down: bool,
        repeat: bool,
    ) -> Result<String, String> {
        if down && !self.alert_allows(id) {
            return Err(format!("view {id} is inert under the open surface"));
        }
        if down {
            self.key_shows_ring(key);
        }
        self.restore_controls();
        // A release's `keyup` handlers at the focus first (#140), whatever
        // took its down; then the release goes where its press went.
        if !down {
            self.key_up(key, code, self.host.now());
        }
        let contact = if code == "Space" {
            u32::MAX - 1
        } else {
            u32::MAX - 2
        };
        if !down
            && matches!(code, "Space" | "Enter" | "NumpadEnter")
            && self.owns_control(id, contact)
        {
            return if self.control_input(id, "up", 0., 0., contact, self.host.now()) {
                Ok(format!("{{\"typed\":{id}}}"))
            } else {
                Err("control release refused".into())
            };
        }
        // Tab's release goes wherever its press moved the focus, and has no
        // default; refocusing the target would undo the move (LLP 1088 D7.3).
        if !down && code == "Tab" {
            return Ok(format!("{{\"typed\":{id}}}"));
        }
        let node = self
            .host
            .kernel()
            .node(id)
            .ok_or_else(|| format!("no view {id}"))?;
        if crate::surfaces::controls::disabled_control(&node) || self.host.route_visibility(id).1 {
            return Err(format!("view {id} is disabled or inert"));
        }
        if node.props.str(PropId::Action).is_some()
            && matches!(code, "Space" | "Enter" | "NumpadEnter")
        {
            if !self.holds_control(id) {
                if let Some(e) = self.set_focus(Some(id), self.host.now()) {
                    return Err(e);
                }
            }
            let (x, y, _, _) = self.rect_of(id).ok_or("control has no box")?;
            return if self.control_input(
                id,
                if down { "down" } else { "up" },
                x,
                y,
                if code == "Space" {
                    u32::MAX - 1
                } else {
                    u32::MAX - 2
                },
                self.host.now(),
            ) {
                Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"))
            } else {
                Err(format!("control {id} refused input"))
            };
        }
        let editable = node.node_type == NodeType::TextInput;
        // A native button activates under any role, as `key_down` presses it.
        let activation = matches!(code, "Space" | "Enter" | "NumpadEnter")
            && (matches!(
                node.props.str(PropId::AccessibilityRole),
                Some("button" | "link")
            ) || exact_kernel::ControlKind::of(node.node_type, node.props)
                == Some(exact_kernel::ControlKind::Button));
        if !self.host.route_visibility(id).1
            && !editable
            && code != "Tab"
            && (!down || !activation)
            && self.input_surface(id).is_some()
        {
            // A world's key goes the keyboard's way on the web first: an
            // `aria-keyshortcuts` button takes it (b6 review B1; the web's
            // capture listener, macOS's `routeKey`), then the `key` handlers
            // at the target and above it hear the down, and one that
            // prevents it keeps it from the world (the platformer's diary,
            // R8). A shortcut's key, down and up, reaches neither.
            if !down && self.shortcut_keys.remove(code) {
                return Ok(format!(
                    "{{\"typed\":{id},\"delivery\":\"recognized\",\"shortcut\":true}}"
                ));
            }
            if down {
                let name = if key == "Space" { " " } else { key };
                if self.shortcut(name, repeat, self.host.now()) {
                    self.shortcut_keys.insert(code.to_string());
                    return Ok(format!(
                        "{{\"typed\":{id},\"delivery\":\"recognized\",\"shortcut\":true}}"
                    ));
                }
            }
            let heard = if down {
                self.canvas_key(id, key, code, repeat)
            } else {
                None
            };
            if heard == Some(true)
                || self.surface_input(id, serde_json::json!({"t":"key","code":code,"key":key,"down":down,"repeat":repeat,"at":self.host.now()}))
                || heard.is_some()
            {
                return Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"));
            }
        }
        // A target that takes no focus leaves it where it is, as the web's
        // `focus()` on one does: the key goes to whatever holds the focus,
        // or to the page's shortcuts when nothing does (pomodoro F5). A
        // surface that refused the key above keeps it refused.
        if !self.focusable(id) && self.input_surface(id).is_some() {
            return Err(format!("view {id} cannot take focus"));
        }
        // A release goes where its press left the focus: a radio's arrow
        // moves it (x2apps survey #2), as Tab's does.
        if down && self.focusable(id) {
            if let Some(e) = self.set_focus(Some(id), self.host.now()) {
                return Err(e);
            }
        }
        if down {
            let name = match key {
                "Space" => " ",
                "NumpadEnter" => "Enter",
                name => name,
            };
            if self.shortcut(name, repeat, self.host.now()) {
                return Ok(format!(
                    "{{\"typed\":{id},\"delivery\":\"recognized\",\"shortcut\":true}}"
                ));
            }
            if !(activation && repeat) {
                self.key_down_with(name, code, repeat, self.host.now());
            }
        }
        Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"))
    }

    /// A key down at a world's canvas (or a node inside one) for the `key`
    /// handlers there and above, the target focused first as a keyboard's
    /// focus is: Some(prevented) when they heard it, None when the target
    /// takes no focus (the world alone has the key, as before).
    fn canvas_key(&mut self, id: ViewId, key: &str, code: &str, repeat: bool) -> Option<bool> {
        if self.focus != Some(id) {
            if !self.focusable(id) {
                return None;
            }
            if let Some(e) = self.set_focus(Some(id), self.host.now()) {
                eprintln!("exact: {e}");
            }
        }
        if self.focus != Some(id) {
            return None;
        }
        let name = if key == "Space" { " " } else { key };
        let (error, prevented) =
            self.key_event(EventKind::Key, name, code, repeat, self.host.now());
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
        Some(prevented)
    }

    /// A Tab stop (LLP 1088 D7.3): focusable, and an explicit `tabindex` ≥ 0
    /// or none at all — a negative one is focusable but skipped — shown,
    /// not inert, with a box (display: none and `hidden` have none).
    pub(super) fn tabbable(&self, id: ViewId, boxed: &BTreeSet<ViewId>) -> bool {
        self.focus_stop(id, boxed)
            && self.radio_tab_stop(id)
            && (!self.alert_message(id) || self.alert_message_overflows(id))
    }

    /// A Tab stop but for HTML's one stop to a radio group: every enabled
    /// radio is one. A TV remote's stops (LLP 1115.001.003 D4).
    pub(super) fn focus_stop(&self, id: ViewId, boxed: &BTreeSet<ViewId>) -> bool {
        let explicit = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.get(PropId::TabIndex).and_then(|v| v.as_int()));
        self.focus_refusal(id).is_none()
            && explicit.is_none_or(|i| i >= 0)
            && self.focusable(id)
            && (boxed.contains(&id) || self.inline_box(id).is_some())
    }

    /// Tab's default action, HTML's sequential focus navigation (LLP 1088
    /// D7.3): the Tab stops in tree order, positive `tabindex` values first
    /// in ascending order; from the focus to the next (Shift: the previous),
    /// wrapping, and from no focus, or one out of the order, to the first
    /// (Shift: the last), or from the starting point a fragment set, to the
    /// first stop after it (Shift: the last before it; LLP 1153 D5). A
    /// `key` handler's `preventDefault()` keeps Tab.
    pub(crate) fn move_focus(&mut self, backward: bool, now_ms: f64) -> Option<String> {
        self.move_focus_filtered(backward, now_ms, |_, _| true)
    }

    pub(super) fn move_focus_filtered(
        &mut self,
        backward: bool,
        now_ms: f64,
        include: impl Fn(&Self, ViewId) -> bool,
    ) -> Option<String> {
        let boxed = self
            .shown_boxes()
            .into_iter()
            .filter(|id| include(self, *id))
            .collect();
        self.move_focus_in(backward, now_ms, &boxed)
    }

    pub(super) fn move_focus_in(
        &mut self,
        backward: bool,
        now_ms: f64,
        boxed: &BTreeSet<ViewId>,
    ) -> Option<String> {
        let mut order: Vec<(i64, usize, ViewId)> = Vec::new();
        let mut start = None;
        for (at, id) in self.host.preorder().into_iter().enumerate() {
            if Some(id) == self.focus_start {
                start = Some(at);
            }
            // A focused node inside an open popover that is no stop goes on
            // from its place in tree order, within the popover's scope
            // (HTML's sequential navigation starting point).
            if Some(id) == self.focus && !self.tabbable(id, boxed) && self.in_popover(id) {
                order.push((i64::MAX, at, id));
                continue;
            }
            if self.tabbable(id, boxed) {
                let index = self
                    .host
                    .kernel()
                    .node(id)
                    .and_then(|n| n.props.get(PropId::TabIndex).and_then(|v| v.as_int()))
                    .filter(|&i| i > 0)
                    .unwrap_or(i64::MAX);
                order.push((index, at, id));
            }
        }
        order.sort_unstable();
        let order = self.alert_tab_order(order);
        let n = order.len();
        let placed = self
            .focus
            .filter(|f| !self.tabbable(*f, boxed) && self.in_popover(*f));
        let after = |s: usize| order.iter().position(|o| o.0 == i64::MAX && o.1 > s);
        let before = |s: usize| order.iter().rposition(|o| o.0 == i64::MAX && o.1 < s);
        let next = match self.focus.and_then(|f| order.iter().position(|o| o.2 == f)) {
            _ if n == 0 || n == 1 && placed.is_some() => return None,
            Some(i) if backward => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
            None if backward => start.and_then(before).unwrap_or(n - 1),
            None => start.and_then(after).unwrap_or(0),
        };
        self.set_focus(Some(order[next].2), now_ms)
    }

    /// The nodes with a box of some size that the display shows: what a Tab
    /// stop needs ([`Self::tabbable`]).
    pub(super) fn shown_boxes(&mut self) -> BTreeSet<ViewId> {
        self.boxes();
        self.boxes
            .iter()
            .filter(|b| b.rect.2 > 0.0 && b.rect.3 > 0.0)
            .filter(|b| self.display.allows(self.host.kernel(), b.id))
            .map(|b| b.id)
            .collect()
    }

    /// A key from the display's keyboard: a character, Enter, or Backspace.
    pub fn key(&mut self, ch: Option<char>, backspace: bool, now_ms: f64) {
        let name = match (ch, backspace) {
            (_, true) => "Backspace".to_string(),
            (Some('\n' | '\r'), _) => "Enter".to_string(),
            (Some(c), _) => c.to_string(),
            (None, _) => return,
        };
        self.key_down(&name, now_ms);
    }

    /// A key down at the focused node, by the web's name (`e.key`). Every
    /// `key` handler at or above it hears it first, as a keydown bubbles;
    /// then, unless one called `preventDefault()`, its default action: Enter or Space presses a button and
    /// Enter a link; Enter submits a single-line input (its `submit`) or
    /// breaks a textarea's line; Backspace deletes; a character is typed —
    /// each an edit the runner hears as one `change`.
    pub(crate) fn key_down(&mut self, name: &str, now_ms: f64) {
        self.key_down_with(name, "", false, now_ms);
    }

    /// A key shows the ring a press left off, as Chrome's `:focus-visible`
    /// starts to match on a keydown that holds none of Ctrl, Alt or Meta
    /// (Shift alone counts), before any handler or shortcut hears it. On
    /// Windows any key counts (`winui_focus`): WinUI's keyboard processor
    /// marks the keyboard as the last input for a modifier too, as Windows
    /// has shown hidden focus rectangles when Alt is pressed
    /// (`WM_UPDATEUISTATE`).
    pub(crate) fn key_shows_ring(&mut self, name: &str) {
        let held = self.modifiers();
        let winui = self.brush.winui_focus;
        if winui
            || (!(held.ctrl || held.alt || held.meta)
                && !matches!(name, "Control" | "Alt" | "Meta" | "AltGraph"))
        {
            self.brush.keyless = false;
            if self.brush.ringless.take().is_some() {
                self.dirty = true;
            }
        }
    }

    /// [`Self::key_down`] with the key's physical position (`code`) and
    /// whether it is an auto-repeat, which its `KeyboardEvent` carries.
    pub(crate) fn key_down_with(&mut self, name: &str, code: &str, repeat: bool, now_ms: f64) {
        // A hatch observes the key after its handlers and its default
        // action (LLP 1075.003.000.001 §2.2.2).
        let mark = self.hatch_key_mark();
        // A key on a held node (the focus there) lets its hold go (LLP 1122 D8).
        self.hold_input(self.focus);
        self.key_shows_ring(name);
        self.key_pressed(name, code, repeat, now_ms);
        self.hatch_key(mark, name, code, true, repeat);
    }

    fn key_pressed(&mut self, name: &str, code: &str, repeat: bool, now_ms: f64) {
        // A held Select chooses nothing in a surface it opened either
        // (LLP 1115.001.003 D3).
        if repeat && code == "Select" && self.page_inert() {
            return;
        }
        // An open surface of the top layer takes every key (`top.rs`).
        if self.alert_key(name, code, repeat, now_ms) || self.surface_key(name, now_ms) {
            return;
        }
        // The page's shortcuts first, focus or none (`shortcuts.rs`); a
        // repeat is the shortcut's, and presses nothing.
        if self.shortcut(name, repeat, now_ms) {
            return;
        }
        // Then a desktop's Back key (LLP 1115.000 D5.4), focus or none.
        if self.back_key(name) {
            self.back_key_down(name, code, repeat, now_ms);
            return;
        }
        let Some(id) = self.focus else {
            // With no focus the root's handlers hear the key; then, unless
            // one prevented it, Tab takes the first stop (LLP 1088 D7.3), and
            // so does a remote's arrow (LLP 1115.001.003 D4).
            let (error, prevented) = self.key_event(EventKind::Key, name, code, repeat, now_ms);
            if let Some(e) = error {
                eprintln!("exact: {e}");
            }
            if prevented || self.focus.is_some() {
                return;
            }
            self.key_default(name, code, repeat, now_ms);
            return;
        };
        // A bar's target is no box of the page's: the remote's keys there are
        // the remote's alone (LLP 1115.001.003 D4).
        if self.remote() && self.bar_node(id) {
            self.remote_key(id, (name, code, repeat), now_ms);
            return;
        }
        if !self.display.allows(self.host.kernel(), id) {
            return;
        }
        if self
            .host
            .kernel()
            .node(id)
            .is_none_or(|n| crate::surfaces::controls::disabled_control(&n))
        {
            self.remote_leaves_disabled(id, name, now_ms);
            return;
        }
        // A grouped grip's keys move its row (LLP 1094 D9).
        if self.group_key(id, name, now_ms) {
            return;
        }
        let (error, prevented) = self.key_event(EventKind::Key, name, code, repeat, now_ms);
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
        // A handler may have prevented the default, moved the focus or removed the node.
        if prevented || self.focus != Some(id) {
            return;
        }
        self.key_default(name, code, repeat, now_ms);
    }

    /// Default keyboard behavior shared by the page and authored top-layer boxes.
    pub(super) fn key_default(&mut self, name: &str, code: &str, repeat: bool, now_ms: f64) {
        // Tab's default action moves the focus, from a canvas or a field too,
        // as the browser's does (LLP 1088 D7.3).
        if name == "Tab" {
            if let Some(e) = self.move_focus(self.modifiers().shift, now_ms) {
                eprintln!("exact: {e}");
            }
            return;
        }
        let Some(id) = self.focus else {
            if let Some(d) = remote::Direction::of(name).filter(|_| self.remote()) {
                self.move_focus_toward(d, now_ms);
            } else {
                self.desktop_scroll_key(name);
            }
            return;
        };
        // A remote's Select and arrows (LLP 1115.001.003 D3).
        if self.remote_key(id, (name, code, repeat), now_ms) {
            return;
        }
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        let role = node.props.str(PropId::AccessibilityRole);
        // A native button presses under any role, a tab's or a menu item's
        // (LLP 1069.011.000 D1); any other pressable as a button does, unless
        // it is a link (chat F14).
        let native = exact_kernel::ControlKind::of(node.node_type, node.props)
            == Some(exact_kernel::ControlKind::Button);
        let pressable = node.node_type != NodeType::TextInput
            && self
                .host
                .runner()
                .handlers_of(id)
                .contains(&EventKind::Press);
        if (role == Some("button") || native || pressable && role != Some("link"))
            && matches!(name, " " | "Enter")
            || role == Some("link") && name == "Enter"
        {
            self.note_press(id, true);
            self.activate_with(id, now_ms, false);
            return;
        }
        // A radio's arrows and Space (x2apps survey #2).
        if self.radio_key(id, name, now_ms) {
            return;
        }
        if self.desktop_scroll_key(name) {
            return;
        }
        // The field's own keys: the caret and selection, editing the text it
        // shows (typed text kept against its bound value; LLP 1069.001 D4).
        self.field_key(id, name, now_ms);
    }

    /// Commit a field typed into since it took the focus: HTML's `change`,
    /// on blur or Enter (LLP 1069.001 D4). `None` when nothing was
    /// dispatched; else the dispatch's error, if any.
    pub(crate) fn commit_text(&mut self, id: ViewId, now_ms: f64) -> Option<Option<String>> {
        if self.edited != Some(id) || self.ime_owns(id) {
            return None;
        }
        self.edited = None;
        if !self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Change)
        {
            return None;
        }
        let value = self.field_text(id);
        let selection = self.reported_selection(id, &value, self.field_selection(id));
        Some(self.host.dispatch_at(
            id,
            Event::Change(ControlValue::Field(value, selection)),
            now_ms,
        ))
    }

    /// A choice or typed text whose bound value any commit since changed is
    /// gone, as the web build's write of `value` replaces it (LLP 1069.001
    /// D4): the value coming back does not bring it back.
    pub(crate) fn forget_replaced_choices(&mut self) -> std::collections::BTreeSet<ViewId> {
        let replaced = self.host.values.take_replaced();
        for id in &replaced {
            self.chosen.remove(id);
        }
        replaced
    }

    /// A field's committed `value`.
    pub(crate) fn bound_text(&self, id: ViewId) -> String {
        self.host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_string))
            .unwrap_or_default()
    }

    /// What a field shows and commits: its typed text while its bound
    /// value is the one it had then (`keep_typed`), else that value.
    pub(crate) fn field_text(&self, id: ViewId) -> String {
        if let Some(value) = self.ime_value(id) {
            return value.to_owned();
        }
        let Some(node) = self.host.kernel().node(id) else {
            return String::new();
        };
        crate::paint::control::choice(&node, self.chosen.get(&id))
            .unwrap_or_else(|| node.props.str(PropId::Value).unwrap_or(""))
            .to_string()
    }

    /// After an edit's dispatch: a field whose `value` the edit's action did
    /// not write keeps the typed text, as the web's element keeps its own
    /// value and the web build writes `value` only when the bound value
    /// changes (LLP 1069.001 D4: an unbound field holds its own text).
    pub(crate) fn keep_typed(&mut self, id: ViewId, typed: String, before: String) {
        // No handler need hear an edit for it to show.
        self.dirty = true;
        // A commit since the edit began (the watch set before its dispatch) that changed the
        // value replaces the typed text, whatever it reads now.
        let written = self.forget_replaced_choices().contains(&id);
        let bound = self.bound_text(id);
        if !written && self.host.kernel().node(id).is_some() && bound == before && bound != typed {
            self.host.values.watch(id, Some(bound.clone()));
            self.chosen.insert(id, (typed, bound));
        } else {
            self.host.values.watch(id, None);
            self.chosen.remove(&id);
        }
    }
}
