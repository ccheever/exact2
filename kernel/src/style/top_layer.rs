//! HTML's top layer on kernel hosts: an open popover is `position: fixed`
//! in the UA sheet, out of its parent's flow, so opening one moves nothing
//! around where it was written. A host lifts and places it; on iOS
//! Harmony's room composer, a popover written after it, had jumped up by its
//! height. A closed one is covered whole (`display: none`).
use crate::arena::NodeArena;
use crate::PositionType;

/// How `slot` is positioned for layout: its authored `position`, but
/// absolute in the top layer. Every read of `position` that means layout
/// goes through this ([`crate::NodeRef::layout_position`] on the Rust hosts,
/// `NodeView.layoutPosition` in Swift); `scripts/position-reads.test.mjs`
/// fails on a new direct read of the authored row outside its allowlist.
pub(crate) fn layout_position(arena: &NodeArena, slot: u32) -> PositionType {
    let style = arena.computed_source(slot, crate::StyleId::PositionType);
    of(style, arena.props(slot))
}

/// [`layout_position`] from a node's rows and props, for a tree that isn't
/// the arena (`paint_order::Facts`).
pub(crate) fn of(style: &crate::StyleProps, props: &crate::PropList) -> PositionType {
    match style.position_type {
        PositionType::Sticky => PositionType::Sticky,
        _ if props.str(crate::PropId::Popover).is_some() => PositionType::Absolute,
        authored => authored,
    }
}

/// A top-layer box's engine style: absolute, out of flow. Its containing
/// block stays its positioned ancestor, not the viewport (QUEUE). One its
/// author made `sticky` keeps its own machinery (`cover::scroller_insets_bottom`).
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    if layout_position(arena, slot) == PositionType::Absolute {
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
