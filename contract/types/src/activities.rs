//! @ref LLP 1126.001 WP9 — reserved mutation sends, available in every plan.
use crate::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

pub(crate) fn send(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    result: &Ty,
    span: Span,
) -> Result<(), TypeError> {
    let valid_result = match result {
        Ty::Record(name) => shapes
            .map
            .get(name)
            .is_some_and(|fields| fields.as_slice() == [("id".into(), Ty::String)]),
        _ => false,
    };
    if !valid_result {
        return err(
            "type-activity-result",
            format!("{name} answers a record with only id: string"),
            span,
        );
    }
    let usage = match name {
        "startActivity" => "startActivity(kind, state)",
        "updateActivity" => "updateActivity(id, state, alert, staleDate): alert is option<string>, staleDate is option<number> (Unix milliseconds)",
        _ => "endActivity(id, finalState, dismissalPolicy): immediate, default, or Unix milliseconds",
    };
    let count = match name {
        "startActivity" => args.len() == 2,
        "updateActivity" => (2..=4).contains(&args.len()),
        _ => args.len() == 3,
    };
    if !count {
        return err("type-activity-send", usage, span);
    }
    if !infer(&args[0], scope, shapes)?.is_string()
        || !matches!(infer(&args[1], scope, shapes)?, Ty::Record(_))
    {
        return err(
            "type-activity-send",
            format!("{usage}: a string kind or id, then a state record"),
            span,
        );
    }
    for (i, arg) in args.iter().enumerate().skip(2) {
        let ty = infer(arg, scope, shapes)?;
        let ok = if name == "endActivity" {
            ty == Ty::Number
                || crate::enums::accepts_expr(
                    &Ty::Enum(vec!["immediate".into(), "default".into()]),
                    arg,
                    &ty,
                    scope,
                    shapes,
                )
        } else if i == 2 {
            Ty::Option(Box::new(Ty::String)).unify(&ty).is_some()
        } else {
            Ty::Option(Box::new(Ty::Number)).unify(&ty).is_some()
        };
        if !ok {
            return err("type-activity-send", usage, arg.span());
        }
    }
    Ok(())
}

/// The optional update arguments have fixed types, including a bare `none`.
/// Lowering uses the same types for the send's baked argument descriptors.
pub fn option_type(name: &str, index: usize) -> Option<Ty> {
    match (name, index) {
        ("updateActivity", 2) => Some(Ty::Option(Box::new(Ty::String))),
        ("updateActivity", 3) => Some(Ty::Option(Box::new(Ty::Number))),
        _ => None,
    }
}
