//! @ref LLP 1043.000 §3 D1–D4, §8 — sparse, derived exclusions in leaf coordinates.
//!
//! Frame publication collects exclusions in preorder. This second pass visits
//! only their wrapping contexts, skipping hidden and exclusion-owned subtrees.
//! Work is O(sum(context subtree sizes) + changed-leaf ordering + output geometry); storage is
//! O(exclusions + affected leaves + output geometry). No artificial truncation:
//! larger trees keep that bound. With no exclusions and no old sets this pass
//! returns immediately, allocating nothing. It never revises paragraph inputs.
//!
//! Auto-height text flows where the admission rule below holds: every wrapping
//! context above the leaf is an ordinary block, every box between them is an
//! in-flow block, and every exclusion's top and height are its own. There a
//! leaf's offset and every exclusion's box depend only on content *before* the
//! leaf, so `LayoutTree::settle_flow` reaches the one fixed point by re-laying
//! out (at most one pass per admitted leaf, in document order) and checks it
//! bitwise. Elsewhere the leaf keeps ordinary layout and names its refusal.

use crate::arena::NodeArena;
use crate::id::{Frame, IdMap};
use crate::{AlignContent, Dimension, Display, NodeKey, NodeType, PositionType, WrapFlow};
use exact_textflow::{meets, FlowShape};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub(crate) struct FlowState {
    pub shapes: Vec<FlowShape>,
    pub refusal: Option<FlowRefusal>,
    root: u32,
    pending: Vec<FlowShape>,
    pending_refusal: Option<FlowRefusal>,
}

/// Why an auto-height paragraph that meets an exclusion keeps ordinary layout
/// (and overlaps it). Journalled once per leaf; `message` says what to change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowRefusal {
    /// A wrapping context (an exclusion's parent) is flex or grid, or sets
    /// `align-content`: the leaf's offset would depend on its own height.
    Context,
    /// An exclusion's top or height depends on its context's auto height.
    Placement,
    /// The leaf, or a box between it and a wrapping context, is absolutely
    /// positioned, flex or grid, sets `align-content`, or is offset
    /// vertically by a percentage.
    Chain,
    /// Content regions lay auto-height text out unobstructed.
    Region,
    /// Layout did not reach its fixed point: a kernel defect, never expected.
    Unsettled,
}

impl FlowRefusal {
    /// What the author can change, for the journal line.
    pub fn message(self) -> &'static str {
        match self {
            FlowRefusal::Context => "its wrapping context (the exclusion's parent) is flex or grid, sets align-content, or is position: static; make that parent display: block and position: relative, or give the text a height",
            FlowRefusal::Placement => "an exclusion's top or height depends on its context's height; give every exclusion there a top and a height in points (not %, not bottom), or give the text a height",
            FlowRefusal::Chain => "the text, or a box between it and the wrapping context, is absolutely positioned, flex or grid, sets align-content, or has a percentage top or bottom; keep ordinary in-flow blocks between them, or give the text a height",
            FlowRefusal::Region => "text inside a content region flows only with a definite height",
            FlowRefusal::Unsettled => "layout did not settle around the exclusions; this is a kernel defect, please report it",
        }
    }
}

/// How a pass treats auto-height (measured) leaves that meet an exclusion.
pub(crate) enum AutoFlow<'a> {
    /// The ordinary tree: admitted leaves flow; `unsettled` names defects.
    Admit(&'a crate::id::IdSet<u32>),
    /// A content region's trial: every auto-height leaf is refused.
    Region,
}

pub(crate) fn is_exclusion(arena: &NodeArena, slot: u32) -> bool {
    let s = arena.style(slot);
    s.position_type == PositionType::Absolute && s.wrap_flow == WrapFlow::Both
}

// A length that resolves without the containing block's height.
fn own_length(d: Dimension) -> bool {
    matches!(d, Dimension::Points(_) | Dimension::Env(..))
}

// Taffy places an absolute child after the context's in-flow content, against
// the context's final size: only an authored top and height keep its box
// independent of what the flowed text makes that size.
fn placed(arena: &NodeArena, exclusion: u32) -> bool {
    let s = arena.style(exclusion);
    let bounded = |d: Dimension| matches!(d, Dimension::Auto) || own_length(d);
    own_length(s.top)
        && (own_length(s.height) || (s.height == Dimension::Auto && s.bottom == Dimension::Auto))
        && bounded(s.min_height)
        && bounded(s.max_height)
}

