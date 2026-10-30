//! @ref LLP 1115.000 D16 — a list's rows, for a host that draws them itself.
//!
//! Contract decides which lists' rows are cells and marks each (`listRows`,
//! `contract/lower/src/cells.rs`); a row is a child of such a list. What a
//! row then is belongs to its platform, and what its author wrote on it
//! still wins. This is the part every painted host reads the same way, so
//! it is said once: which node is a row, and whether the platform's
//! separator belongs under it. The web's sheet has to say the same in CSS
//! (`host/web/neutral.css`), since a page keeps no kernel: the same six
//! rows are held to the same answers for this (`contract/cli/tests/it/
//! list_rows.rs`) and for the sheet (`host/web/tests/neutral-look.test.mjs`).

use crate::generated::{NodeType, PropId, StyleId};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};

/// The node whose rows say what a row's author wrote: the row itself, or,
/// for the wrapper the runner puts round a windowed list's row (it carries
/// `listItemKey`), the row inside it.
fn authored<'a>(kernel: &'a Kernel, row: &NodeRef<'a>) -> Option<NodeRef<'a>> {
    if row.props.str(PropId::ListItemKey).is_none() {
        return kernel.node(row.id);
    }
    row.children().first().and_then(|c| kernel.node(*c))
}

/// @ref LLP 1138 — the platform's cell margin on a cells list's row: the
/// row's padding at each end its author wrote none of, and the row's box the
/// cell's (`border-box`, unless its author wrote `box-sizing`), so a row
/// written `width: 100%` still spans its list. The margin is the one the host
/// reports for the list (`Kernel::set_list_margins`: UIKit's, which follows
/// the orientation and the device), else the look's (`list-row-inset`); a
/// terminal's rows take none. Layout only, as a grouped list's margins are:
/// the row's own rows are the author's.
pub(crate) fn apply_margins(
    arena: &crate::arena::NodeArena,
    slot: u32,
    s: &mut taffy::style::Style,
) {
    use taffy::style::LengthPercentage as Lp;
    let Some(m) = row_inset(arena, slot) else {
        return;
    };
    let written = &arena.style(slot).written;
    if !written.has(StyleId::PaddingLeft) {
        s.padding.left = Lp::length(m[0]);
    }
    if !written.has(StyleId::PaddingRight) {
        s.padding.right = Lp::length(m[1]);
    }
    if !written.has(StyleId::BoxSizing) {
        s.box_sizing = taffy::style::BoxSizing::BorderBox;
    }
}

/// Restyle what a prop decides a row's place by (LLP 1138): a list's
/// `listRows` its rows, a wrapper's `listItemKey` and a swipe's
/// `swipeContent` and `swipeLeading` the box and what it holds. Whether it
/// restyled.
pub(crate) fn prop_changed(
    arena: &crate::arena::NodeArena,
    layout: &mut dyn crate::layout::LayoutMirror,
    slot: u32,
    prop: PropId,
    touched: &mut Vec<crate::NodeKey>,
) -> bool {
    let rows: Vec<u32> = match prop {
        // A cell's spans move its table's grid (`layout/table.rs`), and a
        // table's or a description list's tag brings its UA sheet's layout
        // (`style/table.rs`).
        PropId::ColSpan | PropId::RowSpan => {
            if let Some(node) = arena.taffy(slot) {
                layout.restyle(arena, slot, node);
            }
            return true;
        }
        PropId::SemanticTag if crate::style::table::styled(arena, slot) => {
            if let Some(node) = arena.taffy(slot) {
                layout.restyle(arena, slot, node);
            }
            return true;
        }
        PropId::ListRows => arena.children(slot).to_vec(),
        PropId::ListItemKey | PropId::SwipeContent | PropId::SwipeLeading => vec![slot],
        _ => return false,
    };
    for row in rows {
        restyle_touched(arena, layout, row, touched);
    }
    true
}

