//! Records built in expressions (LLP 1035.005.000 D3): `Shape(field=expr,
//! …)` names every field of a declared shape once, none defaulted, and
//! `Shape(base, field=expr, …)` copies `base` with the named fields replaced.
//! Lowering reads the same arguments (`contract-lower`'s `record`).

use super::{checks, err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

/// The compiler shapes an app builds as it builds its own: the Media
/// Session's `MediaMetadata`, by the API's constructor (LLP 1098 D1), and a
/// textarea's `TextDecorations` with its spans (LLP 1114.007), and a
/// Markdown node's `AtomFace` (LLP 1045.000 D2).
pub const COMPILER_RECORDS: &[&str] = &[
    "MediaMetadata",
    "TextDecorations",
    "TextDecorationSpan",
    "AtomFace",
];

/// Whether `name(…)` builds a record: a shape the app declares or one of
/// [`COMPILER_RECORDS`], never another compiler shape (`Router`,
/// `Geometry`) or a `fn`.
pub fn is_record_call(name: &str, shapes: &Shapes) -> bool {
    (shapes.declared.contains(name) || COMPILER_RECORDS.contains(&name))
        && !shapes.fns.contains_key(name)
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
        let t = infer(value, scope, shapes)?;
        if !checks::accepts_expr(want, value, &t, scope, shapes) {
            return err(
                "type-argument",
                format!("field `{name}` of `{shape}` is `{want}`, given `{t}`"),
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

/// A refusal that names two shapes with the same fields, told that a shape
/// is its name: the euxy port declared `Port` in one file and `PortRow` in
/// another and met only `expects PortRow[], given Port[]`. The edit keeps
/// the shape a `use` brings, which is the one already shared.
pub(crate) fn nominal(e: &mut TypeError, shapes: &Shapes, file: &contract_syntax::File) {
    let mut named: Vec<&str> = Vec::new();
    // The odd pieces are inside backticks.
    for quoted in e.message.split('`').skip(1).step_by(2) {
        let words = quoted.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'));
        for w in words.filter(|w| shapes.declared.contains(*w)) {
            if !named.contains(&w) {
                named.push(w);
            }
        }
    }
    let fields = |name: &str| {
        let mut f = shapes.map.get(name).cloned().unwrap_or_default();
        f.sort_by(|a, b| a.0.cmp(&b.0));
        f
    };
    let pair = named.iter().enumerate().find_map(|(i, a)| {
        named[i + 1..]
            .iter()
            .find(|b| fields(a) == fields(b))
            .map(|b| (*a, *b))
    });
    let Some((a, b)) = pair else {
        return;
    };
    // Whether a shape is declared in the refused line's file; the loader
    // has merged the used files' declarations by now, each with its span.
    let here = |name: &str| {
        file.shapes
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.span.source_id == e.span.source_id)
    };
    let edit = match (here(a), here(b)) {
        (Some(false), Some(true)) => format!("write `{a}` for `{b}`, naming `{a}` in the `use` of the file that declares it, and delete `shape {b}`"),
        (Some(true), Some(false)) => format!("write `{b}` for `{a}`, naming `{b}` in the `use` of the file that declares it, and delete `shape {a}`"),
        _ => "declare it once, in a file both use, and `use` it in the other".to_string(),
    };
    e.message.push_str(&format!(
        "; `{a}` and `{b}` have the same fields, but a shape matches only itself, by name: {edit}"
    ));
}
