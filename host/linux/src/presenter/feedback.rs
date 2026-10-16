//! Which control a pointer hovers and which a contact holds down, for the
//! neutral look (LLP 1115.000 D17): a state adds one thing there, a wash
//! where a pointer hovers and a stronger one while pressed, and a painted
//! control is this host's to show them on. Kept on the painter, which reads
//! them as it walks, and a change of either paints again. A host that does
//! not paint the neutral look keeps neither.
//!
//! What presses in a row of a list whose rows are cells answers the same
//! way (LLP 1115.000 D16): the look's row is this host's to draw too, and a
//! row's press is its wash, as the web's sheet has it (`neutral.css`).
use super::*;

impl<D: DataSource> Presenter<D> {
    /// What shows a state at or above `id`: the control (a press on a
    /// button's title is the button's), or the nearest node that presses
    /// when it is in a list's row. A row's own press look replaces the
    /// wash, as it does on the web: `-exact-press-scale`, or `pointerdown`
    /// or `pointerup` handlers, which draw one. A disabled row shows none.
    fn feedback_at(&self, id: ViewId) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let mut at = Some(id);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            if node.node_type == NodeType::Control {
                return Some(id);
            }
            if node.node_type != NodeType::TextInput
                && self.presses_itself(id)
                && kernel.row_holding(id).is_some()
            {
                let handlers = self.host.runner().handlers_of(id);
                let own = node.style.written.has(exact_kernel::StyleId::PressScale)
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
        if !crate::paint::look::neutral() {
            return;
        }
        // A finger hovers nothing; a held contact is not also a hover.
        let hover = match self.hover_point {
            Some((x, y)) if self.contact.is_none() && !self.direct() => self.hit(x, y),
            _ => None,
        }
        .and_then(|id| self.feedback_at(id));
        let pressed = self
            .contact_hit()
            .and_then(|key| self.host.kernel().node_by_key(key).map(|node| node.id))
            .and_then(|id| self.feedback_at(id));
        if self.brush.feedback != (hover, pressed) {
            self.brush.feedback = (hover, pressed);
            self.dirty = true;
        }
    }
}