/// Restyle `slot` as [`crate::grouped::restyle`] does, marking what it
/// restyles touched: a content region one of them owns is laid out again,
/// its padding having changed while its border box may not have.
fn restyle_touched(
    arena: &crate::arena::NodeArena,
    layout: &mut dyn crate::layout::LayoutMirror,
    slot: u32,
    touched: &mut Vec<crate::NodeKey>,
) {
    crate::grouped::restyle(arena, layout, slot);
    let mut walk = vec![(slot, 3u8)];
    while let Some((s, depth)) = walk.pop() {
        touched.push(arena.key(s));
        if depth > 0 {
            walk.extend(arena.children(s).iter().map(|&c| (c, depth - 1)));
        }
    }
}

/// A swipe's children in a cells list moved: which is first, and so its row
/// (`row_of`), may have changed.
pub(crate) fn children_changed(
    arena: &crate::arena::NodeArena,
    layout: &mut dyn crate::layout::LayoutMirror,
    slot: u32,
    touched: &mut Vec<crate::NodeKey>,
) {
    if arena.props(slot).str(PropId::SwipeContent).is_some() && in_cells(arena, slot) {
        for &child in arena.children(slot) {
            restyle_touched(arena, layout, child, touched);
        }
    }
}

/// The look's row inset at each end, `None` in a terminal or where the
/// look has none.
fn look_margins() -> Option<[f32; 2]> {
    use crate::style::metrics::{column, length, Column};
    if column() == Column::Terminal {
        return None;
    }
    let inset = length(crate::generated::metric::LIST_ROW_INSET, 0.0, 0.0)?;
    Some([inset, inset])
}

/// The inset `slot` takes at its left and right as a cells list's row:
/// the margin its host reported for the list, else the look's.
pub(crate) fn row_inset(arena: &crate::arena::NodeArena, slot: u32) -> Option<[f32; 2]> {
    arena
        .list_margins(inset_row(arena, slot)?)
        .or_else(look_margins)
}

/// The cells list whose inset `slot` takes as its row: a row that is no
/// form control, which keeps its own padding (the web's sheet resets a
/// switch's with `!important`); a button is a row like any box.
fn inset_row(arena: &crate::arena::NodeArena, slot: u32) -> Option<u32> {
    let control = match arena.node_type(slot) {
        NodeType::TextInput => true,
        NodeType::Control => {
            crate::ControlKind::of(NodeType::Control, arena.props(slot))
                != Some(crate::ControlKind::Button)
        }
        _ => false,
    };
    (!control)
        .then(|| row_of(arena, slot))
        .flatten()
        .map(|(list, _)| list)
}

impl Kernel {
    /// Whether `view` takes its cells list's row inset at an end its author
    /// wrote no padding at (LLP 1138): a host that draws a native button's
    /// title by its own insets places it by these sides instead.
    pub fn takes_row_inset(&self, view: crate::NodeKey) -> bool {
        self.arena()
            .resolve(view)
            .is_some_and(|slot| inset_row(self.arena(), slot).is_some())
    }
}

/// Whether `slot` stands within a cells list's rows, as deep as a row's
/// inset reaches (a windowed row's wrapper, its swipe, the swipe's content,
/// and what arrives beside that content, which can make it the first child):
/// what arrives or leaves there is styled again.
pub(crate) fn in_cells(arena: &crate::arena::NodeArena, slot: u32) -> bool {
    std::iter::successors(arena.parent(slot), |s| arena.parent(*s))
        .take(4)
        .any(|s| {
            arena.node_type(s) == NodeType::List
                && arena.props(s).bool(PropId::ListRows) == Some(true)
        })
}

