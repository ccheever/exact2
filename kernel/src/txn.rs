//! The transactional apply engine: validate the whole batch, then apply.
//!
//! A batch is validated in its entirety against a staged view of the tree
//! (the current arena plus the batch's own earlier ops) before anything is
//! written. Rejection yields a typed [`ApplyError`] and leaves the arena, the
//! layout engine, the selector index, and every receipt exactly as they were.
//! Both ingress forms — decoded EXWF frames and in-process structured apply —
//! enter here; there is no other write path.
//!
//! The apply phase assumes what validation established. If that assumption
//! ever fails it stops and reports [`ApplyError::Internal`] — loud and typed,
//! never a silent skip and never a panic.

use crate::id::{IdMap, IdSet};
use crate::sorted::{SortedMap, SortedSet};

use crate::arena::NodeArena;
use crate::error::{ApplyError, StyleDomainError};
use crate::generated::{InheritedStyle, NodeType, PropId, StyleMask};
use crate::id::{NodeFlags, NodeKey, ViewId};
use crate::layout::LayoutMirror;
use crate::selector::SelectorIndex;
use crate::wire::Op;

/// The deepest a node may sit below the top of its tree (a root is 0). Layout
/// recurses once per level: a release build uses up to ~3.5 KiB of stack a
/// level (flex; grid 3.3, block 1.9, nested inline runs 0.7), and hosts lay
/// out on their main thread, whose stack is 1 MiB on iOS. 128 levels take
/// ~450 KiB of it; the deepest app tree measured is 9 (Caltrain). A deeper
/// batch is refused with [`ApplyError::TooDeep`] instead of aborting the
/// process on a stack overflow (20,000 levels did).
pub const MAX_DEPTH: u32 = 128;

/// A destroyed node that leaves with its `-exact-exit-animation` (LLP 1063): the
/// root of a destroyed subtree whose parent survived the batch and that no
/// batch-local creation owns, or a virtualized list's row whose item left the
/// data. Its descendants go with it; their own exit rows never play.
#[derive(Debug, Clone, PartialEq)]
pub struct Exit {
    /// The destroyed node; it resolves to nothing after the commit.
    pub key: NodeKey,
    /// The surviving parent it leaves from.
    pub parent: NodeKey,
    /// Its `-exact-exit-animation` row as it was when destroyed.
    pub animations: exact_motion::Animations,
}

/// What a committed batch changed. Every key is generation-checked; a
/// `destroyed` key resolves to nothing after the commit by construction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommitReceipt {
    /// Producer batch number from the frame (0 for structured apply without one).
    pub batch: u64,
    /// Root scope from the frame (0 = whole kernel).
    pub root_id: u32,
    /// The kernel epoch after this commit.
    pub epoch: u64,
    /// Nodes allocated by this batch and still live at its end, in allocation order.
    pub created: Vec<NodeKey>,
    /// Nodes destroyed by this batch (subtrees included), in destruction order.
    pub destroyed: Vec<NodeKey>,
    /// Live nodes whose props, style, or children changed (created nodes excluded).
    pub touched: Vec<NodeKey>,
    /// Whether any change can move geometry; a host that only paints may skip layout otherwise.
    pub layout_invalidated: bool,
    /// The destroyed nodes that play an exit before a host removes them.
    pub exits: Vec<Exit>,
    /// Shared-element names handed from a destroyed node to a created one
    /// (LLP 1013.000 D3).
    pub handoffs: Vec<crate::Handoff>,
    /// Live nodes whose `display` changed: their descendants' animations
    /// are cancelled or restarted (LLP 1055.000 D15; CSS Animations 1 §3).
    pub display_changed: Vec<NodeKey>,
    /// Consumers whose `animation-timeline` name resolves to another
    /// timeline after this commit, beyond those it created or touched (LLP
    /// 1057.003 D4): a node inserted, removed or moved, or a row set on
    /// another node, can change what a name finds.
    pub timelines: Vec<NodeKey>,
    /// Live nodes a producer reused for a new mount (LLP 1078: a list row
    /// rebound to another item), whatever else the batch did to them: to
    /// motion and to a host's per-node state each is a node destroyed and
    /// created by this commit (no transition from what it showed, its
    /// animations from their start). The kernel never fills it; the producer
    /// that rebinds does.
    pub renewed: Vec<NodeKey>,
    /// List rows that showed in their port for the first time since they
    /// were mounted out of it ([`crate::Kernel::reveal`]; LLP 1055 D13):
    /// the animations below them that waited for that start now. The
    /// producer that mounts rows fills it, as it does `renewed`.
    pub revealed: Vec<NodeKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Live(NodeType),
    Created(NodeType),
    Destroyed,
    Unknown,
}

/// How an ancestor walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    Ended,
    TooDeep,
    Cycle,
}

/// The batch's view of the tree during validation: the arena plus overrides.
struct Staged<'a> {
    arena: &'a NodeArena,
    created: IdMap<ViewId, NodeType>,
    destroyed: IdSet<ViewId>,
    parents: IdMap<ViewId, Option<ViewId>>,
    children: IdMap<ViewId, Vec<ViewId>>,
    roots_added: IdSet<ViewId>,
    roots_removed: IdSet<ViewId>,
}

impl<'a> Staged<'a> {
    fn new(arena: &'a NodeArena) -> Self {
        Staged {
            arena,
            created: IdMap::default(),
            destroyed: IdSet::default(),
            parents: IdMap::default(),
            children: IdMap::default(),
            roots_added: IdSet::default(),
            roots_removed: IdSet::default(),
        }
    }

