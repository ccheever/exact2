//! `map`, `filter` and `join` (LLP 1017.003 D4): the roster's list
//! operations, typed here because a callback is not a value the roster
//! table can describe; and `concat`, `slice` and `includes` (LLP 1088
//! §9.1), whose types follow their first argument's: a list's item type,
//! or text for `slice` and `includes`, the web's same-named methods.

use crate::{checks, err, infer, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};
use exact_plan::Stdlib;

/// Whether `f` is one of them.
pub(crate) fn is_list_op(f: Stdlib) -> bool {
    matches!(
        f,
        Stdlib::Map
            | Stdlib::Filter
            | Stdlib::Join
            | Stdlib::Concat
            | Stdlib::Slice
            | Stdlib::Includes
    )
}

/// `concat(list<T>, list<T>)`, `slice(string | list<T>, number, number?)`
/// and `includes(string, string)` or `includes(list<T>, T)` with `T` a
/// string, number or bool: the web compares an object by identity, so
/// `includes` takes what `==` and `join` take of a list's items.
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
                "argument {} of `{name}` expects `{want}`, given `{given}`",
                i + 1
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
                Ty::String | Ty::List(_) => Ok(first),
                other => argument(0, "string | list", &other),
            }
        }
        _ => {
            let second = infer(&args[1], scope, shapes)?;
            match &first {
                Ty::String if second == Ty::String => Ok(Ty::Bool),
                Ty::String => argument(1, "string", &second),
                Ty::List(item) => match item.unify(&second) {
                    Some(Ty::String | Ty::Number | Ty::Bool) => Ok(Ty::Bool),
                    Some(other) => err(
                        "type-argument",
                        format!(
                            "`includes` finds a string, number or bool in a list, given `{other}`: test a field, `length(filter(xs, x => x.id == id)) > 0`"
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
    if matches!(f, Stdlib::Concat | Stdlib::Slice | Stdlib::Includes) {
        return infer_built(f, args, span, scope, shapes);
    }
    let name = f.name();
    let [list, second] = args else {
        let signature = if f == Stdlib::Join {
            "(list, separator)"
        } else {
            "(list, (item, index) => …)"
        };
        return err(
            "type-arity",
            format!(
                "`{name}` takes 2 arguments, `{name}{signature}`, given {}",
                args.len()
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
                format!("argument 1 of `{name}` expects a list, given `{other}`"),
                list.span(),
            )
        }
    };
    if f == Stdlib::Join {
        if !matches!(item, Ty::String | Ty::Number | Ty::Bool | Ty::Unknown) {
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
    let Expr::Arrow { params, body, .. } = second else {
        return err(
            "type-argument",
            format!(
                "argument 2 of `{name}` is an arrow function: `{name}(list, (item, index) => …)`"
            ),
            second.span(),
        );
    };
    if params.len() > 2 || (params.len() == 2 && params[0] == params[1]) {
        return err(
            "type-arrow-parameters",
            format!(
                "a `{name}` callback takes the item and its index, two different names at most: `(item, index) => …`"
            ),
            second.span(),
        );
    }
    let mut inner = scope.clone();
    inner.push(
        params
            .iter()
            .zip([item.clone(), Ty::Number])
            .map(|(p, t)| (p.clone(), Ref::Local(0), t))
            .collect(),
    );
    // `map(items, i => Row(item=i))`: the JSX habit. A declared shape's
    // name builds a record (`F(x=i)`, `F(f, x=…)`; LLP 1035.005.000 D3), a
    // value a callback may return.
    if let Expr::Call(callee, _, at) = &**body {
        if callee.starts_with(|c: char| c.is_ascii_uppercase())
            && !shapes.fns.contains_key(callee)
            && !crate::records::is_record_call(callee, shapes)
        {
            return err(
                "type-callback-view",
                format!("a callback returns one value, not a view: repeat `{callee}` with `each x in xs key=x.id` under its parent"),
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
            Ty::String => "compare it: `x.name != \"\"`",
            Ty::Number => "compare it: `x.count != 0`",
            Ty::Option(_) => "compare it: `x.note != none`",
            Ty::List(_) => "test it: `length(x.tags) > 0`",
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