// A block places children in order: a child's offset depends on what precedes
// it and never on its own height, unless alignment distributes free space.
fn orders(arena: &NodeArena, slot: u32) -> bool {
    let s = arena.style(slot);
    s.display == Display::Block && s.align_content == AlignContent::Normal
}

fn in_flow(arena: &NodeArena, slot: u32) -> bool {
    let s = arena.style(slot);
    let offset = |d: Dimension| d == Dimension::Auto || own_length(d);
    // A static box's insets do nothing; a relative one's must not depend on the context's height.
    s.position_type == PositionType::Static
        || (s.position_type == PositionType::Relative && offset(s.top) && offset(s.bottom))
}

/// Each visible context under the root and whether it refuses auto height.
pub(crate) type Contexts = IdMap<u32, Option<FlowRefusal>>;

pub(crate) fn contexts(arena: &NodeArena, exclusions: &[u32]) -> Contexts {
    let mut out = Contexts::default();
    for &exclusion in exclusions {
        let Some(context) = arena.parent(exclusion) else {
            continue;
        };
        // @ref LLP 1074 T1 — the exclusion is placed against its containing
        // block. Auto-height flow is proven only where that is the context.
        let contains =
            arena.is_root(context) || arena.style(context).position_type != PositionType::Static;
        let refusal = out.entry(context).or_insert_with(|| {
            (!orders(arena, context) || !contains).then_some(FlowRefusal::Context)
        });
        if refusal.is_none() && !placed(arena, exclusion) {
            *refusal = Some(FlowRefusal::Placement);
        }
    }
    out
}

/// Structural, never geometric: the answer cannot flip as heights settle.
/// O(depth). A leaf under several contexts must be admitted by all of them.
pub(crate) fn leaf_refusal(
    arena: &NodeArena,
    contexts: &Contexts,
    leaf: u32,
) -> Option<FlowRefusal> {
    let mut chain = in_flow(arena, leaf);
    let mut at = leaf;
    while let Some(parent) = arena.parent(at) {
        if let Some(context) = contexts.get(&parent) {
            if context.is_some() {
                return *context;
            }
            if !chain {
                return Some(FlowRefusal::Chain);
            }
        }
        chain &= in_flow(arena, parent) && orders(arena, parent);
        at = parent;
    }
    None
}

/// The admission rule alone, for a host that lays text out without Taffy
/// (the web's browser): which contexts exist does not depend on geometry.
/// O(E × depth).
pub(crate) fn structural_refusal(arena: &NodeArena, leaf: u32) -> Option<FlowRefusal> {
    let visible: Vec<u32> = arena
        .exclusion_slots
        .iter()
        .filter(|&slot| {
            let mut at = Some(slot);
            while let Some(s) = at {
                if arena.style(s).display == Display::None {
                    return false;
                }
                at = arena.parent(s);
            }
            true
        })
        .collect();
    leaf_refusal(arena, &contexts(arena, &visible), leaf)
}

/// Visible exclusions under `root`, in preorder: publication's list, found
/// without a tree walk. O(E × depth + E log E).
pub(crate) fn visible_exclusions(arena: &NodeArena, root: u32) -> Vec<u32> {
    let mut out: Vec<u32> = arena
        .exclusion_slots
        .iter()
        .filter(|&slot| {
            let mut at = slot;
            loop {
                if arena.style(at).display == Display::None {
                    return false;
                }
                if at == root {
                    return true;
                }
                match arena.parent(at) {
                    Some(p) => at = p,
                    None => return false,
                }
            }
        })
        .collect();
    out.sort_by_cached_key(|&s| preorder(arena, s));
    out
}

fn preorder(arena: &NodeArena, slot: u32) -> Vec<usize> {
    let mut path = Vec::new();
    let mut at = slot;
    while let Some(parent) = arena.parent(at) {
        path.push(arena.child_index(at));
        at = parent;
    }
    path.reverse();
    path
}

