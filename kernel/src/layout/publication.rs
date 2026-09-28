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
    let mut stack = if full || has_sources {
        vec![(root, 0.0, 0.0, false, full, false)]
    } else {
        Vec::new()
    };
    while let Some((slot, ox, oy, hidden, full, parent_moved)) = stack.pop() {
        #[cfg(test)]
        {
            tree.publication_visits += 1;
        }
        let own_hidden = arena.style(slot).display == crate::Display::None;
        if own_hidden {
            arena.flags_mut(slot).insert(NodeFlags::HIDDEN);
        } else {
            arena.flags_mut(slot).remove(NodeFlags::HIDDEN);
        }
        let hidden = hidden || own_hidden;
        if !hidden && crate::flow::is_exclusion(arena, slot) {
            exclusions.push(slot);
        }
        let inline = arena.is_inline_run(slot);
        let old = arena.frame(slot);
        let old_content = arena.content(slot);
        let frame = if inline {
            Frame::default()
        } else {
            let Some(node) = arena.taffy(slot) else {
                continue;
            };
            let l = tree.layout(node);
            arena.set_content(
                slot,
                (
                    l.scrollable_overflow_rect.right,
                    l.scrollable_overflow_rect.bottom,
                ),
            );
            Frame {
                x: ox + l.location.x,
                y: oy + l.location.y,
                width: l.size.width,
                height: l.size.height,
            }
        };
        let flags = arena.flags(slot);
        let first = flags.has(NodeFlags::CREATED);
        let moved = !inline && (first || !old.bits_eq(frame));
        let origin_moved =
            old.x.to_bits() != frame.x.to_bits() || old.y.to_bits() != frame.y.to_bits();
        // Topology changes can switch a Text subtree into/out of inline runs.
        let full = full || flags.has(NodeFlags::CHILDREN_DIRTY);
        let descend_all = full || origin_moved;
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
                stack.push((child, frame.x, frame.y, hidden, full, origin_moved));
            }
        } else if let Some(children) = paths.get(&slot) {
            for &(_, child) in children.iter().rev() {
                stack.push((child, frame.x, frame.y, hidden, false, origin_moved));
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
        flow_passes: 0,
        flow_comparisons: 0,
    }
}
