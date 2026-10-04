//! Pointer focus, canvas action ownership, and restored contact routing.
use super::{ControlBinding, DataSource, Presenter, Value};
use exact_kernel::{NodeType, PropId, ViewId};
use exact_plan::EventKind;
use exact_runner::Event;
use serde_json::json;
use std::collections::BTreeSet;

impl<D: DataSource> Presenter<D> {
    pub(crate) fn control_target(&self, id: u32) -> Option<u32> {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            let node = self.host.kernel().node(id)?;
            if node.props.bool(exact_kernel::PropId::Disabled) == Some(true)
                || self.host.route_visibility(id).1
            {
                return None;
            }
            if node.props.str(exact_kernel::PropId::Action).is_some() {
                return Some(id);
            }
            cursor = node.parent;
        }
        None
    }
    pub(crate) fn control_input(
        &mut self,
        id: u32,
        phase: &str,
        x: f32,
        y: f32,
        contact: u32,
        at: f64,
    ) -> bool {
        self.restore_controls();
        let surface = if phase == "down" {
            self.input_surface(id)
        } else {
            self.control_bindings
                .iter()
                .find(|((_, c), b)| *c == contact && b.view == Some(id))
                .map(|((surface, _), _)| *surface)
                .or_else(|| self.input_surface(id))
        };
        let Some(surface) = surface else {
            return false;
        };
        let key = (surface, contact);
        if phase == "down" && self.control_bindings.contains_key(&key) {
            return true;
        }
        if phase == "down" {
            let Some(name) = self
                .host
                .kernel()
                .node(id)
                .and_then(|n| n.props.str(exact_kernel::PropId::Action))
                .map(str::to_owned)
            else {
                return false;
            };
            let Some((ox, oy, _, _)) = self.rect_of(id) else {
                return false;
            };
            if !self
                .focus
                .and_then(|v| self.host.kernel().node(v))
                .is_some_and(|n| n.node_type == exact_kernel::NodeType::TextInput)
            {
                self.focus = Some(id);
            }
            self.control_bindings.insert(
                key,
                ControlBinding {
                    view: Some(id),
                    surface,
                    generation: self.surfaces.canvases[&surface].id,
                    name,
                    offset: (ox, oy),
                },
            );
        }
        let Some(owner) = self.control_bindings.get(&key).cloned() else {
            return false;
        };
        if matches!(phase, "up" | "cancel") {
            self.control_bindings.remove(&key);
        }
        let accepted = self.surfaces.input(owner.surface, json!({"t":"control","name":owner.name,"id":contact,"phase":phase,"x":x-owner.offset.0,"y":y-owner.offset.1,"at":at}));
        if !accepted && phase == "down" {
            self.control_bindings.remove(&key);
        }
        accepted
    }
    pub(crate) fn input_surface(&self, id: u32) -> Option<u32> {
        let mut cursor = Some(id);
        while let Some(view) = cursor {
            if self.surfaces.wants_input(view) {
                return Some(view);
            }
            cursor = self.host.kernel().node(view).and_then(|n| n.parent);
        }
        None
    }
    pub(crate) fn holds_control(&self, id: u32) -> bool {
        self.control_bindings
            .values()
            .any(|binding| binding.view == Some(id))
    }
    pub(crate) fn owns_control(&self, id: u32, contact: u32) -> bool {
        self.input_surface(id)
            .is_some_and(|s| self.control_bindings.contains_key(&(s, contact)))
    }
    pub(crate) fn restore_controls(&mut self) {
        let surface_ids: BTreeSet<_> = self.surfaces.canvases.keys().copied().collect();
        for (&surface, canvas) in &mut self.surfaces.canvases {
            let Some(contacts) = canvas.restored_controls.take() else {
                continue;
            };
            self.control_bindings.retain(|(s, _), _| *s != surface);
            if self.control_contact.is_some_and(|(v, _, _)| {
                let mut cursor = Some(v);
                while let Some(id) = cursor {
                    if id == surface {
                        return true;
                    }
                    cursor = self.host.kernel().node(id).and_then(|n| n.parent);
                }
                false
            }) {
                self.control_contact = None;
            }
            for contact in contacts {
                let (Some(id), Some(name)) = (contact["id"].as_u64(), contact["action"].as_str())
                else {
                    continue;
                };
                let mut matches = self.host.preorder().into_iter().filter(|v| {
                    let Some(node) = self.host.kernel().node(*v) else {
                        return false;
                    };
                    if node.props.str(exact_kernel::PropId::Action) != Some(name) {
                        return false;
                    }
                    let mut cursor = node.parent;
                    while let Some(id) = cursor {
                        if id == surface {
                            return true;
                        }
                        // A nested canvas owns its own controls.
                        if surface_ids.contains(&id) {
                            return false;
                        }
                        cursor = self.host.kernel().node(id).and_then(|n| n.parent);
                    }
                    false
                });
                let first = matches.next();
                let view = if matches.next().is_none() {
                    first
                } else {
                    None
                };
                self.control_bindings.insert(
                    (surface, id as u32),
                    ControlBinding {
                        view,
                        surface,
                        generation: canvas.id,
                        name: name.into(),
                        offset: (0., 0.),
                    },
                );
            }
        }
    }
    pub(crate) fn cancel_removed_controls(&mut self) {
        let removed: Vec<_> = self
            .control_bindings
            .iter()
            .filter(|(_, b)| {
                self.surfaces
                    .canvases
                    .get(&b.surface)
                    .is_none_or(|c| c.id != b.generation)
                    || (b.view.is_none()
                        && !self.host.preorder().into_iter().any(|view| {
                            self.input_surface(view) == Some(b.surface)
                                && self
                                    .host
                                    .kernel()
                                    .node(view)
                                    .and_then(|n| n.props.str(exact_kernel::PropId::Action))
                                    == Some(b.name.as_str())
                        }))
                    || b.view.is_some_and(|view| {
                        self.input_surface(view) != Some(b.surface)
                            || self
                                .host
                                .kernel()
                                .node(view)
                                .and_then(|n| n.props.str(exact_kernel::PropId::Action))
                                .is_none()
                    })
            })
            .map(|(key, b)| (*key, b.clone()))
            .collect();
        for (key, b) in removed {
            self.control_bindings.remove(&key);
            if self
                .surfaces
                .canvases
                .get(&b.surface)
                .is_some_and(|c| c.id == b.generation)
            {
                self.surfaces.input(b.surface,json!({"t":"control","name":b.name,"phase":"cancel","id":key.1,"x":0,"y":0,"at":self.host.now()}));
            }
            if key.1 == 1
                && self
                    .control_contact
                    .is_some_and(|(view, _, _)| b.view == Some(view))
            {
                self.control_contact = None;
            }
        }
    }
    pub(crate) fn cancel_controls(&mut self) {
        self.restore_controls();
        for ((surface, contact), b) in std::mem::take(&mut self.control_bindings) {
            self.surfaces.input(surface,json!({"t":"control","name":b.name,"phase":"cancel","id":contact,"x":0,"y":0,"at":self.host.now()}));
        }
    }
    /// Drop focus.
    pub fn blur(&mut self) {
        self.cancel_controls();
        let mut views: Vec<_> = self
            .surfaces
            .canvases
            .iter()
            .filter(|(_, c)| !c.held.is_empty())
            .map(|(view, _)| *view)
            .collect();
        for id in self
            .control_contact
            .take()
            .map(|(id, _, _)| id)
            .into_iter()
            .chain(self.focus)
        {
            if let Some(view) = self.input_surface(id) {
                views.push(view);
            }
        }
        views.sort_unstable();
        views.dedup();
        for view in views {
            self.surfaces
                .input(view, serde_json::json!({"t":"blur","at":self.host.now()}));
        }
        if let Some(e) = self.set_focus(None, self.host.now()) {
            eprintln!("exact: {e}");
        }
    }

    /// Route device keys through the same targeted path as agent keys.
    pub fn hardware_key(&mut self, code: &str, key: &str, down: bool, repeat: bool) {
        self.restore_controls();
        self.hold_modifier(code, down);
        // A Control or Meta chord is a shortcut, as a browser's: the focus's
        // `key` handlers hear it, and no canvas or control starts with it.
        if down && self.held & 0b1100_1100 != 0 {
            if self.focus.is_some() {
                let name = if code == "NumpadEnter" { "Enter" } else { key };
                self.key_down(name, self.host.now());
            }
            return;
        }
        let contact = if code == "Space" {
            u32::MAX - 1
        } else {
            u32::MAX - 2
        };
        if !down
            && matches!(code, "Space" | "Enter" | "NumpadEnter")
            && self.control_bindings.keys().any(|(_, c)| *c == contact)
        {
            let surfaces: Vec<_> = self
                .control_bindings
                .keys()
                .filter(|(_, c)| *c == contact)
                .map(|(s, _)| *s)
                .collect();
            for surface in surfaces {
                self.control_input(surface, "up", 0., 0., contact, self.host.now());
            }
            return;
        }
        if down && matches!(code, "Space" | "Enter" | "NumpadEnter") {
            let owner = self
                .control_bindings
                .iter()
                .find(|((_, c), _)| *c == contact)
                .or_else(|| {
                    self.control_bindings
                        .iter()
                        .find(|((_, c), _)| *c < u32::MAX - 2)
                })
                .map(|(_, b)| b.clone());
            if let Some(owner) = owner {
                let surface = owner.surface;
                if self.control_bindings.contains_key(&(surface, contact)) {
                    return;
                }
                if self.surfaces.input(surface, json!({"t":"control","name":owner.name,"phase":"down","id":contact,"x":0,"y":0,"at":self.host.now()})) {
                    self.control_bindings.insert((surface, contact), owner);
                }
                return;
            }
        }
        // A release belongs to the canvas that received the press, even after focus
        // moves into an editor. The existing held set is the ownership record.
        if !down {
            let owners: Vec<_> = self
                .surfaces
                .canvases
                .iter()
                .filter(|(_, c)| c.held.contains(code))
                .map(|(view, _)| *view)
                .collect();
            for view in owners {
                self.surfaces.input(view, json!({"t":"key","code":code,"key":key,"down":false,"repeat":false,"at":self.host.now()}));
            }
            return;
        }
        if let Some(id) = self.focus.or_else(|| {
            self.surfaces.canvases.keys().copied().find(|view| {
                self.surfaces.wants_input(*view)
                    && self.host.route_visibility(*view) == (false, false)
                    && self
                        .host
                        .kernel()
                        .node(*view)
                        .is_some_and(|n| n.props.bool(PropId::Disabled) != Some(true))
            })
        }) {
            let _ = self.type_key(id, code, key, down, repeat);
        }
        if code == "Escape"
            && self.focus.is_some_and(|id| {
                self.host
                    .kernel()
                    .node(id)
                    .is_some_and(|n| n.node_type == NodeType::TextInput)
                    || self.input_surface(id).is_none()
            })
        {
            self.blur();
        }
    }

    pub(crate) fn control_tap(&mut self, q: &Value) -> Option<Value> {
        self.restore_controls();
        let phase = q["phase"].as_str();
        if let Some(contact) = q["contact"]
            .as_u64()
            .filter(|_| matches!(phase, Some("up" | "cancel")))
        {
            let surface = self.input_surface(q["id"].as_u64()? as u32)?;
            let owner = self
                .control_bindings
                .get(&(surface, contact as u32))?
                .clone();
            let ok = self.control_input(
                owner.surface,
                phase.unwrap(),
                owner.offset.0,
                owner.offset.1,
                contact as u32,
                self.host.now(),
            );
            if self
                .control_contact
                .is_some_and(|(view, _, _)| owner.view == Some(view))
            {
                self.control_contact = None;
            }
            return Some(if ok {
                json!({"phase":phase,"delivery":"recognized"})
            } else {
                json!({"error":"control release refused"})
            });
        }
        let continuing = phase.is_some_and(|p| p != "down");
        let (id, sx, sy) = if continuing {
            self.control_contact?
        } else {
            let id = q["id"].as_u64()? as u32;
            let id = self.control_target(id)?;
            let (x, y, w, h) = self.rect_of(id)?;
            (id, x + w / 2., y + h / 2.)
        };
        let x = q["x"]
            .as_f64()
            .unwrap_or(sx as f64 + q["dx"].as_f64().unwrap_or(0.)) as f32;
        let y = q["y"]
            .as_f64()
            .unwrap_or(sy as f64 + q["dy"].as_f64().unwrap_or(0.)) as f32;
        if !x.is_finite() || !y.is_finite() {
            return Some(json!({"error":"control needs finite points"}));
        }
        if !continuing && self.hit(x, y).and_then(|hit| self.control_target(hit)) != Some(id) {
            return Some(json!({"error":"control is covered"}));
        }
        if phase == Some("down") && self.control_contact.is_some() {
            return Some(json!({"error":"a contact is already down"}));
        }
        let phases = match phase {
            None => &["down", "up"][..],
            Some("hold") if x == sx && y == sy => &[],
            Some("hold") => &["move"],
            Some("down" | "move" | "up" | "cancel") => phase.as_slice(),
            _ => return Some(json!({"error":"unknown control phase"})),
        };
        for step in phases {
            if !self.control_input(id, step, x, y, 1, self.host.now()) {
                return Some(json!({"error":"control surface refused input"}));
            }
        }
        self.control_contact = match phase {
            Some("down" | "move" | "hold") => Some((id, x, y)),
            _ => None,
        };
        Some(json!({"tapped":id,"phase":phase,"at":[x,y],"delivery":"recognized"}))
    }
}

