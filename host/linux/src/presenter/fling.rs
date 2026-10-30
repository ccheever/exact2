//! LLP 1057.001 §8 on the Android reader: `swipe` is the platform's fling,
//! which the reader recognizes over the whole touch (`Flings.kt`) and hands
//! here before the lift, with its direction and the most pointers the touch
//! had down at once. The presenter's contact is one pointer and a second ends
//! it, so the touch's first hit is kept past that end for the fling, until a
//! gesture of the presenter's own takes the contact.
use super::*;
use exact_kernel::{ControlKind, Display, NodeKey, PropValue};

impl<D: DataSource> Presenter<D> {
    /// Dispatches the touch's fling in `direction` (left, right, up or down)
    /// to the innermost node on its first hit's path whose swipe takes that
    /// direction with `pointers` down (rule 3), and returns whether one took
    /// it. The fling first ends the contact, and with it an armed press.
    pub fn fling(&mut self, direction: &str, pointers: u32, now_ms: f64) -> Result<bool, String> {
        let Some(hit) = self.landed.take() else {
            return Ok(false);
        };
        let Some(target) = self.fling_target(hit, direction, pointers) else {
            return Ok(false);
        };
        self.dirty = true;
        // End a contact that is still alive, then find the swipe again: the
        // cancel runs the authored up handlers (LLP 1005 §3), which may
        // disable, hide or remove it. A contact that already ended past the
        // slop has nothing to end: the up handlers run at the lift, or ran at
        // an earlier cancel such as a second finger's.
        let target = if self.contact.is_some() {
            if let Err(e) = self.pointer_cancel(now_ms) {
                self.host.log(e);
            }
            self.fling_target(hit, direction, pointers)
        } else {
            Some(target)
        };
        let Some(view) = target else {
            return self.after_commit().map_or(Ok(false), Err);
        };
        let error = self
            .host
            .dispatch_at(view, Event::Swipe(direction.into()), now_ms)
            .or(self.after_commit());
        error.map_or(Ok(true), Err)
    }

    /// The node on the path from the touch's first hit, `hit`, whose swipe
    /// takes the fling. A scroller on the path that scrolls on the fling's
    /// axis keeps it, as the reader's scrolling takes the touch, and so does
    /// a live text input, control or `press` node below the swipe. A node
    /// that is not rendered, or whose input is not live (`disabled`, or
    /// inside such a node), passes it on (§8.1).
    fn fling_target(&mut self, hit: NodeKey, direction: &str, pointers: u32) -> Option<ViewId> {
        let first = self.host.kernel().node_by_key(hit)?.id;
        let sideways = matches!(direction, "left" | "right");
        if self
            .chain_from(first)
            .iter()
            .any(|s| if sideways { s.axes.0 } else { s.axes.1 })
        {
            return None;
        }
        let kernel = self.host.kernel();
        let path: Vec<ViewId> =
            std::iter::successors(Some(first), |&id| kernel.node(id)?.parent).collect();
        // Start above the highest node that is not rendered, which hides every node below it
        let start = path
            .iter()
            .rposition(|&id| {
                kernel
                    .node(id)
                    .is_some_and(|n| n.style.display == Display::None)
                    || matches!(
                        self.brush.placements.get(&id),
                        Some(crate::placement::Placement::Hidden)
                    )
            })
            .map_or(0, |i| i + 1);
        for &id in &path[start..] {
            let node = kernel.node(id)?;
            let handlers = self.host.runner().handlers_of(id);
            let live = self.input_live(node.key);
            if live
                && handlers.contains(&EventKind::Swipe)
                && takes(node.props, direction, pointers)
            {
                return Some(id);
            }
            if live
                && (handlers.contains(&EventKind::Press)
                    || node.node_type == NodeType::TextInput
                    || node.node_type == NodeType::Pressable
                    || ControlKind::of(node.node_type, node.props).is_some())
            {
                return None;
            }
        }
        None
    }

    /// Ends the touch's fling: a gesture of the presenter's took its contact.
    pub(super) fn contact_taken(&mut self) {
        self.landed = None;
    }
}

fn takes(props: &exact_kernel::PropList, direction: &str, pointers: u32) -> bool {
    props
        .str(PropId::SwipeDirection)
        .is_some_and(|words| words.split_ascii_whitespace().any(|w| w == direction))
        && props
            .get(PropId::TouchCount)
            .and_then(PropValue::as_int)
            .is_none_or(|n| n == i64::from(pointers))
}

#[cfg(test)]
#[path = "fling_tests.rs"]
mod fling_tests;
