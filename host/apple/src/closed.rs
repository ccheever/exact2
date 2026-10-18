//! Closed top-layer boxes are not laid out (@ref LLP 1021 §5, LLP 1115 D6).
//!
//! A browser lays out neither a closed `dialog` nor a closed popover: its
//! style sheet makes `dialog:not([open])` and `[popover]:not(:popover-open)`
//! `display: none`. On iOS and tvOS the presenter shows a confirmation as
//! the platform's alert and a menu as `UIMenu`, and shows the rest (a
//! dialog's sheet, a content popover, everything under the agent) in
//! Exact's own views. So each `dialog` and each popover is covered whole
//! from its creation (`display: none`: nothing in it is measured), and the
//! presenter gives back the box it is about to show in its views and covers
//! it again once it has closed (`MenusIOS.swift`, `layOut`). A box given
//! back inside closed ones gives them back too while it is open, since
//! nothing under a covered box is laid out; they are covered again when no
//! box open inside them needs them. A node that stops being one is given
//! back here. Other hosts lay closed boxes out as before, hidden by their
//! views.

use super::*;
use exact_kernel::HostCover;

#[cfg(test)]
thread_local! {
    /// The policy in a test, which runs on the Mac.
    pub(crate) static CLOSED_WAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this platform leaves a closed top-layer box out of layout.
fn closed_wait() -> bool {
    #[cfg(test)]
    if CLOSED_WAIT.get() {
        return true;
    }
    // LLP 1126 D10: watchOS has no UIKit presenter to return covered top-layer boxes.
    cfg!(any(target_os = "ios", target_os = "tvos"))
}

/// Whether `node` is a box of the top layer: a `dialog` or a popover.
fn top_layer(node: &exact_kernel::NodeRef<'_>) -> bool {
    node.props.get(PropId::Popover).is_some()
        || node.props.str(PropId::SemanticTag) == Some("dialog")
}

/// The top-layer boxes this host covers while they are closed.
#[derive(Debug, Default)]
pub(crate) struct Closed {
    boxes: IdSet<NodeKey>,
    /// The boxes the presenter gave back itself.
    opened: IdSet<NodeKey>,
    /// A box given back for the open boxes inside it, which name it.
    lent: IdMap<NodeKey, IdSet<NodeKey>>,
}

impl<D: DataSource> Host<D> {
    /// At boot, before the first layout: every top-layer box in `order`.
    pub(crate) fn close_top_layer(&mut self, order: &[ViewId]) {
        if closed_wait() {
            self.close_boxes(order.iter().copied());
        }
    }

    /// Before a commit's views: the top-layer boxes it made are covered, and
    /// a node it changed into or out of one is covered or given back.
    pub(crate) fn close_new_top_layer(&mut self, receipts: &[Timed]) {
        for key in receipts.iter().flat_map(|t| &t.receipt.destroyed) {
            self.forget_box(*key);
        }
        if !closed_wait() {
            return;
        }
        let kernel = self.runner.kernel();
        let (mut made, mut left) = (Vec::new(), Vec::new());
        for t in receipts {
            for key in t.receipt.created.iter().chain(&t.receipt.touched) {
                let Some(node) = kernel.node_by_key(*key) else {
                    continue;
                };
                match (top_layer(&node), self.closed.boxes.contains(key)) {
                    (true, false) => made.push(node.id),
                    (false, true) => left.push((*key, node.id)),
                    _ => {}
                }
            }
        }
        self.close_boxes(made.into_iter());
        for (key, id) in left {
            self.forget_box(key);
            // A node the kernel has is always coverable.
            let _ = self.runner.kernel_mut().set_host_cover(id, None);
        }
    }

    /// The covers the presenter gave (`set_claims`), with what they mean for
    /// the boxes around those it opens or closes: a box opened inside closed
    /// ones gives them back while it is open, and one asked closed stays
    /// laid out while a box open inside it needs it.
    pub(crate) fn top_layer_claims(
        &mut self,
        covers: &[(ViewId, Option<HostCover>)],
    ) -> Vec<(ViewId, Option<HostCover>)> {
        let mut out = Vec::with_capacity(covers.len());
        for &(view, cover) in covers {
            let Some(key) = self.runner.kernel().node(view).map(|n| n.key) else {
                out.push((view, cover));
                continue;
            };
            if !self.closed.boxes.contains(&key) {
                out.push((view, cover));
                continue;
            }
            let opens = cover.is_none();
            if opens {
                self.closed.opened.insert(key);
            } else {
                self.closed.opened.remove(&key);
            }
            let lent = self.closed.lent.get(&key).is_some_and(|l| !l.is_empty());
            if opens || !lent {
                out.push((view, cover));
            }
            for (above, above_key) in self.boxes_above(view) {
                let held = self.closed.lent.entry(above_key).or_default();
                if opens {
                    held.insert(key);
                    out.push((above, None));
                } else if held.remove(&key)
                    && held.is_empty()
                    && !self.closed.opened.contains(&above_key)
                {
                    out.push((above, Some(HostCover::Whole)));
                }
            }
        }
        out
    }

    /// The top-layer boxes this host covers that hold `view`, nearest first.
    fn boxes_above(&self, view: ViewId) -> Vec<(ViewId, NodeKey)> {
        let kernel = self.runner.kernel();
        let mut above = Vec::new();
        let mut at = kernel.node(view).and_then(|n| n.parent);
        while let Some(node) = at.and_then(|id| kernel.node(id)) {
            if self.closed.boxes.contains(&node.key) {
                above.push((node.id, node.key));
            }
            at = node.parent;
        }
        above
    }

    /// A box gone or no longer one: the boxes it held open are covered again
    /// unless another holds them.
    fn forget_box(&mut self, key: NodeKey) {
        if !self.closed.boxes.remove(&key) {
            return;
        }
        self.closed.opened.remove(&key);
        self.closed.lent.remove(&key);
        let mut freed = Vec::new();
        for (above, held) in &mut self.closed.lent {
            if held.remove(&key) && held.is_empty() && !self.closed.opened.contains(above) {
                freed.push(*above);
            }
        }
        for above in freed {
            let id = self.runner.kernel().node_by_key(above).map(|n| n.id);
            if let Some(id) = id {
                let _ = self
                    .runner
                    .kernel_mut()
                    .set_host_cover(id, Some(HostCover::Whole));
            }
        }
    }

    fn close_boxes(&mut self, ids: impl Iterator<Item = ViewId>) {
        for id in ids {
            let Some(node) = self.runner.kernel().node(id) else {
                continue;
            };
            if !top_layer(&node) {
                continue;
            }
            let key = node.key;
            if self.closed.boxes.insert(key) {
                let _ = self
                    .runner
                    .kernel_mut()
                    .set_host_cover(id, Some(HostCover::Whole));
            }
        }
    }
}
