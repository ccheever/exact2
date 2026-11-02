//! LLP 1057.001 §8: `swipe`, a discrete swipe in the directions a node
//! names, and its two props. A literal is checked here; a computed value is
//! the runner's to check as it changes.
use crate::{err, values::numeric_literal, LowerError};
use contract_syntax::{Attr, Expr, Span};

/// `swipe-direction`: one or more of left, right, up and down, physical,
/// space-separated; lowercased, each word once (`swipe_directions`).
pub(crate) fn direction(value: &Expr, span: Span) -> Result<Expr, LowerError> {
    let Expr::Str(s, at) = value else {
        return Ok(value.clone());
    };
    match exact_kernel::props::swipe_directions(s) {
        Some(words) => Ok(Expr::Str(words, *at)),
        None => err(
            "lower-swipe-direction",
            format!("`swipe-direction` names where a swipe goes, one or more of left, right, up and down, space-separated (for example \"down\" or \"left right\"), and \"{s}\" does not"),
            span,
        ),
    }
}

/// `touch-count`: a whole number of at least 1.
pub(crate) fn count(value: &Expr, span: Span) -> Result<Expr, LowerError> {
    // The refused literal, as the message names it; a computed count is the
    // runner's to check
    let refused = match value {
        Expr::Str(s, _) => Some(format!("the text \"{s}\"")),
        Expr::Bool(b, _) => Some(b.to_string()),
        _ => numeric_literal(value)
            .filter(|n| !(n.is_finite() && n.fract() == 0.0 && *n >= 1.0))
            .map(|n| n.to_string()),
    };
    let Some(shown) = refused else {
        return Ok(value.clone());
    };
    err(
        "lower-swipe-count",
        format!("`touch-count` is how many touches a swipe takes, a whole number of at least 1, which {shown} is not; leave it out for the platform's own swipe"),
        span,
    )
}

/// A node's `swipe` and its props together: either prop without `swipe`
/// does nothing, `swipe` without `swipe-direction` recognizes nothing, and
/// `swiperight` shares a node only with a swipe its literals prove is no
/// one-finger swipe to the right, which is the same contact.
pub(crate) fn check(attrs: &[Attr]) -> Result<(), LowerError> {
    let find = |name: &str| attrs.iter().find(|a| a.name == name);
    let swipe = find("swipe");
    for prop in ["swipe-direction", "touch-count"] {
        if let (None, Some(a)) = (swipe, find(prop)) {
            return err(
                "lower-swipe-unused",
                format!("`{prop}` describes this node's `swipe`, which it does not have: add `swipe=action`, or remove `{prop}`"),
                a.span,
            );
        }
    }
    let Some(swipe) = swipe else { return Ok(()) };
    let Some(direction) = find("swipe-direction") else {
        return err(
            "lower-swipe-direction",
            "`swipe` needs `swipe-direction`: one or more of left, right, up and down (for example `swipe-direction=\"down\"`)",
            swipe.span,
        );
    };
    let other_count =
        find("touch-count").is_some_and(|a| numeric_literal(&a.value).is_some_and(|n| n != 1.0));
    let not_right = matches!(&direction.value, Expr::Str(s, _) if !s.split_ascii_whitespace().any(|w| w.eq_ignore_ascii_case("right")));
    if let Some(right) = find("swiperight").filter(|_| !other_count && !not_right) {
        return err(
            "lower-swipe-twice",
            "`swiperight` and a `swipe` that may be a one-finger swipe to the right would both take that contact: keep one, or give the `swipe` a literal `swipe-direction` without `right` or a literal `touch-count` other than 1",
            right.span,
        );
    }
    Ok(())
}