impl<D: DataSource> Presenter<D> {
    /// The nearest node at or above `id` with a handler for `kind`.
    pub(crate) fn handler_target(&self, id: ViewId, kind: EventKind) -> Option<ViewId> {
        if kind != EventKind::Scroll && self.brush.region_blocks_action(id) {
            return None;
        }
        if self.host.route_visibility(id).1 {
            return None;
        }
        let kernel = self.host.kernel();
        let mut at = Some(id);
        while let Some(n) = at {
            let node = kernel.node(n)?;
            if node.props.bool(PropId::Disabled) == Some(true) {
                return None;
            }
            if self.host.runner().handlers_of(n).contains(&kind) {
                return Some(n);
            }
            at = self.display.parent(kernel, n);
        }
        None
    }

    /// The web's focusable nodes: controls, inputs, buttons and links, and a
    /// node with a `focus`, `blur` or `key` handler (the web gives it a
    /// `tabindex`), as the Apple hosts take the first responder.
    pub(crate) fn focusable(&self, id: ViewId) -> bool {
        self.host.kernel().node(id).is_some_and(|n| {
            n.props.bool(PropId::Disabled) != Some(true)
                && (n.props.str(PropId::Action).is_some()
                    || n.node_type == NodeType::TextInput
                    // A native button is a button under any role (LLP 1069.011.000 D1).
                    || exact_kernel::ControlKind::of(n.node_type, n.props)
                        == Some(exact_kernel::ControlKind::Button)
                    || matches!(
                        n.props.str(PropId::AccessibilityRole),
                        Some("button" | "link")
                    )
                    || self
                        .host
                        .runner()
                        .handlers_of(id)
                        .iter()
                        .any(|k| matches!(k, EventKind::Focus | EventKind::Blur | EventKind::Key)))
        })
    }