/// Every text leaf a batch of same-context exclusions meets under `frame`,
/// with those shapes in the leaf's border-box coordinates, in publication
/// order. `resolve` and `settle_flow` share it, so what a leaf is measured
/// around is bitwise what is published for it.
fn visit(
    arena: &NodeArena,
    exclusions: &[u32],
    frame: &impl Fn(u32) -> Frame,
    included: &impl Fn(u32) -> bool,
    mut each: impl FnMut(u32, &mut dyn Iterator<Item = FlowShape>),
) {
    let mut stack = Vec::new();
    // Consecutive exclusions with the same context share its subtree walk.
    // Keep batches in publication order so nested contexts cannot reorder shapes.
    let mut remaining = exclusions;
    while let Some(&first) = remaining.first() {
        let context = arena.parent(first);
        let count = remaining
            .iter()
            .take_while(|&&s| arena.parent(s) == context)
            .count();
        let (batch, rest) = remaining.split_at(count);
        remaining = rest;
        let Some(context) = context else {
            continue;
        };
        let resolve_shape = |exclusion| {
            let f = frame(exclusion);
            let s = arena.style(exclusion);
            s.shape_outside
                .resolve(f.width, f.height)
                .grow(s.shape_margin)
                .translate(f.x, f.y)
        };
        let first_shape = resolve_shape(first);
        let grouped;
        let shapes = if batch.len() == 1 {
            std::slice::from_ref(&first_shape)
        } else {
            grouped = std::iter::once(first_shape)
                .chain(batch[1..].iter().copied().map(resolve_shape))
                .collect::<Vec<_>>();
            &grouped
        };
        stack.extend(arena.children(context).iter().rev().copied());
        while let Some(slot) = stack.pop() {
            if is_exclusion(arena, slot)
                || !included(slot)
                || arena.style(slot).display == Display::None
            {
                continue;
            }
            if arena.node_type(slot) == NodeType::Text && !arena.is_inline_run(slot) {
                let f = frame(slot);
                let mut touching = shapes.iter().filter(|shape| {
                    meets(
                        std::slice::from_ref(*shape),
                        f.x,
                        f.y,
                        f.x + f.width,
                        f.y + f.height,
                    )
                });
                if let Some(first) = touching.next() {
                    each(
                        slot,
                        &mut std::iter::once(first)
                            .chain(touching)
                            .map(|shape| shape.translate(-f.x, -f.y)),
                    );
                }
            }
            stack.extend(arena.children(slot).iter().rev().copied());
        }
    }
}

/// What each admitted auto-height leaf must be measured around, given the
/// engine's current frames: exactly the set `resolve` would publish.
pub(crate) fn targets(
    arena: &NodeArena,
    exclusions: &[u32],
    contexts: &Contexts,
    frame: impl Fn(u32) -> Frame,
    height_measured: impl Fn(u32) -> bool,
) -> IdMap<u32, Vec<FlowShape>> {
    let mut out = IdMap::<u32, Vec<FlowShape>>::default();
    visit(arena, exclusions, &frame, &|_| true, |leaf, shapes| {
        if height_measured(leaf) && leaf_refusal(arena, contexts, leaf).is_none() {
            out.entry(leaf).or_default().extend(shapes);
        }
    });
    out
}

/// Both lists are in paragraph preorder; a disappearing set counts once.
pub(crate) fn resolve(
    arena: &mut NodeArena,
    root: u32,
    exclusions: &[u32],
    height_measured: impl Fn(&NodeArena, u32) -> bool,
    included: impl Fn(u32) -> bool,
    auto: AutoFlow<'_>,
) -> (Vec<NodeKey>, Vec<NodeKey>) {
    if exclusions.is_empty() && arena.flow.is_empty() {
        return (Vec::new(), Vec::new());
    }
    // Reuse both shape buffers and the sparse map. Unchanged frames allocate
    // no per-pass HashMap; disappearing entries are released, never history.
    let mut flow = std::mem::take(&mut arena.flow);
    for state in flow.values_mut().filter(|s| s.root == root) {
        state.pending.clear();
        state.pending_refusal = None;
    }
    {
        let arena = &*arena;
        let contexts = match auto {
            AutoFlow::Admit(_) => contexts(arena, exclusions),
            AutoFlow::Region => Contexts::default(),
        };
        visit(
            arena,
            exclusions,
            &|s| arena.frame(s),
            &included,
            |leaf, shapes| {
                let refusal = if !height_measured(arena, leaf) {
                    None
                } else {
                    match auto {
                        AutoFlow::Admit(unsettled) if unsettled.contains(&leaf) => {
                            Some(FlowRefusal::Unsettled)
                        }
                        AutoFlow::Admit(_) => leaf_refusal(arena, &contexts, leaf),
                        AutoFlow::Region => Some(FlowRefusal::Region),
                    }
                };
                let state = flow.entry(leaf).or_default();
                state.root = root;
                match refusal {
                    Some(r) => {
                        state.pending_refusal.get_or_insert(r);
                    }
                    None => state.pending.extend(shapes),
                }
            },
        );
    }
    let mut changed_slots = Vec::new();
    let mut skipped_slots = Vec::new();
    flow.retain(|&slot, state| {
        if state.root != root {
            return true;
        }
        // A refused leaf was measured unobstructed; it is painted that way.
        if state.pending_refusal.is_some() {
            state.pending.clear();
        }
        if !shapes_eq(&state.shapes, &state.pending) {
            changed_slots.push(slot);
        }
        state.refusal = state.pending_refusal;
        if state.refusal.is_some() {
            skipped_slots.push(slot);
        }
        std::mem::swap(&mut state.shapes, &mut state.pending);
        state.pending.clear();
        state.refusal.is_some() || !state.shapes.is_empty()
    });
    arena.flow = flow;
    // Only changed/skip leaves need document order, never a whole-root walk.
    changed_slots.sort_by_cached_key(|&s| preorder(arena, s));
    skipped_slots.sort_by_cached_key(|&s| preorder(arena, s));
    (
        changed_slots.into_iter().map(|s| arena.key(s)).collect(),
        skipped_slots.into_iter().map(|s| arena.key(s)).collect(),
    )
}

