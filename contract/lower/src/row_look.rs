//! `-exact-row-look="platform"` (LLP 1139): an element whose box's look
//! (its background, border, padding and shadow) is its own where it stands
//! alone and the platform's row where it is a grouped list's row, as Exact
//! Content's answer tile is a list's cell in a poll. There its look is
//! dropped and the sheet draws the row (`grouped::row`); anywhere else the
//! attribute is (`swipe_rows::stray`).

use contract_syntax::{Attr, Node};

pub(crate) const ROW_LOOK: &str = "-exact-row-look";

/// Whether `a` is a look a node marked [`ROW_LOOK`] yields to a grouped
/// list's row (the design server's write check keeps no row of it,
/// `theme::scan`).
pub(crate) fn look(a: &Attr) -> bool {
    ["background", "border", "padding", "box-shadow"]
        .iter()
        .any(|p| a.name.starts_with(p))
}

/// `node` without the look it yields to a grouped list's row, and without
/// the attribute; `None` when it yields none.
pub(crate) fn yielded(node: &Node) -> Option<Node> {
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        span,
        instance,
    } = node
    else {
        return None;
    };
    if !attrs.iter().any(|a| a.name == ROW_LOOK) {
        return None;
    }
    Some(Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: attrs
            .iter()
            .filter(|a| a.name != ROW_LOOK && !look(a))
            .cloned()
            .collect(),
        children: children.clone(),
        span: *span,
        instance: *instance,
    })
}
