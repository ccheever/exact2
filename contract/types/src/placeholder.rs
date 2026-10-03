//! A resource's placeholder when nothing is kept and its source has not
//! answered (LLP 1054.000.002): its type's zero, or `else empty(field=value,
//! …)`, the zero with named fields replaced by constants. One pure function
//! checks it and builds it; the type check reports its refusals and lowering
//! stores the value it builds.

use crate::{Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span, TemplatePart, UnOp};
use exact_plan::Value;

/// The source name that means "the zero, with these fields", in `else`.
pub const EMPTY: &str = "empty";

/// `ty`'s zero: `0`, `""`, `false`, `()`, `none`, `[]`, and a record's
/// fields' zeros in declaration order. `None` for a type with no value
/// (an action, or one not known).
pub fn zero(ty: &Ty, shapes: &Shapes) -> Option<Value> {
    Some(match ty {
        Ty::Number => Value::Number(0.0),
        Ty::String => Value::str(""),
        // A choice's zero is its first literal, as sorted (LLP 1035.005.000
        // D4a): `""` is not one of them.
        Ty::Choice(literals) => Value::str(literals.first()?),
        Ty::Bool => Value::Bool(false),
        Ty::Unit => Value::Unit,
        Ty::Option(_) => Value::Option(None),
        Ty::List(_) => Value::list(Vec::new()),
        Ty::Record(name) => Value::record(
            shapes
                .map
                .get(name)?
                .iter()
                .map(|(_, t)| zero(t, shapes))
                .collect::<Option<Vec<_>>>()?,
        ),
        Ty::Action(_) | Ty::Unknown => return None,
    })
}

/// `empty(args…)` as a value of `ty`, or every refusal it earns. `path`
/// names the resource (and the fields above), for the messages.
pub fn materialize(
    ty: &Ty,
    args: &[Expr],
    shapes: &Shapes,
    path: &str,
    span: Span,
) -> Result<Value, Vec<TypeError>> {
    let mut errors = Vec::new();
    let value = build(ty, args, shapes, path, span, &mut errors);
    match value {
        Some(v) if errors.is_empty() => Ok(v),
        _ => Err(errors),
    }
}

fn error(errors: &mut Vec<TypeError>, id: &'static str, message: String, span: Span) {
    errors.push(TypeError { id, message, span });
}

fn build(
    ty: &Ty,
    args: &[Expr],
    shapes: &Shapes,
    path: &str,
    span: Span,
    errors: &mut Vec<TypeError>,
) -> Option<Value> {
    let Some(mut value) = zero(ty, shapes) else {
        error(
            errors,
            "type-placeholder-type",
            format!("`{path}` has no empty value"),
            span,
        );
        return None;
    };
    if args.is_empty() {
        return Some(value);
    }
    let Ty::Record(shape) = ty else {
        error(
            errors,
            "type-placeholder-fields",
            format!("`{path}` is `{ty}`, not a record: `empty()` takes fields only for a shape"),
            span,
        );
        return None;
    };
    let fields = shapes.map.get(shape)?;
    let Value::Record(slots) = &mut value else {
        return None;
    };
    let mut owned = slots.to_vec();
    let mut seen: Vec<&str> = Vec::new();
    for arg in args {
        let Expr::NamedArg(name, inner, at) = arg else {
            error(
                errors,
                "type-placeholder-fields",
                format!("`{path}`: `empty` takes `field=value` pairs; `{shape}` got a positional argument"),
                arg.span(),
            );
            continue;
        };
        let Some(index) = fields.iter().position(|(f, _)| f == name) else {
            error(
                errors,
                "type-placeholder-field",
                format!("`{path}`'s placeholder: `{shape}` has no field `{name}`"),
                *at,
            );
            continue;
        };
        if seen.contains(&name.as_str()) {
            error(
                errors,
                "type-placeholder-duplicate",
                format!("`{path}.{name}` is given twice"),
                *at,
            );
            continue;
        }
        seen.push(name);
        let field_path = format!("{path}.{name}");
        if let Some(v) = constant(&fields[index].1, inner, shapes, &field_path, errors) {
            owned[index] = v;
        }
    }
    *slots = owned.into();
    Some(value)
}

/// A constant of type `ty`: a literal, `-` a number, `none`, `some(v)`,
/// `[]`, or a nested `empty(…)` for a record.
fn constant(
    ty: &Ty,
    e: &Expr,
    shapes: &Shapes,
    path: &str,
    errors: &mut Vec<TypeError>,
) -> Option<Value> {
    let mismatch = |errors: &mut Vec<TypeError>, given: &str| {
        error(
            errors,
            "type-placeholder-type",
            format!("`{path}` is `{ty}`, given {given}"),
            e.span(),
        );
        None
    };
    match (ty, e) {
        (Ty::Number, Expr::Number(n, _)) => Some(Value::Number(*n)),
        (Ty::Number, Expr::Unary(UnOp::Neg, inner, _)) if matches!(**inner, Expr::Number(..)) => {
            let Expr::Number(n, _) = **inner else {
                unreachable!()
            };
            Some(Value::Number(-n))
        }
        (Ty::String, Expr::Str(s, _)) => Some(Value::str(s)),
        (Ty::Choice(literals), Expr::Str(s, _)) if literals.contains(s) => Some(Value::str(s)),
        (Ty::String, Expr::Template(parts, _))
            if parts.iter().all(|p| matches!(p, TemplatePart::Text(_))) =>
        {
            let text: String = parts
                .iter()
                .map(|p| match p {
                    TemplatePart::Text(t) => t.as_str(),
                    TemplatePart::Expr(_) => "",
                })
                .collect();
            Some(Value::str(&text))
        }
        (Ty::Bool, Expr::Bool(b, _)) => Some(Value::Bool(*b)),
        (Ty::Option(_), Expr::None(_)) => Some(Value::Option(None)),
        (Ty::List(_), Expr::EmptyList(_)) => Some(Value::list(Vec::new())),
        (Ty::Option(inner), Expr::Some(v, _)) => {
            constant(inner, v, shapes, path, errors).map(Value::some)
        }
        (Ty::Record(_), Expr::Call(name, args, at)) if name == EMPTY => {
            build(ty, args, shapes, path, *at, errors)
        }
        (_, Expr::Number(..)) => mismatch(errors, "a number"),
        (_, Expr::Str(..)) => mismatch(errors, "a string"),
        (_, Expr::Bool(..)) => mismatch(errors, "a bool"),
        (_, Expr::None(_) | Expr::Some(..)) => mismatch(errors, "an option"),
        (_, Expr::EmptyList(_)) => mismatch(errors, "a list"),
        (_, Expr::Call(name, _, _)) if name == EMPTY => mismatch(errors, "a record"),
        _ => {
            error(
                errors,
                "type-placeholder-value",
                format!(
                    "`{path}` must be a constant: a literal, `none`, `some(…)`, `[]`, or `empty(…)`; use `else source(…)` for a computed placeholder"
                ),
                e.span(),
            );
            None
        }
    }
}