pub(crate) fn shapes_eq(a: &[FlowShape], b: &[FlowShape]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            use FlowShape::*;
            let eq =
                |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits());
            let pairs = |a: &[(f32, f32)], b: &[(f32, f32)]| {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| eq(&[a.0, a.1], &[b.0, b.1]))
            };
            match (a, b) {
                (Circle { cx, cy, r }, Circle { cx: x, cy: y, r: z }) => {
                    eq(&[*cx, *cy, *r], &[*x, *y, *z])
                }
                (
                    Ellipse { cx, cy, rx, ry },
                    Ellipse {
                        cx: x,
                        cy: y,
                        rx: a,
                        ry: b,
                    },
                ) => eq(&[*cx, *cy, *rx, *ry], &[*x, *y, *a, *b]),
                (
                    RoundRect {
                        x,
                        y,
                        width,
                        height,
                        radius,
                    },
                    RoundRect {
                        x: a,
                        y: b,
                        width: c,
                        height: d,
                        radius: e,
                    },
                ) => eq(&[*x, *y, *width, *height, *radius], &[*a, *b, *c, *d, *e]),
                (Polygon(a), Polygon(b)) | (EvenOddPolygon(a), EvenOddPolygon(b)) => pairs(a, b),
                (
                    Spans {
                        x,
                        y,
                        row_height,
                        rows,
                    },
                    Spans {
                        x: a,
                        y: b,
                        row_height: c,
                        rows: d,
                    },
                ) => eq(&[*x, *y, *row_height], &[*a, *b, *c]) && pairs(rows, d),
                _ => false,
            }
        })
}

/// The region path publishes frames separately; carry the same height proof and
/// use only selected live keys, never the unselected placeholder/content branch.
/// @ref LLP 1043.000 §3 D4 — RegionLayoutReceipt.shell carries both flow lists.
pub(crate) fn resolve_region(
    arena: &mut NodeArena,
    root: u32,
    shell: &[crate::RegionFrame],
    content: &[crate::RegionFrame],
) -> (Vec<NodeKey>, Vec<NodeKey>) {
    if arena.flow.is_empty()
        && !shell.iter().chain(content).any(|f| {
            arena
                .resolve(f.node)
                .is_some_and(|s| is_exclusion(arena, s))
        })
    {
        return (Vec::new(), Vec::new());
    }
    let live: HashMap<_, _> = shell
        .iter()
        .chain(content)
        .filter_map(|f| arena.resolve(f.node).map(|s| (s, f.height_measured)))
        .collect();
    let mut exclusions = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((slot, hidden)) = stack.pop() {
        let hidden = hidden || arena.style(slot).display == Display::None;
        if !hidden && live.contains_key(&slot) && is_exclusion(arena, slot) {
            exclusions.push(slot);
        }
        stack.extend(arena.children(slot).iter().rev().map(|s| (*s, hidden)));
    }
    resolve(
        arena,
        root,
        &exclusions,
        |_, s| live.get(&s).copied().unwrap_or(true),
        |s| live.contains_key(&s),
        AutoFlow::Region,
    )
}
