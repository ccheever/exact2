//! @ref LLP 1089 D3, LLP 1035.005.002 C2 — a slot read that a call makes
//! stale is warned of, as the same read in the action's own body is.
//!
//! Every statement of an action reads the state the action started with,
//! and its assignments land when it ends (D2). Inside one body that is on
//! the page (`doubled = count * 2` after `count = …`, `rereads.rs`); behind
//! a call, the write and the read are in different bodies and neither
//! shows it. So a read of a state, mutation or router slot in one frame —
//! the action's own body, or one call's — is named when some path to it
//! assigns that slot in another frame. It is one rule with the body's, one
//! id (`analyze-read-after-assign`) at one severity, with the body's
//! exemptions: a read on a path exclusive with the assignment's, a
//! condition the assignment's path decides, and a field a record copy left
//! unchanged. In 1,029 bake-off builds the body's warning was met in 86
//! and this one in 2 (LLP 1035.005.002 §1.2); refusing the rarer frame of
//! the same read was the exception C2 removed.
//!
//! The walk is `sends.rs`'s: each arm starts from what the paths into it
//! wrote, the branch leaves their union, and sequential `if`s testing one
//! unchanged name against disjoint literals are exclusive. It runs on the
//! expanded root, so a child's derive is the slot expression it was
//! substituted as. A derive, a resource and `pending(m)` read settled
//! values no statement changes; a send is not a write (its answer lands
//! before every assignment). A name an arrow, a `match` or a `let` binds is
//! not a slot.

use super::sends::{exclusive, facts, made_in, Frame, Guard};
use super::{AnalyzeError, Related};
use contract_syntax::inline::calls::shown;
use contract_syntax::{BinOp, Component, Expr, Span, Stmt, TemplatePart};
use std::collections::{BTreeMap, BTreeSet};

/// An assignment on some path: its frame, where, under what guard, the
/// frame's call, the action's own statement that holds it, and the fields
/// it replaces when it copies its own slot's record.
#[derive(Clone)]
struct Write<'a> {
    frame: usize,
    span: Span,
    guard: Guard,
    call: Frame<'a>,
    top: Span,
    fields: Option<BTreeSet<&'a str>>,
}

type Written<'a> = BTreeMap<&'a str, Vec<Write<'a>>>;

struct Walk<'a> {
    action: &'a str,
    /// The component's states and mutations, shared by every caller's walk.
    slots: std::rc::Rc<BTreeSet<&'a str>>,
    /// The frame being walked (0 is the action's own body) and its call.
    frame: (usize, Frame<'a>),
    frames: usize,
    /// The action's own statement being walked.
    top: Span,
    read: BTreeSet<Span>,
    errors: Vec<AnalyzeError>,
}