    fn state(&self, id: ViewId) -> State {
        if self.destroyed.contains(&id) {
            return State::Destroyed;
        }
        if let Some(t) = self.created.get(&id) {
            return State::Created(*t);
        }
        match self.arena.slot_of(id) {
            Some(slot) => State::Live(self.arena.node_type(slot)),
            None => State::Unknown,
        }
    }

    /// The node type of a live-or-created id, or the rejection for anything else.
    fn require(&self, op_index: usize, id: ViewId) -> Result<NodeType, ApplyError> {
        match self.state(id) {
            State::Live(t) | State::Created(t) => Ok(t),
            State::Destroyed => Err(ApplyError::DestroyedInBatch { op_index, id }),
            State::Unknown => Err(ApplyError::UnknownView { op_index, id }),
        }
    }

    fn parent_of(&self, id: ViewId) -> Option<ViewId> {
        if let Some(p) = self.parents.get(&id) {
            return *p;
        }
        let slot = self.arena.slot_of(id)?;
        self.arena.parent(slot).map(|p| self.arena.local_id(p))
    }

    /// The staged children of `id`. A child that left in this batch still
    /// sits in the list it left (so leaving is O(1)); its staged parent
    /// says it is gone.
    fn children_of(&self, id: ViewId) -> Vec<ViewId> {
        let kept = |c: &ViewId| self.parent_of(*c) == Some(id);
        if let Some(c) = self.children.get(&id) {
            #[cfg(test)]
            count(c.len(), 0);
            return c.iter().copied().filter(kept).collect();
        }
        match self.arena.slot_of(id) {
            Some(slot) => {
                #[cfg(test)]
                count(self.arena.children(slot).len(), 0);
                self.arena
                    .children(slot)
                    .iter()
                    .map(|c| self.arena.local_id(*c))
                    .filter(kept)
                    .collect()
            }
            None => Vec::new(),
        }
    }

    fn is_root(&self, id: ViewId) -> bool {
        if self.roots_removed.contains(&id) {
            return false;
        }
        if self.roots_added.contains(&id) {
            return true;
        }
        self.arena
            .slot_of(id)
            .is_some_and(|s| self.arena.is_root(s))
    }

    /// Every ancestor of `id`, walked once per `SetChildren` rather than once
    /// per child, and at most [`MAX_DEPTH`] steps up: with that many above
    /// `id`, its children would already sit past the bound. Every validated
    /// SetChildren rejects cycles, so a revisit is impossible; it is refused
    /// as one rather than looping.
    fn ancestors(&self, id: ViewId) -> (IdSet<ViewId>, Walk) {
        let mut out = IdSet::default();
        let mut cur = self.parent_of(id);
        while let Some(p) = cur {
            if !out.insert(p) {
                return (out, Walk::Cycle);
            }
            if out.len() >= MAX_DEPTH as usize {
                return (out, Walk::TooDeep);
            }
            cur = self.parent_of(p);
        }
        (out, Walk::Ended)
    }

    /// Every node this batch attached somewhere, and its subtree, ends at most
    /// [`MAX_DEPTH`] below the top of its tree. Depths are memoized upward and
    /// a checked subtree is not entered again, so a batch walks each affected
    /// node once.
    fn check_depth(&self, arrivals: &[(usize, ViewId)]) -> Result<(), ApplyError> {
        let mut depths: IdMap<ViewId, u32> = IdMap::default();
        let mut checked: IdSet<ViewId> = IdSet::default();
        for &(op_index, top) in arrivals {
            if checked.contains(&top)
                || matches!(self.state(top), State::Destroyed | State::Unknown)
            {
                continue;
            }
            let mut stack = vec![(top, self.depth(top, &mut depths))];
            while let Some((id, depth)) = stack.pop() {
                if depth > MAX_DEPTH {
                    return Err(ApplyError::TooDeep {
                        op_index,
                        id,
                        depth,
                    });
                }
                if checked.insert(id) {
                    stack.extend(self.children_of(id).into_iter().map(|c| (c, depth + 1)));
                }
            }
        }
        Ok(())
    }

    /// How many ancestors `id` has in the staged tree, memoized along the way.
    /// A walk past [`MAX_DEPTH`] stops there: that is already a refusal.
    fn depth(&self, id: ViewId, memo: &mut IdMap<ViewId, u32>) -> u32 {
        let mut path = Vec::new();
        let mut cur = id;
        let mut depth = loop {
            if let Some(&known) = memo.get(&cur) {
                break known;
            }
            if path.len() > MAX_DEPTH as usize {
                return MAX_DEPTH + 1;
            }
            match self.parent_of(cur) {
                Some(parent) => {
                    path.push(cur);
                    cur = parent;
                }
                None => {
                    memo.insert(cur, 0);
                    break 0;
                }
            }
        };
        for node in path.into_iter().rev() {
            depth += 1;
            memo.insert(node, depth);
        }
        depth
    }

    fn destroy(&mut self, id: ViewId) {
        // Its parent's staged list keeps it; `children_of` filters it out.
        self.parents.insert(id, None);
        if self.is_root(id) {
            self.roots_added.remove(&id);
            self.roots_removed.insert(id);
        }
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            self.destroyed.insert(n);
            self.created.remove(&n);
            stack.extend(self.children_of(n));
        }
    }
}

