//! `lower-state-dead` (LLP 1121 D8, stage 8 A; theme2's ruling (a)): a
//! node's own `:disabled` part where it writes no `disabled`, or its own
//! `:checked` where it writes none of `checked`, `aria-checked`,
//! `aria-selected` and `aria-pressed`: the state never holds, so its rows
//! never show. A bound expression counts as written. A class's part is not
//! judged: a shared style's `:disabled` on a node that is never disabled is
//! the ordinary case.

use crate::LowerError;
use contract_syntax::state::state_of;
use contract_syntax::Node;

/// What each runner-known state reads.
const READS: [(&str, &[&str]); 2] = [
    ("disabled", &["disabled"]),
    (
        "checked",
        &["checked", "aria-checked", "aria-selected", "aria-pressed"],
    ),
];

/// Every node's own runner-known state part that its node never meets.
pub(super) fn dead(view: &[Node], out: &mut Vec<LowerError>) {
    let mut stack: Vec<&Node> = view.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                attrs, children, ..
            } => {
                for (state, reads) in READS {
                    let part = attrs
                        .iter()
                        .find(|a| state_of(&a.name).is_some_and(|(s, _)| s == state));
                    if let Some(part) =
                        part.filter(|_| !attrs.iter().any(|a| reads.contains(&a.name.as_str())))
                    {
                        out.push(LowerError {
                            id: "lower-state-dead",
                            message: format!(
                                "`:{state}` never holds here: the node writes no {}, so its rows never show (LLP 1121 D8)",
                                reads.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(" or ")
                            ),
                            span: part.span,
                        });
                    }
                }
                stack.extend(children);
            }
            Node::When {
                then, otherwise, ..
            } => stack.extend(then.iter().chain(otherwise)),
            Node::Each { body, .. } => stack.extend(body),
            Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none)),
            Node::Use { children, .. } => stack.extend(children),
            Node::Children { .. } => {}
        }
    }
}
