//! Records built in expressions (LLP 1035.005.000 D3): `Shape(field=expr,
//! …)` names every field of a declared shape once, none defaulted, and
//! `Shape(base, field=expr, …)` copies `base` with the named fields replaced.
//! Lowering reads the same arguments (`contract-lower`'s `record`).

use super::{checks, err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

/// Whether `name(…)` builds a record: a shape the app declares, never a
/// compiler shape (`Router`, `Geometry`) or a `fn`.
pub fn is_record_call(name: &str, shapes: &Shapes) -> bool {
    shapes.declared.contains(name) && !shapes.fns.contains_key(name)
}

/// The base a copy starts from (the one positional argument), if any.
pub fn base(args: &[Expr]) -> Option<&Expr> {
    args.first().filter(|a| !matches!(a, Expr::NamedArg(..)))
}

/// Type `shape(args)`: the shape, once every field is named exactly once
/// (or copied from a base) with a value of its type.
pub(crate) fn infer_record(
    shape: &str,
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    let fields = &shapes.map[shape];
    let list = || {
        fields
            .iter()
            .map(|(f, _)| f.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let base = base(args);
    if let Some(stray) = args
        .iter()
        .skip(usize::from(base.is_some()))
        .find(|a| !matches!(a, Expr::NamedArg(..)))
    {
        return err(
            "type-record-base",
            format!(
                "`{shape}(…)` takes one positional argument, first: the `{shape}` it copies; every other argument is `field=value`"
            ),
            stray.span(),
        );
    }
    if let Some(b) = base {
        let t = infer(b, scope, shapes)?;
        let want = Ty::Record(shape.to_owned());
        if !checks::can_unify(&want, &t) {
            return err(
                "type-record-base",
                format!("`{shape}(base, …)` copies a `{shape}`, given `{t}`"),
                b.span(),
            );
        }
    }
    let mut given: Vec<&str> = Vec::new();
    for arg in args {
        let Expr::NamedArg(name, value, at) = arg else {
            continue;
        };
        let Some((_, want)) = shapes.field(shape, name) else {
            return err(
                "type-record-unknown-field",
                format!("`{shape}` has no field `{name}`; its fields are {}", list()),
                *at,
            );
        };
        if given.contains(&name.as_str()) {
            return err(
                "type-record-duplicate",
                format!("`{name}` is given twice in `{shape}(…)`"),
                *at,
            );
        }
        given.push(name);
        let t = crate::choices::given(want, value, scope, shapes)?;
        if !want.accepts(&t) {
            return err(
                "type-argument",
                format!(
                    "field `{name}` of `{shape}` is `{want}`, given `{t}`{}",
                    crate::choices::hint(want, &t)
                ),
                value.span(),
            );
        }
    }
    if base.is_none() {
        let missing: Vec<&str> = fields
            .iter()
            .map(|(f, _)| f.as_str())
            .filter(|f| !given.contains(f))
            .collect();
        if !missing.is_empty() {
            return err(
                "type-record-missing",
                format!(
                    "`{shape}(…)` names every field, and {} {} missing: a field is never defaulted; `{shape}(base, field=value)` copies the rest from `base`",
                    missing
                        .iter()
                        .map(|f| format!("`{f}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    if missing.len() == 1 { "is" } else { "are" }
                ),
                span,
            );
        }
    }
    Ok(Ty::Record(shape.to_owned()))
}
