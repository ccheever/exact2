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
        // A subtree a host gave a containing block is another layout's: the
        // pass that starts at it publishes it (LLP 1115.002 D8.2).
        while at != root && !arena.blocks.holds(at) {
            let Some(parent) = arena.parent(at) else {
                break;
            };
            path.push((parent, at));
            at = parent;
        }
        if at != root {
            if !arena.is_root(at) && !arena.blocks.holds(at) {
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
    // A block's root: hidden with what holds it, whole when it took its block.
    let (hidden, whole) = crate::kernel::publication_start(arena, root);
    let mut changed = Vec::new();
    let mut updated = Vec::new();
    let mut exclusions = Vec::new();
    // @ref LLP 1093 D7 — a box in a multi-column flow is published at its
    // place in the flow thread plus its placement: its column's translation,
    // relative to the container's own (`base`), or a straddling box's union.
    // `t` is the translation its subtree inherits; (ox, oy) stay flow-thread
    // origins, so a child is placed from its parent's unfragmented box.
    let mut stack = if full || has_sources || whole {
        vec![(
            root,
            0.0,
            0.0,
            hidden,
            full || whole,
            false,
            Shift::default(),
        )]
    } else {
        Vec::new()
    };
    while let Some((slot, ox, oy, hidden, full, parent_moved, shift)) = stack.pop() {
        #[cfg(test)]
        {
            tree.publication_visits += 1;
            tree.work.0 += 1;
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
        let old_field_content = arena.field_content_rect(slot);
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
            arena.field_content.remove(&slot);
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
            if let Some(content) = tree.field_content_rect(arena, slot, node) {
                arena.field_content.insert(slot, content);
            } else {
                arena.field_content.remove(&slot);
            }
            // A multi-column container's overflow is its columns' (D7), which
            // a multi-column `text`, a leaf to the engine, never reports.
            let o = arena
                .frag
                .multicol
                .get(&slot)
                .and_then(|m| m.overflow)
                .unwrap_or(l.scrollable_overflow_rect);
            // A scroller the platform insets: its extent less those insets.
            let content = crate::kernel::published_extent(arena, slot, (o.right, o.bottom));
            arena.set_content(slot, content);
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
        if !inline
            && (moved
                || parent_moved
                || old_content != arena.content(slot)
                || old_field_content != arena.field_content_rect(slot))
        {
            updated.push(arena.key(slot));
        }
        if descend_all {
            for &child in arena.children(slot).iter().rev() {
                if arena.blocks.holds(child) {
                    continue; // its own layout's, in its own space
                }
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

/// Lay out `root_slot` and publish frames and resolved flow. The receipt uses
/// epoch zero; the kernel facade supplies its transaction epoch. Inline
/// runs have no geometry of their own: their frames are zero and they never
/// appear in the changed list, but their dirty bits are consumed like any
/// other node's.
pub fn compute(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root_slot: u32,
    offer: Offer,
) -> Result<LayoutReceipt, LayoutError> {
    let root = arena
        .taffy(root_slot)
        .ok_or_else(|| LayoutError::Engine("root has no engine node".into()))?;
    tree.compute(root, offer, arena, measurer)?;
    let (flow_passes, flow_comparisons) =
        tree.settle_flow(root, root_slot, offer, arena, measurer)?;
    crate::fragment::settle(arena, tree, measurer, root_slot, offer)?;
    for (&node, record) in &tree.button_records {
        if let Some(&slot) = tree.slots.get(&node) {
            arena.button_bases.insert(slot, record.parent_width);
        }
    }
    let refitted = fit::settle(arena, tree);
    let mut receipt = publish(arena, tree, root_slot);
    for key in refitted {
        if !receipt.updated.contains(&key) {
            receipt.updated.push(key);
        }
    }
    receipt.flow_passes = flow_passes;
    receipt.flow_comparisons = flow_comparisons;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use crate::{
        Kernel, MonospaceMeasurer, NodeFlags, NodeType, Offer, Op, PropId, StyleId, StyleProps,
        StyleValue, TextMeasureRequest, TextMeasurer, TextMetrics,
    };
    use std::{cell::Cell, rc::Rc};
    struct Count(Rc<Cell<usize>>);
    impl TextMeasurer for Count {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            self.0.set(self.0.get() + 1);
            MonospaceMeasurer::default().measure(request)
        }
    }
    fn style(id: u32, row: StyleId, value: &str) -> Op {
        let mut patch = StyleProps::default();
        patch
            .set_dynamic(row, &StyleValue::Text(value.into()))
            .unwrap();
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    }
    fn prop(id: u32, prop: PropId, value: &str) -> Op {
        Op::SetProp {
            id,
            prop,
            value: value.into(),
        }
    }
    fn equal_fresh(k: &Kernel, offer: Offer) {
        let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
        fresh.compute_layout(1, offer).unwrap();
        let arena = k.node(1).unwrap().arena;
        for slot in arena.iter_live() {
            let node = k.node(arena.local_id(slot)).unwrap();
            let other = fresh.node(node.id).unwrap();
            assert!(node.frame.bits_eq(other.frame), "frame {}", node.id);
            assert_eq!(node.content, other.content, "overflow {}", node.id);
            assert_eq!(
                node.arena.flags(node.slot).has(NodeFlags::HIDDEN),
                other.arena.flags(other.slot).has(NodeFlags::HIDDEN),
                "hidden {}",
                node.id
            );
        }
    }
    #[test]
    fn paint_and_prop_edits_skip_publication_but_geometry_edits_still_publish() {
        let calls = Rc::new(Cell::new(0));
        let mut k = Kernel::new(Box::new(Count(calls.clone())));
        let offer = Offer::definite(400.0, 300.0);
        let mut ops = Vec::new();
        for id in [1, 2, 3].into_iter().chain(10..210) {
            ops.push(Op::CreateView {
                id,
                node_type: if id == 1 {
                    NodeType::View
                } else {
                    NodeType::Text
                },
            });
            if id >= 10 {
                ops.push(prop(id, PropId::Text, "word "));
            }
        }
        ops.extend([
            Op::SetChildren {
                id: 2,
                children: (10..110).collect(),
            },
            Op::SetChildren {
                id: 3,
                children: (110..210).collect(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::AttachRoot { id: 1 },
        ]);
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, offer).unwrap();
        calls.set(0);
        let receipt = k
            .apply(
                0,
                1,
                &[
                    style(1, StyleId::TextColor, "#ff0000"),
                    style(11, StyleId::BackgroundColor, "#001122"),
                    prop(12, PropId::Href, "https://example.invalid"),
                    prop(13, PropId::TestId, "painted-run"),
                ],
            )
            .unwrap();
        assert_eq!(receipt.touched.len(), 203);
        let layout = k.compute_layout(1, offer).unwrap();
        assert_eq!(k.tree().publication_visits, 0);
        assert_eq!(calls.get(), 0);
        assert!(layout.changed.is_empty() && layout.updated.is_empty());
        let run = k.node(12).unwrap();
        assert!(run.arena.flags(run.slot).has(NodeFlags::PROPS_DIRTY));
        assert!(run.arena.flags(run.slot).has(NodeFlags::PAINT_DIRTY));
        assert_eq!(run.props.str(PropId::Href), Some("https://example.invalid"));
        equal_fresh(&k, offer);
        for ops in [
            vec![
                prop(10, PropId::Text, "changed text with a different width"),
                style(11, StyleId::Display, "none"),
            ],
            vec![
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3, 10],
                },
                style(10, StyleId::BackgroundColor, "#112233"),
            ],
            vec![style(1, StyleId::Display, "none")],
            vec![
                style(1, StyleId::TextColor, "#0000ff"),
                Op::ClearProp {
                    id: 12,
                    prop: PropId::Href,
                },
            ],
            vec![
                style(1, StyleId::Display, "block"),
                style(11, StyleId::Display, "block"),
            ],
        ] {
            k.apply(0, 1, &ops).unwrap();
            k.compute_layout(1, offer).unwrap();
            equal_fresh(&k, offer);
        }
        assert!(calls.get() > 0);
    }
}