fn validate(arena: &NodeArena, ops: &[Op]) -> Result<(), ApplyError> {
    let mut staged = Staged::new(arena);
    let mut creates: u64 = 0;
    // Children placed under a parent they did not have: what can deepen a tree.
    let mut arrivals: Vec<(usize, ViewId)> = Vec::new();
    for (op_index, op) in ops.iter().enumerate() {
        match op {
            Op::CreateView { id, node_type } => match staged.state(*id) {
                State::Live(existing) | State::Created(existing) => {
                    if existing != *node_type {
                        return Err(ApplyError::TypeMismatch {
                            op_index,
                            id: *id,
                            existing,
                            requested: *node_type,
                        });
                    }
                }
                State::Destroyed | State::Unknown => {
                    staged.destroyed.remove(id);
                    staged.created.insert(*id, *node_type);
                    staged.parents.insert(*id, None);
                    staged.children.insert(*id, Vec::new());
                    creates += 1;
                }
            },
            Op::DestroyView { id } => {
                staged.require(op_index, *id)?;
                staged.destroy(*id);
            }
            Op::SetProp { id, prop, value } => {
                staged.require(op_index, *id)?;
                if value.kind() != prop.kind() {
                    return Err(ApplyError::PropKindMismatch {
                        op_index,
                        prop: *prop,
                        expected: prop.kind(),
                        actual: value.kind(),
                    });
                }
            }
            Op::SetStyle { id, patch } => {
                staged.require(op_index, *id)?;
                if let Err(error) = patch.validate_domain() {
                    return Err(match error {
                        StyleDomainError::InvalidLineHeight => {
                            ApplyError::InvalidLineHeight { op_index }
                        }
                        StyleDomainError::NonFinite(style) => {
                            ApplyError::NonFiniteStyle { op_index, style }
                        }
                        StyleDomainError::AutoNotAdmitted(style) => {
                            ApplyError::AutoNotAdmitted { op_index, style }
                        }
                        StyleDomainError::TooManyTracks { style, count } => {
                            ApplyError::TooManyTracks {
                                op_index,
                                style,
                                count,
                            }
                        }
                        StyleDomainError::InvalidGridTrack(style) => {
                            ApplyError::InvalidGridTrack { op_index, style }
                        }
                        StyleDomainError::InvalidGridSpan(style) => {
                            ApplyError::InvalidGridSpan { op_index, style }
                        }
                        StyleDomainError::InvalidTransition(error) => {
                            ApplyError::InvalidTransition { op_index, error }
                        }
                        StyleDomainError::InvalidAnimation(error) => {
                            ApplyError::InvalidAnimation { op_index, error }
                        }
                    });
                }
            }
            Op::ClearProp { id, .. } | Op::ClearStyle { id, .. } => {
                staged.require(op_index, *id)?;
            }
            Op::SetChildren { id, children } => {
                let node_type = staged.require(op_index, *id)?;
                if !node_type.can_hold_children() && !children.is_empty() {
                    return Err(ApplyError::LeafCannotHoldChildren {
                        op_index,
                        id: *id,
                        node_type,
                    });
                }
                let mut seen = IdSet::with_capacity_and_hasher(children.len(), Default::default());
                let (ancestors, walk) = if children.is_empty() {
                    (IdSet::default(), Walk::Ended)
                } else {
                    staged.ancestors(*id)
                };
                for child in children {
                    if *child == *id {
                        return Err(ApplyError::SelfChild { op_index, id: *id });
                    }
                    if !seen.insert(*child) {
                        return Err(ApplyError::DuplicateChild {
                            op_index,
                            parent: *id,
                            child: *child,
                        });
                    }
                    let child_type = staged.require(op_index, *child)?;
                    if node_type == NodeType::Text && child_type != NodeType::Text {
                        return Err(ApplyError::InlineRunNotText {
                            op_index,
                            parent: *id,
                            child: *child,
                            node_type: child_type,
                        });
                    }
                    // @ref LLP 1055 D3 — an `svg` or `g` holds SVG elements
                    // and nothing else; an SVG element lives only there.
                    let svg_parent = node_type.is_svg_container();
                    if svg_parent != child_type.is_svg_element() {
                        return Err(ApplyError::SvgContent {
                            op_index,
                            parent: *id,
                            child: *child,
                        });
                    }
                    if staged.is_root(*child) {
                        return Err(ApplyError::RootAsChild {
                            op_index,
                            parent: *id,
                            child: *child,
                        });
                    }
                    if walk == Walk::Cycle || ancestors.contains(child) {
                        return Err(ApplyError::Cycle {
                            op_index,
                            parent: *id,
                            child: *child,
                        });
                    }
                }
                if walk == Walk::TooDeep {
                    return Err(ApplyError::TooDeep {
                        op_index,
                        id: children[0],
                        depth: MAX_DEPTH + 1,
                    });
                }
                for old in staged.children_of(*id) {
                    if !seen.contains(&old) {
                        staged.parents.insert(old, None);
                    }
                }
                // A child arriving from another parent leaves it by this
                // write alone: that parent's staged list filters it out.
                for child in children {
                    if staged.parent_of(*child) != Some(*id) {
                        arrivals.push((op_index, *child));
                    }
                    staged.parents.insert(*child, Some(*id));
                }
                staged.children.insert(*id, children.clone());
            }
            Op::AttachRoot { id } => {
                staged.require(op_index, *id)?;
                if staged.parent_of(*id).is_some() {
                    return Err(ApplyError::RootHasParent { op_index, id: *id });
                }
                staged.roots_removed.remove(id);
                staged.roots_added.insert(*id);
            }
        }
    }
    staged.check_depth(&arrivals)?;
    // Conservative: ignores free-slot reuse, so it only ever refuses a batch that
    // would have fit within a few slots of the four-billion-node ceiling.
    if arena.slot_space() as u64 + creates > u32::MAX as u64 {
        return Err(ApplyError::SlotSpaceExhausted);
    }
    Ok(())
}

