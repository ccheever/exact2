//! Which control a pointer hovers and which a contact holds down, for the
//! neutral look (LLP 1115.000 D17): a state adds one thing there, a wash
//! where a pointer hovers and a stronger one while pressed, and a painted
//! control is this host's to show them on. Kept on the painter, which reads
//! them as it walks, and a change of either paints again. A host that does
//! not paint the neutral look keeps neither.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The control at or above `id`: a press on a button's title is the
    /// button's.
    fn control_at(&self, id: ViewId) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let mut at = Some(id);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            if node.node_type == NodeType::Control {
                return Some(id);
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
        .and_then(|id| self.control_at(id));
        let pressed = self
            .contact_hit()
            .and_then(|key| self.host.kernel().node_by_key(key).map(|node| node.id))
            .and_then(|id| self.control_at(id));
        if self.brush.feedback != (hover, pressed) {
            self.brush.feedback = (hover, pressed);
            self.dirty = true;
        }
    }
}
