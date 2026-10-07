//! The events beyond press and change (LLP 1005 §3), as the web and Apple
//! hosts dispatch them: `focus` and `blur` as the focus moves from node to
//! node, `key` and `keyup` by the web's name at the focused node with the
//! physical key (`code`) and auto-repeat (#140), `submit` for Enter in a
//! single-line input, and `hover` in and out as the pointer crosses nodes.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// CSS cursor at the current painted hit. Hosts choose the system cursor;
    /// `auto` leaves their existing control/default behavior intact.
    pub fn cursor_at(&mut self, x: f32, y: f32) -> exact_kernel::Cursor {
        self.hit(x, y)
            .and_then(|id| self.host.kernel().node(id))
            .map(|node| {
                node.computed_style(exact_kernel::StyleMask::INHERITED)
                    .cursor
            })
            .unwrap_or_default()
    }

    /// `blur()` drops the focus; `blur(id)` only when that node holds it.
    pub(crate) fn blur_command(&mut self, args: &[exact_plan::Value]) {
        let holds = |name: &str| {
            self.focus
                .and_then(|id| self.host.kernel().node(id))
                .is_some_and(|n| n.props.str(PropId::Id) == Some(name))
        };
        if args
            .first()
            .and_then(exact_plan::Value::as_str)
            .is_none_or(holds)
        {
            self.blur();
        }
    }

    /// `focus(id)` from an action: the node whose `id` that is, when it is
    /// focusable, as `element.focus()` takes it on the web; otherwise the
    /// journal says why, as the web host's does (it had been an unknown
    /// command here).
    /// The node it focused, if it did.
    pub(crate) fn focus_command(&mut self, args: &[exact_plan::Value]) -> Option<ViewId> {
        let Some(name) = args.first().and_then(exact_plan::Value::as_str) else {
            eprintln!("exact: focus requires an element id");
            return None;
        };
        let kernel = self.host.kernel();
        let found = kernel
            .rows(None)
            .unwrap_or_default()
            .into_iter()
            .map(|row| row.id)
            .find(|&id| {
                kernel
                    .node(id)
                    .is_some_and(|n| n.props.str(PropId::Id) == Some(name))
            });
        let reason = match found {
            None => "no live node with that id",
            Some(id) if !self.focusable(id) => "not focusable",
            Some(id) => {
                if let Some(e) = self.set_focus(Some(id), self.host.now()) {
                    eprintln!("exact: {e}");
                }
                return (self.focus == Some(id)).then_some(id);
            }
        };
        self.host.log(format!("focus \"{name}\" refused: {reason}"));
        None
    }

    /// Move the focus: `blur` at the node that loses it, then `focus` at the
    /// node that gains it — each at its own handler, since the web's focus
    /// events do not bubble.
    pub(crate) fn set_focus(&mut self, next: Option<ViewId>, now_ms: f64) -> Option<String> {
        let previous = self.focus;
        if previous == next {
            return None;
        }
        self.focus = next;
        self.dirty = true;
        // A typed field commits as it loses the focus, before its `blur`.
        let (mut error, mut dispatched) = match previous {
            Some(id) => match self.commit_text(id, now_ms) {
                Some(result) => (result, true),
                None => (None, false),
            },
            None => (None, false),
        };
        for (id, event, kind) in [
            (previous, Event::Blur, EventKind::Blur),
            (next, Event::Focus, EventKind::Focus),
        ] {
            if let Some(id) = id.filter(|&id| self.host.runner().handlers_of(id).contains(&kind)) {
                error = error.or(self.host.dispatch_at(id, event, now_ms));
                dispatched = true;
            }
        }
        if dispatched {
            error = error.or(self.after_commit());
        }
        error
    }

    /// A modifier key went down or up, by its code (`ShiftLeft`…): what
    /// the next key's event says is held.
    pub(crate) fn hold_modifier(&mut self, code: &str, down: bool) {
        let bit = match code {
            "ShiftLeft" => 1,
            "ShiftRight" => 2,
            "ControlLeft" => 4,
            "ControlRight" => 8,
            "AltLeft" => 16,
            "AltRight" => 32,
            "MetaLeft" => 64,
            "MetaRight" => 128,
            _ => return,
        };
        if down {
            self.held |= bit;
        } else {
            self.held &= !bit;
        }
    }

    /// The modifiers held, as a `key` event carries them.
    pub(crate) fn modifiers(&self) -> exact_runner::KeyModifiers {
        exact_runner::KeyModifiers {
            shift: self.held & 3 != 0,
            ctrl: self.held & 12 != 0,
            alt: self.held & 48 != 0,
            meta: self.held & 192 != 0,
        }
    }

    /// A key at the focused node, by the web's name, with its physical key
    /// (`code`, "" when unknown) and whether it is an auto-repeat: every
    /// `key` handler (`kind`; `keyup` for a release, #140) at or above it
    /// hears it, innermost first, as a keydown bubbles — the path fixed
    /// before the first runs; one that called `stopPropagation()` is the
    /// last. True when one called `preventDefault()`: the caller skips the
    /// key's default action (docs/contract-grammar.md#events).
    pub(crate) fn key_event(
        &mut self,
        kind: EventKind,
        name: &str,
        code: &str,
        repeat: bool,
        now_ms: f64,
    ) -> (Option<String>, bool) {
        let mut path = Vec::new();
        let mut at = self.focus.and_then(|id| self.handler_target(id, kind));
        while let Some(id) = at {
            path.push(id);
            at = self
                .host
                .kernel()
                .node(id)
                .and_then(|n| n.parent)
                .and_then(|p| self.handler_target(p, kind));
        }
        let (mut error, mut prevented) = (None, false);
        for id in path {
            let key = exact_runner::KeyboardEvent {
                key: name.to_owned(),
                code: code.to_owned(),
                repeat: repeat && kind == EventKind::Key,
                held: self.modifiers(),
            };
            let event = if kind == EventKind::Keyup {
                Event::Keyup(key)
            } else {
                Event::Key(key)
            };
            error = error.or(self
                .host
                .dispatch_at(id, event, now_ms)
                .or(self.after_commit()));
            let queued = self.commands.len();
            self.commands.retain(|c| c.name != "preventDefault");
            prevented |= self.commands.len() != queued;
            // `stopPropagation()`: no ancestor hears it (files diary F8).
            let queued = self.commands.len();
            self.commands.retain(|c| c.name != "stopPropagation");
            if self.commands.len() != queued {
                break;
            }
        }
        (error, prevented)
    }

    /// A key's release at the focus: its `keyup` handlers (#140), by the
    /// name its keydown had (`Space` is " "). A release has no default here.
    pub(crate) fn key_up(&mut self, key: &str, code: &str, now_ms: f64) {
        let name = match key {
            "Space" => " ",
            "NumpadEnter" => "Enter",
            name => name,
        };
        if let Some(e) = self
            .key_event(EventKind::Keyup, name, code, false, now_ms)
            .0
        {
            eprintln!("exact: {e}");
        }
    }

    /// Enter in a single-line input: the web's implicit submission, at the
    /// input's own `submit` handler.
    pub(crate) fn submit_event(&mut self, input: ViewId, now_ms: f64) -> Option<String> {
        if !self
            .host
            .runner()
            .handlers_of(input)
            .contains(&EventKind::Submit)
        {
            return None;
        }
        self.host
            .dispatch_at(input, Event::Submit, now_ms)
            .or(self.after_commit())
    }

    /// The pointer at a point (or gone): `hover` out of every node with a
    /// handler it left and into every one it entered, outermost first, each
    /// on its own — the web's `mouseleave`/`mouseenter`.
    pub(crate) fn hover_at(&mut self, at: Option<(f32, f32)>, now_ms: f64) -> Option<String> {
        self.hover_point = at;
        let mut under = Vec::new();
        let mut node = at.and_then(|(x, y)| self.hit(x, y));
        while let Some(id) = node {
            if self
                .host
                .runner()
                .handlers_of(id)
                .contains(&EventKind::Hover)
            {
                under.push(id);
            }
            node = self.host.kernel().node(id).and_then(|n| n.parent);
        }
        let left: Vec<_> = self
            .hovered
            .iter()
            .copied()
            .filter(|id| !under.contains(id))
            .collect();
        let entered: Vec<_> = under
            .iter()
            .rev()
            .copied()
            .filter(|id| !self.hovered.contains(id))
            .collect();
        if left.is_empty() && entered.is_empty() {
            return None;
        }
        self.hovered = under;
        let mut error = None;
        for (id, over) in left
            .into_iter()
            .map(|id| (id, false))
            .chain(entered.into_iter().map(|id| (id, true)))
        {
            if self.host.kernel().node(id).is_some() {
                error = error.or(self.host.dispatch_at(id, Event::Hover(over), now_ms));
            }
        }
        error.or(self.after_commit())
    }

    /// After a painted frame: what the layout, a scroll or the tree moved
    /// under a resting pointer is hovered, as a browser's hover follows
    /// layout and scrolling without a move (a synthetic mouse move's
    /// boundary events; #139). Only a pointer that has moved (a display's
    /// boot-time cursor hovers nothing), and nothing while a button or a
    /// contact is down. Never inside `frame()`: its pixels and witness are
    /// the tree it painted.
    pub fn follow_pointer(&mut self) {
        if self.contact.is_some() || self.pointer_held.is_some() || self.pointer_buttons != 0 {
            return;
        }
        let Some(at) = self.hover_point else { return };
        if let Some(error) = self.hover_at(Some(at), self.host.now()) {
            self.host.log(error);
        }
    }

    /// The agent's hover (`tap … hover`, LLP 1012): the pointer to the node's
    /// projected center, as a mouse moved there — never a press.
    pub fn hover(&mut self, id: ViewId) -> Result<String, String> {
        let (x, y) = self.pointer_target(id, None)?;
        self.set_pointer(Some((x, y)));
        if let Some(error) = self.hover_at(Some((x, y)), self.host.now()) {
            return Err(error);
        }
        Ok(format!(
            "{{\"tapped\":{id},\"hover\":true,\"delivery\":\"recognized\"}}"
        ))
    }

    pub(crate) fn mouse_request(
        &mut self,
        id: Option<ViewId>,
        request: &serde_json::Value,
    ) -> Result<String, String> {
        let mouse = request["mouse"].as_bool() == Some(true);
        if mouse
            && [
                "contextmenu",
                "dblclick",
                "phase",
                "wheel",
                "hover",
                "pinch",
                "history",
                "into",
                "resize",
            ]
            .iter()
            .any(|name| request.get(name).is_some())
        {
            return Err("mouse cannot be combined with another input mode".into());
        }
        let at = match request.get("at") {
            None => None,
            Some(value) => match value.as_array().map(Vec::as_slice) {
                Some([x, y]) => match (x.as_f64(), y.as_f64()) {
                    (Some(x), Some(y)) if (x as f32).is_finite() && (y as f32).is_finite() => {
                        Some((x as f32, y as f32))
                    }
                    _ => return Err("mouse/contextmenu at needs two finite numbers".into()),
                },
                _ => return Err("mouse/contextmenu at needs two finite numbers".into()),
            },
        };
        let id = id.ok_or("mouse/contextmenu needs an id")?;
        if mouse {
            self.mouse_click(id, at, false)
        } else {
            self.contextmenu(id, at)
        }
    }

    /// A secondary mouse click on a canvas, through the device input path.
    /// Other native context menus remain unsupported, never a primary press.
    pub(crate) fn contextmenu(
        &mut self,
        id: ViewId,
        at: Option<(f32, f32)>,
    ) -> Result<String, String> {
        self.mouse_click(id, at, true)
    }

    /// An explicit primary mouse click; ordinary agent taps remain fingers.
    pub(crate) fn mouse_click(
        &mut self,
        id: ViewId,
        at: Option<(f32, f32)>,
        secondary: bool,
    ) -> Result<String, String> {
        let kind = if secondary { "contextmenu" } else { "mouse" };
        if self.contact_position().is_some() {
            return Err(format!("{kind} requires the held contact to be released"));
        }
        let (x, y) = self.pointer_target(id, at)?;
        let canvas = self.hover_canvas(x, y);
        if canvas.is_none() || canvas != self.input_surface(id) {
            return Err(format!("view {id} does not carry canvas {kind} input"));
        }
        let now = self.host.now();
        self.set_pointer(Some((x, y)));
        self.pointer_move(x, y, now)?;
        if secondary {
            self.pointer_aux(2, true, x, y, now);
            self.pointer_aux(2, false, x, y, now);
        } else {
            self.pointer_down(x, y, now)?;
            self.pointer_up(x, y, now)?;
        }
        Ok(format!(
            "{{\"tapped\":{id},\"{kind}\":true,\"at\":[{},{}],\"delivery\":\"presenter\"}}",
            num(r2(x)),
            num(r2(y))
        ))
    }

    fn pointer_target(&mut self, id: ViewId, at: Option<(f32, f32)>) -> Result<(f32, f32), String> {
        self.boxes();
        if self.host.route_visibility(id).1 || self.placement_hidden(id) {
            return Err(format!("view {id} is hidden or inert"));
        }
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let (x, y) = match at {
            Some((x, y))
                if x.is_finite()
                    && y.is_finite()
                    && x >= 0.
                    && y >= 0.
                    && x < b.rect.2
                    && y < b.rect.3 =>
            {
                (b.rect.0 + x, b.rect.1 + y)
            }
            Some(_) => {
                return Err(format!(
                    "view {id}: mouse/contextmenu at must be a finite point inside its box"
                ))
            }
            None => b.center(),
        };
        if x < 0. || y < 0. || x >= self.viewport.0 || y >= self.viewport.1 {
            return Err(format!("view {id}: pointer point is outside the viewport"));
        }
        let mut hit = self.hit(x, y);
        while hit.is_some() && hit != Some(id) {
            hit = hit.and_then(|n| self.host.kernel().node(n).and_then(|n| n.parent));
        }
        if hit != Some(id) {
            return Err(format!(
                "view {id} is covered or not hit at the requested point"
            ));
        }
        Ok((x, y))
    }
}
