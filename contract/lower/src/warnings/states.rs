//! `lower-state-dead` (LLP 1121 D8, stage 8 A; theme2's ruling (a)): a
//! node's own `:disabled` part where it writes no `disabled`, or its own
//! `:checked` where it writes none of `checked`, `aria-checked`,
//! `aria-selected` and `aria-pressed`, or its own `:active` where it has no
//! `press`, which the host's press needs (theme2's 07:06 question): the
//! state never holds, so its rows never show. A pressable link (`link`,
//! `button`) with an `href` and no `press` is linted too, as unreliable:
//! only Apple presses it (and the web, by touch; theme2's 11:48 ruling, the
//! hosts' disagreement being LLP 1061's). A bound expression counts as written. A class's part is not
//! judged: a shared style's `:disabled` on a node that is never disabled is
//! the ordinary case.

use crate::LowerError;
use contract_syntax::state::state_of;
use contract_syntax::Node;

/// What each state reads.
const READS: [(&str, &[&str]); 3] = [
    ("disabled", &["disabled"]),
    (
        "checked",
        &["checked", "aria-checked", "aria-selected", "aria-pressed"],
    ),
    ("active", &["press"]),
];

/// Every node's own state part that its node never meets.
pub(super) fn dead(view: &[Node], out: &mut Vec<LowerError>) {
    let mut stack: Vec<&Node> = view.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                for (state, reads) in READS {
                    let part = attrs
                        .iter()
                        .find(|a| state_of(&a.name).is_some_and(|(s, _)| s == state));
                    if let Some(part) =
                        part.filter(|_| !attrs.iter().any(|a| reads.contains(&a.name.as_str())))
                    {
                        // A pressable link (`link`, `button`) with an `href` (bound, or a
                        // literal URL), which Apple presses; a `text` link is never pressed.
                        let link = state == "active"
                            && matches!(tag.as_str(), "link" | "button")
                            && attrs.iter().any(|a| {
                                a.name == "href" && !matches!(&a.value, contract_syntax::Expr::Str(u, _) if u.is_empty())
                            });
                        out.push(LowerError {
                            id: "lower-state-dead",
                            message: if link {
                                "`:active` is unreliable here: a link with no `press` is pressed only on Apple (and the web by touch); give it `press`, or move the part (LLP 1121 D8)".to_string()
                            } else {
                                format!(
                                    "`:{state}` never holds here: the node writes no {}, so its rows never show (LLP 1121 D8)",
                                    reads.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(" or ")
                                )
                            },
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