/// Everything the apply engine writes.
pub struct Target<'a> {
    /// The arena.
    pub arena: &'a mut NodeArena,
    /// The layout engine's side of the commit. A kernel that builds its
    /// tree only when a layout is first asked for (a browser host, whose
    /// browser lays out) passes [`crate::layout::Unmirrored`]: new nodes get
    /// no engine node, and every other engine call follows a node's engine
    /// handle, so none reaches a tree.
    pub layout: &'a mut dyn LayoutMirror,
    /// The selector index.
    pub selectors: &'a mut SelectorIndex,
}

fn live_slot(arena: &NodeArena, op_index: usize, id: ViewId) -> Result<u32, ApplyError> {
    arena.slot_of(id).ok_or(ApplyError::Internal {
        op_index,
        what: "validated op targets a dead id",
    })
}

/// Validate every op, then apply them all. On `Err`, nothing changed — except
/// for [`ApplyError::Internal`], which is a kernel defect (see its docs).
pub fn apply(
    target: Target<'_>,
    ops: &[Op],
    batch: u64,
    root_id: u32,
    epoch: u64,
) -> Result<CommitReceipt, ApplyError> {
    apply_document(target, ops, batch, root_id, epoch, None)
}

pub(crate) fn apply_document(
    target: Target<'_>,
    ops: &[Op],
    batch: u64,
    root_id: u32,
    epoch: u64,
    language: Option<(&str, crate::Direction)>,
) -> Result<CommitReceipt, ApplyError> {
    validate(target.arena, ops)?;
    let Target {
        arena,
        layout,
        selectors,
    } = target;
    let mut receipt = CommitReceipt {
        batch,
        root_id,
        epoch,
        ..CommitReceipt::default()
    };
    let mut touched: Vec<NodeKey> = Vec::new();
    let mut created: IdSet<u32> = IdSet::default();
    let mut detach = Detach::default();
    // Children detached by this batch that declare an exit: the parent each left.
    let mut left: IdMap<u32, u32> = IdMap::default();
    // Destroyed nodes carrying a shared-element name (LLP 1013.000 D3).
    let mut leavers = Vec::new();
    // A node this batch creates and then styles: its first engine style
    // would be derived twice, at the create (from the default style) and at
    // the style. It is created with the engine's default instead and derived
    // once: at its style when that changes a layout row, else at the end of
    // the batch (`unstyled`). A list row's mount is a create and a style per
    // node.
    let styled: IdSet<u32> = ops
        .iter()
        .filter_map(|op| match op {
            Op::SetStyle { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    let mut unstyled: IdSet<u32> = IdSet::default();

    let applied = (|| -> Result<(), ApplyError> {
        for (op_index, op) in ops.iter().enumerate() {
            if !matches!(op, Op::DestroyView { .. }) {
                detach.flush(arena, layout, selectors);
            }
            match op {
                Op::CreateView { id, node_type } => {
                    if arena.slot_of(*id).is_some() {
                        continue; // live no-op
                    }
                    let slot = arena
                        .alloc(*id, *node_type)
                        .map_err(|_| ApplyError::Internal {
                            op_index,
                            what: "validated batch exhausted slot space",
                        })?;
                    let leaf = if styled.contains(id) {
                        unstyled.insert(slot);
                        layout.new_leaf_unstyled(slot, node_type.is_measured_leaf())
                    } else {
                        layout.new_leaf(arena, slot, node_type.is_measured_leaf())
                    };
                    if let Some(node) = leaf {
                        arena.set_taffy(slot, Some(node));
                    }
                    created.insert(slot);
                    receipt.created.push(arena.key(slot));
                    receipt.layout_invalidated = true;
                }
                Op::DestroyView { id } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    if let Some(exit) = exit(arena, &created, &left, slot) {
                        receipt.exits.push(exit);
                    }
                    if let Some(parent) = arena.parent(slot) {
                        if arena.node_type(parent) == NodeType::Text {
                            invalidate_text(arena, layout, parent);
                        }
                        detach.parent(parent);
                        arena.flags_mut(parent).insert(NodeFlags::CHILDREN_DIRTY);
                        touched.push(arena.key(parent));
                    }
                    detach.begin();
                    for s in arena.subtree(slot) {
                        if let Some(test_id) = arena.props(s).str(PropId::TestId) {
                            detach.selectors.push((test_id.to_string(), s));
                        }
                        if let Some(id) = arena.props(s).str(PropId::Id) {
                            detach.selectors.push((SelectorIndex::id_key(id), s));
                        }
                        if let Some(node) = arena.taffy(s) {
                            detach.nodes.push(node);
                        }
                        if !created.contains(&s) {
                            leavers.extend(crate::handoff::leaver(arena, s));
                        }
                        receipt.destroyed.push(arena.key(s));
                        created.remove(&s);
                        arena.free_slot(s);
                    }
                    receipt.layout_invalidated = true;
                }
                Op::SetProp { id, prop, value } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    if arena.props(slot).get(*prop) == Some(value) {
                        continue;
                    }
                    let old = arena.set_prop(slot, *prop, value.clone());
                    if *prop == PropId::TestId {
                        selectors.update(
                            slot,
                            old.as_ref().and_then(|v| v.as_str()),
                            value.as_str(),
                        );
                    }
                    if *prop == PropId::Id {
                        selectors.update_id(
                            slot,
                            old.as_ref().and_then(|v| v.as_str()),
                            value.as_str(),
                        );
                    }
                    arena.flags_mut(slot).insert(NodeFlags::PROPS_DIRTY);
                    view_box_changed(arena, layout, slot, *prop, &mut receipt);
                    if matches!(
                        prop,
                        PropId::Type | PropId::AccessibilityRole | PropId::Href
                    ) {
                        if let Some(node) = arena.taffy(slot) {
                            layout.restyle(arena, slot, node);
                        }
                        receipt.layout_invalidated = true;
                    }
                    if prop.affects_measure() {
                        invalidate_text(arena, layout, slot);
                        receipt.layout_invalidated = true;
                    } else {
                        arena.revise_text(slot, false);
                        if *prop == PropId::Href {
                            invalidate_text_sources(arena, slot);
                        }
                    }
                    touched.push(arena.key(slot));
                }
                Op::ClearProp { id, prop } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    if let Some(old) = arena.remove_prop(slot, *prop) {
                        if *prop == PropId::TestId {
                            selectors.update(slot, old.as_str(), None);
                        }
                        if *prop == PropId::Id {
                            selectors.update_id(slot, old.as_str(), None);
                        }
                        arena.flags_mut(slot).insert(NodeFlags::PROPS_DIRTY);
                        view_box_changed(arena, layout, slot, *prop, &mut receipt);
                        if matches!(
                            prop,
                            PropId::Type | PropId::AccessibilityRole | PropId::Href
                        ) {
                            if let Some(node) = arena.taffy(slot) {
                                layout.restyle(arena, slot, node);
                            }
                            receipt.layout_invalidated = true;
                        }
                        if prop.affects_measure() {
                            invalidate_text(arena, layout, slot);
                            receipt.layout_invalidated = true;
                        } else {
                            arena.revise_text(slot, false);
                            if *prop == PropId::Href {
                                invalidate_text_sources(arena, slot);
                            }
                        }
                        touched.push(arena.key(slot));
                    }
                }
                Op::SetStyle { id, patch } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    let changed = arena.style(slot).changed_mask(patch);
                    if changed.is_empty() {
                        continue;
                    }
                    let excluded = crate::flow::is_exclusion(arena, slot);
                    let mut next = arena.style(slot).clone();
                    next.apply_patch(patch);
                    arena.set_style(slot, next);
                    arena.update_exclusion_count(slot, excluded);
                    if changed.has(crate::StyleId::Display) {
                        receipt.display_changed.push(arena.key(slot));
                    }
                    style_changed(arena, layout, slot, changed, &mut receipt);
                    if changed.intersects(StyleMask::LAYOUT) {
                        unstyled.remove(&slot);
                    }
                    touched.push(arena.key(slot));
                    propagate_inherited(arena, layout, slot, changed, &mut touched, &mut receipt);
                }
                Op::ClearStyle { id, mask } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    let changed = arena.style(slot).cleared_mask(*mask);
                    if changed.is_empty() {
                        continue;
                    }
                    let excluded = crate::flow::is_exclusion(arena, slot);
                    let mut next = arena.style(slot).clone();
                    next.clear(*mask);
                    arena.set_style(slot, next);
                    arena.update_exclusion_count(slot, excluded);
                    if changed.has(crate::StyleId::Display) {
                        receipt.display_changed.push(arena.key(slot));
                    }
                    style_changed(arena, layout, slot, changed, &mut receipt);
                    touched.push(arena.key(slot));
                    propagate_inherited(arena, layout, slot, changed, &mut touched, &mut receipt);
                }
                Op::SetChildren { id, children } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    let new: Vec<u32> = children
                        .iter()
                        .map(|c| arena.slot_of(*c))
                        .collect::<Option<Vec<u32>>>()
                        .ok_or(ApplyError::Internal {
                            op_index,
                            what: "validated child is not live",
                        })?;
                    if arena.children(slot) == new.as_slice() {
                        continue;
                    }
                    let old: Vec<u32> = arena.children(slot).to_vec();
                    let retained: IdSet<u32> = new.iter().copied().collect();
                    let detached: Vec<_> = old
                        .iter()
                        .copied()
                        .filter(|o| !retained.contains(o))
                        .map(|o| (o, arena.computed_inherited(o)))
                        .collect();
                    for o in &old {
                        if !retained.contains(o) {
                            arena.set_parent(*o, None);
                            // A producer detaches before it destroys: the
                            // parent an exit leaves from is the one it had.
                            if leaving_with(arena, *o).is_some() {
                                left.insert(*o, slot);
                            }
                        }
                    }
                    // A child arriving from another parent takes its inherited
                    // rows from its new ancestors: remember what it computed
                    // under the old ones, to propagate only what differs. Orphans
                    // and fresh nodes already compute their own/default rows too.
                    let moved: Vec<(u32, InheritedStyle)> = new
                        .iter()
                        .copied()
                        .filter(|n| arena.parent(*n) != Some(slot))
                        .map(|n| {
                            let before = arena.computed_inherited(n);
                            (n, before)
                        })
                        .collect();
                    // Each arriving child leaves its previous parent, and each
                    // such parent is pruned once — one pass and one engine update
                    // however many of its children moved here.
                    let mut sources: Vec<u32> = Vec::new();
                    let mut seen: IdSet<u32> = IdSet::default();
                    for n in &new {
                        if let Some(p) = arena.parent(*n) {
                            if p != slot && seen.insert(p) {
                                sources.push(p);
                            }
                        }
                        arena.set_parent(*n, Some(slot));
                    }
                    for p in sources {
                        if arena.node_type(p) == NodeType::Text {
                            invalidate_text(arena, layout, p);
                        }
                        #[cfg(test)]
                        count(arena.children(p).len(), 0);
                        arena.prune_children(p);
                        sync_children(arena, layout, p);
                        crate::kernel::cover_children_changed(arena, layout, p);
                        arena.flags_mut(p).insert(NodeFlags::CHILDREN_DIRTY);
                        touched.push(arena.key(p));
                    }
                    arena.set_children(slot, new);
                    sync_children(arena, layout, slot);
                    crate::kernel::cover_children_changed(arena, layout, slot);
                    arena.flags_mut(slot).insert(NodeFlags::CHILDREN_DIRTY);
                    if arena.node_type(slot) == NodeType::Text {
                        invalidate_text(arena, layout, slot);
                    }
                    touched.push(arena.key(slot));
                    receipt.layout_invalidated = true;
                    for (orphan, before) in detached {
                        inherited_after_move(
                            arena,
                            layout,
                            orphan,
                            before,
                            &mut touched,
                            &mut receipt,
                        );
                    }
                    for (m, before) in moved {
                        inherited_after_move(arena, layout, m, before, &mut touched, &mut receipt);
                    }
                }
                Op::AttachRoot { id } => {
                    let slot = live_slot(arena, op_index, *id)?;
                    if !arena.is_root(slot) {
                        arena.set_root(slot, true);
                        // A root's engine style differs from a child's (a static
                        // root is positioned): re-derive it now that the node is one.
                        if let Some(node) = arena.taffy(slot) {
                            layout.restyle(arena, slot, node);
                        }
                        touched.push(arena.key(slot));
                        receipt.layout_invalidated = true;
                    }
                }
            }
        }
        Ok(())
    })();
    detach.flush(arena, layout, selectors);
    applied?;
    for slot in unstyled {
        if let Some(node) = arena.is_live(slot).then(|| arena.taffy(slot)).flatten() {
            layout.restyle(arena, slot, node);
        }
    }

    if let Some((lang, direction)) = language {
        let changed = arena.document_language != lang;
        let turned = arena.document_style.direction != direction;
        if changed || turned {
            arena.document_language = lang.into();
            arena.document_style.direction = direction;
            arena.document_style.mask.set(crate::StyleId::Direction);
            let rows = StyleMask::of(crate::StyleId::Direction);
            let slots: Vec<_> = arena.iter_live().collect();
            for slot in slots {
                if turned
                    && arena
                        .inherited_source(slot, crate::StyleId::Direction)
                        .is_none()
                {
                    inherited_changed(arena, layout, slot, rows, &mut receipt);
                    touched.push(arena.key(slot));
                } else if changed
                    && matches!(arena.node_type(slot), NodeType::Text | NodeType::TextInput)
                {
                    invalidate_text(arena, layout, slot);
                    receipt.layout_invalidated = true;
                    touched.push(arena.key(slot));
                }
            }
        }
    }

    relative::resolve(arena, layout, &mut touched, &mut receipt);

    // A later op can destroy an exit's parent: then that op's root leaves
    // (and exits, if it declares one) and this node goes inside it.
    receipt
        .exits
        .retain(|exit| arena.resolve(exit.parent).is_some());
    // Compare whole keys: a slot freed and reallocated in one batch carries two generations.
    receipt.created.retain(|key| {
        arena
            .resolve(*key)
            .is_some_and(|slot| created.contains(&slot))
    });
    receipt.handoffs = crate::handoff::pair(arena, &receipt.created, leavers);
    // Publication needs sorted unique live keys, not a tree update for every op.
    // Generations discard touches from a node destroyed earlier in this batch:
    // only a slot's live key resolves, so a set of slots, read back in order,
    // is the sorted unique live keys, with no sort.
    let mut live = crate::sorted::SlotSet::default();
    for key in touched {
        if let Some(slot) = arena.resolve(key).filter(|slot| !created.contains(slot)) {
            live.insert(slot);
        }
    }
    let mut touched: Vec<NodeKey> = live.iter().map(|slot| arena.key(slot)).collect();
    // Receipts outlive this batch; do not retain scratch for duplicates or dead nodes.
    touched.shrink_to_fit();
    receipt.touched = touched;
    Ok(receipt)
}

