//! A resource over a list (LLP 1017.000 P4c, amended 2026-11-04): `resource
//! r = source(x) for x in list [key=k] as shape S`. The list is read in the
//! root's scope; the arguments and the key read the element, which the
//! runner passes each element's request as its one parameter. A key is a
//! string, number or bool, as a `for` key is, and is the element itself when
//! none is written.

use super::{infer, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::ResourceDecl;

/// The scope `r`'s arguments read: `scope` with the element named, when `r`
/// is over a list. A refusal of the list or the key comes back with it, and
/// the element is then `?` so the arguments are still checked.
pub(crate) fn args_scope(
    r: &ResourceDecl,
    scope: &Scope,
    shapes: &Shapes,
) -> (Option<Scope>, Vec<TypeError>) {
    let Some(each) = &r.each else {
        return (None, Vec::new());
    };
    let mut errors = Vec::new();
    let item = match infer(&each.list, scope, shapes) {
        Ok(Ty::List(item)) if item.is_complete() => *item,
        Ok(Ty::List(_)) => {
            errors.push(TypeError {
                id: "type-cannot-infer",
                message: format!(
                    "cannot infer what `{}` holds: a resource over a list needs a typed list",
                    each.item
                ),
                span: each.list.span(),
            });
            Ty::Unknown
        }
        Ok(t) => {
            errors.push(TypeError {
                id: "type-resource-for-list",
                message: format!("`{}`'s `for` needs a list, given `{t}`", r.name),
                span: each.list.span(),
            });
            Ty::Unknown
        }
        Err(e) => {
            errors.push(e);
            Ty::Unknown
        }
    };
    let mut inner = scope.clone();
    inner.push(vec![(each.item.clone(), Ref::Param(0), item.clone())]);
    let key = match &each.key {
        Some(key) => infer(key, &inner, shapes).map(|t| (t, key.span())),
        None => Ok((item, each.span)),
    };
    match key {
        Ok((Ty::String | Ty::Enum(_) | Ty::Number | Ty::Bool | Ty::Unknown, _)) => {}
        Ok((t, span)) => errors.push(TypeError {
            id: "type-resource-for-key",
            message: match &each.key {
                Some(_) => format!("a key must be a string, number, or bool, not `{t}`"),
                None => format!(
                    "`{}`'s elements are `{t}`: add `key={}.<its id field>`, a string, number or bool no two elements share",
                    r.name, each.item
                ),
            },
            span,
        }),
        Err(e) => errors.push(e),
    }
    (Some(inner), errors)
}

/// What `r`'s source answers each request: the element of its list type
/// when `r` is over a list, else the type `r` reads.
pub(crate) fn answer(r: &ResourceDecl, read: &Ty) -> Ty {
    match (&r.each, read) {
        (Some(_), Ty::List(t)) => (**t).clone(),
        (_, t) => t.clone(),
    }
}
