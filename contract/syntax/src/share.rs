//! A `fn` body's repeated calls, bound once. A `fn` is expanded wherever it
//! is called (LLP 1017 P5), so `f1(x) = f0(x) + f0(x)` would expand `f0`
//! twice, and a chain of such functions would grow exponentially. Each
//! call the body repeats with the same arguments is instead evaluated once
//! and read from a local (`Expr::Let`).
//!
//! Only a call some occurrence of which is always evaluated is bound, and
//! at the smallest expression that always evaluates one: the value is
//! computed on exactly the paths that computed it before, once.

use crate::ast::{BinOp, Expr, TemplatePart};
use crate::spans::VisitSpans;

/// `body` with its repeated calls to functions `is_fn` names bound once.
pub fn share_calls(body: &Expr, is_fn: &dyn Fn(&str) -> bool) -> Expr {
    let mut next = 0;
    share(body.clone(), is_fn, &mut next)
}

fn share(e: Expr, is_fn: &dyn Fn(&str) -> bool, next: &mut u32) -> Expr {
    // The calls this expression always evaluates, in evaluation order.
    let mut strict = Vec::new();
    strict_calls(&e, is_fn, &mut strict);
    let repeated = strict.into_iter().find(|call| {
        let key = canonical(call);
        count(&e, &key, &free(call)) >= 2
    });
    let e = match repeated {
        Some(call) => {
            *next += 1;
            let Expr::Call(callee, _, _) = &call else {
                unreachable!("a strict call")
            };
            let name = format!("{callee}@{next}");
            let key = canonical(&call);
            let body = replace(e, &key, &free(&call), &name);
            Expr::Let {
                name,
                span: call.span(),
                value: Box::new(call),
                body: Box::new(body),
            }
        }
        None => e,
    };
    // Then the same within every part, strict or not.
    map_children(e, &mut |child| share(child, is_fn, next))
}

/// The calls `e` evaluates on every path through it.
fn strict_calls(e: &Expr, is_fn: &dyn Fn(&str) -> bool, out: &mut Vec<Expr>) {
    match e {
        Expr::Call(name, args, _) => {
            if is_fn(name) {
                out.push(e.clone());
            }
            for a in args {
                strict_calls(a, is_fn, out);
            }
        }
        Expr::List(items, _) => items.iter().for_each(|a| strict_calls(a, is_fn, out)),
        Expr::Binary(BinOp::And | BinOp::Or, a, _, _) => strict_calls(a, is_fn, out),
        Expr::Binary(_, a, b, _) => {
            strict_calls(a, is_fn, out);
            strict_calls(b, is_fn, out);
        }
        Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Some(x, _)
        | Expr::Unary(_, x, _) => strict_calls(x, is_fn, out),
        // A call both branches make is made on every path.
        Expr::Ternary(c, a, b, _) => {
            strict_calls(c, is_fn, out);
            both(a, b, &[], is_fn, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            strict_calls(subject, is_fn, out);
            both(some, none, std::slice::from_ref(var), is_fn, out);
        }
        // A bound value is always evaluated; its body's calls may read the
        // bound name, so they stay where they are.
        Expr::Let { value, .. } => strict_calls(value, is_fn, out),
        // A callback runs once per item, perhaps never (LLP 1017.003).
        Expr::Arrow { .. } => {}
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    strict_calls(x, is_fn, out);
                }
            }
        }
        Expr::Ident(..) | Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}

/// The calls both `a` and `b` always make, except those reading a name the
/// first binds (`bound`).
fn both(a: &Expr, b: &Expr, bound: &[String], is_fn: &dyn Fn(&str) -> bool, out: &mut Vec<Expr>) {
    let (mut left, mut right) = (Vec::new(), Vec::new());
    strict_calls(a, is_fn, &mut left);
    strict_calls(b, is_fn, &mut right);
    let right: Vec<String> = right.iter().map(canonical).collect();
    out.extend(left.into_iter().filter(|call| {
        !free(call).iter().any(|n| bound.contains(n)) && right.contains(&canonical(call))
    }));
}