/// The exit a destroyed subtree root plays, if any (LLP 1063). Not for a
/// node this batch created (nothing presented it), a root or inline run
/// (nowhere to stay), or a virtualized list's row the window scrolled away
/// ([`leaving_with`]).
fn exit(
    arena: &NodeArena,
    created: &IdSet<u32>,
    left: &IdMap<u32, u32>,
    slot: u32,
) -> Option<Exit> {
    if created.contains(&slot) {
        return None;
    }
    let parent = arena
        .parent(slot)
        .or_else(|| left.get(&slot).copied())
        .filter(|p| arena.is_live(*p))?;
    if created.contains(&parent) || arena.node_type(parent) == NodeType::Text {
        return None;
    }
    Some(Exit {
        key: arena.key(slot),
        parent: arena.key(parent),
        animations: leaving_with(arena, slot)?.clone(),
    })
}

/// The `-exact-exit-animation` `slot` plays as it leaves: its own row, unless it
/// is a virtualized list's row wrapper (it carries `listItemKey`). The window
/// destroys rows that scroll away as well as rows whose item left the data,
/// and empties the key of the latter alone; such a wrapper leaves as the
/// row, where the window placed it, with its one root's row.
fn leaving_with(arena: &NodeArena, slot: u32) -> Option<&exact_motion::Animations> {
    let own = match arena.props(slot).str(PropId::ListItemKey) {
        None => slot,
        Some("") => match arena.children(slot) {
            [root] => *root,
            _ => return None,
        },
        Some(_) => return None,
    };
    Some(&arena.style(own).rare.exit_animation).filter(|a| !a.0.is_empty())
}

