//! Action composition (LLP 1089): an action calls an action of its own
//! component, an `action` prop or an injected action, as a statement, where
//! any statement may stand. A host command keeps its name (D1). A call is
//! its callee's statements expanded in place (D2): one commit, every
//! statement reading the state as the action found it, the writes landing
//! in statement order. LLP 1017 §11's tail call is one case of it.
//!
//! Expansion runs in three steps:
//!
//! 1. [`expand_file`], before any component is checked: a call of the same
//!    component becomes a [`Stmt::Call`] holding the callee's statements,
//!    each in its own component's scope (D7). Its parameters are `let`s of
//!    the call's arguments; every name the callee binds is renamed apart.
//! 2. [`resolve`], after lifting: a prop or inject call, which lifting marked
//!    with the action the prop or the `provide` named ([`MARK`]), becomes the
//!    same, the owner's lifted statements run in the owner's scope.
//! 3. [`hygiene`], after the type pass: a caller's own parameters and
//!    binders are renamed apart, so no callee's read of the component's
//!    names is captured by one of them where lowering puts the callee.
//!    The type pass reads the authored names first, so its refusals of a
//!    `let` speak in them.
//!
//! A call's body is closed over its caller's names: only the arguments,
//! and the parameters' `let`s that bind them, are the caller's expressions.
//! Cycles are refused (D4), and so is a body that grows past [`LIMIT`]
//! statements, as soon as it does.

use super::subst::{subst_stmts, Subst};
use crate::ast::{Action, Component, Expr, File, Stmt};
use crate::parser::SyntaxError;
use crate::{Span, HOST_COMMANDS};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

/// A prop or inject call between lifting and [`resolve`]: the action the
/// prop named, after this prefix (`@` cannot begin an authored name), on a
/// [`Stmt::Call`] with no body yet.
pub const MARK: &str = "@call:";

/// The statements one action may grow to as its calls expand (D4).
pub const LIMIT: usize = 1024;

/// The prefix of a lifted action's capture parameters (`inline.rs`).
const CAPTURE: &str = "@capture:";

/// Every statement node in `body`, the arms' and the calls' included.
pub fn size(body: &[Stmt]) -> usize {
    body.iter()
        .map(|s| {
            1 + match s {
                Stmt::If {
                    then, otherwise, ..
                } => size(then) + size(otherwise),
                Stmt::Match { some, none, .. } => size(&some.1) + size(none),
                Stmt::Call { body, .. } => size(body),
                _ => 0,
            }
        })
        .sum()
}

/// A statement `name(…)` whose name is both a host command and an action,
/// an `action` prop or an injected action of the component it is written
/// in (LLP 1089 D1, as amended 2026-10-05). It is refused at every
/// statement position, whatever its arguments, in a same-named action too:
/// a host command added later must never silently take a call over.
/// Declaring or binding such a name stays legal.
#[derive(Debug, Clone, PartialEq)]
pub struct Ambiguous {
    /// The call statement.
    pub span: Span,
    /// The name it calls.
    pub name: String,
    /// `action`, `action prop` or `injected action`.
    pub what: &'static str,
    /// The component it is written in.
    pub owner: String,
    /// Where that component declares the name.
    pub declared: Span,
}

impl Ambiguous {
    /// The refusal's id.
    pub const ID: &'static str = "syntax-call-ambiguous";

    /// What the refusal says.
    pub fn message(&self) -> String {
        let (name, owner, what) = (&self.name, &self.owner, self.what);
        let rename = match what {
            "action" => "Rename the action and update its bindings",
            "action prop" => "Rename the prop and update its bindings",
            _ => "Rename the inject and the `provide` that fills it",
        };
        format!(
            "`{name}()` names both a host command and {what} `{owner}.{name}`. {rename}. Call the renamed {what} to invoke it; keep `{name}()` to invoke the host command"
        )
    }
}

