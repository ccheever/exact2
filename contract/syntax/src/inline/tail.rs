//! The tail call (LLP 1017 §11; Charlie, 2026-10-03): a child's action may
//! end by calling one of its `action` props, as `close()` ends a viewer's
//! release, also as the last statement of the branches of a last `if` or
//! `match`. Inlining marks the call with the root action the prop named
//! (`@tail:<action>`, the prop's curried arguments first); this pass puts
//! that action's statements in its place. Its parameters become `let`s of
//! the call's arguments, so the call is exactly the callee's statements run
//! last in the caller: one commit, every statement reading the state as the
//! action found it, the callee's writes after the caller's.
//!
//! Hygiene: every name the caller binds (its parameters, its `let`s and
//! `match` bindings) and every parameter and binder of a callee is renamed
//! apart (`@` cannot begin an authored name), so an inlined statement reads
//! the root's state, never a caller's local of the same spelling, and a
//! chain's inner action never reads an outer one's parameter. Each call
//! keeps an `@check:<action>` statement with its arguments, which the type
//! pass holds to the callee's parameter types and lowering drops.

use super::subst::{subst_stmts, Subst};
use super::TAIL;
use crate::ast::{Action, Expr, Stmt};
use crate::parser::SyntaxError;
use crate::Span;
use std::collections::{BTreeMap, BTreeSet};

/// The statement a tail call leaves for the type pass: the callee's name
/// after this prefix, the call's arguments.
pub const CHECK: &str = "@check:";

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

fn marked(body: &[Stmt]) -> bool {
    tail_positions(body)
        .iter()
        .any(|s| matches!(s, Stmt::Command { name, .. } if name.starts_with(TAIL)))
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
    for action in actions.iter_mut().filter(|a| marked(&a.body)) {
        cx.path = vec![action.name.clone()];
        // The caller's parameters and binders, renamed apart.
        cx.fresh += 1;
        let k = cx.fresh;
        let map: BTreeMap<String, Expr> = action
            .params
            .iter()
            .filter(|p| !p.name.starts_with('@'))
            .map(|p| {
                (
                    p.name.clone(),
                    Expr::Ident(format!("{}@c{k}", p.name), p.span),
                )
            })
            .collect();
        for p in action
            .params
            .iter_mut()
            .filter(|p| !p.name.starts_with('@'))
        {
            p.name = format!("{}@c{k}", p.name);
        }
        let body = subst_stmts(
            &action.body,
            &mut Subst::new(&map, records),
            &BTreeMap::new(),
        );
        let body = cx.apart(&body);
        match cx.block(&body) {
            Ok(body) => action.body = body,
            Err(e) => errors.push(e),
        }
    }
}

impl Cx<'_> {
    /// Every `let` and `match` binding in `body` renamed to a name no author
    /// can write, its reads with it.
    fn apart(&mut self, body: &[Stmt]) -> Vec<Stmt> {
        let mut out = Vec::new();
        let mut rest = body.to_vec();
        while !rest.is_empty() {
            let stmt = rest.remove(0);
            match stmt {
                Stmt::Let { name, expr, span } => {
                    self.fresh += 1;
                    let renamed = format!("{name}@b{}", self.fresh);
                    let map = BTreeMap::from([(name, Expr::Ident(renamed.clone(), span))]);
                    rest =
                        subst_stmts(&rest, &mut Subst::new(&map, self.records), &BTreeMap::new());
                    out.push(Stmt::Let {
                        name: renamed,
                        expr,
                        span,
                    });
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    span,
                } => out.push(Stmt::If {
                    cond,
                    then: self.apart(&then),
                    otherwise: self.apart(&otherwise),
                    span,
                }),
                Stmt::Match {
                    subject,
                    some,
                    none,
                    span,
                } => {
                    self.fresh += 1;
                    let renamed = format!("{}@b{}", some.0, self.fresh);
                    let map =
                        BTreeMap::from([(some.0.clone(), Expr::Ident(renamed.clone(), span))]);
                    let body = subst_stmts(
                        &some.1,
                        &mut Subst::new(&map, self.records),
                        &BTreeMap::new(),
                    );
                    out.push(Stmt::Match {
                        subject,
                        some: (renamed, self.apart(&body)),
                        none: self.apart(&none),
                        span,
                    });
                }
                other => out.push(other),
            }
        }
        out
    }

    /// A block with its tail position resolved.
    fn block(&mut self, body: &[Stmt]) -> Result<Vec<Stmt>, SyntaxError> {
        let Some(last) = body.last() else {
            return Ok(Vec::new());
        };
        let mut out = body[..body.len() - 1].to_vec();
        match last {
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => out.push(Stmt::If {
                cond: cond.clone(),
                then: self.block(then)?,
                otherwise: self.block(otherwise)?,
                span: *span,
            }),
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => out.push(Stmt::Match {
                subject: subject.clone(),
                some: (some.0.clone(), self.block(&some.1)?),
                none: self.block(none)?,
                span: *span,
            }),
            Stmt::Command { name, args, span } if name.starts_with(TAIL) => {
                out.extend(self.call(&name[TAIL.len()..], args, *span)?);
            }
            other => out.push(other.clone()),
        }
        Ok(out)
    }

    /// The callee's statements for a marked call: the check, `let`s of its
    /// arguments, then its body, its own names renamed apart first and its
    /// own tail calls resolved after.
    fn call(&mut self, target: &str, args: &[Expr], span: Span) -> Result<Vec<Stmt>, SyntaxError> {
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
        self.fresh += 1;
        let k = self.fresh;
        let renamed = |p: &str| format!("{p}@tail{k}");
        let map: BTreeMap<String, Expr> = callee
            .params
            .iter()
            .map(|p| (p.name.clone(), Expr::Ident(renamed(&p.name), p.span)))
            .collect();
        let own = subst_stmts(
            &callee.body,
            &mut Subst::new(&map, self.records),
            &BTreeMap::new(),
        );
        let own = self.apart(&own);
        self.path.push(target.to_string());
        let inner = self.block(&own);
        self.path.pop();
        let mut out = vec![Stmt::Command {
            name: format!("{CHECK}{target}"),
            args: args.to_vec(),
            span,
        }];
        out.extend(callee.params.iter().zip(args).map(|(p, arg)| Stmt::Let {
            name: renamed(&p.name),
            expr: arg.clone(),
            span,
        }));
        out.extend(inner?);
        Ok(out)
    }
}