/// Push the arena's child list for `parent` into the layout engine. A `Text`
/// parent's children are inline runs: they are measured with it, not laid out.
fn sync_children(arena: &NodeArena, layout: &mut dyn LayoutMirror, parent: u32) {
    let Some(node) = arena.taffy(parent) else {
        return;
    };
    let _entries = layout.sync_children(arena, parent, node);
    #[cfg(test)]
    count(_entries, 0);
}

/// A run of `DestroyView`s detaches lazily. Each op records its parent once
/// and its subtree's engine nodes and `testId`s; [`Detach::flush`] — before
/// any other op reads the tree, and when the batch ends — prunes every
/// recorded parent in one pass and one engine update, then removes the
/// engine nodes with no sibling scans. Destroying N children of one parent
/// one op each costs O(N) this way, not O(N²).
#[derive(Default)]
struct Detach {
    parents: Vec<u32>,
    seen: IdSet<u32>,
    selectors: Vec<(String, u32)>,
    nodes: Vec<taffy::NodeId>,
    /// Where each op's subtree starts in `nodes`: root first, then preorder.
    runs: Vec<usize>,
}

impl Detach {
    fn parent(&mut self, parent: u32) {
        if self.seen.insert(parent) {
            self.parents.push(parent);
        }
    }

    fn begin(&mut self) {
        self.runs.push(self.nodes.len());
    }

