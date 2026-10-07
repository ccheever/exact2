//! Capture-avoiding substitution of names by expressions (or by other
//! names), the one operation inlining is built from.
//!
//! A [`Subst`] holds its replacements by reference and the binders in force
//! where it is being applied. A binder hides a replacement of its own name;
//! when some replacement mentions the binder's name, the binder is renamed
//! apart — to `x@1`, a spelling no author can write — so an expression a
//! parent passes in can never be captured by a child's `match` or `let`.

use crate::ast::{Expr, Stmt, TemplatePart};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};

pub(super) enum Replacement<'a> {
    Name(&'a str),
    Expr(&'a Expr),
}

pub(super) trait SubstitutionValue: Clone {
    fn replacement(&self) -> Replacement<'_>;
    /// Add the names that occur free in the replacement.
    fn free_names(&self, out: &mut BTreeSet<String>);
}

impl SubstitutionValue for String {
    fn replacement(&self) -> Replacement<'_> {
        Replacement::Name(self)
    }
    fn free_names(&self, out: &mut BTreeSet<String>) {
        out.insert(self.clone());
    }
}

impl SubstitutionValue for Expr {
    fn replacement(&self) -> Replacement<'_> {
        match self {
            Expr::Ident(name, _) => Replacement::Name(name),
            expr => Replacement::Expr(expr),
        }
    }
    fn free_names(&self, out: &mut BTreeSet<String>) {
        free_names(self, &mut Vec::new(), out)
    }
}

/// Replacements by name, applied under the binders in force.
pub(super) struct Subst<'m, T> {
    map: &'m BTreeMap<String, T>,
    /// Every name free in some replacement, computed at the first binder.
    free: OnceCell<BTreeSet<String>>,
    /// Binders in force, innermost last: each hides its name's replacement,
    /// and a renamed one replaces its name by the new spelling.
    binders: Vec<(String, Option<String>)>,
    /// Whether a call's name is replaced too. Renamed locals (a view's loop
    /// and `match` variables) are values, never callable, so a call that
    /// spells one names a function (`t("key")` beside `each t in …`).
    calls: bool,
    /// The declared record constructors: shapes that are not `fn`s. A call
    /// spelling one constructs that record ahead of any scoped name (LLP
    /// 1035.005.000 D3), so its head is never replaced — its arguments are.
    records: &'m BTreeSet<String>,
    /// What a called action's body is substituted with past its parameters'
    /// `let`s (LLP 1089 D2): nothing, when this substitution is of the
    /// caller's own names, which the callee never reads; the component's
    /// names, when lifting substitutes them into caller and callee alike.
    callee: Option<&'m BTreeMap<String, T>>,
}

impl<'m, T: SubstitutionValue> Subst<'m, T> {
    pub(super) fn new(map: &'m BTreeMap<String, T>, records: &'m BTreeSet<String>) -> Self {
        Subst {
            map,
            free: OnceCell::new(),
            binders: Vec::new(),
            calls: true,
            records,
            callee: None,
        }
    }

    /// The same substitution, and `map` into the bodies of the calls it
    /// meets: the component's own names, which a callee of the same
    /// component reads as its caller does.
    pub(super) fn into_calls(self, map: &'m BTreeMap<String, T>) -> Self {
        Subst {
            callee: Some(map),
            ..self
        }
    }

    /// Renamed values only: a call's head names a function, never these.
    pub(super) fn values(map: &'m BTreeMap<String, T>, records: &'m BTreeSet<String>) -> Self {
        Subst {
            calls: false,
            ..Self::new(map, records)
        }
    }

    /// The replacements, for a view's substitution (no binder is in force
    /// over a view: its binders are renamed apart, never entered).
    pub(super) fn map(&self) -> &'m BTreeMap<String, T> {
        debug_assert!(self.binders.is_empty());
        self.map
    }

    fn get(&self, name: &str) -> Option<Replacement<'_>> {
        for (bound, spelled) in self.binders.iter().rev() {
            if bound == name {
                return spelled.as_deref().map(Replacement::Name);
            }
        }
        self.map.get(name).map(SubstitutionValue::replacement)
    }

    fn free(&self) -> &BTreeSet<String> {
        self.free.get_or_init(|| {
            let mut out = BTreeSet::new();
            for value in self.map.values() {
                value.free_names(&mut out);
            }
            out
        })
    }

    fn introduced(&self, name: &str) -> bool {
        self.binders
            .iter()
            .any(|(_, spelled)| spelled.as_deref() == Some(name))
    }

    /// Enter a binder `var` whose scope mentions a name when `occurs` says
    /// so; returns the binder's spelling in the result. Renaming whenever a
    /// replacement mentions `var` is conservative and always correct.
    fn enter(&mut self, var: &str, occurs: &dyn Fn(&str) -> bool) -> String {
        let spelled = if self.free().contains(var) || self.introduced(var) {
            (1u32..)
                .map(|k| format!("{var}@{k}"))
                .find(|c| !occurs(c) && !self.free().contains(c) && !self.introduced(c))
                .expect("an unused binder name")
        } else {
            var.to_owned()
        };
        let renamed = (spelled != var).then(|| spelled.clone());
        self.binders.push((var.to_owned(), renamed));
        spelled
    }

    fn leave(&mut self) {
        self.binders.pop();
    }
}

