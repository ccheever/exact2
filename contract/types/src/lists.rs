//! `map`, `filter` and `join` (LLP 1017.003 D4) and `sortBy` (LLP 1116.016
//! §5): the roster's list operations, typed here because a callback is not
//! a value the roster table can describe; and `concat`, `slice`, `includes` and `indexOf`
//! (LLP 1088 §9.1), whose types follow their first argument's: a list's
//! item type, or text for all but `concat`, the web's same-named methods;
//! and `range` (LLP 1017.003 §Refused, 2026-10-17), whose literal count is
//! checked where it is written, and `flat`, a list of lists' items; and
//! `x.length`, which reads a record's field of that name.

use crate::{checks, err, infer, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span, UnOp};
use exact_plan::Stdlib;
use std::collections::BTreeSet;

/// How each `x.length` resolved, by its span (@ref LLP 1116.015 D2, as
/// amended 2026-10-17): the parser writes `length(x)` for it, and the checker
/// reads a record's `length` field instead when the receiver is a record whose
/// shape has one. What writes the expression out again without types (the
/// Lean embedding, `contract lean`) asks [`Shapes::reads_length_field`];
/// lowering decides by the type it compiles.
#[derive(Debug, Default)]
pub(crate) struct LengthReads {
    fields: BTreeSet<Span>,
    counts: BTreeSet<Span>,
}

impl Shapes {
    /// Whether the `x.length` written at `span` reads a record's field.
    pub fn reads_length_field(&self, span: Span) -> bool {
        self.lengths
            .lock()
            .is_ok_and(|reads| reads.fields.contains(&span))
    }
}

/// `x.length`: a list's or text's count, or the `length` field of a record
/// whose shape declares one. A site read both ways (no program whose names
/// all have declared types writes one) is refused rather than written out
/// one way for both.
fn infer_length(
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    let [x] = args else {
        return err(
            "type-arity",
            checks::call_arity("length", args.len(), ["string | list"]),
            span,
        );
    };
    let t = infer(x, scope, shapes)?;
    let (field, ty) = match &t {
        Ty::String | Ty::Enum(_) | Ty::List(_) => (false, Ty::Number),
        Ty::Record(shape) => match shapes.field(shape, "length") {
            Some((_, ty)) => (true, ty.clone()),
            None => {
                let mut e = shapes.unknown_field(shape, "length", x.span());
                e.message = format!(
                    "`.length` counts a list or text, and `{shape}` is a record: {}",
                    e.message
                );
                return Err(e);
            }
        },
        other => {
            return err(
                "type-argument",
                format!(
                    "{} expects `string | list`, given `{other}`",
                    contract_syntax::idioms::argument("length", 0)
                ),
                x.span(),
            )
        }
    };
    if let Ok(mut guard) = shapes.lengths.lock() {
        let reads = &mut *guard;
        let (mine, other) = match field {
            true => (&mut reads.fields, &reads.counts),
            false => (&mut reads.counts, &reads.fields),
        };
        if other.contains(&span) {
            return err(
                "type-argument",
                "`.length` here reads a record's `length` field in one use and counts a list or text in another: declare the prop's type",
                span,
            );
        }
        mine.insert(span);
    }
    Ok(ty)
}

/// Whether `f` is one of them.
pub(crate) fn is_list_op(f: Stdlib) -> bool {
    matches!(
        f,
        Stdlib::Map
            | Stdlib::Filter
            | Stdlib::SortBy
            | Stdlib::Join
            | Stdlib::Concat
            | Stdlib::Slice
            | Stdlib::Includes
            | Stdlib::IndexOf
            | Stdlib::Range
            | Stdlib::Flat
            | Stdlib::Length
    )
}