    fn flush(
        &mut self,
        arena: &mut NodeArena,
        layout: &mut dyn LayoutMirror,
        selectors: &mut SelectorIndex,
    ) {
        if self.runs.is_empty() {
            return;
        }
        // A parent destroyed later in the run was freed with its list.
        for parent in self.parents.drain(..) {
            if arena.is_live(parent) {
                #[cfg(test)]
                count(arena.children(parent).len(), 0);
                arena.prune_children(parent);
                sync_children(arena, layout, parent);
                crate::kernel::cover_children_changed(arena, layout, parent);
            }
        }
        self.seen.clear();
        let mut leaving: SortedMap<String, IdSet<u32>> = SortedMap::new();
        for (test_id, slot) in self.selectors.drain(..) {
            leaving
                .get_or_insert_with(test_id, IdSet::default)
                .insert(slot);
        }
        for (test_id, slots) in leaving {
            selectors.remove_all(&test_id, &slots);
        }
        // A later op can only have destroyed an ancestor of an earlier one's
        // root (a destroyed node's descendants go with it), and each subtree
        // lists its root first. Removing the ops' nodes in reverse therefore
        // removes every engine parent before its children: no removal scans
        // a sibling list.
        let mut end = self.nodes.len();
        for start in self.runs.drain(..).rev() {
            for &node in &self.nodes[start..end] {
                #[cfg(test)]
                count(0, layout.attached(node) as usize);
                layout.remove(node);
            }
            end = start;
        }
        self.nodes.clear();
    }
}

#[cfg(test)]
thread_local! {
    /// Child-list entries this thread's transactions scanned, pruned or
    /// handed to the engine, and engine nodes removed while still attached
    /// to a parent: the batching above, counted.
    pub(crate) static CHILD_WORK: std::cell::Cell<(usize, usize)> =
        const { std::cell::Cell::new((0, 0)) };
}

#[cfg(test)]
fn count(entries: usize, attached: usize) {
    CHILD_WORK.with(|w| {
        let (e, a) = w.get();
        w.set((e + entries, a + attached));
    });
}

fn invalidate_text(arena: &mut NodeArena, layout: &mut dyn LayoutMirror, slot: u32) {
    let owner = arena.measure_owner(slot);
    arena.revise_text(owner, true);
    arena.flags_mut(owner).insert(NodeFlags::TEXT_DIRTY);
    if let Some(node) = arena.taffy(owner) {
        layout.mark_dirty(node);
    }
}

/// A changed logical ancestry can change run-origin navigation/paint metadata
/// without changing metrics. Scratch is bounded by this subtree's live owners.
fn invalidate_text_sources(arena: &mut NodeArena, slot: u32) {
    let owners: SortedSet<u32> = arena
        .subtree(slot)
        .into_iter()
        .filter(|s| matches!(arena.node_type(*s), NodeType::Text | NodeType::TextInput))
        .map(|s| arena.measure_owner(s))
        .collect();
    for owner in owners {
        arena.revise_text(owner, false);
    }
}

fn inherited_after_move(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    before: InheritedStyle,
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    // A formerly inline node may now expose its own paragraph. Its old local
    // revision did not track edits consumed by its former owner.
    if matches!(arena.node_type(slot), NodeType::Text | NodeType::TextInput) {
        invalidate_text(arena, layout, slot);
    }
    invalidate_text_sources(arena, slot);
    let after = arena.computed_inherited(slot);
    let changed = before.changed_mask(&after);
    if !changed.is_empty() {
        inherited_changed(arena, layout, slot, changed, receipt);
        touched.push(arena.key(slot));
        propagate_inherited(arena, layout, slot, changed, touched, receipt);
    }
}

