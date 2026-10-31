//! A row's value goes under its title where the two do not fit side by
//! side, as UIKit's value cell puts its secondary text (iOS 27: a title
//! longer than its line at the default size, and every pair at the
//! accessibility sizes, where `prefersSideBySideTextAndSecondaryText` is
//! off), giving the title the row's width. Round 26's graders named the
//! amount beside a title broken into letters a tell ("Transpo/rt" in
//! expense-tracker).
//!
//! The title and its value are boxed in a row that wraps: the value keeps
//! the far edge while both fit (on iOS through the largest standard size:
//! the gap between them, `grouped-value-gap`, is wider than any line at the
//! accessibility sizes), and below that starts under the title, at
//! its leading edge, while a leading symbol or control and a trailing
//! accessory stay centred in the row's height, as the cell's are. How the
//! title is sized in the row (its `flex`, its `min-width`, the author's or
//! the sheet's) and its margins move to the box, which now holds that
//! place; a box whose title had no margins takes the cell's space above and
//! below, as a stack of lines does, unless the row's author padded it
//! (`lines`). In the box the title grows from its words' width, so the box
//! wraps when its words and the value's do not fit one line, under an
//! author's `flex=1` too.
//! More texts after the value (a lap's time and its total) go in the box
//! too, each wrapping under the one before, as a hand-built cell's stack of
//! labels turns vertical at the accessibility sizes; beside them a bare
//! title was squeezed to no width there (stopwatch, round 30).
//! A title that is a bare text is boxed only in a swipe's row, which the
//! platform's list carries as a custom row anyway, or before two or more
//! values: elsewhere a row of two bare texts is the value cell itself, which
//! the platform's list draws on iOS, and the box would make it custom. A title that is a `column` (one
//! text, or a title over a subtitle) is custom already. The Mac's form
//! keeps the two side by side (`grouped-value-wrap`).
//!
//! A value an `if` shows (one text, no `else`) is boxed the same way.
//!
//! A text followed by a worded `button` that ends the row (a message and its
//! Try Again) is boxed the same way, as a hand-built cell switches its stack
//! to vertical at the accessibility sizes (Apple's guidance for Dynamic
//! Type): the text keeps at least `BUTTON_TITLE_BASIS` beside the button,
//! and below that the button goes under it, at its leading edge. Without
//! it a message beside Try Again at the largest size broke inside every
//! word ("Ca/n't rea/ch", household-list, round 27). A button holding only
//! a symbol stays where it is: an info button is the row's accessory.

use super::{attr, m, n, s, Count};
use contract_syntax::{Attr, Expr, Node, Span};

/// The width a text keeps beside its row's trailing button: about ten
/// words' letters at the default size, beside a button's label on a phone
/// through the largest standard size, and past the row's width at the
/// accessibility sizes, which put the button under the text.
const BUTTON_TITLE_BASIS: &str = "9rem";

/// A `button` that holds a `text`: a labelled action, not a symbol alone.
fn worded_button(node: &Node) -> bool {
    matches!(node, Node::Element { tag, children, .. } if tag == "button"
        && children.iter().any(|c| matches!(c, Node::Element { tag, .. } if tag == "text")))
}