/// `xs.flat()`: JavaScript's at its default depth, 1, the only one taken;
/// a list of lists' items, `T[][]` to `T[]`.
fn infer_flat(args: &[Expr], span: Span, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    let [list] = args else {
        return err(
            "type-arity",
            "`.flat()` takes no depth: it flattens one level, as `flat()` and `flat(1)` do; two levels are `xs.flat().flat()`",
            span,
        );
    };
    match infer(list, scope, shapes)? {
        Ty::Unknown => Ok(Ty::Unknown),
        Ty::List(item) => match *item {
            Ty::List(_) => Ok(*item),
            Ty::Unknown => Ok(Ty::List(item)),
            other => err(
                "type-argument",
                format!("`.flat()` flattens a list of lists, given `{other}[]`, whose items are not lists"),
                list.span(),
            ),
        },
        other => err(
            "type-argument",
            format!("the value before `.flat` expects a list of lists, given `{other}`"),
            list.span(),
        ),
    }
}

/// `range(n)`: one number, the count; a literal one is a whole number at
/// least 0, as the runner traps any other.
fn infer_range(args: &[Expr], span: Span, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    let [n] = args else {
        return err(
            "type-arity",
            format!(
                "`range(n)` takes one argument, the count (`range(3)` is `[0, 1, 2]`), given {}; the numbers from `a` to `b - 1` are `range(b).slice(a)`, and a window of them `range(n).slice(a, b)`",
                args.len()
            ),
            span,
        );
    };
    let t = infer(n, scope, shapes)?;
    if t != Ty::Number {
        return err(
            "type-argument",
            format!("argument 1 of `range` expects `number`, given `{t}`"),
            n.span(),
        );
    }
    let literal = match n {
        Expr::Number(v, _) => Some(*v),
        Expr::Unary(UnOp::Neg, inner, _) => match &**inner {
            Expr::Number(v, _) => Some(-v),
            _ => None,
        },
        _ => None,
    };
    if let Some(v) = literal.filter(|v| !(v.fract() == 0.0 && *v >= 0.0)) {
        return err(
            "type-argument",
            format!("`range(n)` counts from 0 to n - 1, so `n` is a whole number at least 0; given `{v}`"),
            n.span(),
        );
    }
    Ok(Ty::List(Box::new(Ty::Number)))
}

/// `concat(T[], T[])`, `slice(string | T[], number, number?)`
/// and `includes(string, string)` or `includes(T[], T)` with `T` a
/// string, number or bool, `indexOf` likewise: the web compares an object
/// by identity, so they take what `==` and `join` take of a list's items.
fn infer_built(
    f: Stdlib,
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    let name = f.name();
    let params: &[&str] = match f {
        Stdlib::Concat => &["list", "list"],
        Stdlib::Slice => &["string | list", "number", "number?"],
        _ => &["string | list", "string | item"],
    };
    let required = params.iter().filter(|p| !p.ends_with('?')).count();
    if !(required..=params.len()).contains(&args.len()) {
        return err(
            "type-arity",
            checks::call_arity(name, args.len(), params),
            span,
        );
    }
    let first = infer(&args[0], scope, shapes)?;
    let argument = |i: usize, want: &str, given: &Ty| {
        err(
            "type-argument",
            format!(
                "{} expects `{want}`, given `{given}`",
                contract_syntax::idioms::argument(name, i)
            ),
            args[i].span(),
        )
    };
    match f {
        Stdlib::Concat => {
            let second = infer(&args[1], scope, shapes)?;
            match (&first, &second) {
                (Ty::Unknown, _) | (_, Ty::Unknown) => Ok(Ty::Unknown),
                (Ty::List(_), Ty::List(_)) => first.unify(&second).ok_or_else(|| TypeError {
                    id: "type-argument",
                    message: format!(
                        "`concat` joins two lists of one item type, given `{first}` and `{second}`"
                    ),
                    span: args[1].span(),
                }),
                (Ty::String, _) => err(
                    "type-argument",
                    "`concat` joins two lists; text joins with `+` or a template, `${a}${b}`",
                    span,
                ),
                (Ty::List(_), other) => argument(1, &first.to_string(), other),
                (other, _) => argument(0, "list", other),
            }
        }
        Stdlib::Slice => {
            for (i, arg) in args.iter().enumerate().skip(1) {
                let t = infer(arg, scope, shapes)?;
                if t != Ty::Number {
                    return argument(i, "number", &t);
                }
            }
            match first {
                Ty::Enum(_) => Ok(Ty::String),
                Ty::String | Ty::List(_) => Ok(first),
                other => argument(0, "string | list", &other),
            }
        }
        _ => {
            let second = infer(&args[1], scope, shapes)?;
            let found = if f == Stdlib::IndexOf {
                Ty::Number
            } else {
                Ty::Bool
            };
            match &first {
                Ty::String | Ty::Enum(_) if second.is_string() => Ok(found),
                Ty::String | Ty::Enum(_) => argument(1, "string", &second),
                Ty::List(item) => match item.unify(&second) {
                    Some(Ty::String | Ty::Enum(_) | Ty::Number | Ty::Bool) => Ok(found),
                    Some(other) => err(
                        "type-argument",
                        format!(
                            "`{name}` finds a string, number or bool in a list, given `{other}`: test a field, `xs.filter(x => x.id == id).length > 0`"
                        ),
                        args[1].span(),
                    ),
                    None => argument(1, &item.to_string(), &second),
                },
                other => argument(0, "string | list", other),
            }
        }
    }
}

