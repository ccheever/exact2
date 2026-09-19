//! Canvas action ownership and restored contact routing.
use super::{ControlBinding, DataSource, Presenter, Value};
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
    fn input_surface(&self, id: u32) -> Option<u32> {
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
    /// Route a hardware activation key through focused controls or the canvas.
    pub fn activation_key(&mut self, key: &str, down: bool) {
        self.restore_controls();
        let contact = if key == "Space" {
            u32::MAX - 1
        } else {
            u32::MAX - 2
        };
        if !down && self.control_bindings.keys().any(|(_, c)| *c == contact) {
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
        if down && matches!(key, "Space" | "Enter" | "NumpadEnter") {
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
        if let Some(id) = self.focus {
            let _ = self.type_key(id, key, down);
        } else if let Some(view) = self
            .surfaces
            .canvases
            .keys()
            .copied()
            .find(|v| self.surfaces.wants_input(*v))
        {
            self.surface_input(
                view,
                json!({"t":"key","code":key,"key":key,"down":down,"at":self.host.now()}),
            );
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
            None => vec!["down", "up"],
            Some("hold") if x == sx && y == sy => vec![],
            Some("hold") => vec!["move"],
            Some(p @ ("down" | "move" | "up" | "cancel")) => vec![p],
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
