//! The tail call (LLP 1017 P4c; Charlie, 2026-10-03): a child's action may
//! end by calling one of its `action` props, as `close()` ends a viewer's
//! release, also as the last statement of an `if` or `match` branch that is
//! itself last. Inlining marks the call with the root action the prop named
//! (`@tail:<action>`, the prop's curried arguments first); this pass puts
//! that action's statements in its place. Its parameters become `let`s of
//! the call's arguments, so the call is exactly the callee's statements run
//! last in the caller: one commit, every statement reading the state as the
//! action found it, the callee's writes after the caller's. No new opcode
//! and no runtime change. A cycle is refused, and so is a caller `let` that
//! would hide a name the callee reads.

use super::subst::{stmt_occurs, subst_stmts, Subst};
use super::TAIL;
use crate::ast::{Action, Expr, Stmt};
use crate::parser::SyntaxError;
use crate::Span;
use std::collections::{BTreeMap, BTreeSet};

/// The statements in tail position: the last one, or the last ones of the
/// branches of a last `if` or `match`.
pub fn tail_positions(body: &[Stmt]) -> Vec<&Stmt> {
    match body.last() {
        Some(Stmt::If {
            then, otherwise, ..
        }) => {
            let mut out = tail_positions(then);
            out.extend(tail_positions(otherwise));
            out
        }
        Some(Stmt::Match { some, none, .. }) => {
            let mut out = tail_positions(&some.1);
            out.extend(tail_positions(none));
            out
        }
        Some(last) => vec![last],
        None => Vec::new(),
    }
}

struct Cx<'a> {
    actions: &'a [Action],
    records: &'a BTreeSet<String>,
    path: Vec<String>,
    fresh: u32,
}

/// Resolve every marked tail call in `actions`, in place.
pub(super) fn resolve(
    actions: &mut [Action],
    records: &BTreeSet<String>,
    errors: &mut Vec<SyntaxError>,
) {
    let originals: Vec<Action> = actions.to_vec();
    let mut cx = Cx {
        actions: &originals,
        records,
        path: Vec::new(),
        fresh: 0,
    };
    for action in actions.iter_mut() {
        cx.path = vec![action.name.clone()];
        match cx.block(&action.body, &[]) {
            Ok(body) => action.body = body,
            Err(e) => errors.push(e),
        }
    }
}

impl Cx<'_> {
    /// A block with its tail position resolved; `outer` are the names bound
    /// around it (the caller's `let`s and `match` bindings in force).
    fn block(&mut self, body: &[Stmt], outer: &[String]) -> Result<Vec<Stmt>, SyntaxError> {
        let Some(last) = body.last() else {
            return Ok(Vec::new());
        };
        let mut bound = outer.to_vec();
        bound.extend(body.iter().filter_map(|s| match s {
            Stmt::Let { name, .. } => Some(name.clone()),
            _ => None,
        }));
        let mut out = body[..body.len() - 1].to_vec();
        match last {
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => out.push(Stmt::If {
                cond: cond.clone(),
                then: self.block(then, &bound)?,
                otherwise: self.block(otherwise, &bound)?,
                span: *span,
            }),
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => {
                let mut inner = bound.clone();
                inner.push(some.0.clone());
                out.push(Stmt::Match {
                    subject: subject.clone(),
                    some: (some.0.clone(), self.block(&some.1, &inner)?),
                    none: self.block(none, &bound)?,
                    span: *span,
                });
            }
            Stmt::Command { name, args, span } if name.starts_with(TAIL) => {
                out.extend(self.call(&name[TAIL.len()..], args, *span, &bound)?);
            }
            other => out.push(other.clone()),
        }
        Ok(out)
    }

    /// The callee's statements for a marked call: `let`s of its arguments,
    /// then its body, renamed apart.
    fn call(
        &mut self,
        target: &str,
        args: &[Expr],
        span: Span,
        bound: &[String],
    ) -> Result<Vec<Stmt>, SyntaxError> {
        let refuse = |id, message: String| Err(SyntaxError { id, message, span });
        let actions = self.actions;
        let Some(callee) = actions.iter().find(|a| a.name == target) else {
            return refuse("syntax-tail-call", format!("`{target}` is not an action"));
        };
        if self.path.iter().any(|p| p == target) {
            return refuse("syntax-tail-cycle", format!("calling `{target}` last comes back to an action already on the way: {} → {target}", self.path.join(" → ")));
        }
        if args.len() != callee.params.len() {
            return refuse(
                "syntax-tail-call",
                format!(
                    "`{target}` takes {} argument(s) here, given {}",
                    callee.params.len(),
                    args.len()
                ),
            );
        }
        self.path.push(target.to_string());
        let inner = self.block(&callee.body, &[]);
        self.path.pop();
        let inner = inner?;
        self.fresh += 1;
        let k = self.fresh;
        let renamed = |p: &str| format!("{p}@tail{k}");
        let map: BTreeMap<String, Expr> = callee
            .params
            .iter()
            .map(|p| (p.name.clone(), Expr::Ident(renamed(&p.name), p.span)))
            .collect();
        let inlined = subst_stmts(
            &inner,
            &mut Subst::new(&map, self.records),
            &BTreeMap::new(),
        );
        if let Some(n) = bound
            .iter()
            .find(|n| inlined.iter().any(|s| stmt_occurs(s, n)))
        {
            return refuse("syntax-tail-capture", format!("the local `{n}` here hides the `{n}` that `{target}` reads; give the local another name"));
        }
        let mut out: Vec<Stmt> = callee
            .params
            .iter()
            .zip(args)
            .map(|(p, arg)| Stmt::Let {
                name: renamed(&p.name),
                expr: arg.clone(),
                span,
            })
            .collect();
        out.extend(inlined);
        Ok(out)
    }
}
