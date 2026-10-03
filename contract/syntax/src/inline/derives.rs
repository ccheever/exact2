//! A child's derives, resolved through one another before the use's props
//! are substituted (so a parent `a` passed in is never mistaken for the
//! child's derive `a`). Type inference admits either declaration order, so
//! resolution does too.
//!
//! A resolved derive is its body with its dependencies in scope, evaluated
//! exactly when the body evaluated them before: a dependency the body reads
//! once is written where it is read; one it reads more than once is bound
//! once (`Expr::Let`) at the smallest part of the body that evaluates it on
//! every path — inside the branch that needs it, never ahead of a condition
//! or a short-circuit that would have skipped it. Reads are counted through
//! other derives, so a chain of derives each reading the one before twice
//! resolves in linear size, not exponential.

use super::{err, substituted};
use crate::ast::{BinOp, Binding, Component, Expr, Node, Stmt, TemplatePart};
use crate::parser::SyntaxError;
use std::collections::{BTreeMap, BTreeSet};

/// The derives of `c` that its view, states, or actions read, each with its
/// resolved expression. A derive read only by other derives is resolved
/// where they read it.
/// `records` are the declared record constructors, whose call heads a
/// renamed binder never replaces.
pub(super) fn resolved_derives<'c>(
    c: &'c Component,
    records: &BTreeSet<String>,
) -> Result<Vec<(&'c Binding, Expr)>, SyntaxError> {
    let mut indices = BTreeMap::new();
    for (i, derive) in c.derives.iter().enumerate() {
        if indices.insert(derive.name.as_str(), i).is_some() {
            return err(
                "type-duplicate-name",
                format!("`{}` declared twice", derive.name),
                derive.span,
            );
        }
    }
    if c.derives.is_empty() {
        return Ok(Vec::new());
    }
    // Binders renamed apart first, so a `let` placed inside a `match` arm
    // can never capture a name its value reads.
    let mut fresh = 0;
    let bodies: Vec<Expr> = c
        .derives
        .iter()
        .map(|d| freshen(&d.expr, &mut fresh, records))
        .collect();
    let reads: Vec<BTreeSet<usize>> = bodies
        .iter()
        .map(|body| {
            let mut out = BTreeSet::new();
            dependencies(body, &indices, &mut out);
            out
        })
        .collect();
    let mut order = Vec::with_capacity(c.derives.len());
    let mut state = vec![0u8; c.derives.len()];
    for i in 0..c.derives.len() {
        visit(i, c, &reads, &mut state, &mut order)?;
    }
    let mut rank = vec![0; order.len()];
    for (at, &i) in order.iter().enumerate() {
        rank[i] = at;
    }
    // Each derive's evaluation in summary, dependencies first.
    let n = c.derives.len();
    let mut summaries = vec![Summary::new(n); n];
    for &i in &order {
        summaries[i] = summarize(&bodies[i], &indices, &summaries);
    }
    let cx = Cx {
        indices: &indices,
        names: c.derives.iter().map(|d| d.name.as_str()).collect(),
        bodies: &bodies,
        summaries: &summaries,
        rank: &rank,
    };
    let read = read_outside_derives(c);
    Ok(c.derives
        .iter()
        .enumerate()
        .filter(|(_, d)| read.contains(d.name.as_str()))
        .map(|(i, derive)| (derive, cx.place(&bodies[i], &mut Vec::new(), true)))
        .collect())
}

/// Depth-first dependency order (dependencies first); a cycle is refused
/// at the derive where it closes.
fn visit(
    i: usize,
    c: &Component,
    reads: &[BTreeSet<usize>],
    state: &mut [u8],
    order: &mut Vec<usize>,
) -> Result<(), SyntaxError> {
    match state[i] {
        2 => return Ok(()),
        1 => {
            return err(
                "type-derive-cycle",
                format!(
                    "cannot resolve `{}`: it depends on itself through other derives",
                    c.derives[i].name
                ),
                c.derives[i].span,
            )
        }
        _ => {}
    }
    state[i] = 1;
    for &dependency in &reads[i] {
        visit(dependency, c, reads, state, order)?;
    }
    state[i] = 2;
    order.push(i);
    Ok(())
}

