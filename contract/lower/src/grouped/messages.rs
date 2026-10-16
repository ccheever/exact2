//! The grouped list's refusals that name an edit (LLP 1116.014 D1, LLP 1116.018).

use super::{is, Stray};
use contract_syntax::Node;

/// LLP 1116.014 D1: the edit that puts a `header` or `footer` where UIKit
/// draws one. Its words may change (`text (on ? "A" : "B")`); what comes and
/// goes is a section, since a `header` holds one `text` and nothing else.
pub(super) fn stray_message(stray: &Stray) -> String {
    let (tag, place) = (
        &stray.tag,
        if stray.tag == "header" {
            "first"
        } else {
            "last"
        },
    );
    match stray.flow {
        None => format!(
            "move `{tag}` to the section's {place} line: a section's `header` comes first and its `footer` last"
        ),
        Some("for") => format!(
            "move `{tag}` to the section's {place} line, outside `for`: a section has one `{tag}`"
        ),
        Some(flow) => format!(
            "move `{tag}` to the section's {place} line, outside `{flow}`: a {tag} that changes is `text (cond ? \"A\" : \"B\")` in it, and one that comes and goes means two `section`s under `{flow}`"
        ),
    }
}

/// How many texts an `if` / `else if` / `else` chain chooses between, when
/// every arm is one `text` and the chain has an `else`.
fn texts_chosen(node: &Node) -> Option<usize> {
    let Node::When {
        then, otherwise, ..
    } = node
    else {
        return None;
    };
    let arm = |nodes: &[Node]| match nodes {
        [only] if is(only, "text") => Some(1),
        [inner @ Node::When { .. }] => texts_chosen(inner),
        _ => None,
    };
    Some(arm(then)? + arm(otherwise)?)
}

/// LLP 1116.018: the edit for a `header` or `footer` that holds more than one
/// `text`. A repair told only the rule moved or dropped the status a chain of
/// texts showed; this names the edit that keeps every arm.
pub(super) fn label(tag: &str, children: &[Node]) -> String {
    match children {
        [chain] => match texts_chosen(chain) {
            Some(n) => format!(
                "write this `{tag}`'s {n} texts as one `text` whose words the same conditions choose, in order: `text (first ? \"A\" : second ? \"B\" : \"C\")` for `if first` / `else if second` / `else`. UIKit draws a section's `{tag}` as one string"
            ),
            None if matches!(chain, Node::When { otherwise, .. } if otherwise.is_empty()) => format!(
                "a `{tag}` shown only while a condition holds is two `section`s under that `if`, the same rows in each and the `{tag}` in one: UIKit draws a section's `{tag}` as one string, and a section with one is spaced differently"
            ),
            None => format!("a section's `{tag}` holds one `text`, the words UIKit draws there"),
        },
        _ => format!(
            "a section's `{tag}` holds one `text`, the words UIKit draws there: join its texts into one, or move the others into the section's rows"
        ),
    }
}
