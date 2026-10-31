//! `lower-header-nested`: a route's `header` one or more boxes down from
//! it, the first child of a box that is the route's first child, as a
//! screen component's root `column` holds it when the route column that
//! uses the component is in the root file. iOS makes the bar only of the
//! route's own first child (`exact_kernel::bars::header_shape`), so this
//! header is drawn in the page as written. The euxy port's tabs did this.
//! A header that draws itself (`appearance="none"`) is not warned, here or
//! as the route's first child (`no_bar`).

use super::{computed, literal, NOT_THE_BAR, OR_NO_BAR};
use crate::LowerError;
use contract_syntax::Node;

/// The boxes a route's first child may be on the way down to its header:
/// plain layout boxes. A `section` is left out (a grouped list's section
/// opens with its own `header`), and so is a scroller, a list and anything
/// else that gives its first child a meaning of its own.
pub(super) const WRAPPERS: &[&str] = &["view", "box", "column", "row", "main", "article"];

/// The `header` at the end of the route's chain of first children, past at
/// least one box, with the tag of the box that is the route's first child.
/// An `if`, `match`, `for`, use or slot on the way is given the benefit.
pub(super) fn nested_header(children: &[Node], out: &mut Vec<LowerError>) {
    let Some(Node::Element {
        tag: outer,
        attrs,
        children,
        ..
    }) = children.first()
    else {
        return;
    };
    if !WRAPPERS.contains(&outer.as_str()) || hidden(attrs) {
        return;
    }
    let mut at = children;
    while let Some(Node::Element {
        tag,
        attrs,
        children,
        ..
    }) = at.first()
    {
        if hidden(attrs) {
            return;
        }
        if tag == "header" {
            // A header that draws itself is no bar wherever it sits: moving
            // it would project nothing.
            if no_bar(attrs) {
                return;
            }
            out.push(LowerError {
                id: "lower-header-nested",
                message: format!("this `header` is not its route's first child (it is inside the `{outer}` that is), so on iOS it is not the navigation bar: {NOT_THE_BAR} Make it the route's first child: move the route's `navigationKey`, with its `navigationPresentation`, `navigationScroll` and the rest, onto the box that holds the header (a screen component's root), or move the header up{OR_NO_BAR} (docs/recipes/navigation-stack.md)"),
                span: at[0].span(),
            });
            return;
        }
        if !WRAPPERS.contains(&tag.as_str()) {
            return;
        }
        at = children;
    }
}

fn hidden(attrs: &[contract_syntax::Attr]) -> bool {
    literal(attrs, "display") == Some("none") || computed(attrs, "display")
}

/// A header hidden, or drawing itself (`appearance="none"`, LLP 1075.003
/// §9.10, as a site's does; a computed one has the benefit): iOS makes no bar
/// of it, not even a pushed route's system Back alone, so it hides nothing.
pub(super) fn no_bar(attrs: &[contract_syntax::Attr]) -> bool {
    hidden(attrs) || literal(attrs, "appearance") == Some("none") || computed(attrs, "appearance")
}
