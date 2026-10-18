//! A grouped row's actions and disclosures (LLP 1116.019 D6): what the
//! platform's list draws in the tint, and in its disclosure's grey.

use super::{accessory, hidden, over, s, symbol, with, LABEL, MARK, TERTIARY};
use contract_syntax::Node;

/// Whether a row's shown parts are an action cell's: one text, a symbol
/// that is no accessory before it at most.
pub(super) fn action_cell(children: &[Node]) -> bool {
    let shown: Vec<&Node> = children.iter().filter(|c| !hidden(c)).collect();
    let text = |n: &Node| matches!(n, Node::Element { tag, .. } if tag == "text");
    match shown.as_slice() {
        [title] => text(title),
        [icon, title] => symbol(icon).is_some_and(|s| accessory(s).is_none()) && text(title),
        _ => false,
    }
}

/// @ref LLP 1116.019 D6 — a forward chevron anywhere in a row is the
/// platform's disclosure ink, as a hand-built row draws its accessory
/// (UIKit's is `tertiaryLabel`: measured `#c5c5c7` on a white cell, iOS 27),
/// and a `link` that ends in one is a row that pushes, in the label's
/// colour rather than a link's. Sheet rows, so a colour the author wrote on
/// the chevron or the link wins.
pub(super) fn disclosure(node: &Node) -> Node {
    let chevron = |n: &Node| symbol(n).is_some_and(|s| accessory(s) == Some("disclosure"));
    let result = over(node, &mut |el| {
        let Node::Element {
            tag,
            attrs,
            children,
            span,
            ..
        } = el
        else {
            return Ok(el.clone());
        };
        let tinted = attrs
            .iter()
            .any(|a| a.name == format!("{MARK}-exact-tint-color"));
        let sheet = if chevron(el) && !tinted {
            vec![s("-exact-tint-color", TERTIARY, *span)]
        } else if tag == "link" && children.iter().rfind(|c| !hidden(c)).is_some_and(chevron) {
            vec![s("color", LABEL, *span)]
        } else {
            Vec::new()
        };
        let mut out = with(el, sheet);
        if let Node::Element { children, .. } = &mut out {
            *children = children.iter().map(disclosure).collect();
        }
        Ok(out)
    });
    result.unwrap_or_else(|_| node.clone())
}
