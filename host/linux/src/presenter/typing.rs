//! Text and keyboard input, sharing the presenter's focus owner.
use super::*;
use std::collections::BTreeSet;

impl<D: DataSource> Presenter<D> {
    /// Set an input's value as typing does and commit it: focused, the
    /// value replaced, an `input` then a `change` heard by the runner, each
    /// where the node has a handler for it (LLP 1069.001 D4).
    pub fn type_text(&mut self, id: ViewId, text: &str) -> Result<String, String> {
        if self.host.route_visibility(id).1 {
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
        let text = exact_kernel::control::limit_text(node.props, text).to_string();
        let now = self.host.now();
        if let Some(e) = self.set_focus(Some(id), now) {
            return Err(e);
        }
        let mut error = None;
        for (event, kind) in [
            (Event::Input(text.clone().into()), EventKind::Input),
            (Event::Change(text.clone().into()), EventKind::Change),
        ] {
            if self.host.runner().handlers_of(id).contains(&kind) {
                error = error.or(self.host.dispatch_at(id, event, now));
            }
        }
        self.edited = None;
        let e = self.after_commit();
        if let Some(e) = error.or(e) {
            return Err(e);
        }
        let value = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_string))
            .unwrap_or_default();
        let mut s = format!("{{\"typed\":{id},\"value\":");
        quote(&value, &mut s);
        s.push('}');
        Ok(s)
    }

    /// `type <id> copy|cut|paste [text]` (spreadsheet F6): the clipboard's
    /// event with the focus at `id`, heard by the nearest node with a
    /// handler — itself or an ancestor — as on the web; a paste carries
    /// `text` as the clipboard's. This host has no clipboard of its own.
    pub fn clipboard(&mut self, id: ViewId, edit: &str, text: &str) -> Result<String, String> {
        let kind = match edit {
            "copy" => EventKind::Copy,
            "cut" => EventKind::Cut,
            "paste" => EventKind::Paste,
            _ => return Err(format!("type: {edit} is not copy, cut or paste")),
        };
        if self.host.route_visibility(id).1 {
            return Err(format!("view {id} is hidden or inert"));
        }
        let mut at = Some(id);
        let target = loop {
            let Some(node) = at.and_then(|n| self.host.kernel().node(n)) else {
                return Err(format!("no {edit} handler at view {id} or above it"));
            };
            if self.host.runner().handlers_of(node.id).contains(&kind)
                && node.props.bool(PropId::Disabled) != Some(true)
            {
                break node.id;
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
            // The page's shortcuts hear the chord first, as the web's capture
            // listener does: a button declaring Control+V takes it.
            let (error, prevented) = if self.shortcut("v", false, now) {
                (None, true)
            } else {
                self.key_event("v", now)
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
        let error = self
            .host
            .dispatch_at(target, Event::Clipboard(kind, text.to_owned()), now);
        if let Some(e) = error.or(self.after_commit()) {
            return Err(e);
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
        self.restore_controls();
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
        if node.props.bool(PropId::Disabled) == Some(true) || self.host.route_visibility(id).1 {
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
        if !self.host.route_visibility(id).1 && !editable && code != "Tab" && (!down || !activation)
            && self.surface_input(id, serde_json::json!({"t":"key","code":code,"key":key,"down":down,"repeat":repeat,"at":self.host.now()})) {
            return Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"));
        }
        // A target that takes no focus leaves it where it is, as the web's
        // `focus()` on one does: the key goes to whatever holds the focus,
        // or to the page's shortcuts when nothing does (pomodoro F5). A
        // surface that refused the key above keeps it refused.
        if !self.focusable(id) && self.input_surface(id).is_some() {
            return Err(format!("view {id} cannot take focus"));
        }
        if self.focusable(id) {
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
                self.key_down(name, self.host.now());
            }
        }
        Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"))
    }

    /// A Tab stop (LLP 1088 D7.3): focusable, and an explicit `tabindex` ≥ 0
    /// or none at all — a negative one is focusable but skipped — shown,
    /// not inert, with a box (display: none and `hidden` have none).
    fn tabbable(&self, id: ViewId, boxed: &BTreeSet<ViewId>) -> bool {
        let explicit = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.get(PropId::TabIndex).and_then(|v| v.as_int()));
        explicit.is_none_or(|i| i >= 0)
            && self.focusable(id)
            && self.host.route_visibility(id) == (false, false)
            && boxed.contains(&id)
    }

    /// Tab's default action, HTML's sequential focus navigation (LLP 1088
    /// D7.3): the Tab stops in tree order, positive `tabindex` values first
    /// in ascending order; from the focus to the next (Shift: the previous),
    /// wrapping, and from no focus, or one out of the order, to the first
    /// (Shift: the last). A `key` handler's `preventDefault()` keeps Tab.
    pub(crate) fn move_focus(&mut self, backward: bool, now_ms: f64) -> Option<String> {
        self.boxes();
        let boxed: BTreeSet<ViewId> = self
            .boxes
            .iter()
            .filter(|b| b.rect.2 > 0.0 && b.rect.3 > 0.0)
            .filter(|b| self.display.allows(self.host.kernel(), b.id))
            .map(|b| b.id)
            .collect();
        let mut order: Vec<(i64, usize, ViewId)> = Vec::new();
        for (at, id) in self.host.preorder().into_iter().enumerate() {
            if self.tabbable(id, &boxed) {
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
        let n = order.len();
        let next = match self.focus.and_then(|f| order.iter().position(|o| o.2 == f)) {
            _ if n == 0 => return None,
            Some(i) if backward => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
            None if backward => n - 1,
            None => 0,
        };
        self.set_focus(Some(order[next].2), now_ms)
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
        // The page's shortcuts first, focus or none (`shortcuts.rs`).
        if self.shortcut(name, false, now_ms) {
            return;
        }
        let Some(id) = self.focus else {
            // From no focus, Tab takes the first stop (LLP 1088 D7.3).
            if name == "Tab" {
                if let Some(e) = self.move_focus(self.modifiers().shift, now_ms) {
                    eprintln!("exact: {e}");
                }
            }
            return;
        };
        if !self.display.allows(self.host.kernel(), id) {
            return;
        }
        if self
            .host
            .kernel()
            .node(id)
            .is_none_or(|n| n.props.bool(PropId::Disabled) == Some(true))
        {
            return;
        }
        let (error, prevented) = self.key_event(name, now_ms);
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
        // A handler may have prevented the default, moved the focus or removed the node.
        if prevented || self.focus != Some(id) {
            return;
        }
        // Tab's default action moves the focus, from a canvas or a field too,
        // as the browser's does (LLP 1088 D7.3).
        if name == "Tab" {
            if let Some(e) = self.move_focus(self.modifiers().shift, now_ms) {
                eprintln!("exact: {e}");
            }
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
            self.dispatch_press(id, now_ms, false);
            return;
        }
        if node.node_type != NodeType::TextInput || node.props.bool(PropId::Editable) == Some(false)
        {
            return;
        }
        let textarea = node.props.str(PropId::SemanticTag) == Some("textarea");
        let mut value = node.props.str(PropId::Value).unwrap_or("").to_string();
        match name {
            "Enter" if !textarea => {
                if let Some(Some(e)) = self.commit_text(id, now_ms) {
                    eprintln!("exact: {e}");
                }
                if let Some(e) = self.submit_event(id, now_ms) {
                    eprintln!("exact: {e}");
                }
                return;
            }
            "Enter" => value.push('\n'),
            "Backspace" => {
                if value.pop().is_none() {
                    return;
                }
            }
            // A Control or Meta chord types nothing, as in a browser.
            s if s.chars().count() == 1 && self.held & 0b1100_1100 == 0 => value.push_str(s),
            _ => return,
        }
        if exact_kernel::control::text_maxlength(node.props).is_some_and(|limit| {
            value.encode_utf16().count() > limit
                && value.encode_utf16().count()
                    > node
                        .props
                        .str(PropId::Value)
                        .unwrap_or("")
                        .encode_utf16()
                        .count()
        }) {
            return;
        }
        self.edited = Some(id);
        if self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Input)
        {
            if let Some(e) = self
                .host
                .dispatch_at(id, Event::Input(value.into()), now_ms)
            {
                eprintln!("exact: {e}");
            }
            if let Some(e) = self.after_commit() {
                eprintln!("exact: {e}");
            }
        }
    }

    /// Commit a field typed into since it took the focus: HTML's `change`,
    /// on blur or Enter (LLP 1069.001 D4). `None` when nothing was
    /// dispatched; else the dispatch's error, if any.
    pub(crate) fn commit_text(&mut self, id: ViewId, now_ms: f64) -> Option<Option<String>> {
        if self.edited != Some(id) {
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
        let value = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_string))
            .unwrap_or_default();
        Some(
            self.host
                .dispatch_at(id, Event::Change(value.into()), now_ms),
        )
    }
}