impl<'a> Walk<'a> {
    fn block(
        &mut self,
        stmts: &'a [Stmt],
        guard: &mut Guard,
        written: &mut Written<'a>,
        bound: &mut Vec<&'a str>,
    ) {
        let depth = bound.len();
        for stmt in stmts {
            if self.frame.0 == 0 {
                self.top = stmt.span();
            }
            match stmt {
                Stmt::Let { name, expr, .. } => {
                    self.reads(expr, guard, &Guard::new(), written, bound);
                    bound.push(name);
                }
                // A condition tests the starting state, which no assignment
                // or send changes (`rereads.rs`), so its facts stay on the
                // path: a chain over `phase` steps once an action, behind a
                // call as in the body.
                Stmt::Assign { target, expr, span } => {
                    self.reads(expr, guard, &Guard::new(), written, bound);
                    written.entry(target).or_default().push(Write {
                        frame: self.frame.0,
                        span: *span,
                        guard: guard.clone(),
                        call: self.frame.1,
                        top: self.top,
                        fields: copied(target, expr),
                    });
                }
                Stmt::Command { args, .. } => {
                    for a in args {
                        self.reads(a, guard, &Guard::new(), written, bound);
                    }
                }
                Stmt::Send { args, .. } => {
                    for a in args {
                        self.reads(a, guard, &Guard::new(), written, bound);
                    }
                }
                Stmt::Refresh { .. } => {}
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
                    let mut outer = guard.clone();
                    self.block(then, &mut inner, written, bound);
                    self.block(otherwise, &mut outer, &mut other, bound);
                    merge(written, other);
                    guard.retain(|f| inner.contains(f) && outer.contains(f));
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.reads(subject, guard, &Guard::new(), written, bound);
                    let mut other = written.clone();
                    let (mut a, mut b) = (guard.clone(), guard.clone());
                    bound.push(&some.0);
                    self.block(&some.1, &mut a, written, bound);
                    bound.pop();
                    self.block(none, &mut b, &mut other, bound);
                    merge(written, other);
                    guard.retain(|f| a.contains(f) && b.contains(f));
                }
                // A new frame: its parameters' `let`s, its arguments, read
                // in it, then its body.
                Stmt::Call {
                    action, body, span, ..
                } => {
                    self.frames += 1;
                    let outer =
                        std::mem::replace(&mut self.frame, (self.frames, Some((action, *span))));
                    self.block(body, guard, written, bound);
                    self.frame = outer;
                }
            }
        }
        bound.truncate(depth);
    }

    /// Name each slot `e` reads that another frame assigned on a path
    /// here; `tested` is what the condition `e` is part of says.
    fn reads(
        &mut self,
        e: &'a Expr,
        guard: &Guard,
        tested: &Guard,
        written: &Written<'a>,
        bound: &mut Vec<&'a str>,
    ) {
        match e {
            // The right of an `and` runs where its left held, and a
            // ternary's first arm where its test did (`rereads.rs`).
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
                slot_reads(e, &self.slots, bound, &mut found);
                for (slot, at, field) in found {
                    let stale = |w: &&Write<'_>| {
                        w.frame != self.frame.0
                            && !exclusive(&w.guard, guard)
                            && !exclusive(&w.guard, tested)
                            && !field.is_some_and(|f| {
                                w.fields.as_ref().is_some_and(|set| !set.contains(f))
                            })
                    };
                    let Some(w) = written.get(slot).and_then(|ws| ws.iter().find(stale)) else {
                        continue;
                    };
                    if !self.read.insert(at) {
                        continue;
                    }
                    self.errors.push(self.stale(slot, at, w));
                }
            }
        }
    }

    fn stale(&self, slot: &str, at: Span, w: &Write<'_>) -> AnalyzeError {
        let action = shown(self.action);
        let slot = shown(slot);
        // A call's arguments are read in the callee's frame, but on the
        // call's own line: the caller wrote that read.
        let argument = self.frame.1.filter(|(_, call)| call.line == at.line);
        let reader = match (self.frame.1, argument) {
            (None, _) => format!("`{action}`"),
            (_, Some((callee, _))) => format!("the argument of `{}`", shown(callee)),
            (Some((callee, _)), None) => format!("`{}`", shown(callee)),
        };
        let reads = match (self.frame.1, argument) {
            (None, _) => format!("`{action}` reads `{slot}` at line {}", at.line),
            (_, Some((callee, call))) => format!(
                "an argument of `{}(…)` at line {} reads `{slot}`",
                shown(callee),
                call.line
            ),
            (Some((callee, call)), None) => format!(
                "`{}` (called at line {}) reads `{slot}`",
                shown(callee),
                call.line
            ),
        };
        let assigns = match w.call {
            None => format!("`{action}` assigns at line {}", w.span.line),
            Some((callee, call)) => format!(
                "`{}` (called at line {}) assigns at line {}",
                shown(callee),
                call.line,
                w.span.line
            ),
        };
        let mut related = vec![Related {
            span: w.span,
            note: format!("`{slot}` is assigned here"),
        }];
        if let Some((_, call)) = w.call {
            related.push(Related {
                span: call,
                note: made_in(action, w.call, w.span),
            });
        }
        AnalyzeError {
            id: "analyze-read-after-assign",
            message: format!(
                "{reads}, which {assigns}; {reader} sees the value `{slot}` had when the action started: writes land when the action ends. Pass the value it should see: a `let` bound before line {} keeps the starting value, and the value assigned at line {} gives the new one",
                w.top.line, w.span.line
            ),
            span: at,
            related,
        }
    }
}

