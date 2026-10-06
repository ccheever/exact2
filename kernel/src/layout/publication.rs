//! Sparse absolute-frame publication. Child indices are maintained on topology
//! writes, so a dirty path never searches its parent's unrelated child list.
use super::*;
use std::collections::BTreeSet;

pub(super) fn publish(arena: &mut NodeArena, tree: &mut LayoutTree, root: u32) -> LayoutReceipt {
    #[cfg(test)]
    {
        tree.publication_visits = 0;
    }
    for node in tree.taffy.take_layout_changes() {
        if let Some(&slot) = tree.slots.get(&node) {
            arena.layout_dirty.insert(slot);
        }
    }
    let mut paths: IdMap<u32, BTreeSet<(usize, u32)>> = IdMap::default();
    let mut has_sources = false;
    let mut detached = Vec::new();
    for slot in arena.layout_dirty.iter() {
        let mut at = slot;
        let mut path = Vec::new();
        while at != root {
            let Some(parent) = arena.parent(at) else {
                break;
            };
            path.push((parent, at));
            at = parent;
        }
        if at != root {
            if !arena.is_root(at) {
                detached.push(slot);
            }
            continue;
        }
        has_sources = true;
        for (parent, child) in path {
            paths
                .entry(parent)
                .or_default()
                .insert((arena.child_index(child), child));
        }
    }
    // Detached nodes keep their flags, but need no repeated search. Attaching
    // them dirties the parent's topology and publishes the entire moved subtree.
    for slot in detached {
        arena.layout_dirty.remove(slot);
    }
    // Exclusions can affect a different subtree, and old exclusions must clear.
    // Keep their existing full publication/resolution semantics.
    let full = !arena.exclusion_slots.is_empty() || !arena.flow.is_empty();
    arena.begin_layout_publication(root);
    let mut changed = Vec::new();
    let mut updated = Vec::new();
    let mut exclusions = Vec::new();
    // @ref LLP 1093 D7 — a box in a multi-column flow is published at its
    // place in the flow thread plus its placement: its column's translation,
    // relative to the container's own (`base`), or a straddling box's union.
    // `t` is the translation its subtree inherits; (ox, oy) stay flow-thread
    // origins, so a child is placed from its parent's unfragmented box.
    let mut stack = if full || has_sources {
        vec![(root, 0.0, 0.0, false, full, false, Shift::default())]
    } else {
        Vec::new()
    };
    while let Some((slot, ox, oy, hidden, full, parent_moved, shift)) = stack.pop() {
        #[cfg(test)]
        {
            tree.publication_visits += 1;
        }
        let own_hidden = arena.style(slot).display == crate::Display::None;
        let hidden = hidden || own_hidden;
        if hidden {
            arena.flags_mut(slot).insert(NodeFlags::HIDDEN);
        } else {
            arena.flags_mut(slot).remove(NodeFlags::HIDDEN);
        }
        if !hidden && crate::flow::is_exclusion(arena, slot) {
            exclusions.push(slot);
        }
        let inline = arena.is_inline_run(slot);
        let old = arena.frame(slot);
        let old_content = arena.content(slot);
        let placement = arena
            .frag
            .placements
            .get(&slot)
            .copied()
            .filter(|_| !hidden);
        let shift = match placement {
            Some(p) => Shift {
                t: (shift.base.0 + p.dx, shift.base.1 + p.dy),
                ..shift
            },
            None => shift,
        };
        let frame = if hidden {
            arena.set_content(slot, (0.0, 0.0));
            Frame {
                x: ox,
                y: oy,
                ..Frame::default()
            }
        } else if inline {
            Frame::default()
        } else {
            let Some(node) = arena.taffy(slot) else {
                continue;
            };
            let l = tree.layout(node);
            // A multi-column container's overflow is its columns' (D7), which
            // a multi-column `text`, a leaf to the engine, never reports.
            let o = arena
                .frag
                .multicol
                .get(&slot)
                .and_then(|m| m.overflow)
                .unwrap_or(l.scrollable_overflow_rect);
            arena.set_content(slot, (o.right, o.bottom));
            let (width, height) = placement
                .and_then(|p| p.size)
                .unwrap_or((l.size.width, l.size.height));
            Frame {
                x: ox + l.location.x + shift.t.0,
                y: oy + l.location.y + shift.t.1,
                width,
                height,
            }
        };
        let (fx, fy) = match arena.taffy(slot).filter(|_| !hidden && !inline) {
            Some(node) => {
                let l = tree.layout(node);
                (ox + l.location.x, oy + l.location.y)
            }
            None => (frame.x, frame.y),
        };
        let republish = arena.frag.republish.remove(slot);
        let shift = if arena.frag.multicol.contains_key(&slot) {
            Shift {
                base: shift.t,
                ..shift
            }
        } else {
            shift
        };
        let flags = arena.flags(slot);
        let first = flags.has(NodeFlags::CREATED);
        let moved = !inline && (first || !old.bits_eq(frame));
        let origin_moved =
            old.x.to_bits() != frame.x.to_bits() || old.y.to_bits() != frame.y.to_bits();
        // A changed child list visits every child; a child that arrived
        // (created, or moved from another parent: a Text subtree switching
        // into/out of inline runs) is published whole, a kept one sparsely.
        // A multi-column cut that changed publishes this subtree whole
        // (LLP 1093): placements moved without each child's layout bit.
        let full = full || hidden || republish;
        let descend_all = full || origin_moved || flags.has(NodeFlags::CHILDREN_DIRTY);
        arena.set_frame(slot, frame);
        arena.consume_layout_flags(slot);
        if moved {
            arena.mark_geometry_changed(slot, root);
            changed.push(arena.key(slot));
        }
        if !inline && (moved || parent_moved || old_content != arena.content(slot)) {
            updated.push(arena.key(slot));
        }
        if descend_all {
            for &child in arena.children(slot).iter().rev() {
                let full = full || arena.flags(child).has(NodeFlags::ATTACHED);
                stack.push((child, fx, fy, hidden, full, origin_moved, shift));
            }
        } else if let Some(children) = paths.get(&slot) {
            for &(_, child) in children.iter().rev() {
                stack.push((child, fx, fy, hidden, false, origin_moved, shift));
            }
        }
    }
    let (flow_changed, flow_skipped) = crate::flow::resolve(
        arena,
        root,
        &exclusions,
        |arena, slot| {
            arena
                .taffy(slot)
                .is_none_or(|node| tree.height_measured(node))
        },
        |_| true,
        crate::flow::AutoFlow::Admit(&tree.unsettled),
    );
    LayoutReceipt {
        epoch: 0,
        root: arena.key(root),
        changed,
        updated,
        flow_changed,
        flow_skipped,
        fragment_skipped: crate::fragment::skipped(arena),
        flow_passes: 0,
        flow_comparisons: 0,
    }
}

/// The translation publication adds under a multi-column container: `t` for
/// this subtree, `base` the container's own, which placements are relative to.
#[derive(Clone, Copy, Default)]
struct Shift {
    t: (f32, f32),
    base: (f32, f32),
}
