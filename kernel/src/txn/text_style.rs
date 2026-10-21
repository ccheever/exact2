//! A platform text style's weight and leading move with what names it (LLP
//! 1115.000 D12.2): its node's size, and whether the `font` shorthand named
//! it. Either change reaches every node that inherits them, a child with a
//! size of its own included.
use super::*;
use crate::StyleId;

/// `changed` with the rows a text style gives, when the size of `slot`
/// moved and it named a style before (`named`) or does now.
pub(super) fn with_style_rows(
    arena: &NodeArena,
    slot: u32,
    changed: StyleMask,
    named: bool,
) -> StyleMask {
    if changed.has(StyleId::FontSize) && (named || arena.named_text_style(slot).is_some()) {
        changed
            .union(StyleMask::of(StyleId::FontWeight))
            .union(StyleMask::of(StyleId::LineHeight))
    } else {
        changed
    }
}

/// `prop` set or cleared on `slot`: when it is `fontShorthand`, its weight
/// and leading move for it and what inherits them.
pub(super) fn shorthand_changed(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    slot: u32,
    prop: PropId,
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    if prop != PropId::FontShorthand {
        return;
    }
    let rows = StyleMask::of(StyleId::FontWeight).union(StyleMask::of(StyleId::LineHeight));
    style_changed(arena, layout, slot, rows, receipt);
    propagate_inherited(arena, layout, slot, rows, touched, receipt);
}
