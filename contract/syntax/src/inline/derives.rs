//! A child's derives, resolved through one another before the use's props
//! are substituted (so a parent `a` passed in is never mistaken for the
//! child's derive `a`). Type inference admits either declaration order, so
//! resolution does too.
//!
//! A resolved derive is its own body with its dependencies in scope. A
//! dependency read once in the whole closure is written in place, as a
//! paste always was; one read more than once is bound once (`Expr::Let`)
//! ahead of the body. A chain of derives, each reading the one before
//! twice, therefore resolves in linear size, not exponential.

use super::{err, substituted};
use crate::ast::{Binding, Component, Expr, TemplatePart};
use crate::parser::SyntaxError;
use std::collections::{BTreeMap, BTreeSet};

/// Every derive of `c` with its resolved expression, in declaration order.
pub(super) fn resolved_derives(c: &Component) -> Result<Vec<(&Binding, Expr)>, SyntaxError> {
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
    // Each derive's direct dependencies, with how often its body reads each.
    let reads: Vec<BTreeMap<usize, usize>> = c
        .derives
        .iter()
        .map(|d| {
            let mut out = BTreeMap::new();
            count_reads(&d.expr, &indices, &BTreeSet::new(), &mut out);
            out
        })
        .collect();
    let mut order = Vec::with_capacity(c.derives.len());
    let mut state = vec![0u8; c.derives.len()];
    for i in 0..c.derives.len() {
        visit(i, c, &reads, &mut state, &mut order)?;
    }
    let rank: Vec<usize> = {
        let mut rank = vec![0; order.len()];
        for (at, &i) in order.iter().enumerate() {
            rank[i] = at;
        }
        rank
    };
    Ok(c.derives
        .iter()
        .enumerate()
        .map(|(i, derive)| (derive, resolve(i, c, &reads, &rank)))
        .collect())
}

/// Depth-first dependency order (dependencies first); a cycle is refused
/// at the derive where it closes.
fn visit(
    i: usize,
    c: &Component,
    reads: &[BTreeMap<usize, usize>],
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
    for &dependency in reads[i].keys() {
        visit(dependency, c, reads, state, order)?;
    }
    state[i] = 2;
    order.push(i);
    Ok(())
}

fn resolve(i: usize, c: &Component, reads: &[BTreeMap<usize, usize>], rank: &[usize]) -> Expr {
    // The closure: every derive `i` reaches, in dependency order.
    let mut closure = BTreeSet::new();
    let mut stack: Vec<usize> = reads[i].keys().copied().collect();
    while let Some(d) = stack.pop() {
        if closure.insert(d) {
            stack.extend(reads[d].keys().copied());
        }
    }
    let mut closure: Vec<usize> = closure.into_iter().collect();
    closure.sort_by_key(|&d| rank[d]);
    // How often the closure's bodies, and `i`'s own, read each member.
    let mut count = BTreeMap::<usize, usize>::new();
    for &j in closure.iter().chain([&i]) {
        for (&d, &n) in &reads[j] {
            *count.entry(d).or_default() += n;
        }
    }
    let mut pasted = BTreeMap::<String, Expr>::new();
    let mut bound = Vec::new();
    for &d in &closure {
        let value = substituted(&c.derives[d].expr, &pasted);
        if count[&d] == 1 {
            pasted.insert(c.derives[d].name.clone(), value);
        } else {
            bound.push((c.derives[d].name.clone(), value));
        }
    }
    let span = c.derives[i].expr.span();
    bound.into_iter().rev().fold(
        substituted(&c.derives[i].expr, &pasted),
        |body, (name, value)| Expr::Let {
            name,
            value: Box::new(value),
            body: Box::new(body),
            span,
        },
    )
}

/// Count the reads of other derives in `expr`, skipping names a binder shadows.
fn count_reads(
    expr: &Expr,
    indices: &BTreeMap<&str, usize>,
    bound: &BTreeSet<String>,
    out: &mut BTreeMap<usize, usize>,
) {
    let mut read = |name: &str| {
        if !bound.contains(name) {
            if let Some(&i) = indices.get(name) {
                *out.entry(i).or_default() += 1;
            }
        }
    };
    match expr {
        Expr::Ident(name, _) => read(name),
        Expr::Call(name, args, _) => {
            read(name);
            for arg in args {
                count_reads(arg, indices, bound, out);
            }
        }
        Expr::Member(object, _, _)
        | Expr::Some(object, _)
        | Expr::Unary(_, object, _)
        | Expr::NamedArg(_, object, _) => count_reads(object, indices, bound, out),
        Expr::Binary(_, left, right, _) => {
            count_reads(left, indices, bound, out);
            count_reads(right, indices, bound, out);
        }
        Expr::Ternary(cond, then, otherwise, _) => {
            count_reads(cond, indices, bound, out);
            count_reads(then, indices, bound, out);
            count_reads(otherwise, indices, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            count_reads(subject, indices, bound, out);
            let mut inner = bound.clone();
            inner.insert(var.clone());
            count_reads(some, indices, &inner, out);
            count_reads(none, indices, bound, out);
        }
        Expr::Let {
            name, value, body, ..
        } => {
            count_reads(value, indices, bound, out);
            let mut inner = bound.clone();
            inner.insert(name.clone());
            count_reads(body, indices, &inner, out);
        }
        Expr::Template(parts, _) => {
            for part in parts {
                if let TemplatePart::Expr(expr) = part {
                    count_reads(expr, indices, bound, out);
                }
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}