    /// Pointer activation returns HUD button focus to its input canvas after press.
    pub fn press_at(&mut self, x: f32, y: f32, now_ms: f64) -> Option<ViewId> {
        // An open select menu takes the press: a row, or light dismiss.
        if let Some(select) = self.menu_press(x, y) {
            return Some(select);
        }
        let hit = self.hit(x, y)?;
        if crate::navigation::popover_invoker(self.host.kernel(), hit) {
            self.host.log(crate::navigation::POPOVER_UNSUPPORTED);
            return None;
        }
        if self.brush.region_blocks_action(hit) {
            return self.retained_press(hit, now_ms);
        }
        if let Some(control) = self.control_target(hit) {
            for phase in ["down", "up"] {
                if !self.control_input(control, phase, x, y, 1, now_ms) {
                    return None;
                }
            }
            return Some(control);
        }
        self.focus_for_press(hit, now_ms);
        if self.press_range(hit, x) || self.toggle_control(hit, now_ms) {
            return Some(hit);
        }
        let Some(target) = self.handler_target(hit, EventKind::Press) else {
            return self.surface_pointer(hit, x, y, now_ms);
        };
        self.dispatch_press(target, now_ms, true);
        Some(target)
    }

    /// A press moves focus to the nearest focusable node at or above `hit`,
    /// except that a canvas press leaves an editor focused.
    fn focus_for_press(&mut self, hit: ViewId, now_ms: f64) {
        let mut focus = Some(hit);
        while let Some(id) = focus {
            if self.focusable(id) {
                break;
            }
            focus = self.host.kernel().node(id).and_then(|n| n.parent);
        }
        let editing = self.surfaces.wants_input(hit)
            && self
                .focus
                .and_then(|id| self.host.kernel().node(id))
                .is_some_and(|node| node.node_type == NodeType::TextInput);
        if self.focus != focus && !editing {
            if let Some(e) = self.set_focus(focus, now_ms) {
                eprintln!("exact: {e}");
            }
            self.queue_collections();
        }
    }

