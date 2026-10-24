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
/// the arena (`paint_order::Facts`). A popover keeps its `sticky` only where
/// its author wrote it: a platform start's (the neutral route header's) is
/// no author's, and the box goes to the top layer as any other popover.
pub(crate) fn of(style: &crate::StyleProps, props: &crate::PropList) -> PositionType {
    let authored = style.position_type;
    let sticky =
        authored == PositionType::Sticky && style.written.has(crate::StyleId::PositionType);
    if props.str(crate::PropId::Popover).is_some() && !sticky {
        PositionType::Absolute
    } else {
        authored
    }
}

/// Whether `slot` is in the top layer: a popover its author didn't make
/// `sticky`. It is laid out in the viewport (`Kernel::imply_blocks`).
pub(crate) fn lifted(arena: &NodeArena, slot: u32) -> bool {
    arena.props(slot).str(crate::PropId::Popover).is_some()
        && layout_position(arena, slot) == PositionType::Absolute
}

/// A top-layer box's engine style: absolute, out of flow, its insets
/// offsets (a platform start's sticky row had laid them out as `auto`).
/// Its containing block is the viewport, a block the kernel implies
/// (`block::apply` then unsets the insets: a host places it), unless it
/// is a root or an inline run. One its author made `sticky` keeps its own
/// machinery (`cover::scroller_insets_bottom`).
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    if layout_position(arena, slot) == PositionType::Absolute {
        s.position = taffy::style::Position::Absolute;
        let (style, env) = (arena.effective_style(slot), arena.env());
        s.inset = taffy::geometry::Rect {
            top: style.top.to_lpa(env),
            right: style.right.to_lpa(env),
            bottom: style.bottom.to_lpa(env),
            left: style.left.to_lpa(env),
        };
    }
}

/// A box's `popover` prop set or cleared: it moves in or out of its
/// parent's flow, so its engine style is derived again, it joins or leaves
/// its covered box's bottom-edge chain (`cover::chain_changed`), and the
/// sticky machinery takes it or lets it go (a platform start's sticky).
pub(crate) fn prop_changed(
    arena: &mut NodeArena,
    layout: &mut dyn crate::layout::LayoutMirror,
    slot: u32,
    prop: crate::PropId,
) -> bool {
    if prop != crate::PropId::Popover {
        return false;
    }
    arena.refresh_sticky(slot);
    if let Some(node) = arena.taffy(slot) {
        layout.restyle(arena, slot, node);
    }
    crate::kernel::cover_chain_changed(arena, layout, slot);
    true
}
