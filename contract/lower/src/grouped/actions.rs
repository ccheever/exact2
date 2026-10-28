//! A grouped row's actions and disclosures (LLP 1116.019 D6): what the
//! platform's list draws in the tint, and in its disclosure's grey.

use super::{accessory, hidden, over, s, symbol, with, LABEL, MARK, TERTIARY};
use contract_syntax::Node;

/// Whether a row's shown parts are an action cell's: one text, a symbol
/// that is no accessory before it at most. A symbol an `if` shows (every
/// branch a symbol or nothing) is one before it at most too.
/// The sheet's word on whether a grouped `button` row is an action cell,
/// which the lowering sets as `groupedAction`, no author can write, and the
/// kernel takes over the shape it sees (`GroupedRow::action`): the sheet
/// sees every state of the source, a choice row's mark and an `if`'s two
/// branches, where the kernel sees the one that shows.
pub(crate) const ACTION_MARK: &str = "ua:grouped-action";

/// The mark for a `button` row (`button`), saying `acting`.
pub(super) fn mark(
    button: bool,
    acting: bool,
    span: contract_syntax::Span,
) -> Option<contract_syntax::Attr> {
    button.then(|| contract_syntax::Attr {
        name: ACTION_MARK.into(),
        value: contract_syntax::Expr::Bool(acting, span),
        span,
        written: crate::IMPLIED,
    })
}

/// The sheet's word on a row's attributes, if it gave one.
pub(crate) fn said(attrs: &[contract_syntax::Attr]) -> Option<bool> {
    attrs
        .iter()
        .find(|a| a.name == ACTION_MARK)
        .and_then(|a| match a.value {
            contract_syntax::Expr::Bool(b, _) => Some(b),
            _ => None,
        })
}

pub(super) fn action_cell(children: &[Node]) -> bool {
    let shown: Vec<&Node> = children.iter().filter(|c| !hidden(c)).collect();
    let text = |n: &Node| matches!(n, Node::Element { tag, .. } if tag == "text");
    match shown.as_slice() {
        [title] => text(title),
        [icon, title] => leading(icon) && text(title),
        _ => false,
    }
}

/// A symbol that is no accessory, or an `if` whose every branch shows one
/// such at most.
fn leading(node: &Node) -> bool {
    match node {
        Node::When {
            then, otherwise, ..
        } => [then, otherwise].iter().all(|branch| {
            let shown: Vec<&Node> = branch.iter().filter(|c| !hidden(c)).collect();
            match shown.as_slice() {
                [] => true,
                [one] => leading(one),
                _ => false,
            }
        }),
        _ => symbol(node).is_some_and(|s| accessory(s).is_none()),
    }
}

/// @ref LLP 1116.019 D6 — a forward chevron anywhere in a row is the
/// platform's disclosure ink, as a hand-built row draws its accessory
/// (UIKit's is `tertiaryLabel`: measured `#c5c5c7` on a white cell, iOS 27),
/// and a `link` that ends in one is a row that pushes, in the label's
/// colour rather than a link's. The chevron's is its `color`, the ink a
/// symbol is drawn in (LLP 1081 stage 2), so a colour its author wrote on
/// it wins, and one on the link does not reach it, as UIKit's disclosure
/// stays grey in a row of any colour. Sheet rows, so the author's win.
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
        let inked = attrs.iter().any(|a| a.name == format!("{MARK}color"));
        let sheet = if chevron(el) && !inked {
            vec![s("color", TERTIARY, *span)]
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
