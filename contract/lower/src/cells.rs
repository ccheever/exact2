//! @ref LLP 1115.000 D16 — rows of a vertical `list` are cells: each host
//! draws a row as its platform's own list does (UIKit's cell, the Mac's
//! table, the neutral look's row, Material's list item), with the
//! platform's separator and press, and what the author writes on a row
//! still wins. The rule is the element, not a guess about which pressable
//! looks like a row, and it is decided here, once: Contract marks the list
//! (`listRows`), and every host reads the mark and takes the list's
//! children for its rows.
//!
//! A `list` is marked when its rows go down the page and its author said
//! nothing else of it:
//!
//! - not `appearance="none"`, the author's opt-out (a transcript, a
//!   document, a feed of cards: rows that are no cells);
//! - not `appearance="auto"`, the grouped form, which has its own rows
//!   (`grouped.rs`);
//! - not laid out sideways: a block, or a flex column. A flex row is a
//!   carousel, a grid is the author's arrangement, and a `display` or a
//!   `flex-direction` that is not a literal is not known to go down the
//!   page, so the list keeps its author's rows;
//! - not a list that scrolls sideways only;
//! - no child is a `section`: a sectioned list's rows are in its
//!   sections, and that list is the grouped form (`appearance="auto"
//!   listStyle="plain"` is the plain one; warning `lower-list-section`
//!   without it).
//!
//! What a row is: a child of the list. A heading, a filter or a count that
//! should not be a row goes outside the list.

use contract_syntax::{Attr, Expr, Node};

fn literal<'a>(attrs: &'a [Attr], name: &str) -> Option<Option<&'a str>> {
    attrs
        .iter()
        .rev()
        .find(|a| a.name == name)
        .map(|a| match &a.value {
            Expr::Str(s, _) => Some(s.as_str()),
            _ => None,
        })
}

/// Whether any node a list holds directly, through the regions that only
/// choose or repeat (`for`, `if`, `match`), is a `section`.
pub(crate) fn sectioned(children: &[Node]) -> bool {
    children.iter().any(|child| match child {
        Node::Element { tag, .. } => tag == "section",
        Node::When {
            then, otherwise, ..
        } => sectioned(then) || sectioned(otherwise),
        Node::Each { body, .. } => sectioned(body),
        Node::Match { some, none, .. } => sectioned(&some.1) || sectioned(none),
        Node::Use { .. } | Node::Children { .. } => false,
    })
}

/// Whether `tag`'s rows are cells: the rule above, read from the element's
/// attributes as its classes expanded them and from its children as the
/// inliner left them. `grouped` is `grouped::style`'s answer.
pub(crate) fn rows(tag: &str, attrs: &[Attr], children: &[Node], grouped: bool) -> bool {
    if tag != "list" || grouped {
        return false;
    }
    // `grouped::style` refuses an `appearance` that is no literal.
    if literal(attrs, "appearance").is_some() {
        return false;
    }
    let down = match literal(attrs, "display") {
        None | Some(Some("block")) => true,
        Some(Some("flex")) => literal(attrs, "flex-direction") == Some(Some("column")),
        Some(_) => false,
    };
    let scrolls = |name: &str| matches!(literal(attrs, name), Some(Some("scroll" | "auto")));
    let shut = |name: &str| matches!(literal(attrs, name), Some(Some("hidden" | "clip")));
    let sideways = attrs.iter().any(|a| a.name == "estimated-item-width")
        || (scrolls("overflow-x") && shut("overflow-y"));
    down && !sideways && !sectioned(children)
}
