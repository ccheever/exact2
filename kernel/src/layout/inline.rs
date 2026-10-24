//! Inline boxes are independent Taffy leaves; the paragraph's shaper places them.
use super::*;

fn belongs(arena: &NodeArena, mut slot: u32, root: u32) -> bool {
    loop {
        if slot == root {
            return true;
        }
        let Some(parent) = arena.parent(slot) else {
            return false;
        };
        slot = parent;
    }
}

pub(super) fn prepare(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root: u32,
) -> Result<bool, LayoutError> {
    let mut changed = false;
    let mut slots: Vec<_> = arena
        .inline_slots
        .iter()
        .filter(|s| belongs(arena, *s, root))
        .collect();
    slots.sort_by_key(|s| std::cmp::Reverse(arena.depth(*s)));
    for slot in slots {
        let Some(node) = arena.taffy(slot) else {
            continue;
        };
        let owner = arena.measure_owner(arena.parent(slot).expect("parent"));
        let containing = arena
            .taffy(owner)
            .map(|n| tree.layout(n))
            .unwrap_or_default();
        let width = containing.size.width
            - containing.padding.left
            - containing.padding.right
            - containing.border.left
            - containing.border.right;
        tree.compute(
            node,
            Offer {
                width: if width > 0.0 {
                    AxisOffer::Definite(width)
                } else {
                    AxisOffer::MaxContent
                },
                height: AxisOffer::MaxContent,
            },
            arena,
            measurer,
        )?;
        let l = tree.layout(node);
        let baseline = if arena.node_type(slot) == NodeType::Text
            && arena.style(slot).overflow_x == crate::Overflow::Visible
            && arena.style(slot).overflow_y == crate::Overflow::Visible
        {
            let mut runs = Vec::new();
            arena.text_runs(slot, &mut runs);
            let baseline = measurer.inline_baseline(&TextMeasureRequest {
                runs: &runs,
                paragraph: arena.paragraph(slot),
                width: AxisOffer::Definite(
                    (l.size.width
                        - l.padding.left
                        - l.padding.right
                        - l.border.left
                        - l.border.right)
                        .max(0.0),
                ),
                height: AxisOffer::MaxContent,
                exclusions: &[],
            });
            baseline.unwrap_or(l.size.height) + l.padding.top + l.border.top
        } else {
            l.size.height + l.margin.bottom
        };
        let size = crate::text::InlineBox {
            width: l.size.width + l.margin.left + l.margin.right,
            height: l.size.height + l.margin.top + l.margin.bottom,
            baseline: baseline + l.margin.top,
        };
        if arena.inline_boxes.get(&slot) != Some(&size) {
            changed = true;
            arena.inline_boxes.insert(slot, size);
            let owner = arena.measure_owner(arena.parent(slot).expect("parent"));
            arena.revise_text(owner, true);
            if let Some(parent) = arena.taffy(owner) {
                tree.mark_dirty(parent);
            }
            arena.layout_dirty.insert(owner);
        }
    }
    Ok(changed)
}

pub(super) fn place(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root: u32,
) -> Result<(), LayoutError> {
    let mut owners = crate::sorted::SlotSet::default();
    for slot in arena
        .inline_slots
        .iter()
        .filter(|s| belongs(arena, *s, root))
    {
        owners.insert(arena.measure_owner(arena.parent(slot).expect("parent")));
    }
    let slots: Vec<_> = arena
        .inline_slots
        .iter()
        .filter(|s| belongs(arena, *s, root))
        .collect();
    for slot in slots {
        tree.inline_frames.remove(&slot);
        arena.layout_dirty.insert(slot);
    }
    for owner in owners.iter() {
        let Some(node) = arena.taffy(owner) else {
            continue;
        };
        let l = tree.layout(node);
        let inset = (l.padding.left + l.border.left, l.padding.top + l.border.top);
        let mut runs = Vec::new();
        arena.text_runs(owner, &mut runs);
        let shapes: Vec<_> = tree
            .taffy
            .get_node_context(node)
            .map(|c| {
                c.flow
                    .iter()
                    .map(|s| s.translate(-inset.0, -inset.1))
                    .collect()
            })
            .unwrap_or_default();
        let positions = measurer.inline_positions(&TextMeasureRequest {
            runs: &runs,
            paragraph: arena.paragraph(owner),
            width: AxisOffer::Definite(
                (l.size.width - inset.0 - l.padding.right - l.border.right).max(0.0),
            ),
            height: AxisOffer::MaxContent,
            exclusions: &shapes,
        });
        // Canonical leaves have exactly the same order as the shaper's runs.
        let mut leaves = Vec::new();
        collect(arena, owner, &mut leaves);
        for p in positions {
            if !p.x.is_finite() || !p.y.is_finite() {
                return Err(LayoutError::Engine("invalid inline position".into()));
            }
            let Some(&slot) = leaves.get(p.run) else {
                return Err(LayoutError::Engine("invalid inline run".into()));
            };
            let Some(n) = arena.taffy(slot) else {
                continue;
            };
            let b = tree.layout(n);
            tree.inline_frames.insert(
                slot,
                Frame {
                    x: inset.0 + p.x + b.margin.left,
                    y: inset.1 + p.y + b.margin.top,
                    width: b.size.width,
                    height: b.size.height,
                },
            );
            arena.layout_dirty.insert(slot);
        }
    }
    Ok(())
}

fn collect(arena: &NodeArena, slot: u32, out: &mut Vec<u32>) {
    if arena.props(slot).str(crate::PropId::Text).is_some() {
        out.push(slot);
        return;
    }
    for &child in arena.children(slot) {
        if arena.style(child).display == crate::Display::None {
            continue;
        }
        if arena.is_inline_box(child) {
            out.push(child);
        } else if arena.node_type(child) == NodeType::Text {
            collect(arena, child, out);
        }
    }
}