/// An expression's structure without its source positions.
fn canonical(e: &Expr) -> String {
    let mut e = e.clone();
    e.visit_spans(&mut |span| *span = Default::default());
    format!("{e:?}")
}

fn free(e: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    names(e, &mut out);
    out
}

fn names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ident(n, _) => out.push(n.clone()),
        Expr::Call(_, args, _) | Expr::List(args, _) => args.iter().for_each(|a| names(a, out)),
        Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Some(x, _)
        | Expr::Unary(_, x, _) => names(x, out),
        Expr::Binary(_, a, b, _) => {
            names(a, out);
            names(b, out);
        }
        Expr::Ternary(a, b, c, _) => {
            names(a, out);
            names(b, out);
            names(c, out);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            names(subject, out);
            names(some, out);
            names(none, out);
        }
        Expr::Let { value, body, .. } => {
            names(value, out);
            names(body, out);
        }
        Expr::Arrow { body, .. } => names(body, out),
        Expr::Template(parts, _) => parts.iter().for_each(|p| {
            if let TemplatePart::Expr(x) = p {
                names(x, out);
            }
        }),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}

/// How many times `key` occurs in `e` where none of `free` is rebound.
fn count(e: &Expr, key: &str, free: &[String]) -> usize {
    if matches!(e, Expr::Call(..)) && canonical(e) == key {
        return 1;
    }
    let mut n = 0;
    each_child(e, free, &mut |child| n += count(child, key, free));
    n
}

/// `e` with every occurrence of `key` not under a rebinding of `free`
/// replaced by the name `with`.
fn replace(e: Expr, key: &str, free: &[String], with: &str) -> Expr {
    if matches!(e, Expr::Call(..)) && canonical(&e) == key {
        return Expr::Ident(with.to_owned(), e.span());
    }
    match e {
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => Expr::Match {
            subject: Box::new(replace(*subject, key, free, with)),
            some: if free.contains(&var) {
                some
            } else {
                Box::new(replace(*some, key, free, with))
            },
            none: Box::new(replace(*none, key, free, with)),
            var,
            span,
        },
        Expr::Let {
            name,
            value,
            body,
            span,
        } => Expr::Let {
            value: Box::new(replace(*value, key, free, with)),
            body: if free.contains(&name) {
                body
            } else {
                Box::new(replace(*body, key, free, with))
            },
            name,
            span,
        },
        Expr::Arrow { params, body, span } => Expr::Arrow {
            body: if params.iter().any(|p| free.contains(p)) {
                body
            } else {
                Box::new(replace(*body, key, free, with))
            },
            params,
            span,
        },
        e => map_children(e, &mut |child| replace(child, key, free, with)),
    }
}

