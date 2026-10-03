//! Closed choices of strings (LLP 1035.005.000 D4a). `"a" | "b"` is a type
//! whose values are strings: a choice stands wherever a string is read, and a
//! string literal stands where a choice is wanted when it is one of the
//! choice's literals. A string that is not a literal never becomes a choice;
//! a value from the data seam is checked against the literals as it lands.
//! `match` over a choice names every literal in its arms, and a literal it
//! or a comparison names that is not the choice's is refused.

use super::{arms, err, infer, Scope, Shapes, Ty, TypeError};
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
/// name when it is not; a `?:` of such literals, and `some` of one, the
/// same. Anything else is inferred as it stands, for the caller to hold to
/// `want` with [`Ty::accepts`].
pub(crate) fn given(want: &Ty, e: &Expr, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    match (want, e) {
        (Ty::Choice(literals), Expr::Str(s, span)) => {
            if literals.contains(s) {
                Ok(want.clone())
            } else {
                Err(unknown(s, literals, *span))
            }
        }
        (Ty::Choice(_), Expr::Ternary(_, x, y, _)) => {
            let (a, b) = arms(e, scope, shapes, infer)?;
            let a = if a.is_text() {
                given(want, x, scope, shapes)?
            } else {
                a
            };
            let b = if b.is_text() {
                given(want, y, scope, shapes)?
            } else {
                b
            };
            Ok(if want.accepts(&a) && want.accepts(&b) {
                want.clone()
            } else {
                a.join(&b).unwrap_or(a)
            })
        }
        (Ty::Option(inner), Expr::Some(x, _)) if inner.has_choice() => {
            Ok(Ty::Option(Box::new(given(inner, x, scope, shapes)?)))
        }
        _ => infer(e, scope, shapes),
    }
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
fn unknown(s: &str, literals: &[String], span: Span) -> TypeError {
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

/// Whether `e` is a string literal whatever it evaluates to: what a use
/// passes to a choice prop, substituted into the component's `match`. The
/// use was held to the prop's choice, and the component's own check held
/// its `match` to it.
fn literal(e: &Expr) -> bool {
    match e {
        Expr::Str(..) => true,
        Expr::Ternary(_, a, b, _) => literal(a) && literal(b),
        Expr::Match { some, none, .. } => literal(some) && literal(none),
        _ => false,
    }
}

/// One arm's test in a `match` over a choice: the subject is a choice, the
/// arm's literals are its literals, and the first test, carrying every
/// arm's, names each of its literals.
pub(crate) fn case(
    subject: &Expr,
    literals: &[String],
    all: Option<&[String]>,
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    let t = infer(subject, scope, shapes)?;
    let choice = match &t {
        Ty::Choice(choice) => choice,
        Ty::Unknown => return Ok(Ty::Bool),
        Ty::String if literal(subject) => return Ok(Ty::Bool),
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
    for l in literals.iter().chain(all.into_iter().flatten()) {
        if !choice.contains(l) {
            return Err(unknown(l, choice, span));
        }
    }
    if let Some(all) = all {
        let missing: Vec<String> = choice
            .iter()
            .filter(|l| !all.contains(l))
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
    }
    Ok(Ty::Bool)
}