/// What evaluating an expression does to each derive: how often its value
/// is read, through other derives too (0, 1, or 2 for "more"), and whether
/// it is read on every path through the expression.
#[derive(Clone)]
struct Summary {
    reads: Vec<u8>,
    always: Vec<bool>,
}

impl Summary {
    fn new(n: usize) -> Self {
        Summary {
            reads: vec![0; n],
            always: vec![false; n],
        }
    }

    fn add(&mut self, other: &Summary) {
        self.add_sometimes(other);
        for (mine, theirs) in self.always.iter_mut().zip(&other.always) {
            *mine |= theirs;
        }
    }

    /// Reads from a part evaluated on only some paths: counted, not always.
    fn add_sometimes(&mut self, other: &Summary) {
        for (mine, theirs) in self.reads.iter_mut().zip(&other.reads) {
            *mine = (*mine + theirs).min(2);
        }
    }
}

/// Summarize `e`, whose derive reads are summarized in `summaries`.
fn summarize(e: &Expr, indices: &BTreeMap<&str, usize>, summaries: &[Summary]) -> Summary {
    let mut out = Summary::new(summaries.len());
    let part = |x: &Expr| summarize(x, indices, summaries);
    match e {
        Expr::Ident(name, _) => {
            if let Some(&d) = indices.get(name.as_str()) {
                out.add(&summaries[d]);
                out.reads[d] = (out.reads[d] + 1).min(2);
                out.always[d] = true;
            }
        }
        Expr::Binary(BinOp::And | BinOp::Or, a, b, _) => {
            out.add(&part(a));
            out.add_sometimes(&part(b));
        }
        Expr::Ternary(c, a, b, _) => {
            out.add(&part(c));
            branches(&mut out, &part(a), &part(b));
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            out.add(&part(subject));
            branches(&mut out, &part(some), &part(none));
        }
        // A callback runs once per item, perhaps never (LLP 1017.003).
        Expr::Arrow { body, .. } => out.add_sometimes(&part(body)),
        _ => each_child(e, &mut |child| out.add(&part(child))),
    }
    out
}

/// Two exclusive branches: every read counts, and what both always read is
/// always read.
fn branches(out: &mut Summary, a: &Summary, b: &Summary) {
    out.add_sometimes(a);
    out.add_sometimes(b);
    for (i, always) in out.always.iter_mut().enumerate() {
        *always |= a.always[i] && b.always[i];
    }
}

struct Cx<'a> {
    indices: &'a BTreeMap<&'a str, usize>,
    names: Vec<&'a str>,
    bodies: &'a [Expr],
    summaries: &'a [Summary],
    rank: &'a [usize],
}

impl Cx<'_> {
    /// `e` with every derive it reads resolved; `bound` are the derives an
    /// enclosing `let` binds. `whole` is true where no enclosing part of the
    /// same body always evaluates `e` (the body itself, a branch, the right
    /// of `and`/`or`): only there can a derive first qualify to be bound.
    fn place(&self, e: &Expr, bound: &mut Vec<usize>, whole: bool) -> Expr {
        let mut lets = Vec::new();
        if whole {
            let s = summarize(e, self.indices, self.summaries);
            let mut shared: Vec<usize> = (0..s.reads.len())
                .filter(|&d| s.reads[d] >= 2 && s.always[d] && !bound.contains(&d))
                .collect();
            shared.sort_by_key(|&d| self.rank[d]);
            for d in shared {
                let value = self.place(&self.bodies[d], bound, true);
                lets.push((d, value));
                bound.push(d);
            }
        }
        let body = match e {
            Expr::Ident(name, _) => match self.indices.get(name.as_str()) {
                Some(d) if !bound.contains(d) => self.place(&self.bodies[*d], bound, true),
                _ => e.clone(),
            },
            Expr::Binary(op @ (BinOp::And | BinOp::Or), a, b, span) => Expr::Binary(
                *op,
                Box::new(self.place(a, bound, false)),
                Box::new(self.place(b, bound, true)),
                *span,
            ),
            Expr::Ternary(c, a, b, span) => Expr::Ternary(
                Box::new(self.place(c, bound, false)),
                Box::new(self.place(a, bound, true)),
                Box::new(self.place(b, bound, true)),
                *span,
            ),
            Expr::Match {
                subject,
                var,
                some,
                none,
                span,
            } => Expr::Match {
                subject: Box::new(self.place(subject, bound, false)),
                var: var.clone(),
                some: Box::new(self.place(some, bound, true)),
                none: Box::new(self.place(none, bound, true)),
                span: *span,
            },
            // Evaluated once per item, perhaps never: a part of its own.
            Expr::Arrow { params, body, span } => Expr::Arrow {
                params: params.clone(),
                body: Box::new(self.place(body, bound, true)),
                span: *span,
            },
            _ => map_children(e, &mut |child| self.place(child, bound, false)),
        };
        bound.truncate(bound.len() - lets.len());
        let span = e.span();
        lets.into_iter()
            .rev()
            .fold(body, |body, (d, value)| Expr::Let {
                name: self.names[d].to_owned(),
                value: Box::new(value),
                body: Box::new(body),
                span,
            })
    }
}

