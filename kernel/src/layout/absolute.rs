//! A change inside an absolutely positioned box replays at its containing
//! block. What such a box and everything in it are sized by asks nothing of
//! the block's other content, and nothing the block's ancestors asked of the
//! block (its intrinsic sizes, its baselines) reads an absolutely positioned
//! child, so replaying the block's saved final layout settles the change, and
//! the ancestors keep their caches unless the replay's output differs, its
//! scrollable overflow included. A step strip's playhead dot, made and
//! dropped in another cell eight times a second, laid the whole page out
//! each time, through every fit-content probe on the way up.
use super::*;

/// What changed since the last layout that decides it.
#[derive(Debug, Default, Clone)]
pub(super) struct Changes {
    /// Each parent's children in flow before its first child-list change.
    before: IdMap<NodeId, Vec<NodeId>>,
    /// Boxes made since: no flow held them.
    made: IdSet<NodeId>,
    /// Boxes already laid out whose position changed: they may have joined
    /// or left their parent's flow.
    repositioned: IdSet<NodeId>,
    /// Boxes marked dirty for their own content (a text, an intrinsic size).
    dirtied: IdSet<NodeId>,
}

/// Whether two layouts of one box reach as far in its parent's scrollable
/// overflow: their border box and their overflow past it agree (Taffy's
/// `compute_scrollable_overflow_contribution`). Overflow inside the border
/// box reaches no further.
pub(super) fn same_reach(a: &taffy::tree::LayoutOutput, b: &taffy::tree::LayoutOutput) -> bool {
    let reach = |o: &taffy::tree::LayoutOutput| {
        let r = o.scrollable_overflow_rect;
        [
            r.left.min(0.0).to_bits(),
            r.right.max(o.size.width).to_bits(),
            r.top.min(0.0).to_bits(),
            r.bottom.max(o.size.height).to_bits(),
        ]
    };
    a.size == b.size && reach(a) == reach(b)
}

impl Changes {
    pub(super) fn clear(&mut self) {
        self.before.clear();
        self.made.clear();
        self.repositioned.clear();
        self.dirtied.clear();
    }
}

impl LayoutTree {
    fn in_flow(&self, children: impl Iterator<Item = NodeId>) -> Vec<NodeId> {
        children
            .filter(|c| {
                self.taffy
                    .style(*c)
                    .is_ok_and(|s| s.position != taffy::Position::Absolute)
            })
            .collect()
    }

    /// What a boundary replays: a containing block's saved final layout
    /// whatever its ancestors asked of it (its overflow, which reaches them,
    /// is compared after), else the record while they asked nothing else.
    pub(super) fn replay_record(
        &self,
        node: NodeId,
        block: bool,
    ) -> Option<(LayoutInput, taffy::tree::LayoutOutput)> {
        match block {
            true => self.taffy.saved_layout(node),
            false => self.taffy.last_layout_input(node),
        }
    }

    /// Whether `node` is under another root than `root`, one laid out before.
    pub(super) fn under_another(&self, node: NodeId, root: NodeId) -> bool {
        let mut top = node;
        while let Some(parent) = self.taffy.parent(top) {
            top = parent;
        }
        top != root && self.offers.contains_key(&top)
    }

    /// Before `parent`'s children change: its children in flow, the first time.
    pub(super) fn children_changing(&mut self, parent: NodeId) {
        if !self.absolute.before.contains_key(&parent) {
            let before = self.in_flow(self.taffy.child_ids(parent));
            self.absolute.before.insert(parent, before);
        }
    }

    /// A box was made, repositioned or dirtied for its own content.
    pub(super) fn made(&mut self, node: NodeId) {
        self.absolute.made.insert(node);
    }
    pub(super) fn repositioned(&mut self, node: NodeId) {
        if !self.absolute.made.contains(&node) {
            self.absolute.repositioned.insert(node);
            // Its parent's flow may have changed, whatever its children are
            // by the layout (a box made absolute, then dropped).
            if let Some(parent) = self.taffy.parent(node) {
                self.absolute.dirtied.insert(parent);
            }
        }
    }
    pub(super) fn dirtied(&mut self, node: NodeId) {
        self.absolute.dirtied.insert(node);
    }

    /// Whether `node`'s only change is to which absolutely positioned
    /// children it has.
    fn only_absolute_children(&self, node: NodeId) -> bool {
        let Some(before) = self.absolute.before.get(&node) else {
            return false;
        };
        !self.absolute.dirtied.contains(&node)
            && !self
                .taffy
                .child_ids(node)
                .any(|c| self.absolute.repositioned.contains(&c))
            && self.in_flow(self.taffy.child_ids(node)) == *before
    }

    /// The containing block a source's change replays at when the change is
    /// confined to what the block holds absolutely: the source itself, when
    /// only its absolutely positioned children came or went, else the parent
    /// of the nearest absolutely positioned box at or above the source. The
    /// block is a positioned (it contains them) flex or grid container, not
    /// the root, not restyled, repositioned, dirty or on a hoist's path, has a
    /// saved final layout, and is shown under `root`.
    pub(super) fn absolute_boundary(
        &self,
        source: NodeId,
        root: NodeId,
        deferred: &IdMap<NodeId, bool>,
    ) -> Option<NodeId> {
        let restyled = |n| deferred.get(&n) == Some(&true);
        let block = if !restyled(source) && self.only_absolute_children(source) {
            source
        } else {
            let mut at = source;
            while self.taffy.style(at).ok()?.position != taffy::Position::Absolute
                || self.absolute.repositioned.contains(&at)
            {
                at = self.taffy.parent(at)?;
            }
            self.taffy.parent(at)?
        };
        // Static: it does not contain them. A block container's children's
        // margins may collapse through it with its parent's, so only a flex
        // or grid container, a formatting context of its own, replays. The
        // root's replay is the root pass.
        let style = self.taffy.style(block).ok()?;
        if style.position == taffy::Position::Static
            || !matches!(style.display, taffy::Display::Flex | taffy::Display::Grid)
            || block == root
            || restyled(block)
            || self.absolute.repositioned.contains(&block)
            || self.hoist_paths.contains(&block)
            || self.hoist_paths_prior.contains(&block)
            || self.taffy.dirty(block).unwrap_or(true)
            || self.taffy.saved_layout(block).is_none()
        {
            return None;
        }
        let mut at = Some(block);
        while let Some(n) = at {
            if self.taffy.style(n).ok()?.display == taffy::Display::None {
                return None;
            }
            if n == root {
                return Some(block);
            }
            at = self.taffy.parent(n);
        }
        None
    }
}
