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
//! it again once it has closed (`MenusIOS.swift`, `layOut`). A node that
//! stops being one is given back here. Other hosts lay closed boxes out as
//! before, hidden by their views.

use super::*;

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
    cfg!(any(target_os = "ios", target_os = "tvos"))
}

/// Whether `node` is a box of the top layer: a `dialog` or a popover.
fn top_layer(node: &exact_kernel::NodeRef<'_>) -> bool {
    node.props.get(PropId::Popover).is_some()
        || node.props.str(PropId::SemanticTag) == Some("dialog")
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
            self.closed.remove(key);
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
                match (top_layer(&node), self.closed.contains(key)) {
                    (true, false) => made.push(node.id),
                    (false, true) => left.push((*key, node.id)),
                    _ => {}
                }
            }
        }
        self.close_boxes(made.into_iter());
        for (key, id) in left {
            self.closed.remove(&key);
            // A node the kernel has is always coverable.
            let _ = self.runner.kernel_mut().set_host_cover(id, None);
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
            if self.closed.insert(key) {
                let _ = self
                    .runner
                    .kernel_mut()
                    .set_host_cover(id, Some(exact_kernel::HostCover::Whole));
            }
        }
    }
}