/// `parts` with the title and the value after it boxed, where `counts`
/// (each part's texts before and after it) say the title is a `column`, or
/// a `text` in a swipe's row (`swiped`), and the value the shown `text`
/// straight after it.
pub(super) fn boxed(
    children: &[Node],
    mut parts: Vec<Node>,
    counts: &[(Count, Count)],
    swiped: bool,
    lines: bool,
    grow: Expr,
    span: Span,
) -> Vec<Node> {
    let counted = |k: usize, before: usize| matches!(&counts[k], (Count::Known(b), Count::Known(a)) if *b == before && *a == before + 1);
    let tag = |k: usize| match &children[k] {
        Node::Element { tag, .. } => Some(tag.as_str()),
        _ => None,
    };
    let Some(title) = (0..children.len()).find(|&k| counted(k, 0)) else {
        return parts;
    };
    let value = title + 1;
    let button = value + 1 == children.len() && worded_button(&children[value]);
    // The shown texts straight after the value: a lap's time and its total.
    let mut end = value;
    while end + 1 < children.len()
        && tag(end + 1) == Some("text")
        && counted(end + 1, end + 1 - title)
    {
        end += 1;
    }
    // A value shown only sometimes: an `if` holding one text and no `else`
    // (water-tracker's "Goal met" beside its day's bar, round 33).
    let sometimes = |k: usize| {
        matches!(&children[k], Node::When { then, otherwise, .. } if otherwise.is_empty()
            && matches!(then.as_slice(), [Node::Element { tag, .. }] if tag == "text"))
            && matches!(&counts[k].0, Count::Known(1))
    };
    if value >= children.len()
        || !(tag(title) == Some("column")
            || (swiped || button || end > value) && tag(title) == Some("text"))
        || !(button || tag(value) == Some("text") && counted(value, 1) || sometimes(value))
    {
        return parts;
    }
    let mut held: Vec<Node> = parts.drain(title..=end).collect();
    let sizing = |a: &Attr| {
        matches!(
            a.name.as_str(),
            "flex"
                | "flex-grow"
                | "flex-shrink"
                | "flex-basis"
                | "min-width"
                | "margin-top"
                | "margin-bottom"
        )
    };
    let mut moved = Vec::new();
    let mut instance = 0;
    if let Node::Element {
        attrs,
        instance: at,
        ..
    } = &mut held[0]
    {
        instance = *at;
        moved = attrs.iter().filter(|a| sizing(a)).cloned().collect();
        attrs.retain(|a| !sizing(a));
        attrs.splice(
            0..0,
            [
                attr("flex-grow", Expr::Number(1.0, span), span),
                n("flex-shrink", 1.0, span),
                n("min-width", 0.0, span),
            ],
        );
        if button {
            attrs.insert(0, s("flex-basis", BUTTON_TITLE_BASIS, span));
        }
    }
    let mut attrs = vec![
        s("display", "flex", span),
        s("flex-direction", "row", span),
        m("flex-wrap", "value-wrap", span),
        s("align-items", "center", span),
        m("column-gap", "value-gap", span),
        m("row-gap", "stack-gap", span),
        attr("flex-grow", grow, span),
        n("flex-shrink", 1.0, span),
        n("min-width", 0.0, span),
    ];
    if lines && !moved.iter().any(|a| a.name.starts_with("margin")) {
        attrs.extend([
            m("margin-top", "lines-padding-block", span),
            m("margin-bottom", "lines-padding-block", span),
        ]);
    }
    attrs.extend(moved);
    parts.insert(
        title,
        Node::Element {
            tag: "row".into(),
            positional: Vec::new(),
            attrs,
            children: held,
            span,
            instance,
        },
    );
    parts
}

/// Whether the author wrote how wide `node` is in its row.
pub(super) fn sized(node: &Node) -> bool {
    matches!(node, Node::Element { attrs, .. } if attrs.iter().any(|a| matches!(a.name.as_str(), "flex" | "flex-grow" | "flex-basis" | "width" | "min-width")))
}

/// Whether the author wrote that `node` grows in its row: a `flex` or
/// `flex-grow` other than none or zero, or one computed.
pub(super) fn grows(node: &Node) -> bool {
    matches!(node, Node::Element { attrs, .. } if attrs.iter().any(|a| match (a.name.as_str(), &a.value) {
        ("flex" | "flex-grow", Expr::Number(v, _)) => *v > 0.0,
        ("flex" | "flex-grow", Expr::Str(v, _)) => !(v.trim() == "none" || v.trim_start().starts_with('0')),
        ("flex" | "flex-grow", _) => true,
        _ => false,
    }))
}