/// `e` with every `match` binder renamed to a spelling no author can write
/// (`x@b1`), each distinct.
fn freshen(e: &Expr, fresh: &mut u32, records: &BTreeSet<String>) -> Expr {
    match e {
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            *fresh += 1;
            let name = format!("{var}@b{fresh}");
            let some = freshen(some, fresh, records);
            let renamed = BTreeMap::from([(var.clone(), name.clone())]);
            let some = substituted(&some, &renamed, records);
            Expr::Match {
                subject: Box::new(freshen(subject, fresh, records)),
                var: name,
                some: Box::new(some),
                none: Box::new(freshen(none, fresh, records)),
                span: *span,
            }
        }
        Expr::Arrow { params, body, span } => {
            let mut renamed = BTreeMap::new();
            let params = params
                .iter()
                .map(|p| {
                    *fresh += 1;
                    let name = format!("{p}@b{fresh}");
                    renamed.insert(p.clone(), name.clone());
                    name
                })
                .collect();
            let body = freshen(body, fresh, records);
            Expr::Arrow {
                params,
                body: Box::new(substituted(&body, &renamed, records)),
                span: *span,
            }
        }
        _ => map_children(e, &mut |child| freshen(child, fresh, records)),
    }
}

/// The derives `expr` reads directly (binders are fresh, so none shadows).
fn dependencies(expr: &Expr, indices: &BTreeMap<&str, usize>, out: &mut BTreeSet<usize>) {
    if let Expr::Ident(name, _) | Expr::Call(name, _, _) = expr {
        if let Some(&i) = indices.get(name.as_str()) {
            out.insert(i);
        }
    }
    each_child(expr, &mut |child| dependencies(child, indices, out));
}