/// Visit the children of `e`, skipping a binder's scope when it rebinds one
/// of `free`.
fn each_child(e: &Expr, free: &[String], f: &mut dyn FnMut(&Expr)) {
    match e {
        Expr::Call(_, args, _) | Expr::List(args, _) => args.iter().for_each(f),
        Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Some(x, _)
        | Expr::Unary(_, x, _) => f(x),
        Expr::Binary(_, a, b, _) => {
            f(a);
            f(b);
        }
        Expr::Ternary(a, b, c, _) => {
            f(a);
            f(b);
            f(c);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            f(subject);
            if !free.contains(var) {
                f(some);
            }
            f(none);
        }
        Expr::Let {
            name, value, body, ..
        } => {
            f(value);
            if !free.contains(name) {
                f(body);
            }
        }
        Expr::Arrow { params, body, .. } => {
            if !params.iter().any(|p| free.contains(p)) {
                f(body);
            }
        }
        Expr::Template(parts, _) => parts.iter().for_each(|p| {
            if let TemplatePart::Expr(x) = p {
                f(x);
            }
        }),
        Expr::Ident(..) | Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}

/// `e` with `f` applied to each child.
fn map_children(e: Expr, f: &mut dyn FnMut(Expr) -> Expr) -> Expr {
    let mut b = |x: Box<Expr>| Box::new(f(*x));
    match e {
        Expr::Call(n, args, s) => Expr::Call(n, args.into_iter().map(&mut *f).collect(), s),
        Expr::List(items, s) => Expr::List(items.into_iter().map(&mut *f).collect(), s),
        Expr::Member(x, field, s) => Expr::Member(b(x), field, s),
        Expr::NamedArg(n, x, s) => Expr::NamedArg(n, b(x), s),
        Expr::Typed(x, t, s) => Expr::Typed(b(x), t, s),
        Expr::Some(x, s) => Expr::Some(b(x), s),
        Expr::Unary(op, x, s) => Expr::Unary(op, b(x), s),
        Expr::Binary(op, x, y, s) => {
            let x = b(x);
            Expr::Binary(op, x, b(y), s)
        }
        Expr::Ternary(x, y, z, s) => {
            let (x, y) = (b(x), b(y));
            Expr::Ternary(x, y, b(z), s)
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let (subject, some) = (b(subject), b(some));
            Expr::Match {
                subject,
                var,
                some,
                none: b(none),
                span,
            }
        }
        Expr::Let {
            name,
            value,
            body,
            span,
        } => {
            let value = b(value);
            Expr::Let {
                name,
                value,
                body: b(body),
                span,
            }
        }
        Expr::Arrow { params, body, span } => Expr::Arrow {
            params,
            body: b(body),
            span,
        },
        Expr::Template(parts, s) => Expr::Template(
            parts
                .into_iter()
                .map(|p| match p {
                    TemplatePart::Expr(x) => TemplatePart::Expr(f(x)),
                    text => text,
                })
                .collect(),
            s,
        ),
        leaf => leaf,
    }
}

#[cfg(test)]
mod tests {
    use super::share_calls;
    use crate::ast::Expr;

    fn body(src: &str) -> Expr {
        let file = crate::parse(&format!(
            "fn g(x: number): number = {src}\ncomponent A\n  view\n    text \"a\"\n"
        ))
        .unwrap();
        file.fns[0].body.clone()
    }

    fn lets(e: &Expr) -> Vec<(String, bool)> {
        // Each bound name, and whether its let is the whole body.
        let mut out = Vec::new();
        fn walk(e: &Expr, top: bool, out: &mut Vec<(String, bool)>) {
            if let Expr::Let {
                name, value, body, ..
            } = e
            {
                out.push((name.clone(), top));
                walk(value, false, out);
                walk(body, top, out);
                return;
            }
            let mut e = e.clone();
            super::map_children(
                std::mem::replace(&mut e, Expr::None(Default::default())),
                &mut |c| {
                    walk(&c, false, out);
                    c
                },
            );
        }
        walk(e, true, &mut out);
        out
    }

    #[test]
    fn a_call_made_on_every_path_is_bound_once_where_it_is_always_made() {
        let is_fn = |n: &str| n == "f";
        for (src, expected) in [
            ("f(x) + f(x)", vec![("f@1", true)]),
            ("f(x) + f(x + 1)", vec![]),
            ("(x > 0 ? f(x) : 0) + f(x)", vec![("f@1", true)]),
            ("x > 0 ? f(x) : f(x)", vec![("f@1", true)]),
            // Only one branch makes it twice: bound inside that branch.
            ("x > 0 ? f(x) + f(x) : 0", vec![("f@1", false)]),
            // Neither path always makes it: left alone.
            ("x > 0 and f(x) > 0 ? 1 : f(x)", vec![]),
            ("f(f(x)) + f(f(x))", vec![("f@1", true)]),
            ("max(f(x), f(x))", vec![("f@1", true)]),
        ] {
            let shared = share_calls(&body(src), &is_fn);
            let found = lets(&shared);
            let found: Vec<(&str, bool)> = found.iter().map(|(n, t)| (n.as_str(), *t)).collect();
            assert_eq!(found, expected, "{src}: {shared:?}");
        }
    }
}
