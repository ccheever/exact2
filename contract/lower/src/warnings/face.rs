//! What the iOS bar draws of a header's boxes: a filled box before the
//! heading is the title's avatar, a line of texts and symbols after it the
//! subtitle, and a readout (`role="status"`, LLP 1075.003 §9.15) is drawn
//! from the texts, symbols and sized dots it holds (`ChipFace`,
//! host/apple/Sources/ExactKit/IOS/BarFaceIOS.swift). A readout that holds
//! none of them, a `progress` alone, is no item of the bar
//! (NavigationBarIOS.swift) and is hidden with the rest of the header.

use super::{computed, has, holds_heading, kids, literal, part, Part, BOXES};
use crate::LowerError;
use contract_syntax::{Expr, Node, Span};

/// A box with a literal fill: before the heading, the title's avatar.
pub(super) fn filled(node: &Node) -> bool {
    let Node::Element { attrs, .. } = node else {
        return false;
    };
    has(attrs, "background-color") || has(attrs, "background")
}

/// A passive box of texts and symbol images, one of each at least: a
/// subtitle's line of glyphs (Signal's "🔕 Muted ⏱ 1w").
pub(super) fn glyph_line(node: &Node) -> bool {
    let (mut symbol, mut text) = (false, false);
    for k in kids(node) {
        match (part(k), k) {
            (Part::Text, _) => text = true,
            (
                Part::Other,
                Node::Element {
                    tag, positional, ..
                },
            ) if tag == "image" => match positional.first() {
                // A computed source may be a symbol: the benefit.
                Some(Expr::Str(s, _)) if !s.starts_with("symbol:") => return false,
                Some(_) => symbol = true,
                None => return false,
            },
            (Part::Hidden, _) => {}
            _ => return false,
        }
    }
    symbol && text && !filled(node)
}

/// The readouts at `node` or under it, through boxes and the branches of an
/// `if`, `match` or `for`, that draw nothing on the bar.
pub(super) fn faceless(node: &Node, out: &mut Vec<Span>) {
    let inside: Vec<&Node> = match (part(node), node) {
        // A status around the heading is no readout: looked into for one.
        (Part::Status, _) if holds_heading(node) => kids(node).iter().collect(),
        (Part::Status, _) => {
            if !draws(node) {
                out.push(node.span());
            }
            return;
        }
        (Part::Box, _) => kids(node).iter().collect(),
        (
            Part::Doubt,
            Node::When {
                then, otherwise, ..
            },
        ) => then.iter().chain(otherwise).collect(),
        (Part::Doubt, Node::Match { some, none, .. }) => some.1.iter().chain(none).collect(),
        (Part::Doubt, Node::Each { body, .. }) => body.iter().collect(),
        _ => return,
    };
    for n in inside {
        faceless(n, out);
    }
}

/// Whether `node`, in a readout, may draw on the bar: a text, a symbol
/// (`image "symbol:…"`), or a dot, a box with a fill, a width and a height
/// and nothing in it (only then is its size sent to the host,
/// host/apple/src/style/kept.rs). A box draws what it holds; a `progress`
/// and a picture draw nothing. Any other node, a computed `display` or
/// source, and an `if`, `for`, `match`, use or slot are given the benefit.
fn draws(node: &Node) -> bool {
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        ..
    } = node
    else {
        return true;
    };
    if computed(attrs, "display") {
        return true;
    }
    let sized = |names: [&str; 2]| names.iter().any(|n| has(attrs, n));
    match tag.as_str() {
        _ if literal(attrs, "display") == Some("none") => false,
        "progress" => false,
        "image" => {
            !matches!(positional.first(), Some(Expr::Str(s, _)) if !s.starts_with("symbol:"))
        }
        t if BOXES.contains(&t) && children.is_empty() => {
            filled(node) && sized(["width", "inline-size"]) && sized(["height", "block-size"])
        }
        t if BOXES.contains(&t) => children.iter().any(draws),
        _ => true,
    }
}

/// `lower-header-unplaced` for a readout [`faceless`] found.
pub(super) fn unshown(span: Span) -> LowerError {
    LowerError {
        id: "lower-header-unplaced",
        message: "this readout (`role=\"status\"`) in this route's `header` is not shown on iOS: the navigation bar draws a readout from the texts, symbols (`image \"symbol:…\"`) and sized dots (an empty box with a `background-color`, a `width` and a `height` in points) it holds, and this one holds none (a `progress` or a picture is not drawn there). Give it a `text` or a symbol, or move it below the `header`, into the route's content (docs/contract-for-agents.md, \"Tabs and stacks\")".into(),
        span,
    }
}