/// An `svg`'s `viewBox` is its natural aspect ratio (LLP 1055.000 D4): a
/// change restyles and re-measures the box.
fn view_box_changed(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    prop: PropId,
    receipt: &mut CommitReceipt,
) {
    if prop != PropId::ViewBox || arena.node_type(slot) != NodeType::Svg {
        return;
    }
    arena.flags_mut(slot).insert(NodeFlags::STYLE_DIRTY);
    if let Some(node) = arena.taffy(slot) {
        layout.restyle(arena, slot, node);
        layout.mark_dirty(node);
    }
    receipt.layout_invalidated = true;
}

fn style_changed(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    mask: StyleMask,
    receipt: &mut CommitReceipt,
) {
    if mask.has(crate::StyleId::Appearance) && arena.node_type(slot) == NodeType::TextInput {
        control_env_changed(arena, layout, slot);
        receipt.layout_invalidated = true;
    }
    if mask.intersects(StyleMask::LAYOUT) {
        arena.flags_mut(slot).insert(NodeFlags::STYLE_DIRTY);
        if let Some(node) = arena.taffy(slot) {
            layout.restyle(arena, slot, node);
        }
        // CSS `order` (feed F19) is the engine's child order in a flex or
        // grid container: an item's moves it, a container's `display`
        // decides whether its ordered children are reordered.
        if mask.has(crate::StyleId::Order) {
            if let Some(p) = arena.parent(slot) {
                sync_children(arena, layout, p);
            }
        }
        if mask.has(crate::StyleId::Display)
            && arena
                .children(slot)
                .iter()
                .any(|&c| arena.style(c).order != 0)
        {
            sync_children(arena, layout, slot);
        }
        // A replaced header's padding is part of its route's top cover.
        if arena.cover(slot) == Some(crate::kernel::HostCover::Whole) {
            if let Some((p, node)) = arena.parent(slot).and_then(|p| Some((p, arena.taffy(p)?))) {
                layout.restyle(arena, p, node);
                layout.mark_dirty(node);
            }
        }
        receipt.layout_invalidated = true;
    }
    if mask.intersects(StyleMask::TEXT) {
        invalidate_text(arena, layout, slot);
        receipt.layout_invalidated = true;
    }
    if !mask.minus(StyleMask::LAYOUT).is_empty() {
        arena.flags_mut(slot).insert(NodeFlags::PAINT_DIRTY);
        if !mask.intersects(StyleMask::TEXT) {
            arena.revise_text(slot, false);
        }
    }
}

/// CSS inheritance: an inherited row that changed on `slot` changes the
/// computed value of every logical descendant that does not set the row
/// itself. Mark and touch them, stopping under a descendant that overrides
/// every changed row, so the receipt names what changed and no host has to
/// re-derive descendants per frame (LLP 1035.000 D4).
fn propagate_inherited(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    changed: StyleMask,
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    let changed = changed.intersect(StyleMask::INHERITED);
    if changed.is_empty() {
        return;
    }
    let mut stack: Vec<(u32, StyleMask)> =
        arena.children(slot).iter().map(|c| (*c, changed)).collect();
    while let Some((s, rows)) = stack.pop() {
        let pass = rows.minus(arena.style(s).mask).minus(
            arena
                .control_text_start(s)
                .map_or(StyleMask::EMPTY, |start| start.mask),
        );
        if pass.is_empty() {
            continue;
        }
        inherited_changed(arena, layout, s, pass, receipt);
        touched.push(arena.key(s));
        stack.extend(arena.children(s).iter().map(|c| (*c, pass)));
    }
}

/// What a changed inherited value does at a node: text rows remeasure its
/// paragraph (the nearest measure owner); layout rows such as direction
/// also rederive the engine style of containers.
fn inherited_changed(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    rows: StyleMask,
    receipt: &mut CommitReceipt,
) {
    if rows.intersects(StyleMask::LAYOUT) {
        if let Some(node) = arena.taffy(slot) {
            layout.restyle(arena, slot, node);
        }
        receipt.layout_invalidated = true;
    }
    if rows.intersects(StyleMask::TEXT)
        && matches!(arena.node_type(slot), NodeType::Text | NodeType::TextInput)
    {
        invalidate_text(arena, layout, slot);
        receipt.layout_invalidated = true;
    }
    if !rows.minus(StyleMask::TEXT).is_empty() {
        arena.flags_mut(slot).insert(NodeFlags::PAINT_DIRTY);
        if !rows.intersects(StyleMask::TEXT) {
            arena.revise_text(slot, false);
        }
    }
}

/// A control's computed starting style changed outside an authored commit.
pub(crate) fn control_env_changed(arena: &mut NodeArena, layout: &mut dyn LayoutMirror, slot: u32) {
    arena.flags_mut(slot).insert(NodeFlags::STYLE_DIRTY);
    arena.flags_mut(slot).insert(NodeFlags::PAINT_DIRTY);
    invalidate_text(arena, layout, slot);
    if let Some(node) = arena.taffy(slot) {
        layout.restyle(arena, slot, node);
        layout.mark_dirty(node);
    }
}

pub(crate) mod relative;

#[cfg(test)]
mod tests;
