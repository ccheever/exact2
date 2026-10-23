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
//!
//! The navigation root's tablist is covered from boot too, where a tab bar
//! takes its place (`cover_root_tablist`).

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

    /// At boot, before the first layout: the navigation root's tablist
    /// (`Kernel::root_tabs`), whose place UIKit's tab bar takes, is covered
    /// whole, so no layout measures its tabs' symbols and button faces only
    /// for the presenter to cover it at its first batch (in euxy on iOS,
    /// 0.5 MB of CoreUI's glyph documents and probe buttons kept). The
    /// presenter takes the cover over when it installs its containers and
    /// gives it back where no tab bar shows (`NavigationIOS.swift`,
    /// `adoptBootCover`). A tablist whose author wrote `appearance: none`
    /// keeps its box, as the presenter leaves it.
    pub(crate) fn cover_root_tablist(&mut self) {
        if !closed_wait() {
            return;
        }
        let kernel = self.runner.kernel();
        let Some(root) = self.runner.roots().first().copied() else {
            return;
        };
        if kernel
            .node(root)
            .and_then(|n| n.props.str(PropId::NavigationBack))
            .is_none()
        {
            return;
        }
        let Some(tablist) = kernel.root_tabs(root).map(|t| t.tablist) else {
            return;
        };
        let authored = kernel.node(tablist).is_some_and(|n| {
            n.style.appearance == exact_kernel::Appearance::None
                && n.style.written.has(exact_kernel::StyleId::Appearance)
        });
        if !authored {
            // A node the kernel has is always coverable.
            let _ = self
                .runner
                .kernel_mut()
                .set_host_cover(tablist, Some(HostCover::Whole));
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

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::MonospaceMeasurer;
    use exact_plan::Value;
    use exact_runner::DataError;

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }

    /// Two tabs under a navigation root; `tablist` adds to the tablist's attributes.
    fn app(tablist: &str) -> String {
        format!(
            r#"routes nav
  tab home "/"
  tab other "/other"
component App
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=`${{top(nav).id}}` navigationBack="back" display="flex" flex-direction="column"
      column flex=1
        for t in nav.tabs key = t.name
          column role="tabpanel" id=`panel-${{t.name}}`
            for e in t.stack key = e.id
              column navigationKey=`${{e.id}}`
                text e.name
      row role="tablist" testId="tabs"{tablist}
        button role="tab" aria-controls="panel-home" press=pick("home")
          image "symbol:home"
          text "Home"
        button role="tab" aria-controls="panel-other" press=pick("other")
          image "symbol:star"
          text "Other"
"#
        )
    }

    /// Whether the boot left the tablist covered whole, under iOS's policy.
    fn covered(source: &str) -> bool {
        CLOSED_WAIT.set(true);
        let plan = contract::compile(source).unwrap().encode();
        let measurer = Box::new(MonospaceMeasurer::default());
        let (host, _) = Host::boot(&plan, NoData, measurer, 390.0, 844.0).unwrap();
        CLOSED_WAIT.set(false);
        let kernel = host.runner.kernel();
        let key = kernel.find_by_test_id("tabs")[0];
        let id = kernel.node_by_key(key).unwrap().id;
        let arena = kernel.arena();
        arena.slot_of(id).and_then(|s| arena.cover(s)) == Some(HostCover::Whole)
    }

    #[test]
    fn the_root_tablist_is_covered_from_boot() {
        assert!(covered(&app("")));
    }

    #[test]
    fn a_tablist_its_author_kept_keeps_its_box() {
        assert!(!covered(&app(r#" appearance="none""#)));
    }

    /// A box in a closed `dialog` is not laid out, so it does not
    /// intersect, as in a browser, where the dialog is `display: none`.
    #[test]
    fn a_box_in_a_closed_dialog_does_not_intersect() {
        let source = r#"component App
  state inside = false
  action seen(e: IntersectionObserverEntry)
    inside = e.isIntersecting
  view
    column
      text "page"
      dialog id="sheet" aria-label="Details"
        box height=40 intersect=seen testId="target"
"#;
        CLOSED_WAIT.set(true);
        let plan = contract::compile(source).unwrap().encode();
        let measurer = Box::new(MonospaceMeasurer::default());
        let (mut host, _) = Host::boot(&plan, NoData, measurer, 390.0, 844.0).unwrap();
        CLOSED_WAIT.set(false);
        let kernel = host.runner.kernel();
        let target = kernel
            .node_by_key(kernel.find_by_test_id("target")[0])
            .unwrap()
            .id;
        let due = host.runner.intersect_due();
        let entry = due
            .iter()
            .find(|e| e.view == target)
            .expect("its first observation");
        assert!(!entry.is_intersecting);
    }
}