/// The fields `target = S(target, f=…, g=…)` replaces; `None` for any other
/// value.
pub(super) fn copied<'a>(target: &str, expr: &'a Expr) -> Option<BTreeSet<&'a str>> {
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

/// The union of two arms' writes; one made before the branch keeps only
/// what both arms still know of its path.
fn merge<'a>(into: &mut Written<'a>, from: Written<'a>) {
    for (slot, ws) in from {
        let have = into.entry(slot).or_default();
        for w in ws {
            match have
                .iter_mut()
                .find(|x| x.span == w.span && x.frame == w.frame)
            {
                Some(x) => x.guard.retain(|f| w.guard.contains(f)),
                None => have.push(w),
            }
        }
    }
}

/// The slots `e` reads, outside the binders in force and written in it,
/// each with the field read of it when it is a record's (`s.f`).
pub(super) fn slot_reads<'e>(
    e: &'e Expr,
    slots: &BTreeSet<&str>,
    bound: &mut Vec<&'e str>,
    out: &mut Vec<(&'e str, Span, Option<&'e str>)>,
) {
    match e {
        Expr::Ident(n, span) => {
            if slots.contains(n.as_str()) && !bound.contains(&n.as_str()) {
                out.push((n, *span, None));
            }
        }
        Expr::Member(x, field, _) if matches!(&**x, Expr::Ident(..)) => {
            let at = out.len();
            slot_reads(x, slots, bound, out);
            if let Some(read) = out.get_mut(at) {
                read.2 = Some(field);
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
        // A settled value: the pending map changes after the body runs.
        Expr::Call(name, _, _) if name == "pending" => {}
        Expr::Call(_, args, _) | Expr::List(args, _) => {
            args.iter().for_each(|a| slot_reads(a, slots, bound, out))
        }
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    slot_reads(x, slots, bound, out);
                }
            }
        }
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => slot_reads(x, slots, bound, out),
        Expr::Binary(_, a, b, _) => {
            slot_reads(a, slots, bound, out);
            slot_reads(b, slots, bound, out);
        }
        Expr::Ternary(a, b, c, _) => {
            slot_reads(a, slots, bound, out);
            slot_reads(b, slots, bound, out);
            slot_reads(c, slots, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            slot_reads(subject, slots, bound, out);
            slot_reads(none, slots, bound, out);
            bound.push(var);
            slot_reads(some, slots, bound, out);
            bound.pop();
        }
        Expr::Let {
            name, value, body, ..
        } => {
            slot_reads(value, slots, bound, out);
            bound.push(name);
            slot_reads(body, slots, bound, out);
            bound.pop();
        }
        Expr::Arrow { params, body, .. } => {
            bound.extend(params.iter().map(String::as_str));
            slot_reads(body, slots, bound, out);
            bound.truncate(bound.len() - params.len());
        }
    }
}

/// Every read, in an action of the expanded root, of a slot another frame
/// assigns on a path to it: the one rule's cross-frame case, a warning.
pub(super) fn check(c: &Component) -> Vec<AnalyzeError> {
    let slots: std::rc::Rc<BTreeSet<&str>> = std::rc::Rc::new(
        c.states
            .iter()
            .map(|s| s.name.as_str())
            .chain(c.mutations.iter().map(|m| m.name.as_str()))
            .collect(),
    );
    let mut errors = Vec::new();
    for a in &c.actions {
        if !has_call(&a.body) {
            continue;
        }
        let mut walk = Walk {
            action: &a.name,
            slots: std::rc::Rc::clone(&slots),
            frame: (0, None),
            frames: 0,
            top: a.span,
            read: BTreeSet::new(),
            errors: Vec::new(),
        };
        walk.block(
            &a.body,
            &mut Guard::new(),
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        // A callee expanded in two callers repeats its warning at one read.
        for e in walk.errors {
            if !errors.iter().any(|x: &AnalyzeError| x.span == e.span) {
                errors.push(e);
            }
        }
    }
    errors
}

fn has_call(body: &[Stmt]) -> bool {
    body.iter().any(|s| match s {
        Stmt::Call { .. } => true,
        Stmt::If {
            then, otherwise, ..
        } => has_call(then) || has_call(otherwise),
        Stmt::Match { some, none, .. } => has_call(&some.1) || has_call(none),
        _ => false,
    })
}
