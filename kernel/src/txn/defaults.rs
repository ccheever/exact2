//! Structural defaults can change with identical inherited ink.
use super::*;
use crate::arena::defaults::DefaultContext;

pub(super) fn before_prop(
    arena: &NodeArena,
    slot: u32,
    prop: PropId,
) -> Vec<(u32, DefaultContext)> {
    if crate::style::metrics::column() != crate::style::metrics::Column::Neutral {
        return Vec::new();
    }
    match prop {
        PropId::SemanticTag => vec![(slot, arena.default_context(slot))],
        PropId::NavigationKey => arena
            .children(slot)
            .iter()
            .copied()
            .filter(|s| arena.props(*s).str(PropId::SemanticTag) == Some("header"))
            .map(|s| (s, arena.default_context(s)))
            .collect(),
        PropId::AccessibilityRole | PropId::AccessibilityControls => {
            let mut before = vec![(slot, arena.default_context(slot))];
            if let Some(parent) = arena.parent(slot) {
                before.push((parent, arena.default_context(parent)));
            }
            before
        }
        _ => Vec::new(),
    }
}

pub(super) fn after_prop(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    before: Vec<(u32, DefaultContext)>,
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    for (slot, context) in before {
        changed(arena, layout, slot, context, touched, receipt);
    }
}

pub(super) fn changed(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    before: DefaultContext,
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    let after = arena.default_context(slot);
    if before == after {
        return;
    }
    if before.header != after.header || before.tab_bar != after.tab_bar {
        arena.refresh_sticky(slot);
        let rows = NodeArena::structural_rows();
        style_changed(arena, layout, slot, rows, receipt);
        propagate_inherited(arena, layout, slot, rows, touched, receipt);
    }
    // Retained buttons can lose/gain their edge with equal inherited text,
    // even when the enclosing surface has authored all its own style rows.
    for child in arena.subtree(slot) {
        arena.mark_export_dirty(child, NodeFlags::PAINT_DIRTY);
        touched.push(arena.key(child));
    }
}
