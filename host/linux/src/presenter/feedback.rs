//! Which control a pointer hovers and which a contact holds down, for the
//! neutral look (LLP 1115.000 D17): a state adds one thing there, a wash
//! where a pointer hovers and a stronger one while pressed, and a painted
//! control is this host's to show them on. Kept on the painter, which reads
//! them as it walks, and a change of either paints again. Material's field
//! (LLP 1115.001.000 D6) shows the same two as its state layers, and a text
//! field is one there.
//!
//! What presses in a row of a list whose rows are cells answers the same
//! way (LLP 1115.000 D16): the look's row is this host's to draw too, and a
//! row's press is its wash, as the web's sheet has it (`neutral.css`).
use super::*;

impl<D: DataSource> Presenter<D> {
    /// What shows a state at or above `id`: the control (a press on a
    /// button's title is the button's), or the nearest node that presses
    /// when it is in a list's row. A row's own press look replaces the
    /// wash, as it does on the web: `-exact-press-scale`, the pressed paint
    /// (LLP 1061 D7, which `:active` lowers to), or `pointerdown`
    /// or `pointerup` handlers, which draw one. A disabled row shows none.
    fn feedback_at(&self, id: ViewId) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let mut at = Some(id);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            if node.node_type == NodeType::Control
                || node.node_type == NodeType::TextInput && !crate::paint::look::neutral()
            {
                return Some(id);
            }
            if node.node_type != NodeType::TextInput
                && self.presses_itself(id)
                && kernel.row_holding(id).is_some()
            {
                let handlers = self.host.runner().handlers_of(id);
                let own = node.style.written.has(exact_kernel::StyleId::PressScale)
                    || crate::host::press::paints(node.style)
                    || handlers.contains(&EventKind::Pointerdown)
                    || handlers.contains(&EventKind::Pointerup);
                let off = node.props.bool(PropId::Disabled) == Some(true);
                return (!own && !off).then_some(id);
            }
            at = node.parent;
        }
        None
    }

    /// Read both again, after anything that moved the pointer or a contact.
    pub(super) fn refresh_feedback(&mut self) {
        // A finger hovers nothing; a held contact is not also a hover (but
        // for a `:hover` look, as CSS's is).
        let hit = match self.hover_point {
            Some((x, y)) if !self.direct() => self.hit(x, y),
            _ => None,
        };
        let hover = hit
            .filter(|_| self.contact.is_none())
            .and_then(|id| self.feedback_at(id));
        let pressed = self
            .contact_hit()
            .and_then(|key| self.host.kernel().node_by_key(key).map(|node| node.id))
            .and_then(|id| self.feedback_at(id));
        if self.brush.feedback != (hover, pressed) {
            self.brush.feedback = (hover, pressed);
            self.dirty = true;
        }
        self.refresh_looks(Some(hit));
    }

    /// The nodes whose `:hover` look holds (LLP 1121 D8): the pointer's hit
    /// and its ancestors that have one, as CSS's `:hover` matches; none for
    /// a finger. The focus's `:focus-visible` look holds where its ring
    /// would show. A state held keeps its look beside them, until what lets
    /// it go (`held.rs`). A node whose looks moved paints again. `hit`: what
    /// the pointer hovers now, after it moved (`None`: a paint keeps the
    /// hover it has, its nodes that live).
    pub(super) fn refresh_looks(&mut self, hit: Option<Option<ViewId>>) {
        let mut at = hit.flatten().filter(|_| !self.direct());
        self.check_hold(hit.map(|_| at));
        let kernel = self.host.kernel();
        let mut hovered = Vec::new();
        if hit.is_none() {
            hovered = (self.host.looks_shown.hover.iter().copied())
                .filter(|id| kernel.node(*id).is_some())
                .collect();
        }
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else { break };
            if node
                .looks()
                .iter()
                .any(|(s, _)| *s == exact_kernel::LOOK_HOVER)
            {
                hovered.push(id);
            }
            at = node.parent;
        }
        let focus = self.focus.filter(|f| self.brush.ringless != Some(*f));
        let focus = focus.filter(|f| {
            kernel.node(*f).is_some_and(|n| {
                n.looks()
                    .iter()
                    .any(|(s, _)| *s == exact_kernel::LOOK_FOCUS_VISIBLE)
            })
        });
        let old = self.host.looks_shown.marks();
        (self.host.looks_shown.hover, self.host.looks_shown.focus) = (hovered, focus);
        let new = self.host.looks_shown.marks();
        let moved: Vec<ViewId> = (old.iter().filter(|m| !new.contains(m)))
            .chain(new.iter().filter(|m| !old.contains(m)))
            .map(|m| m.0)
            .collect();
        self.repaint_looks(moved);
    }

    /// Paint `moved` again: their looks were painted or taken off.
    pub(super) fn repaint_looks(&mut self, moved: Vec<ViewId>) {
        // A look painted or taken off is no flow damage's small region.
        if !moved.is_empty() {
            self.host.flow_damage.repaint();
        }
        for id in moved {
            if let Some(node) = self.host.kernel().node(id) {
                let key = node.key;
                self.host.row_dirty.node(key);
                self.dirty = true;
            }
        }
    }
}

#[path = "held.rs"]
mod held;
#[cfg(test)]
#[path = "looks_tests.rs"]
mod looks_tests;