/// The cells list `slot` is a row of, and whether it is a swipe's content:
/// the list's child (not a windowed row's wrapper, nor a swipe's scroll,
/// whose content is the row), the row in a windowed list's wrapper, or a
/// swipe's content, its scroll's first child (a `delete=` row's). A swipe
/// with leading actions is its author's.
pub(crate) fn row_of(arena: &crate::arena::NodeArena, slot: u32) -> Option<(u32, bool)> {
    let props = |slot: u32| arena.props(slot);
    let cells = |s: u32| {
        (arena.node_type(s) == NodeType::List && props(s).bool(PropId::ListRows) == Some(true))
            .then_some(s)
    };
    // The list a box is a row's place in: its parent, or its windowed wrapper's.
    let place = |s: u32| {
        let parent = arena.parent(s)?;
        cells(parent).or_else(|| {
            props(parent)
                .str(PropId::ListItemKey)
                .and_then(|_| arena.parent(parent))
                .and_then(cells)
        })
    };
    let own = props(slot);
    if own.str(PropId::ListItemKey).is_none() && own.str(PropId::SwipeContent).is_none() {
        if let Some(list) = place(slot) {
            return Some((list, false));
        }
    }
    // A swipe's content: its scroll's first child, where the scroll names no
    // leading actions to stand before it (what the web's sheet can select).
    let scroll = arena.parent(slot)?;
    let swipe = props(scroll);
    (swipe.str(PropId::SwipeContent).is_some()
        && swipe.str(PropId::SwipeLeading).is_none()
        && arena.children(scroll).first() == Some(&slot))
    .then(|| place(scroll).map(|list| (list, true)))
    .flatten()
}

fn wrote(node: &NodeRef<'_>, width: StyleId, line: StyleId) -> bool {
    node.style.written.has(width) || node.style.written.has(line)
}

/// Whether the platform's line between a row and the row after it gives
/// way to their author: `row` (the box that draws it, a windowed list's
/// wrapper included) is hidden from assistive technology or wrote
/// `channel`, the row the web's sheet draws the line with; or the author's
/// box for it (`own`) wrote its bottom edge, or the next row's (`next`) its
/// top edge: a width or a style, `none` included. The author's separator
/// wins, and so does the author's "no separator here". A list's rows and a
/// grouped list's (`Kernel::grouped_separator`) are told by this one rule.
pub(crate) fn yields(
    row: &NodeRef<'_>,
    own: Option<&NodeRef<'_>>,
    next: Option<&NodeRef<'_>>,
    channel: StyleId,
) -> bool {
    row.props.bool(PropId::AccessibilityElementsHidden) == Some(true)
        || row.style.written.has(channel)
        || own.is_some_and(|n| wrote(n, StyleId::BorderWidthBottom, StyleId::BorderStyleBottom))
        || next.is_some_and(|n| wrote(n, StyleId::BorderWidthTop, StyleId::BorderStyleTop))
}

impl Kernel {
    /// The list `id` is a row of: its parent, when that is a `list` whose
    /// rows are cells.
    pub fn row_list(&self, id: ViewId) -> Option<ViewId> {
        let parent = self.node(self.node(id)?.parent?)?;
        (parent.node_type == NodeType::List && parent.props.bool(PropId::ListRows) == Some(true))
            .then_some(parent.id)
    }

    /// The row `id` is in, itself included: the nearest node from it up
    /// that is a row of a list whose rows are cells. What presses inside a
    /// row answers as the row's platform does (a cell's highlight, the
    /// neutral look's wash), and never with the dim of a pressable that is
    /// no row.
    pub fn row_holding(&self, id: ViewId) -> Option<ViewId> {
        let mut at = self.node(id)?;
        loop {
            if self.row_list(at.id).is_some() {
                return Some(at.id);
            }
            at = self.node(at.parent?)?;
        }
    }

    /// Whether the platform's separator belongs under the row `id`: it is a
    /// row with a row after it; it is no spacer of a windowed list (hidden
    /// from assistive technology, as an author's own decorative row is);
    /// and its author wrote neither its bottom edge nor the next row's top
    /// edge (a width or a style, `none` included: the author's separator
    /// wins, and so does the author's "no separator here"), nor a
    /// `box-shadow` on it, which is where the web's sheet draws the line.
    pub fn row_separator(&self, id: ViewId) -> bool {
        let Some(list) = self.row_list(id).and_then(|l| self.node(l)) else {
            return false;
        };
        let rows = list.children();
        let Some(at) = rows.iter().position(|r| *r == id) else {
            return false;
        };
        let (Some(row), Some(next)) = (self.node(id), rows.get(at + 1).and_then(|n| self.node(*n)))
        else {
            return false;
        };
        !yields(
            &row,
            authored(self, &row).as_ref(),
            authored(self, &next).as_ref(),
            StyleId::BoxShadow,
        )
    }
}