/// `e` with renamed locals substituted: every reference, but no call's name.
pub(super) fn renamed_locals(e: &Expr, map: &BTreeMap<String, String>) -> Expr {
    if map.is_empty() {
        return e.clone();
    }
    // No call's head is replaced here, so no record constructor need be known.
    let none = BTreeSet::new();
    let mut s = Subst::new(map, &none);
    s.calls = false;
    subst_expr(e, &mut s)
}

/// `e` with `map` substituted, for a one-off substitution; a call naming one
/// of `records` keeps its head.
pub(super) fn substituted<T: SubstitutionValue>(
    e: &Expr,
    map: &BTreeMap<String, T>,
    records: &BTreeSet<String>,
) -> Expr {
    if map.is_empty() {
        return e.clone();
    }
    subst_expr(e, &mut Subst::new(map, records))
}

/// Substitute prop names by argument expressions. A curried handler
/// `prop(args)` where the prop's argument is an action `f` or `f(a…)`
/// becomes `f(a…, args)`.
pub(super) fn subst_expr<T: SubstitutionValue>(e: &Expr, s: &mut Subst<'_, T>) -> Expr {
    if s.map.is_empty() && s.binders.is_empty() {
        return e.clone();
    }
    match e {
        Expr::Ident(n, span) => match s.get(n) {
            // Renaming a child state does not move its reference to the use
            // site. Keep the expression's source span for diagnostics.
            Some(Replacement::Name(name)) => Expr::Ident(name.to_owned(), *span),
            Some(Replacement::Expr(r)) => r.clone(),
            None => e.clone(),
        },
        Expr::Call(n, args, span) => {
            let args: Vec<Expr> = args.iter().map(|a| subst_expr(a, s)).collect();
            if !s.calls || s.records.contains(n) {
                return Expr::Call(n.clone(), args, *span);
            }
            match s.get(n) {
                Some(Replacement::Name(f)) => Expr::Call(f.to_owned(), args, *span),
                Some(Replacement::Expr(Expr::Call(f, first, _))) => {
                    let mut all = first.clone();
                    all.extend(args);
                    Expr::Call(f.clone(), all, *span)
                }
                _ => Expr::Call(n.clone(), args, *span),
            }
        }
        Expr::Member(o, f, span) => Expr::Member(Box::new(subst_expr(o, s)), f.clone(), *span),
        Expr::Typed(value, ty, span) => {
            Expr::Typed(Box::new(subst_expr(value, s)), ty.clone(), *span)
        }
        Expr::NamedArg(n, value, span) => {
            Expr::NamedArg(n.clone(), Box::new(subst_expr(value, s)), *span)
        }
        Expr::Some(x, span) => Expr::Some(Box::new(subst_expr(x, s)), *span),
        Expr::Unary(op, x, span) => Expr::Unary(*op, Box::new(subst_expr(x, s)), *span),
        Expr::Binary(op, a, b, span) => Expr::Binary(
            *op,
            Box::new(subst_expr(a, s)),
            Box::new(subst_expr(b, s)),
            *span,
        ),
        Expr::Ternary(a, b, c, span) => Expr::Ternary(
            Box::new(subst_expr(a, s)),
            Box::new(subst_expr(b, s)),
            Box::new(subst_expr(c, s)),
            *span,
        ),
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let subject = Box::new(subst_expr(subject, s));
            let none = Box::new(subst_expr(none, s));
            let var = s.enter(var, &|name| occurs(some, name));
            let some = Box::new(subst_expr(some, s));
            s.leave();
            Expr::Match {
                subject,
                var,
                some,
                none,
                span: *span,
            }
        }
        Expr::Let {
            name,
            value,
            body,
            span,
        } => {
            let value = Box::new(subst_expr(value, s));
            let name = s.enter(name, &|n| occurs(body, n));
            let body = Box::new(subst_expr(body, s));
            s.leave();
            Expr::Let {
                name,
                value,
                body,
                span: *span,
            }
        }
        Expr::Arrow { params, body, span } => {
            let spelled: Vec<String> = params
                .iter()
                .map(|p| s.enter(p, &|n| occurs(body, n) || params.iter().any(|q| q == n)))
                .collect();
            let body = Box::new(subst_expr(body, s));
            for _ in params {
                s.leave();
            }
            Expr::Arrow {
                params: spelled,
                body,
                span: *span,
            }
        }
        Expr::Template(parts, span) => Expr::Template(
            parts
                .iter()
                .map(|p| match p {
                    TemplatePart::Text(t) => TemplatePart::Text(t.clone()),
                    TemplatePart::Expr(x) => TemplatePart::Expr(subst_expr(x, s)),
                })
                .collect(),
            *span,
        ),
        Expr::List(items, span) => {
            Expr::List(items.iter().map(|x| subst_expr(x, s)).collect(), *span)
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => e.clone(),
    }
}