/// The type of `f(args)`.
pub(crate) fn infer_call(
    f: Stdlib,
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    if matches!(
        f,
        Stdlib::Concat | Stdlib::Slice | Stdlib::Includes | Stdlib::IndexOf
    ) {
        return infer_built(f, args, span, scope, shapes);
    }
    if f == Stdlib::Range {
        return infer_range(args, span, scope, shapes);
    }
    if f == Stdlib::Flat {
        return infer_flat(args, span, scope, shapes);
    }
    if f == Stdlib::Length {
        return infer_length(args, span, scope, shapes);
    }
    let name = f.name();
    if f == Stdlib::SortBy {
        return infer_sort_by(args, span, scope, shapes);
    }
    let [list, second] = args else {
        let signature = if f == Stdlib::Join {
            "(separator)"
        } else {
            "((item, index) => …)"
        };
        return err(
            "type-arity",
            format!(
                "`.{name}(…)` takes 1 argument, `xs.{name}{signature}`, given {}",
                args.len().saturating_sub(1)
            ),
            span,
        );
    };
    let listed = infer(list, scope, shapes)?;
    let item = match &listed {
        Ty::List(item) => (**item).clone(),
        Ty::Unknown => Ty::Unknown,
        other => {
            return err(
                "type-argument",
                format!(
                    "{} expects a list, given `{other}`",
                    contract_syntax::idioms::argument(name, 0)
                ),
                list.span(),
            )
        }
    };
    if f == Stdlib::Join {
        if !matches!(
            item,
            Ty::String | Ty::Enum(_) | Ty::Number | Ty::Bool | Ty::Unknown
        ) {
            return err(
                "type-argument",
                format!(
                    "`join` prints strings, numbers and bools, given `{listed}`: `map` each item to a string first"
                ),
                list.span(),
            );
        }
        let separator = infer(second, scope, shapes)?;
        if !checks::can_unify(&Ty::String, &separator) {
            return err(
                "type-argument",
                format!("argument 2 of `join` expects `string`, given `{separator}`"),
                second.span(),
            );
        }
        return Ok(Ty::String);
    }
    let (body, inner) = callback(name, second, item, scope)?;
    // `map(items, i => Row(item=i))`: the JSX habit. A declared shape's
    // name builds a record (`F(x=i)`, `F(f, x=…)`; LLP 1035.005.000 D3), a
    // value a callback may return.
    if let Expr::Call(callee, _, at) = body {
        if callee.starts_with(|c: char| c.is_ascii_uppercase())
            && !shapes.fns.contains_key(callee)
            && !crate::records::is_record_call(callee, shapes)
        {
            return err(
                "type-callback-view",
                format!("a callback returns one value, not a view: repeat `{callee}` with `for x in xs key=x.id` under its parent"),
                *at,
            );
        }
    }
    let result = infer(body, &inner, shapes)?;
    if f == Stdlib::Map {
        return Ok(if listed == Ty::Unknown {
            Ty::Unknown
        } else {
            Ty::List(Box::new(result))
        });
    }
    if !matches!(result, Ty::Bool | Ty::Unknown) {
        // No truthiness: say the comparison the web's `filter` implied.
        let compare = match &result {
            Ty::String | Ty::Enum(_) => "compare it: `x.name != \"\"`",
            Ty::Number => "compare it: `x.count != 0`",
            Ty::Option(_) => "compare it: `x.note != none`",
            Ty::List(_) => "test it: `x.tags.length > 0`",
            _ => "return a comparison",
        };
        return err(
            "type-argument",
            format!("a `filter` callback returns a bool, not `{result}` (Contract has no truthiness); {compare}"),
            body.span(),
        );
    }
    Ok(listed)
}