/// Every name written in the component's view, states and actions: the
/// derives any of them may read.
fn read_outside_derives(c: &Component) -> BTreeSet<&str> {
    fn expr<'a>(e: &'a Expr, out: &mut BTreeSet<&'a str>) {
        if let Expr::Ident(name, _) | Expr::Call(name, _, _) = e {
            out.insert(name);
        }
        each_child(e, &mut |child| expr(child, out));
    }
    fn stmts<'a>(body: &'a [Stmt], out: &mut BTreeSet<&'a str>) {
        for st in body {
            match st {
                Stmt::Assign { expr: e, .. } | Stmt::Let { expr: e, .. } => expr(e, out),
                Stmt::Command { args, .. } | Stmt::Send { args, .. } => {
                    args.iter().for_each(|a| expr(a, out))
                }
                Stmt::Refresh { .. } => {}
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    expr(cond, out);
                    stmts(then, out);
                    stmts(otherwise, out);
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    expr(subject, out);
                    stmts(&some.1, out);
                    stmts(none, out);
                }
            }
        }
    }
    fn nodes<'a>(view: &'a [Node], out: &mut BTreeSet<&'a str>) {
        for n in view {
            match n {
                Node::Element {
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    positional.iter().for_each(|e| expr(e, out));
                    attrs.iter().for_each(|a| expr(&a.value, out));
                    nodes(children, out);
                }
                Node::Use { args, children, .. } => {
                    args.iter().for_each(|a| expr(&a.value, out));
                    nodes(children, out);
                }
                Node::When {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    expr(cond, out);
                    nodes(then, out);
                    nodes(otherwise, out);
                }
                Node::Each {
                    list, key, body, ..
                } => {
                    expr(list, out);
                    expr(key, out);
                    nodes(body, out);
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    expr(subject, out);
                    nodes(&some.1, out);
                    nodes(none, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = BTreeSet::new();
    nodes(&c.view, &mut out);
    for b in c.states.iter().chain(&c.provides) {
        expr(&b.expr, &mut out);
    }
    for a in &c.actions {
        stmts(&a.body, &mut out);
    }
    out
}

fn each_child<'a>(e: &'a Expr, f: &mut dyn FnMut(&'a Expr)) {
    match e {
        Expr::Some(x, _)
        | Expr::Unary(_, x, _)
        | Expr::Case { subject: x, .. }
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _) => f(x),
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
            some,
            none,
            ..
        } => {
            f(subject);
            f(some);
            f(none);
        }
        Expr::Let { value, body, .. } => {
            f(value);
            f(body);
        }
        Expr::Arrow { body, .. } => f(body),
        Expr::Call(_, args, _) => args.iter().for_each(f),
        Expr::Template(parts, _) => parts.iter().for_each(|p| {
            if let TemplatePart::Expr(x) = p {
                f(x)
            }
        }),
        Expr::Number(..)
        | Expr::Str(..)
        | Expr::Bool(..)
        | Expr::None(_)
        | Expr::EmptyList(_)
        | Expr::Ident(..) => {}
    }
}

fn map_children(e: &Expr, f: &mut dyn FnMut(&Expr) -> Expr) -> Expr {
    match e {
        Expr::Some(x, s) => Expr::Some(Box::new(f(x)), *s),
        Expr::Unary(op, x, s) => Expr::Unary(*op, Box::new(f(x)), *s),
        Expr::Case {
            subject,
            literals,
            all,
            span,
        } => Expr::Case {
            subject: Box::new(f(subject)),
            literals: literals.clone(),
            all: all.clone(),
            span: *span,
        },
        Expr::Member(x, field, s) => Expr::Member(Box::new(f(x)), field.clone(), *s),
        Expr::NamedArg(n, x, s) => Expr::NamedArg(n.clone(), Box::new(f(x)), *s),
        Expr::Binary(op, x, y, s) => {
            let x = Box::new(f(x));
            Expr::Binary(*op, x, Box::new(f(y)), *s)
        }
        Expr::Ternary(x, y, z, s) => {
            let (x, y) = (Box::new(f(x)), Box::new(f(y)));
            Expr::Ternary(x, y, Box::new(f(z)), *s)
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let (subject, some) = (Box::new(f(subject)), Box::new(f(some)));
            Expr::Match {
                subject,
                var: var.clone(),
                some,
                none: Box::new(f(none)),
                span: *span,
            }
        }
        Expr::Let {
            name,
            value,
            body,
            span,
        } => {
            let value = Box::new(f(value));
            Expr::Let {
                name: name.clone(),
                value,
                body: Box::new(f(body)),
                span: *span,
            }
        }
        Expr::Arrow { params, body, span } => Expr::Arrow {
            params: params.clone(),
            body: Box::new(f(body)),
            span: *span,
        },
        Expr::Call(n, args, s) => Expr::Call(n.clone(), args.iter().map(&mut *f).collect(), *s),
        Expr::Template(parts, s) => Expr::Template(
            parts
                .iter()
                .map(|p| match p {
                    TemplatePart::Expr(x) => TemplatePart::Expr(f(x)),
                    TemplatePart::Text(t) => TemplatePart::Text(t.clone()),
                })
                .collect(),
            *s,
        ),
        leaf => leaf.clone(),
    }
}
