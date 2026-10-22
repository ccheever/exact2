//! HTML's top layer on kernel hosts: an open popover is `position: fixed`
//! in the UA sheet, out of its parent's flow, so opening one moves nothing
//! around where it was written. A host lifts and places it; on iOS
//! Harmony's room composer, a popover written after it, had jumped up by its
//! height. A closed one is covered whole (`display: none`).
use crate::arena::NodeArena;
use crate::PositionType;

/// Whether `slot` lays out in the top layer, out of flow: a popover, but one
/// its author made `sticky`, which keeps its own machinery (`apply`,
/// `cover::scroller_insets_bottom`).
pub(crate) fn top_layer(arena: &NodeArena, slot: u32) -> bool {
    arena.props(slot).str(crate::PropId::Popover).is_some()
        && arena.style(slot).position_type != PositionType::Sticky
}

/// A top-layer box's engine style: absolute, out of flow. Its containing
/// block stays its positioned ancestor, not the viewport (QUEUE).
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    if top_layer(arena, slot) {
        s.position = taffy::style::Position::Absolute;
    }
}

/// A box's `popover` prop set or cleared: it moves in or out of its
/// parent's flow, so its engine style is derived again, and it joins or
/// leaves its covered box's bottom-edge chain (`cover::chain_changed`).
pub(crate) fn prop_changed(
    arena: &NodeArena,
    layout: &mut dyn crate::layout::LayoutMirror,
    slot: u32,
    prop: crate::PropId,
) -> bool {
    if prop != crate::PropId::Popover {
        return false;
    }
    if let Some(node) = arena.taffy(slot) {
        layout.restyle(arena, slot, node);
    }
    crate::kernel::cover_chain_changed(arena, layout, slot);
    true
}
