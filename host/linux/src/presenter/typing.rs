//! Text and keyboard input, sharing the presenter's focus owner.
use super::*;

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
        let mut error = None;
        for (event, kind) in [
            (Event::Input(text.into()), EventKind::Input),
            (Event::Change(text.into()), EventKind::Change),
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
        let activation = matches!(code, "Space" | "Enter" | "NumpadEnter")
            && matches!(
                node.props.str(PropId::AccessibilityRole),
                Some("button" | "link")
            );
        if !self.host.route_visibility(id).1 && !editable && code != "Tab" && (!down || !activation)
            && self.surface_input(id, serde_json::json!({"t":"key","code":code,"key":key,"down":down,"repeat":repeat,"at":self.host.now()})) {
            return Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"));
        }
        if !self.focusable(id) || self.host.route_visibility(id).1 {
            return Err(format!("view {id} cannot take focus"));
        }
        if let Some(e) = self.set_focus(Some(id), self.host.now()) {
            return Err(e);
        }
        if down && !(activation && repeat) {
            let name = match key {
                "Space" => " ",
                "NumpadEnter" => "Enter",
                name => name,
            };
            self.key_down(name, self.host.now());
        }
        Ok(format!("{{\"typed\":{id},\"delivery\":\"recognized\"}}"))
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

    /// A key down at the focused node, by the web's name (`e.key`). The
    /// nearest `key` handler at or above it hears it first, as a keydown
    /// bubbles; then its default action: Enter or Space presses a button and
    /// Enter a link; Enter submits a single-line input (its `submit`) or
    /// breaks a textarea's line; Backspace deletes; a character is typed —
    /// each an edit the runner hears as one `change`.
    pub(crate) fn key_down(&mut self, name: &str, now_ms: f64) {
        let Some(id) = self.focus else { return };
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
        if let Some(e) = self.key_event(name, now_ms) {
            eprintln!("exact: {e}");
        }
        // The handler may have moved the focus or removed the node.
        if self.focus != Some(id) {
            return;
        }
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        let role = node.props.str(PropId::AccessibilityRole);
        if role == Some("button") && matches!(name, " " | "Enter")
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
            s if s.chars().count() == 1 => value.push_str(s),
            _ => return,
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
