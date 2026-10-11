//! @ref LLP 1116.003 D6 — a read of a slot its own action assigned before
//! it is warned of.
//!
//! Every statement of an action reads the state the action started with,
//! and its assignments land together when it ends (LLP 1089 D2): after
//! `now = performanceNow()`, `if now >= endsAt` tests the last tick's time.
//! It is on the page, unlike a read behind a call (`calls.rs`, refused),
//! yet five builds in the bake-off's rounds 4 and 5 wrote it expecting the
//! new value. It is a warning, not a refusal: a swap, `a = b` then `b = a`,
//! means it.
//!
//! The walk is the action's own frame, path-sensitive as `calls.rs`'s: an
//! arm starts from what the paths into it wrote and the branch leaves their
//! union. A call is another frame, refused there. A condition tests the
//! starting state, which no assignment changes, so its facts hold for the
//! rest of the path: a read is left alone where it runs only on paths
//! exclusive with the assignment's (sequential `if k == …`, the right of an
//! `and`, a ternary's arm), and so is a read in a condition those facts
//! decide on the assignment's path (a chain of `if phase == 1 … phase = 2`
//! then `if phase == 2 …` steps once an action). A field the assignment
//! copied unchanged (`s = S(s, f=…)`, then `s.g`) is the same value.

use super::calls::slot_reads;
use super::sends::{exclusive, facts, Guard};
use super::{AnalyzeError, Related};
use contract_syntax::inline::calls::shown;
use contract_syntax::{BinOp, Component, Expr, Span, Stmt};
use std::collections::{BTreeMap, BTreeSet};

/// An assignment on some path: where, under what tests of the starting
/// state, and the fields it replaces when it copies its own slot's record.
#[derive(Clone)]
struct Write<'a> {
    span: Span,
    guard: Guard,
    fields: Option<BTreeSet<&'a str>>,
}

type Written<'a> = BTreeMap<&'a str, Vec<Write<'a>>>;

struct Walk<'a> {
    slots: &'a BTreeSet<&'a str>,
    out: Vec<AnalyzeError>,
}

impl<'a> Walk<'a> {
    fn block(
        &mut self,
        stmts: &'a [Stmt],
        guard: &Guard,
        written: &mut Written<'a>,
        bound: &mut Vec<&'a str>,
    ) {
        let depth = bound.len();
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, expr, .. } => {
                    self.reads(expr, guard, &Guard::new(), written, bound);
                    bound.push(name);
                }
                Stmt::Assign { target, expr, span } => {
                    self.reads(expr, guard, &Guard::new(), written, bound);
                    written.entry(target).or_default().push(Write {
                        span: *span,
                        guard: guard.clone(),
                        fields: copied(target, expr),
                    });
                }
                Stmt::Command { args, .. } | Stmt::Send { args, .. } => {
                    for a in args {
                        self.reads(a, guard, &Guard::new(), written, bound);
                    }
                }
                Stmt::Refresh { .. } | Stmt::Call { .. } => {}
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    let tested = facts(cond);
                    self.reads(cond, guard, &tested, written, bound);
                    let mut inner = guard.clone();
                    inner.extend(tested);
                    let mut other = written.clone();
                    self.block(then, &inner, written, bound);
                    self.block(otherwise, guard, &mut other, bound);
                    merge(written, other);
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.reads(subject, guard, &Guard::new(), written, bound);
                    let mut other = written.clone();
                    bound.push(&some.0);
                    self.block(&some.1, guard, written, bound);
                    bound.pop();
                    self.block(none, guard, &mut other, bound);
                    merge(written, other);
                }
            }
        }
        bound.truncate(depth);
    }

    /// Warn of each slot `e` reads that the action assigned before it on a
    /// path here; `tested` is what the condition `e` is part of says.
    fn reads(
        &mut self,
        e: &'a Expr,
        guard: &Guard,
        tested: &Guard,
        written: &Written<'a>,
        bound: &mut Vec<&'a str>,
    ) {
        match e {
            Expr::Binary(BinOp::And, a, b, _) => {
                self.reads(a, guard, tested, written, bound);
                let mut held = guard.clone();
                held.extend(facts(a));
                self.reads(b, &held, tested, written, bound);
            }
            Expr::Ternary(c, a, b, _) => {
                let test = facts(c);
                self.reads(c, guard, &test, written, bound);
                let mut held = guard.clone();
                held.extend(test);
                self.reads(a, &held, tested, written, bound);
                self.reads(b, guard, tested, written, bound);
            }
            _ => {
                let mut found = Vec::new();
                slot_reads(e, self.slots, bound, &mut found);
                for (slot, at, field) in found {
                    let stale = |w: &&Write<'_>| {
                        // A read written before the assignment (a child's
                        // derive, substituted where it is read) is a
                        // settled value, as a root's derive is.
                        w.span.source_id == at.source_id
                            && (w.span.line, w.span.col) < (at.line, at.col)
                            && !exclusive(&w.guard, guard)
                            && !exclusive(&w.guard, tested)
                            && !field.is_some_and(|f| {
                                w.fields.as_ref().is_some_and(|set| !set.contains(f))
                            })
                    };
                    let Some(w) = written.get(slot).and_then(|ws| ws.iter().rev().find(stale))
                    else {
                        continue;
                    };
                    let slot = shown(slot);
                    self.out.push(AnalyzeError {
                        id: "analyze-read-after-assign",
                        message: format!(
                            "`{slot}` here is its value from before the action, not the one assigned at line {}: writes land when the action ends. Bind the new value and read it: `let next = …`, `{slot} = next`, then `next`",
                            w.span.line
                        ),
                        span: at,
                        related: vec![Related {
                            span: w.span,
                            note: format!("`{slot}` is assigned here"),
                        }],
                    });
                }
            }
        }
    }
}

/// The fields `target = S(target, f=…, g=…)` replaces; `None` for any other
/// value.
fn copied<'a>(target: &str, expr: &'a Expr) -> Option<BTreeSet<&'a str>> {
    let Expr::Call(_, args, _) = expr else {
        return None;
    };
    let (Some(Expr::Ident(from, _)), rest) = (args.first(), args.get(1..)?) else {
        return None;
    };
    if from != target {
        return None;
    }
    rest.iter()
        .map(|a| match a {
            Expr::NamedArg(field, _, _) => Some(field.as_str()),
            _ => None,
        })
        .collect()
}

/// The union of two arms' writes; one made before the branch is in both.
fn merge<'a>(into: &mut Written<'a>, from: Written<'a>) {
    for (slot, ws) in from {
        let have = into.entry(slot).or_default();
        for w in ws {
            if !have.iter().any(|x| x.span == w.span) {
                have.push(w);
            }
        }
    }
}

/// Every read, in an action of the expanded root, of a state or mutation
/// slot the action's own body assigned before it on a path to it.
pub(super) fn check(c: &Component) -> Vec<AnalyzeError> {
    let slots: BTreeSet<&str> = c
        .states
        .iter()
        .map(|s| s.name.as_str())
        .chain(c.mutations.iter().map(|m| m.name.as_str()))
        .collect();
    let mut out: Vec<AnalyzeError> = Vec::new();
    for a in &c.actions {
        let mut walk = Walk {
            slots: &slots,
            out: Vec::new(),
        };
        walk.block(
            &a.body,
            &Guard::new(),
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        // A child's action, lifted once a use, repeats its warning at one read.
        for e in walk.out {
            if !out.iter().any(|x| x.span == e.span) {
                out.push(e);
            }
        }
    }
    out
}
