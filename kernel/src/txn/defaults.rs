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
        PropId::SemanticTag => std::iter::once(slot)
            .chain(rows(arena, slot))
            .map(|s| (s, arena.default_context(s)))
            .collect(),
        PropId::NavigationKey => arena
            .children(slot)
            .iter()
            .copied()
            .filter(|s| arena.props(*s).str(PropId::SemanticTag) == Some("header"))
            .map(|s| (s, arena.default_context(s)))
            .collect(),
        PropId::AccessibilityRole
        | PropId::AccessibilityControls
        | PropId::Type
        | PropId::ButtonStyle
        | PropId::Popover
        | PropId::DefaultSurface
        | PropId::AlertPart => {
            let mut before = vec![(slot, arena.default_context(slot))];
            if let Some(parent) = arena.parent(slot) {
                before.push((parent, arena.default_context(parent)));
                if arena.neutral_alert(parent) {
                    before.extend(
                        arena
                            .children(parent)
                            .iter()
                            .copied()
                            .map(|s| (s, arena.default_context(s))),
                    );
                }
            }
            if matches!(prop, PropId::AccessibilityRole | PropId::AlertPart) {
                before.extend(rows(arena, slot).map(|row| (row, arena.default_context(row))));
            }
            before
        }
        _ => Vec::new(),
    }
}

/// `slot`'s rows, whose defaults its tag, role or part decides: its
/// children, and an alert's rows in its part boxes too (arena/alert.rs)
fn rows(arena: &NodeArena, slot: u32) -> impl Iterator<Item = u32> + '_ {
    arena.children(slot).iter().flat_map(move |&child| {
        let parts = matches!(
            arena.props(child).str(PropId::AlertPart),
            Some("message" | "actions")
        );
        std::iter::once(child).chain(
            parts
                .then(|| arena.children(child))
                .into_iter()
                .flatten()
                .copied(),
        )
    })
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
    // Whether a symbol image draws in its colour (LLP 1121 stage 9 B) can
    // change with the colour equal: its images are sent again.
    if before.ink_said != after.ink_said {
        for s in arena.subtree(slot) {
            let symbol = arena.node_type(s) == NodeType::Image
                && arena
                    .props(s)
                    .str(PropId::ImageSource)
                    .is_some_and(|v| v.starts_with("symbol:"));
            if symbol {
                arena.mark_export_dirty(s, NodeFlags::PAINT_DIRTY);
                touched.push(arena.key(s));
            }
        }
    }
    if before
        == (DefaultContext {
            ink_said: before.ink_said,
            ..after
        })
    {
        return;
    }
    let mut rows = StyleMask::EMPTY;
    if before.alert != after.alert {
        rows = rows.union(NodeArena::alert_rows());
    }
    if before.header != after.header {
        arena.refresh_sticky(slot);
        rows = rows.union(NodeArena::header_rows());
    }
    if before.tab_bar != after.tab_bar {
        rows = rows.union(NodeArena::tab_bar_rows());
    }
    if !rows.is_empty() {
        style_changed(arena, layout, slot, rows, receipt);
        propagate_inherited(arena, layout, slot, rows, touched, receipt);
    }
    // What is said above changed, if no value here did: a bare text style
    // below fills what is not (LLP 1115.000 D12.2).
    let said = before
        .said_style
        .minus(after.said_style)
        .union(after.said_style.minus(before.said_style));
    propagate_inherited(arena, layout, slot, said.minus(rows), touched, receipt);
    if before.native_tab != after.native_tab {
        control_env_changed(arena, layout, slot);
        receipt.layout_invalidated = true;
    }
    // Retained buttons can lose/gain their edge with equal inherited text,
    // even when the enclosing surface has authored all its own style rows.
    for child in arena.subtree(slot) {
        arena.mark_export_dirty(child, NodeFlags::PAINT_DIRTY);
        touched.push(arena.key(child));
    }
}