/// A child action's body, its names substituted: assignment targets renamed
/// with `names`, expressions through `s` (props, injects, renamed states
/// and actions, derives as expressions). A `let` binds for the rest of its
/// block (LLP 1035.005.000 D2) and is renamed apart, as a `match` binding
/// is, when a replacement mentions its name.
pub(super) fn subst_stmts(
    stmts: &[Stmt],
    s: &mut Subst<'_, Expr>,
    names: &BTreeMap<String, String>,
) -> Vec<Stmt> {
    let mut lets = 0;
    let out = stmts
        .iter()
        .enumerate()
        .map(|(i, st)| match st {
            Stmt::Let { name, expr, span } => {
                let expr = subst_expr(expr, s);
                let rest = &stmts[i + 1..];
                let name = s.enter(name, &|n| rest.iter().any(|st| stmt_occurs(st, n)));
                lets += 1;
                Stmt::Let {
                    name,
                    expr,
                    span: *span,
                }
            }
            Stmt::Assign { target, expr, span } => Stmt::Assign {
                target: names.get(target).cloned().unwrap_or_else(|| target.clone()),
                expr: subst_expr(expr, s),
                span: *span,
            },
            Stmt::Command { name, args, span } => Stmt::Command {
                name: name.clone(),
                args: args.iter().map(|a| subst_expr(a, s)).collect(),
                span: *span,
            },
            Stmt::Send {
                target,
                source,
                args,
                span,
            } => Stmt::Send {
                target: target.clone(),
                source: source.clone(),
                args: args.iter().map(|a| subst_expr(a, s)).collect(),
                span: *span,
            },
            Stmt::Refresh { target, span } => Stmt::Refresh {
                target: target.clone(),
                span: *span,
            },
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => Stmt::If {
                cond: subst_expr(cond, s),
                then: subst_stmts(then, s, names),
                otherwise: subst_stmts(otherwise, s, names),
                span: *span,
            },
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => {
                let subject = subst_expr(subject, s);
                let none = subst_stmts(none, s, names);
                let var = s.enter(&some.0, &|name| {
                    some.1.iter().any(|st| stmt_occurs(st, name))
                });
                let body = subst_stmts(&some.1, s, names);
                s.leave();
                Stmt::Match {
                    subject,
                    some: (var, body),
                    none,
                    span: *span,
                }
            }
            // The arguments, and the parameters' `let`s that bind them, are
            // the caller's expressions. The rest of the body is the callee's,
            // closed over the caller's names (LLP 1089 D2), so a
            // substitution of those stops at the `let`s; lifting carries the
            // component's own names on into it.
            Stmt::Call {
                action,
                args,
                body,
                authored,
                curried,
                binding,
                span,
            } => {
                let params = args.len().min(body.len());
                let mut inner: Vec<Stmt> = body[..params]
                    .iter()
                    .map(|st| match st {
                        Stmt::Let { name, expr, span } => Stmt::Let {
                            name: name.clone(),
                            expr: subst_expr(expr, s),
                            span: *span,
                        },
                        other => other.clone(),
                    })
                    .collect();
                match s.callee {
                    Some(map) => {
                        let mut deep = Subst::new(map, s.records).into_calls(map);
                        deep.calls = s.calls;
                        inner.extend(subst_stmts(&body[params..], &mut deep, names));
                    }
                    None => inner.extend(body[params..].iter().cloned()),
                }
                Stmt::Call {
                    action: names.get(action).cloned().unwrap_or_else(|| action.clone()),
                    args: args.iter().map(|a| subst_expr(a, s)).collect(),
                    body: inner,
                    authored: *authored,
                    curried: *curried,
                    binding: *binding,
                    span: *span,
                }
            }
        })
        .collect();
    for _ in 0..lets {
        s.leave();
    }
    out
}