/// Every ambiguous call statement in `file`'s actions, each checked against
/// the component it is written in, before any expansion or lifting.
pub fn ambiguous(file: &File) -> Vec<Ambiguous> {
    fn walk(body: &[Stmt], c: &Component, out: &mut Vec<Ambiguous>) {
        for s in body {
            match s {
                Stmt::Command { name, span, .. } if HOST_COMMANDS.contains(&name.as_str()) => {
                    let action = |p: &&crate::ast::Param| {
                        p.name == *name && p.ty.as_ref().is_some_and(super::is_action)
                    };
                    let found = c
                        .actions
                        .iter()
                        .find(|a| a.name == *name)
                        .map(|a| ("action", a.span))
                        .or_else(|| c.props.iter().find(action).map(|p| ("action prop", p.span)))
                        .or_else(|| {
                            c.injects
                                .iter()
                                .find(action)
                                .map(|p| ("injected action", p.span))
                        });
                    if let Some((what, declared)) = found {
                        out.push(Ambiguous {
                            span: *span,
                            name: name.clone(),
                            what,
                            owner: c.name.clone(),
                            declared,
                        });
                    }
                }
                Stmt::If {
                    then, otherwise, ..
                } => {
                    walk(then, c, out);
                    walk(otherwise, c, out);
                }
                Stmt::Match { some, none, .. } => {
                    walk(&some.1, c, out);
                    walk(none, c, out);
                }
                Stmt::Call { body, .. } => walk(body, c, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        for a in &c.actions {
            walk(&a.body, c, &mut out);
        }
    }
    out
}

/// Whether a statement `name(…)` in `c`'s actions calls one of them.
fn calls_own(c: &Component, name: &str) -> bool {
    !HOST_COMMANDS.contains(&name) && c.actions.iter().any(|a| a.name == name)
}

/// Whether any statement of `body`, in a call's body too, is a call `want`
/// accepts.
fn any_call(body: &[Stmt], want: &dyn Fn(&Stmt) -> bool) -> bool {
    body.iter().any(|s| {
        want(s)
            || match s {
                Stmt::If {
                    then, otherwise, ..
                } => any_call(then, want) || any_call(otherwise, want),
                Stmt::Match { some, none, .. } => any_call(&some.1, want) || any_call(none, want),
                Stmt::Call { body, .. } => any_call(body, want),
                _ => false,
            }
    })
}

/// The file with every same-component call expanded, each component's in
/// its own scope, and what could not be. Running it again changes nothing.
pub fn expand_file(file: &File) -> (Cow<'_, File>, Vec<SyntaxError>) {
    let records = super::record_constructors(file);
    let mut out = Cow::Borrowed(file);
    let mut errors = Vec::new();
    for (ci, c) in file.components.iter().enumerate() {
        let own = |s: &Stmt| matches!(s, Stmt::Command { name, .. } if calls_own(c, name));
        if !c.actions.iter().any(|a| any_call(&a.body, &own)) {
            continue;
        }
        let mut cx = Cx {
            actions: &c.actions,
            component: Some(c),
            records: &records,
            tags: ('s', 'v'),
            path: Vec::new(),
            fresh: 0,
            size: 0,
            over: false,
            errors: Vec::new(),
        };
        for (ai, action) in c.actions.iter().enumerate() {
            if !any_call(&action.body, &own) {
                continue;
            }
            let body = cx.action(action);
            out.to_mut().components[ci].actions[ai].body = body;
        }
        errors.extend(cx.errors);
    }
    dedup(&mut errors);
    (out, errors)
}

/// Resolve every marked prop and inject call in the lifted `actions`, in
/// place: the owner's statements, run where the call stands.
pub(super) fn resolve(
    actions: &mut [Action],
    records: &BTreeSet<String>,
    errors: &mut Vec<SyntaxError>,
) {
    let marked = |s: &Stmt| matches!(s, Stmt::Call { action, .. } if action.starts_with(MARK));
    if !actions.iter().any(|a| any_call(&a.body, &marked)) {
        return;
    }
    let originals: Vec<Action> = actions.to_vec();
    let mut cx = Cx {
        actions: &originals,
        component: None,
        records,
        tags: ('t', 'u'),
        path: Vec::new(),
        fresh: 0,
        size: 0,
        over: false,
        errors: Vec::new(),
    };
    for action in actions.iter_mut() {
        if any_call(&action.body, &marked) {
            action.body = cx.action(action);
        }
    }
    let mut found = cx.errors;
    dedup(&mut found);
    errors.extend(found);
}

/// Rename every caller's own parameters and binders apart (`name@c{k}`,
/// `name@b{k}`; expansion's are `@s`/`@v` and `@t`/`@u`), the arguments and the parameters' `let`s of its calls with
/// them. A callee reads the component's names in the caller's block, where
/// lowering puts it; renamed, no parameter, `let` or `match` binding of the
/// caller spells one of them.
///
/// `k` is drawn as LLP 1017 §11's tail call drew it, a caller's statements
/// and then each callee's, so a plan built before calls existed keeps its
/// parameters' names, and its bytes (LLP 1089 D8).
pub fn hygiene(actions: &mut [Action], records: &BTreeSet<String>) {
    let mut fresh = 0;
    for action in actions.iter_mut() {
        if !any_call(&action.body, &|s| matches!(s, Stmt::Call { .. })) {
            continue;
        }
        fresh += 1;
        let k = fresh;
        let mut map = BTreeMap::new();
        for p in action.params.iter_mut().filter(|p| !p.name.contains('@')) {
            let renamed = format!("{}@c{k}", p.name);
            map.insert(p.name.clone(), Expr::Ident(renamed.clone(), p.span));
            p.name = renamed;
        }
        let body = subst_stmts(
            &action.body,
            &mut Subst::values(&map, records),
            &BTreeMap::new(),
        );
        action.body = apart(&body, 'b', records, &mut fresh);
        fresh += drawn(&action.body);
    }
}

/// The names LLP 1017 §11's resolver drew for the calls in `body`: one for
/// each call's parameters, one for each binder of its body, and its own
/// calls' in turn.
fn drawn(body: &[Stmt]) -> u32 {
    fn binders(body: &[Stmt]) -> u32 {
        body.iter()
            .map(|s| match s {
                Stmt::Let { .. } => 1,
                Stmt::If {
                    then, otherwise, ..
                } => binders(then) + binders(otherwise),
                Stmt::Match { some, none, .. } => 1 + binders(&some.1) + binders(none),
                _ => 0,
            })
            .sum()
    }
    body.iter()
        .map(|s| match s {
            Stmt::If {
                then, otherwise, ..
            } => drawn(then) + drawn(otherwise),
            Stmt::Match { some, none, .. } => drawn(&some.1) + drawn(none),
            Stmt::Call { args, body, .. } => {
                let own = &body[args.len().min(body.len())..];
                1 + binders(own) + drawn(own)
            }
            _ => 0,
        })
        .sum()
}

/// Every `let` and `match` binding in `body` renamed to a name no author
/// can write, its reads with it; a call's body is its own and is left as it
/// is, its arguments and parameters' `let`s renamed with the rest.
fn apart(body: &[Stmt], tag: char, records: &BTreeSet<String>, fresh: &mut u32) -> Vec<Stmt> {
    let mut out = Vec::new();
    let mut rest = body.to_vec();
    while !rest.is_empty() {
        match rest.remove(0) {
            Stmt::Let { name, expr, span } => {
                *fresh += 1;
                let renamed = format!("{name}@{tag}{fresh}");
                let map = BTreeMap::from([(name, Expr::Ident(renamed.clone(), span))]);
                rest = subst_stmts(&rest, &mut Subst::values(&map, records), &BTreeMap::new());
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
                then: apart(&then, tag, records, fresh),
                otherwise: apart(&otherwise, tag, records, fresh),
                span,
            }),
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => {
                *fresh += 1;
                let renamed = format!("{}@{tag}{fresh}", some.0);
                let map = BTreeMap::from([(some.0.clone(), Expr::Ident(renamed.clone(), span))]);
                let body =
                    subst_stmts(&some.1, &mut Subst::values(&map, records), &BTreeMap::new());
                out.push(Stmt::Match {
                    subject,
                    some: (renamed, apart(&body, tag, records, fresh)),
                    none: apart(&none, tag, records, fresh),
                    span,
                });
            }
            other => out.push(other),
        }
    }
    out
}

/// Keep one refusal per place: a cycle met from each action on it is one
/// cycle.
fn dedup(errors: &mut Vec<SyntaxError>) {
    let mut seen = BTreeSet::new();
    errors.retain(|e| seen.insert((e.span, e.id, e.message.clone())));
}

struct Cx<'a> {
    /// The actions a call may name: the component's (step 1), or the
    /// expanded root's (step 2).
    actions: &'a [Action],
    /// Step 1's component: a statement naming one of its actions is a call.
    /// Step 2 expands only the calls lifting marked.
    component: Option<&'a Component>,
    records: &'a BTreeSet<String>,
    /// The letters a callee's parameters and binders are renamed with:
    /// each step its own, so a name one step draws never spells another's.
    tags: (char, char),
    /// The actions on the way to this call, the caller first.
    path: Vec<String>,
    fresh: u32,
    /// The statements the action being expanded holds so far.
    size: usize,
    /// It passed [`LIMIT`]: no further call of it expands.
    over: bool,
    errors: Vec<SyntaxError>,
}

impl Cx<'_> {
    /// `action`'s body with its calls expanded.
    fn action(&mut self, action: &Action) -> Vec<Stmt> {
        self.path = vec![action.name.clone()];
        self.size = size(&action.body);
        self.over = false;
        self.block(&action.body)
    }

    fn refuse(&mut self, id: &'static str, message: String, span: Span) {
        self.errors.push(SyntaxError { id, message, span });
    }

    /// A block with every call in it expanded, at every position: a call's
    /// own body too, where step 2 finds the prop calls of a callee of the
    /// same component.
    fn block(&mut self, body: &[Stmt]) -> Vec<Stmt> {
        body.iter().map(|s| self.stmt(s)).collect()
    }

    fn stmt(&mut self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::If {
                cond,
                then,
                otherwise,
                span,
            } => Stmt::If {
                cond: cond.clone(),
                then: self.block(then),
                otherwise: self.block(otherwise),
                span: *span,
            },
            Stmt::Match {
                subject,
                some,
                none,
                span,
            } => Stmt::Match {
                subject: subject.clone(),
                some: (some.0.clone(), self.block(&some.1)),
                none: self.block(none),
                span: *span,
            },
            Stmt::Command { name, args, span }
                if self.component.is_some_and(|c| calls_own(c, name)) =>
            {
                self.call(name, args.clone(), args.len(), None, *span)
            }
            Stmt::Call {
                action,
                args,
                authored,
                binding,
                span,
                ..
            } if action.starts_with(MARK) => self.call(
                &action[MARK.len()..],
                args.clone(),
                *authored,
                *binding,
                *span,
            ),
            Stmt::Call {
                action,
                args,
                body,
                authored,
                curried,
                binding,
                span,
            } if self.component.is_none() => {
                self.path.push(action.clone());
                let body = self.block(body);
                self.path.pop();
                Stmt::Call {
                    action: action.clone(),
                    args: args.clone(),
                    body,
                    authored: *authored,
                    curried: *curried,
                    binding: *binding,
                    span: *span,
                }
            }
            other => other.clone(),
        }
    }

    /// The call of `target` with `args` (`authored` of them the call's own,
    /// the rest held where the action was passed): the callee's parameters
    /// as `let`s of the arguments, then its statements, every name it binds
    /// renamed apart and its own calls expanded in turn. A call that cannot
    /// be expanded keeps no body: its refusal is made here, or by the type
    /// pass from its counts.
    fn call(
        &mut self,
        target: &str,
        args: Vec<Expr>,
        authored: usize,
        binding: Option<Span>,
        span: Span,
    ) -> Stmt {
        let actions = self.actions;
        let callee = actions.iter().find(|a| a.name == target);
        let captures = callee.map_or(0, |c| {
            c.params
                .iter()
                .take_while(|p| p.name.starts_with(CAPTURE))
                .count()
        });
        let curried = (args.len() - authored).saturating_sub(captures);
        let body = self.body(callee, target, &args, span);
        Stmt::Call {
            action: target.to_string(),
            args,
            body,
            authored,
            curried,
            binding,
            span,
        }
    }

    fn body(
        &mut self,
        callee: Option<&Action>,
        target: &str,
        args: &[Expr],
        span: Span,
    ) -> Vec<Stmt> {
        let Some(callee) = callee else {
            self.refuse(
                "syntax-call-target",
                format!("`{target}` is not an action"),
                span,
            );
            return Vec::new();
        };
        if self.over {
            return Vec::new();
        }
        if self.path.iter().any(|p| p == target) {
            let mut way: Vec<&str> = self.path.iter().map(|p| shown(p)).collect();
            way.push(shown(target));
            let message = format!(
                "calling `{}` comes back to an action already on the way: {}; an action never calls itself, directly or through others",
                shown(target),
                way.join(" → ")
            );
            self.refuse("syntax-call-cycle", message, span);
            return Vec::new();
        }
        // An arity the type pass refuses (`type-call-arity`), from the counts.
        if args.len() != callee.params.len() {
            return Vec::new();
        }
        self.size += callee.params.len() + size(&callee.body);
        if self.size > LIMIT {
            self.over = true;
            let message = format!(
                "`{}` grows past {LIMIT} statements as its calls expand, at this call of `{}`: an action and every action it calls are one body; call fewer, or move the shared work into one action",
                shown(&self.path[0]),
                shown(target)
            );
            self.refuse("syntax-call-size", message, span);
            return Vec::new();
        }
        self.fresh += 1;
        let k = self.fresh;
        let tag = self.tags.0;
        let renamed = |p: &str| format!("{p}@{tag}{k}");
        let map: BTreeMap<String, Expr> = callee
            .params
            .iter()
            .map(|p| (p.name.clone(), Expr::Ident(renamed(&p.name), p.span)))
            .collect();
        let own = subst_stmts(
            &callee.body,
            &mut Subst::values(&map, self.records),
            &BTreeMap::new(),
        );
        let own = apart(&own, self.tags.1, self.records, &mut self.fresh);
        self.path.push(target.to_string());
        let inner = self.block(&own);
        self.path.pop();
        let mut body: Vec<Stmt> = callee
            .params
            .iter()
            .zip(args)
            .map(|(p, arg)| Stmt::Let {
                name: renamed(&p.name),
                expr: arg.clone(),
                span,
            })
            .collect();
        body.extend(inner);
        body
    }
}

/// An action's name as its author wrote it: a lifted one's without its
/// use's number.
pub fn shown(name: &str) -> &str {
    name.split('#').next().unwrap_or(name)
}