/// A callback's body and the scope it reads: the item (of type `item`) and
/// its index bound to its parameters, which are at most two and distinct.
fn callback<'a>(
    name: &str,
    arrow: &'a Expr,
    item: Ty,
    scope: &Scope,
) -> Result<(&'a Expr, Scope), TypeError> {
    let Expr::Arrow { params, body, .. } = arrow else {
        let form = if name == "sortBy" {
            "`sortBy(list, item => key)`".to_owned()
        } else {
            format!("`{name}(list, (item, index) => …)`")
        };
        return err(
            "type-argument",
            format!(
                "{} is an arrow function: {form}",
                contract_syntax::idioms::argument(name, 1)
            ),
            arrow.span(),
        );
    };
    if params.len() > 2 || (params.len() == 2 && params[0] == params[1]) {
        return err(
            "type-arrow-parameters",
            format!(
                "a `{name}` callback takes the item and its index, two different names at most: `(item, index) => …`"
            ),
            arrow.span(),
        );
    }
    let mut inner = scope.clone();
    inner.push(
        params
            .iter()
            .zip([item, Ty::Number])
            .map(|(p, t)| (p.clone(), Ref::Local(0), t))
            .collect(),
    );
    Ok((body, inner))
}

/// `sortBy(xs, x => key)` and `sortBy(xs, x => key, "desc")` (@ref LLP
/// 1116.016 §5): a list's items in a stable order of one key, a number or a
/// string, the direction a literal `"asc"` (the default) or `"desc"`.
fn infer_sort_by(
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    if !(2..=3).contains(&args.len()) {
        return err(
            "type-arity",
            format!(
                "`sortBy` takes a list, a key and an optional direction, `sortBy(xs, x => x.name)` or `sortBy(xs, x => x.count, \"desc\")`, given {} argument{}",
                args.len(),
                if args.len() == 1 { "" } else { "s" }
            ),
            span,
        );
    }
    let listed = infer(&args[0], scope, shapes)?;
    let item = match &listed {
        Ty::List(item) => (**item).clone(),
        Ty::Unknown => Ty::Unknown,
        other => {
            return err(
                "type-argument",
                format!("argument 1 of `sortBy` expects a list, given `{other}`"),
                args[0].span(),
            )
        }
    };
    let (body, inner) = callback("sortBy", &args[1], item, scope)?;
    let key = infer(body, &inner, shapes)?;
    if !matches!(key, Ty::Number | Ty::String | Ty::Enum(_) | Ty::Unknown) {
        return err(
            "type-argument",
            format!(
                "a `sortBy` key is a number or a string, given `{key}`: sort by one field (`x => x.name`); to break its ties by another, sort by that one first, `sortBy(sortBy(xs, x => x.date), x => x.name)`, since `sortBy` keeps equal keys in order"
            ),
            body.span(),
        );
    }
    if let Some(dir) = args.get(2) {
        if !matches!(dir, Expr::Str(d, _) if d == "asc" || d == "desc") {
            return err(
                "type-argument",
                "argument 3 of `sortBy` is the direction, written `\"asc\"` (the default) or `\"desc\"`",
                dir.span(),
            );
        }
    }
    Ok(listed)
}
