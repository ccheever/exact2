//! `map`, `filter` and `join` (LLP 1017.003 D4): the roster's list
//! operations, typed here because a callback is not a value the roster
//! table can describe.

use crate::{checks, err, infer, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};
use exact_plan::Stdlib;

/// Whether `f` is one of them.
pub(crate) fn is_list_op(f: Stdlib) -> bool {
    matches!(f, Stdlib::Map | Stdlib::Filter | Stdlib::Join)
}

/// The type of `f(args)`.
pub(crate) fn infer_call(
    f: Stdlib,
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
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
    // `map(items, i => Row(item=i))`: the JSX habit.
    if let Expr::Call(callee, _, at) = &**body {
        if callee.starts_with(|c: char| c.is_ascii_uppercase()) && !shapes.fns.contains_key(callee)
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