    /// The canvas a held contact on `hit` belongs to: the one `press_at` would
    /// hand a click to, when no menu, control, range, toggle or press handler
    /// takes it first. The canvas then sees the contact's down, every move and
    /// its up as they happen, as the web's pointer events do.
    pub(crate) fn canvas_contact_target(&self, hit: ViewId) -> Option<u32> {
        let node = self.host.kernel().node(hit)?;
        if self.menu.is_some()
            || node.node_type == NodeType::Control
            || crate::navigation::popover_invoker(self.host.kernel(), hit)
            || self.brush.region_blocks_action(hit)
            || self.control_target(hit).is_some()
            || self.handler_target(hit, EventKind::Press).is_some()
        {
            return None;
        }
        self.input_surface(hit)
    }

    /// A held contact's down on its canvas: focus as a press does, then the
    /// pointer's `down`. False when the canvas refuses it.
    pub(crate) fn canvas_contact_down(&mut self, hit: ViewId, x: f32, y: f32, at: f64) -> bool {
        self.focus_for_press(hit, at);
        self.input_surface(hit)
            .is_some_and(|view| self.canvas_pointer(view, "down", 1, x, y, at))
    }

    pub(crate) fn dispatch_press(&mut self, target: ViewId, now_ms: f64, pointer: bool) {
        // Capture ancestry before the handler can remove its button. Pointer
        // activation yields focus to commit autofocus; keyboard activation keeps
        // a surviving button focused. Never clear an editor or explicit focus.
        let fallback = (self.focus == Some(target)).then(|| {
            if pointer {
                self.focus = None;
            }
            let node = self.host.kernel().node(target);
            let hud = node.is_some_and(|n| {
                n.props.str(PropId::AccessibilityRole) == Some("button")
                    && n.props.str(PropId::Action).is_none()
            });
            if hud {
                self.input_surface(target).unwrap_or(target)
            } else {
                target
            }
        });
        if let Some(e) = self.host.dispatch_at(target, Event::Press, now_ms) {
            eprintln!("exact: {e}");
        }
        if let Some(e) = self.after_commit() {
            eprintln!("exact: {e}");
        }
        if self.focus.is_none() {
            self.focus = fallback.filter(|id| {
                self.host.kernel().node(*id).is_some() && !self.host.route_visibility(*id).1
            });
        }
    }
}
