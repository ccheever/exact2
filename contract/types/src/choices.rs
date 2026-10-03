//! Closed choices of strings (LLP 1035.005.000 D4a). `"a" | "b"` is a type
//! whose values are strings: a choice stands wherever a string is read, and a
//! string literal stands where a choice is wanted when it is one of the
//! choice's literals. A string that is not a literal never becomes a choice;
//! a value from the data seam is checked against the literals as it lands.
//! `match` over a choice names every literal in its arms, and a literal it
//! or a comparison names that is not the choice's is refused.

use super::{arms, err, infer, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

/// `"a" | "b"`, as a message spells a choice.
pub(crate) fn spell(literals: &[String]) -> String {
    literals
        .iter()
        .map(|l| format!("\"{l}\""))
        .collect::<Vec<_>>()
        .join(" | ")
}

impl Ty {
    /// A choice of these literals: sorted and distinct.
    pub fn choice(literals: &[String]) -> Ty {
        let mut literals = literals.to_vec();
        literals.sort();
        literals.dedup();
        Ty::Choice(literals)
    }

    /// Whether a value of this type is a string: `string` or a choice.
    pub fn is_text(&self) -> bool {
        matches!(self, Ty::String | Ty::Choice(_))
    }

    /// Whether this type holds a choice anywhere a value of it is read.
    pub fn has_choice(&self) -> bool {
        match self {
            Ty::Choice(_) => true,
            Ty::Option(t) | Ty::List(t) => t.has_choice(),
            _ => false,
        }
    }

    /// The type two branches agree on: [`Ty::unify`], except that two
    /// different strings' types, a choice beside a string or another
    /// choice, agree on `string`.
    pub fn join(&self, other: &Ty) -> Option<Ty> {
        match (self, other) {
            (Ty::Option(a), Ty::Option(b)) => a.join(b).map(|t| Ty::Option(Box::new(t))),
            (Ty::List(a), Ty::List(b)) => a.join(b).map(|t| Ty::List(Box::new(t))),
            (a, b) if a.is_text() && b.is_text() && a != b => Some(Ty::String),
            _ => self.unify(other),
        }
    }

    /// Whether a value of type `given` may stand where this type is wanted:
    /// as [`Ty::unify`] agrees, and besides a choice where a string is
    /// wanted, and a choice of some of a wanted choice's literals.
    pub fn accepts(&self, given: &Ty) -> bool {
        match (self, given) {
            (Ty::String, Ty::Choice(_)) => true,
            (Ty::Choice(want), Ty::Choice(got)) => got.iter().all(|l| want.contains(l)),
            (Ty::Option(a), Ty::Option(b)) | (Ty::List(a), Ty::List(b)) => a.accepts(b),
            _ => crate::checks::can_unify(self, given),
        }
    }
}

/// `e`'s type where `want` is wanted. A string literal where a choice is
/// wanted is that choice when it is one of its literals, and refused by
/// name when it is not; so through `some(…)`, both arms of a `?:` and both
/// arms of an option's `match`. Anything else is inferred as it stands, for
/// the caller to hold to `want` with [`Ty::accepts`]; where an arm is not
/// accepted, that arm's type is the answer, so the caller's refusal names it.
pub(crate) fn given(want: &Ty, e: &Expr, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    if !want.has_choice() {
        return infer(e, scope, shapes);
    }
    let both = |a: Ty, b: Ty| match (want.accepts(&a), want.accepts(&b)) {
        (true, true) => want.clone(),
        (false, _) => a,
        (true, false) => b,
    };
    match (want, e) {
        (Ty::Choice(literals), Expr::Str(s, span)) => {
            if literals.contains(s) {
                Ok(want.clone())
            } else {
                Err(unknown(s, literals, *span))
            }
        }
        (Ty::Option(inner), Expr::Some(x, _)) => {
            Ok(Ty::Option(Box::new(given(inner, x, scope, shapes)?)))
        }
        (_, Expr::Ternary(_, x, y, _)) => {
            // The condition, and each arm as it types on its own.
            arms(e, scope, shapes, infer)?;
            Ok(both(
                given(want, x, scope, shapes)?,
                given(want, y, scope, shapes)?,
            ))
        }
        (
            _,
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            },
        ) => {
            arms(e, scope, shapes, infer)?;
            let Ty::Option(item) = infer(subject, scope, shapes)? else {
                return infer(e, scope, shapes);
            };
            let mut inner = scope.clone();
            inner.push(vec![(var.clone(), Ref::Local(0), *item)]);
            Ok(both(
                given(want, some, &inner, shapes)?,
                given(want, none, scope, shapes)?,
            ))
        }
        _ => infer(e, scope, shapes),
    }
}

