//! Structural changes can alter a header or its buttons with identical ink.
use super::*;

pub(super) fn before_prop(arena: &NodeArena, slot: u32, prop: PropId) -> Vec<(u32, (bool, bool))> {
    if crate::style::metrics::column() != crate::style::metrics::Column::Neutral {
        return Vec::new();
    }
    match prop {
        PropId::SemanticTag => vec![(slot, arena.header_context(slot))],
        PropId::NavigationKey => arena
            .children(slot)
            .iter()
            .copied()
            .filter(|s| arena.props(*s).str(PropId::SemanticTag) == Some("header"))
            .map(|s| (s, arena.header_context(s)))
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn after_prop(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    before: Vec<(u32, (bool, bool))>,
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
    before: (bool, bool),
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    let after = arena.header_context(slot);
    if before == after {
        return;
    }
    if before.0 != after.0 {
        arena.refresh_sticky(slot);
        let rows = NodeArena::header_rows();
        style_changed(arena, layout, slot, rows, receipt);
        propagate_inherited(arena, layout, slot, rows, touched, receipt);
    }
    // Retained buttons can lose/gain their edge with equal inherited text.
    // Descendant paint notification is required even with authored header rows.
    for child in arena.subtree(slot) {
        arena.mark_export_dirty(child, NodeFlags::PAINT_DIRTY);
        touched.push(arena.key(child));
    }
}
