//! @ref LLP 1043.000 §3 D1–D4 — sparse, derived exclusions in leaf coordinates.
//!
//! Frame publication collects exclusions in preorder. This second pass visits
//! only their wrapping contexts, skipping hidden and exclusion-owned subtrees.
//! Work is O(sum(context subtree sizes) + changed-leaf ordering + output geometry); storage is
//! O(exclusions + affected leaves + output geometry). No artificial truncation:
//! larger trees keep that bound. With no exclusions and no old sets this pass
//! returns immediately, allocating nothing. It never revises paragraph inputs.

use crate::arena::NodeArena;
use crate::{Display, NodeKey, NodeType, PositionType, WrapFlow};
use exact_textflow::{meets, FlowShape};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub(crate) struct FlowState {
    pub shapes: Vec<FlowShape>,
    pub skipped: bool,
    root: u32,
    pending: Vec<FlowShape>,
    pending_skipped: bool,
}

pub(crate) fn is_exclusion(arena: &NodeArena, slot: u32) -> bool {
    let s = arena.style(slot);
    s.position_type == PositionType::Absolute && s.wrap_flow == WrapFlow::Both
}

/// Both lists are in paragraph preorder; a disappearing set counts once.
pub(crate) fn resolve(
    arena: &mut NodeArena,
    root: u32,
    exclusions: &[u32],
    height_measured: impl Fn(&NodeArena, u32) -> bool,
    included: impl Fn(u32) -> bool,
) -> (Vec<NodeKey>, Vec<NodeKey>) {
    if exclusions.is_empty() && arena.flow.is_empty() {
        return (Vec::new(), Vec::new());
    }
    // Reuse both shape buffers and the sparse map. Unchanged frames allocate
    // no per-pass HashMap; disappearing entries are released, never history.
    for state in arena.flow.values_mut().filter(|s| s.root == root) {
        state.pending.clear();
        state.pending_skipped = false;
    }
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
            let f = arena.frame(exclusion);
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
                let f = arena.frame(slot);
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
                    let skipped = height_measured(arena, slot);
                    let state = arena.flow.entry(slot).or_default();
                    state.root = root;
                    // M8 lifts exactly this condition once Taffy supplies origins.
                    if skipped {
                        state.pending_skipped = true;
                    } else {
                        state.pending.extend(
                            std::iter::once(first)
                                .chain(touching)
                                .map(|shape| shape.translate(-f.x, -f.y)),
                        );
                    }
                }
            }
            stack.extend(arena.children(slot).iter().rev().copied());
        }
    }
    let mut changed = Vec::new();
    let mut skipped = Vec::new();
    let mut changed_slots = Vec::new();
    let mut skipped_slots = Vec::new();
    arena.flow.retain(|&slot, state| {
        if state.root != root {
            return true;
        }
        if !shapes_eq(&state.shapes, &state.pending) {
            changed_slots.push(slot);
        }
        state.skipped = state.pending_skipped;
        if state.skipped {
            skipped_slots.push(slot);
        }
        std::mem::swap(&mut state.shapes, &mut state.pending);
        state.pending.clear();
        state.skipped || !state.shapes.is_empty()
    });
    // Only changed/skip leaves need document order, never a whole-root walk.
    let order = |&slot: &u32| {
        let mut path = Vec::new();
        let mut at = slot;
        while let Some(parent) = arena.parent(at) {
            path.push(
                arena
                    .children(parent)
                    .iter()
                    .position(|&s| s == at)
                    .unwrap_or(0),
            );
            at = parent;
        }
        path.reverse();
        path
    };
    changed_slots.sort_by_cached_key(order);
    skipped_slots.sort_by_cached_key(order);
    changed.extend(changed_slots.into_iter().map(|s| arena.key(s)));
    skipped.extend(skipped_slots.into_iter().map(|s| arena.key(s)));
    (changed, skipped)
}

fn shapes_eq(a: &[FlowShape], b: &[FlowShape]) -> bool {
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
    )
}