/// A provided value's type, before the injects it fills are known: a
/// string literal, or a `?:` of them, is the choice of those literals, which
/// a `string` inject and any choice holding them accept.
pub(crate) fn provided(e: &Expr, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    fn literals(e: &Expr, out: &mut Vec<String>) -> bool {
        match e {
            Expr::Str(s, _) => {
                out.push(s.clone());
                true
            }
            Expr::Ternary(_, a, b, _) => literals(a, out) && literals(b, out),
            _ => false,
        }
    }
    let t = infer(e, scope, shapes)?;
    let mut found = Vec::new();
    Ok(if t == Ty::String && literals(e, &mut found) {
        Ty::choice(&found)
    } else {
        t
    })
}

/// What a refusal of `given` where `want` is wanted adds when the cause is a
/// string where a choice is wanted: how a value becomes one.
pub(crate) fn hint(want: &Ty, given: &Ty) -> &'static str {
    fn text_for_choice(want: &Ty, given: &Ty) -> bool {
        match (want, given) {
            (Ty::Choice(_), t) => t.is_text(),
            (Ty::Option(a), Ty::Option(b)) | (Ty::List(a), Ty::List(b)) => text_for_choice(a, b),
            _ => false,
        }
    }
    if text_for_choice(want, given) {
        ": a string is one of a choice only as a literal written where the choice is wanted, or read from a field, prop or parameter declared as it"
    } else {
        ""
    }
}

/// `"x"` named where `literals` are the choice: refused, with the one it
/// most plausibly misspells.
pub(crate) fn unknown(s: &str, literals: &[String], span: Span) -> TypeError {
    let hint = contract_syntax::suggestion(s, literals.iter().map(String::as_str))
        .map(|guess| format!("; did you mean `\"{guess}\"`?"))
        .unwrap_or_else(|| {
            "; add it to the choice where the type is declared, or name one of these".into()
        });
    TypeError {
        id: "type-choice-unknown",
        message: format!("`\"{s}\"` is not one of `{}`{hint}", spell(literals)),
        span,
    }
}

/// A comparison of a value of type `t` with `other`: a string literal
/// compared with a choice is one of its literals.
pub(crate) fn compared(t: &Ty, other: &Expr) -> Result<(), TypeError> {
    match (t, other) {
        (Ty::Choice(literals), Expr::Str(s, span)) if !literals.contains(s) => {
            Err(unknown(s, literals, *span))
        }
        _ => Ok(()),
    }
}

/// One arm's test in a `match` over a choice: the subject is a choice, the
/// arm's literals are its literals, and the first test, carrying every
/// arm's, names each of its literals.
pub(crate) fn case(
    subject: &Expr,
    all: Option<&[(String, Span)]>,
    checked: bool,
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    // A component's or a `fn`'s test, with what its use passed substituted
    // in: checked in its own declaration against the declared choice, and
    // the use's value against that choice where it was passed.
    if checked {
        return Ok(Ty::Bool);
    }
    let t = infer(subject, scope, shapes)?;
    let choice = match &t {
        Ty::Choice(choice) => choice,
        Ty::Unknown => return Ok(Ty::Bool),
        _ => {
            return err(
                "type-match-subject",
                format!(
                    "`match` with `case \"…\"` arms needs a choice of strings, given `{t}`: declare the field, prop or parameter as one, `kind: \"a\" | \"b\"`"
                ),
                subject.span(),
            )
        }
    };
    // The first test names every arm's literals; the others are among them.
    let Some(all) = all else {
        return Ok(Ty::Bool);
    };
    for (l, at) in all {
        if !choice.contains(l) {
            return Err(unknown(l, choice, *at));
        }
    }
    let missing: Vec<String> = choice
        .iter()
        .filter(|l| !all.iter().any(|(a, _)| a == *l))
        .cloned()
        .collect();
    if !missing.is_empty() {
        let cases = missing
            .iter()
            .map(|l| format!("`case \"{l}\"`"))
            .collect::<Vec<_>>()
            .join(", ");
        return err(
            "type-match-missing",
            format!(
                "this `match` on `{t}` has no arm for `{}`: add {cases}, or name it in another arm's `case … | \"{}\"`",
                spell(&missing),
                missing[0]
            ),
            span,
        );
    }
    Ok(Ty::Bool)
}