/// Add the names free in `e` (a call's head included: substitution
/// replaces it too), skipping those `bound` holds.
fn free_names(e: &Expr, bound: &mut Vec<String>, out: &mut BTreeSet<String>) {
    let mut name = |n: &str, bound: &Vec<String>| {
        if !bound.iter().any(|b| b == n) {
            out.insert(n.to_owned());
        }
    };
    match e {
        Expr::Ident(n, _) => name(n, bound),
        Expr::Call(n, args, _) => {
            name(n, bound);
            for a in args {
                free_names(a, bound, out);
            }
        }
        Expr::List(items, _) => {
            for x in items {
                free_names(x, bound, out);
            }
        }
        Expr::Member(o, _, _)
        | Expr::NamedArg(_, o, _)
        | Expr::Typed(o, _, _)
        | Expr::Some(o, _)
        | Expr::Unary(_, o, _) => free_names(o, bound, out),
        Expr::Binary(_, a, b, _) => {
            free_names(a, bound, out);
            free_names(b, bound, out);
        }
        Expr::Ternary(a, b, c, _) => {
            free_names(a, bound, out);
            free_names(b, bound, out);
            free_names(c, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            free_names(subject, bound, out);
            free_names(none, bound, out);
            bound.push(var.clone());
            free_names(some, bound, out);
            bound.pop();
        }
        Expr::Let {
            name: n,
            value,
            body,
            ..
        } => {
            free_names(value, bound, out);
            bound.push(n.clone());
            free_names(body, bound, out);
            bound.pop();
        }
        Expr::Arrow { params, body, .. } => {
            bound.extend(params.iter().cloned());
            free_names(body, bound, out);
            bound.truncate(bound.len() - params.len());
        }
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    free_names(x, bound, out);
                }
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}

/// Whether `name` is written anywhere in `e`, free or bound.
fn occurs(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Ident(n, _) => n == name,
        Expr::Call(n, args, _) => n == name || args.iter().any(|a| occurs(a, name)),
        Expr::List(items, _) => items.iter().any(|a| occurs(a, name)),
        Expr::Member(o, _, _)
        | Expr::NamedArg(_, o, _)
        | Expr::Typed(o, _, _)
        | Expr::Some(o, _)
        | Expr::Unary(_, o, _) => occurs(o, name),
        Expr::Binary(_, a, b, _) => occurs(a, name) || occurs(b, name),
        Expr::Ternary(a, b, c, _) => occurs(a, name) || occurs(b, name) || occurs(c, name),
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => var == name || occurs(subject, name) || occurs(some, name) || occurs(none, name),
        Expr::Let {
            name: n,
            value,
            body,
            ..
        } => n == name || occurs(value, name) || occurs(body, name),
        Expr::Arrow { params, body, .. } => params.iter().any(|p| p == name) || occurs(body, name),
        Expr::Template(parts, _) => parts
            .iter()
            .any(|p| matches!(p, TemplatePart::Expr(x) if occurs(x, name))),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => false,
    }
}

pub(super) fn stmt_occurs(st: &Stmt, name: &str) -> bool {
    match st {
        Stmt::Assign { target, expr, .. } => target == name || occurs(expr, name),
        Stmt::Let { name: n, expr, .. } => n == name || occurs(expr, name),
        Stmt::Command { args, .. } | Stmt::Send { args, .. } => {
            args.iter().any(|a| occurs(a, name))
        }
        Stmt::Call { args, body, .. } => {
            args.iter().any(|a| occurs(a, name)) || body.iter().any(|s| stmt_occurs(s, name))
        }
        Stmt::Refresh { .. } => false,
        Stmt::If {
            cond,
            then,
            otherwise,
            ..
        } => {
            occurs(cond, name)
                || then.iter().any(|s| stmt_occurs(s, name))
                || otherwise.iter().any(|s| stmt_occurs(s, name))
        }
        Stmt::Match {
            subject,
            some,
            none,
            ..
        } => {
            some.0 == name
                || occurs(subject, name)
                || some.1.iter().any(|s| stmt_occurs(s, name))
                || none.iter().any(|s| stmt_occurs(s, name))
        }
    }
}
