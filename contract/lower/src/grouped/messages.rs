//! The grouped list's refusals that name an edit (LLP 1116.014 D1, LLP 1116.018).

use super::{is, Stray};
use contract_syntax::{Attr, Expr, Node};

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

/// LLP 1116.017 D3: a `header` or `footer` whose one child is an `if` /
/// `else if` / `else` chain, every arm one `text` with the same literal
/// attributes, is that one `text` with its words chosen by the same
/// conditions in order. `None` leaves the label to its refusal.
pub(super) fn fold_label(label: &Node) -> Option<Node> {
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        span,
        instance,
    } = label
    else {
        return None;
    };
    let [chain] = children.as_slice() else {
        return None;
    };
    let (words, text_attrs, text_span) = fold(chain)?;
    Some(Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: attrs.clone(),
        children: vec![Node::Element {
            tag: "text".into(),
            positional: vec![words],
            attrs: text_attrs,
            children: Vec::new(),
            span: text_span,
            instance: *instance,
        }],
        span: *span,
        instance: *instance,
    })
}

/// The chain's words as one expression, with the attributes every arm shares.
fn fold(node: &Node) -> Option<(Expr, Vec<Attr>, contract_syntax::Span)> {
    let arm = |nodes: &[Node]| -> Option<(Expr, Vec<Attr>, contract_syntax::Span)> {
        match nodes {
            [Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                ..
            }] if tag == "text" && positional.len() == 1 && children.is_empty() => {
                Some((positional[0].clone(), attrs.clone(), *span))
            }
            [inner @ Node::When { .. }] => fold(inner),
            _ => None,
        }
    };
    let Node::When {
        cond,
        then,
        otherwise,
        span,
        ..
    } = node
    else {
        return None;
    };
    let (yes, yes_attrs, at) = arm(then)?;
    let (no, no_attrs, _) = arm(otherwise)?;
    if !same_literals(&yes_attrs, &no_attrs) {
        return None;
    }
    let words = Expr::Ternary(Box::new(cond.clone()), Box::new(yes), Box::new(no), *span);
    Some((words, yes_attrs, at))
}

/// Whether two arms' attributes are the same names with the same literal
/// values, so the one text can carry them.
fn same_literals(a: &[Attr], b: &[Attr]) -> bool {
    let literal = |e: &Expr| match e {
        Expr::Str(s, _) => Some(format!("s{s}")),
        Expr::Number(n, _) => Some(format!("n{n}")),
        Expr::Bool(v, _) => Some(format!("b{v}")),
        _ => None,
    };
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.name == y.name
                && literal(&x.value).is_some()
                && literal(&x.value) == literal(&y.value)
        })
}
