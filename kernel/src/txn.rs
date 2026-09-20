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

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::arena::NodeArena;
use crate::error::{ApplyError, StyleDomainError};
use crate::generated::{InheritedStyle, NodeType, PropId, StyleMask};
use crate::id::{NodeFlags, NodeKey, ViewId};
use crate::layout::LayoutTree;
use crate::selector::SelectorIndex;
use crate::style::taffy_style;
use crate::wire::Op;

/// What a committed batch changed. Every key is generation-checked; a
/// `destroyed` key resolves to nothing after the commit by construction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Live(NodeType),
    Created(NodeType),
    Destroyed,
    Unknown,
}

/// The batch's view of the tree during validation: the arena plus overrides.
struct Staged<'a> {
    arena: &'a NodeArena,
    created: HashMap<ViewId, NodeType>,
    destroyed: HashSet<ViewId>,
    parents: HashMap<ViewId, Option<ViewId>>,
    children: HashMap<ViewId, Vec<ViewId>>,
    roots_added: HashSet<ViewId>,
    roots_removed: HashSet<ViewId>,
}

impl<'a> Staged<'a> {
    fn new(arena: &'a NodeArena) -> Self {
        Staged {
            arena,
            created: HashMap::new(),
            destroyed: HashSet::new(),
            parents: HashMap::new(),
            children: HashMap::new(),
            roots_added: HashSet::new(),
            roots_removed: HashSet::new(),
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

    fn children_of(&self, id: ViewId) -> Vec<ViewId> {
        if let Some(c) = self.children.get(&id) {
            return c.clone();
        }
        match self.arena.slot_of(id) {
            Some(slot) => self
                .arena
                .children(slot)
                .iter()
                .map(|c| self.arena.local_id(*c))
                .collect(),
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

    fn is_ancestor(&self, ancestor: ViewId, node: ViewId) -> bool {
        // Every validated SetChildren rejects cycles, so the chain is finite; the
        // bound only turns an impossible cycle into a rejection instead of a hang.
        let bound = self.arena.live_count() + self.created.len() + 1;
        let mut cur = self.parent_of(node);
        let mut hops = 0usize;
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            hops += 1;
            if hops > bound {
                return true;
            }
            cur = self.parent_of(p);
        }
        false
    }

    fn detach(&mut self, parent: ViewId, child: ViewId) {
        let mut siblings = self.children_of(parent);
        siblings.retain(|c| *c != child);
        self.children.insert(parent, siblings);
        self.parents.insert(child, None);
    }

    fn destroy(&mut self, id: ViewId) {
        if let Some(parent) = self.parent_of(id) {
            self.detach(parent, id);
        }
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
                        StyleDomainError::InvalidGridSpan(style) => {
                            ApplyError::InvalidGridSpan { op_index, style }
                        }
                        StyleDomainError::InvalidTransition(error) => {
                            ApplyError::InvalidTransition { op_index, error }
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
                let mut seen = HashSet::with_capacity(children.len());
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
                    if staged.is_root(*child) {
                        return Err(ApplyError::RootAsChild {
                            op_index,
                            parent: *id,
                            child: *child,
                        });
                    }
                    if staged.is_ancestor(*child, *id) {
                        return Err(ApplyError::Cycle {
                            op_index,
                            parent: *id,
                            child: *child,
                        });
                    }
                }
                for old in staged.children_of(*id) {
                    if !seen.contains(&old) {
                        staged.parents.insert(old, None);
                    }
                }
                for child in children {
                    if let Some(p) = staged.parent_of(*child) {
                        if p != *id {
                            staged.detach(p, *child);
                        }
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
    // Conservative: ignores free-slot reuse, so it only ever refuses a batch that
    // would have fit within a few slots of the four-billion-node ceiling.
    if arena.slot_count() as u64 + creates > u32::MAX as u64 {
        return Err(ApplyError::SlotSpaceExhausted);
    }
    Ok(())
}

/// Everything the apply engine writes.
pub struct Target<'a> {
    /// The arena.
    pub arena: &'a mut NodeArena,
    /// The layout engine.
    pub layout: &'a mut LayoutTree,
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
    let mut created: HashSet<u32> = HashSet::new();

    for (op_index, op) in ops.iter().enumerate() {
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
                let node =
                    layout.new_leaf(taffy_style(arena, slot), slot, node_type.is_measured_leaf());
                arena.set_taffy(slot, Some(node));
                created.insert(slot);
                receipt.created.push(arena.key(slot));
                receipt.layout_invalidated = true;
            }
            Op::DestroyView { id } => {
                let slot = live_slot(arena, op_index, *id)?;
                if let Some(parent) = arena.parent(slot) {
                    if arena.node_type(parent) == NodeType::Text {
                        invalidate_text(arena, layout, parent);
                    }
                    arena.children_mut(parent).retain(|c| *c != slot);
                    sync_children(arena, layout, parent);
                    arena.flags_mut(parent).insert(NodeFlags::CHILDREN_DIRTY);
                    touched.push(arena.key(parent));
                }
                for s in arena.subtree(slot) {
                    if let Some(test_id) = arena.props(s).str(PropId::TestId) {
                        selectors.remove(s, test_id);
                    }
                    if let Some(node) = arena.taffy(s) {
                        layout.remove(node);
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
                let old = arena.props_mut(slot).set(*prop, value.clone());
                if *prop == PropId::TestId {
                    selectors.update(slot, old.as_ref().and_then(|v| v.as_str()), value.as_str());
                }
                arena.flags_mut(slot).insert(NodeFlags::PROPS_DIRTY);
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
                if let Some(old) = arena.props_mut(slot).remove(*prop) {
                    if *prop == PropId::TestId {
                        selectors.update(slot, old.as_str(), None);
                    }
                    arena.flags_mut(slot).insert(NodeFlags::PROPS_DIRTY);
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
                arena.style_mut(slot).apply_patch(patch);
                arena.update_exclusion_count(slot, excluded);
                style_changed(arena, layout, slot, changed, &mut receipt);
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
                arena.style_mut(slot).clear(*mask);
                arena.update_exclusion_count(slot, excluded);
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
                let retained: HashSet<u32> = new.iter().copied().collect();
                let detached: Vec<_> = old
                    .iter()
                    .copied()
                    .filter(|o| !retained.contains(o))
                    .map(|o| (o, arena.computed_inherited(o)))
                    .collect();
                for o in &old {
                    if !retained.contains(o) {
                        arena.set_parent(*o, None);
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
                for n in &new {
                    if let Some(p) = arena.parent(*n) {
                        if p != slot {
                            if arena.node_type(p) == NodeType::Text {
                                invalidate_text(arena, layout, p);
                            }
                            arena.children_mut(p).retain(|c| c != n);
                            sync_children(arena, layout, p);
                            arena.flags_mut(p).insert(NodeFlags::CHILDREN_DIRTY);
                            touched.push(arena.key(p));
                        }
                    }
                    arena.set_parent(*n, Some(slot));
                }
                arena.set_children(slot, new);
                sync_children(arena, layout, slot);
                arena.flags_mut(slot).insert(NodeFlags::CHILDREN_DIRTY);
                if arena.node_type(slot) == NodeType::Text {
                    invalidate_text(arena, layout, slot);
                }
                touched.push(arena.key(slot));
                receipt.layout_invalidated = true;
                for (orphan, before) in detached {
                    inherited_after_move(arena, layout, orphan, before, &mut touched, &mut receipt);
                }
                for (m, before) in moved {
                    inherited_after_move(arena, layout, m, before, &mut touched, &mut receipt);
                }
            }
            Op::AttachRoot { id } => {
                let slot = live_slot(arena, op_index, *id)?;
                if !arena.is_root(slot) {
                    arena.set_root(slot, true);
                    // A root's engine style differs from a child's (it fills its
                    // offered width): re-derive it now that the node is one.
                    if let Some(node) = arena.taffy(slot) {
                        layout.set_style(node, taffy_style(arena, slot));
                    }
                    touched.push(arena.key(slot));
                    receipt.layout_invalidated = true;
                }
            }
        }
    }

    // Compare whole keys: a slot freed and reallocated in one batch carries two generations.
    receipt.created.retain(|key| {
        arena
            .resolve(*key)
            .is_some_and(|slot| created.contains(&slot))
    });
    // Publication needs sorted unique live keys, not a tree update for every op.
    // Generations discard touches from a node destroyed earlier in this batch.
    // Consecutive props commonly touch the same node; collapse those before sorting.
    touched.dedup();
    touched.sort_unstable();
    touched.dedup();
    touched.retain(|key| {
        arena
            .resolve(*key)
            .is_some_and(|slot| !created.contains(&slot))
    });
    // Receipts outlive this batch; do not retain scratch for duplicates or dead nodes.
    touched.shrink_to_fit();
    receipt.touched = touched;
    Ok(receipt)
}

/// Push the arena's child list for `parent` into the layout engine. A `Text`
/// parent's children are inline runs: they are measured with it, not laid out.
fn sync_children(arena: &NodeArena, layout: &mut LayoutTree, parent: u32) {
    let Some(node) = arena.taffy(parent) else {
        return;
    };
    if arena.node_type(parent) == NodeType::Text {
        layout.set_children(node, &[]);
        return;
    }
    let ids: Vec<_> = arena
        .children(parent)
        .iter()
        .filter_map(|c| arena.taffy(*c))
        .collect();
    layout.set_children(node, &ids);
}

fn invalidate_text(arena: &mut NodeArena, layout: &mut LayoutTree, slot: u32) {
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
    let owners: BTreeSet<_> = arena
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
    layout: &mut LayoutTree,
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

fn style_changed(
    arena: &mut NodeArena,
    layout: &mut LayoutTree,
    slot: u32,
    mask: StyleMask,
    receipt: &mut CommitReceipt,
) {
    if mask.intersects(StyleMask::LAYOUT) {
        arena.flags_mut(slot).insert(NodeFlags::STYLE_DIRTY);
        if let Some(node) = arena.taffy(slot) {
            layout.set_style(node, taffy_style(arena, slot));
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
    layout: &mut LayoutTree,
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
        let pass = rows.minus(arena.style(s).mask);
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
    layout: &mut LayoutTree,
    slot: u32,
    rows: StyleMask,
    receipt: &mut CommitReceipt,
) {
    if rows.intersects(StyleMask::LAYOUT) {
        if let Some(node) = arena.taffy(slot) {
            layout.set_style(node, taffy_style(arena, slot));
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
