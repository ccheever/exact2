//! @ref LLP 1088 D8 (flashcards F4) — a mutation keeps one reply: a second
//! send forgets the first's in-flight reply (LLP 1016 D5), so two sends to
//! one mutation on one path through an action are refused, not warned of.
//!
//! It runs on the root's actions with every call expanded (a caller and
//! its callees are one commit, LLP 1089 D6), and names the calls a send is
//! made through. The walk is path-sensitive, over the body's statements (`Action::effects`
//! flattens branches): each `if`/`match` arm starts from what the paths
//! into it may have sent, and the branch leaves their union, so exclusive
//! arms pass while an arm's send followed by another after the branch is
//! refused. Sequential `if`s whose conditions cannot both hold are
//! exclusive too — a key handler's `if k == "1" …` then `if k == "2" …` —
//! when each tests one name against literals (`k == "m" or k == "M"`, alone
//! or as a conjunct of an `and`), the literals differ, and nothing assigns
//! or sends that name between them.
//!
//! A `queue` mutation is exempt (LLP 1092 D6): its sends wait in order and
//! each reply reaches its `then`.

use super::{AnalyzeError, Related};
use contract_syntax::{BinOp, Component, Expr, Span, Stmt};
use std::collections::{BTreeMap, BTreeSet};

/// What a path's enclosing conditions say: each tested name is one of these
/// literals.
pub(super) type Guard = Vec<(String, BTreeSet<String>)>;

/// Two guards no one evaluation can satisfy both of.
pub(super) fn exclusive(a: &Guard, b: &Guard) -> bool {
    a.iter().any(|(name, values)| {
        b.iter()
            .any(|(other, given)| name == other && values.is_disjoint(given))
    })
}

/// What `cond` being true says of the names it tests.
pub(super) fn facts(cond: &Expr) -> Guard {
    fn path(e: &Expr) -> Option<String> {
        match e {
            Expr::Ident(n, _) => Some(n.clone()),
            Expr::Member(inner, field, _) => Some(format!("{}.{field}", path(inner)?)),
            _ => None,
        }
    }
    fn literal(e: &Expr) -> Option<String> {
        match e {
            Expr::Str(s, _) => Some(format!("{s:?}")),
            Expr::Number(n, _) => Some(n.to_string()),
            Expr::Bool(b, _) => Some(b.to_string()),
            _ => None,
        }
    }
    // `name ∈ literals`, from `==` and `or`.
    fn one_of(e: &Expr) -> Option<(String, BTreeSet<String>)> {
        match e {
            Expr::Binary(BinOp::Eq, a, b, _) => {
                let (p, l) = match (path(a), literal(b)) {
                    (Some(p), Some(l)) => (p, l),
                    _ => (path(b)?, literal(a)?),
                };
                Some((p, BTreeSet::from([l])))
            }
            Expr::Binary(BinOp::Or, a, b, _) => {
                let (pa, mut la) = one_of(a)?;
                let (pb, lb) = one_of(b)?;
                (pa == pb).then(|| {
                    la.extend(lb);
                    (pa, la)
                })
            }
            _ => None,
        }
    }
    let mut out = Guard::new();
    match cond {
        Expr::Binary(BinOp::And, a, b, _) => {
            out.extend(facts(a));
            out.extend(facts(b));
        }
        _ => out.extend(one_of(cond)),
    }
    out
}

/// The call a statement is made in: the callee and the call's span, or
/// the action's own body.
pub(super) type Frame<'a> = Option<(&'a str, Span)>;

/// A send: where, the guard it was made under, the call it is made in, and
/// whether an `if` or `match` arm encloses it.
type Sent<'a> = BTreeMap<&'a str, Vec<(Span, Guard, Frame<'a>, bool)>>;

/// Whether a fact is about `name`, or a member of it.
pub(super) fn about(name: &str) -> impl Fn(&(String, BTreeSet<String>)) -> bool + '_ {
    move |(tested, _)| tested.split('.').next() == Some(name)
}

/// Forget what conditions said of `name`, now that it changed.
fn changed(name: &str, guard: &mut Guard, sent: &mut Sent<'_>) {
    guard.retain(|f| !about(name)(f));
    for sends in sent.values_mut() {
        for (_, g, _, _) in sends.iter_mut() {
            g.retain(|f| !about(name)(f));
        }
    }
}

/// Where a send or an assignment is made, for a refusal: in the action's
/// own body, or in the call it is made through.
pub(super) fn made_in(action: &str, frame: Frame<'_>, at: Span) -> String {
    match frame {
        None => format!("in `{action}` at line {}", at.line),
        Some((callee, call)) => format!(
            "in `{}` (called at line {})",
            contract_syntax::inline::calls::shown(callee),
            call.line
        ),
    }
}

struct Walk<'a> {
    c: &'a Component,
    action: &'a str,
    /// The call being walked, innermost.
    frame: Frame<'a>,
    /// The `if` and `match` arms around the statement being walked.
    arms: usize,
    /// The slots the action assigns anywhere, its calls included.
    assigns: BTreeSet<&'a str>,
    errors: Vec<AnalyzeError>,
}

/// Every name `stmts` assigns, on any path, through calls too.
fn assigned<'a>(stmts: &'a [Stmt], into: &mut BTreeSet<&'a str>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, .. } => {
                into.insert(target);
            }
            Stmt::Call { body, .. } => assigned(body, into),
            Stmt::If {
                then, otherwise, ..
            } => {
                assigned(then, into);
                assigned(otherwise, into);
            }
            Stmt::Match { some, none, .. } => {
                assigned(&some.1, into);
                assigned(none, into);
            }
            Stmt::Send { .. } | Stmt::Let { .. } | Stmt::Command { .. } | Stmt::Refresh { .. } => {}
        }
    }
}

impl<'a> Walk<'a> {
    /// The block's statements on every path, from `guard` and what was sent
    /// before it; `guard` leaves holding what still holds after the block.
    fn block(&mut self, stmts: &'a [Stmt], guard: &mut Guard, sent: &mut Sent<'a>) {
        for stmt in stmts {
            match stmt {
                Stmt::Send { target, span, .. } => {
                    let mutation = self.c.mutations.iter().find(|m| &m.name == target);
                    // @ref LLP 1092 D2 — a queue's sends each wait their
                    // turn: none forgets another's reply.
                    if mutation.is_some_and(|m| m.queue) {
                        continue;
                    }
                    let earlier = sent.entry(target).or_default();
                    if let Some((first, _, frame, armed)) =
                        earlier.iter().find(|(_, g, _, _)| !exclusive(g, guard))
                    {
                        let reaches = match mutation {
                            Some(m) => match &m.then {
                                Some((then, _)) => format!("`then {then}`"),
                                None => format!("`{target}`"),
                            },
                            None => format!("`{target}`"),
                        };
                        let action = contract_syntax::inline::calls::shown(self.action);
                        // The fix names `queue` (LLP 1092 D6) unless the
                        // action also assigns the slot: a reply it means to
                        // drop, which a queue would land anyway (D4).
                        let fix = if self.assigns.contains(target.as_str()) {
                            "send once, or use a mutation per request".to_string()
                        } else {
                            format!("send once, use a mutation per request, or declare `mutation {target} … queue` to run both in order")
                        };
                        // Through a call, the refusal names the calls (LLP
                        // 1089 D6): neither body shows both sends.
                        // A send in an arm: say why both can run (chess #2).
                        let message = if *armed || self.arms > 0 {
                            let both = if frame.is_none() && self.frame.is_none() {
                                format!("at lines {} and {}", first.line, span.line)
                            } else {
                                format!(
                                    "{} and {}",
                                    made_in(action, *frame, *first),
                                    made_in(action, self.frame, *span)
                                )
                            };
                            let fix = if self.assigns.contains(target.as_str()) {
                                "send once, make the sends arms of one `if … else if`, or use a mutation per request".to_string()
                            } else {
                                format!("send once, make the sends arms of one `if … else if`, use a mutation per request, or declare `mutation {target} … queue` to run both in order")
                            };
                            format!(
                                "`{action}` sends `{target}` twice on one path, {both}: every `if` is read as one that can run, unless the two sends are arms of one `if`/`else` or `match`, or sit in `if`s testing one unchanged name against different literals. Only the last send's reply reaches {reaches} (LLP 1016 D5): {fix}"
                            )
                        } else if frame.is_none() && self.frame.is_none() {
                            let mut fix = fix;
                            fix[..1].make_ascii_uppercase();
                            format!(
                                "`{action}` sends `{target}` twice; only the last send's reply reaches {reaches} (LLP 1016 D5). {fix}"
                            )
                        } else {
                            format!(
                                "`{action}` sends `{target}` twice on one path: {} and {}. Only the last send's reply reaches {reaches} (LLP 1016 D5): {fix}",
                                made_in(action, *frame, *first),
                                made_in(action, self.frame, *span)
                            )
                        };
                        self.errors.push(AnalyzeError {
                            id: "analyze-send-twice",
                            message,
                            span: *span,
                            related: vec![Related {
                                span: *first,
                                note: format!("`{target}` is sent here first"),
                            }],
                        });
                    }
                    earlier.push((*span, guard.clone(), self.frame, self.arms > 0));
                    changed(target, guard, sent);
                }
                Stmt::Call {
                    action, body, span, ..
                } => {
                    let outer = self.frame.replace((action, *span));
                    self.block(body, guard, sent);
                    self.frame = outer;
                }
                Stmt::Assign { target, .. } => changed(target, guard, sent),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    let mut inner = guard.clone();
                    inner.extend(facts(cond));
                    let mut other = sent.clone();
                    let mut outer = guard.clone();
                    self.arms += 1;
                    self.block(then, &mut inner, sent);
                    self.block(otherwise, &mut outer, &mut other);
                    self.arms -= 1;
                    merge(sent, other);
                    // What an arm changed no longer holds after the branch.
                    guard.retain(|f| inner.contains(f) && outer.contains(f));
                }
                Stmt::Match { some, none, .. } => {
                    let mut other = sent.clone();
                    let (mut a, mut b) = (guard.clone(), guard.clone());
                    self.arms += 1;
                    self.block(&some.1, &mut a, sent);
                    self.block(none, &mut b, &mut other);
                    self.arms -= 1;
                    merge(sent, other);
                    guard.retain(|f| a.contains(f) && b.contains(f));
                }
                Stmt::Let { .. } | Stmt::Command { .. } | Stmt::Refresh { .. } => {}
            }
        }
    }
}

/// The union of two arms' sends. One source and innermost call can be reached
/// under different outer guards; keep those alternatives, including any that
/// lost facts when an arm changed a name.
fn merge<'a>(into: &mut Sent<'a>, from: Sent<'a>) {
    for (target, sends) in from {
        let have = into.entry(target).or_default();
        for (span, guard, frame, armed) in sends {
            match have
                .iter_mut()
                .find(|(s, g, f, _)| *s == span && *g == guard && *f == frame)
            {
                Some((_, _, _, kept_armed)) => *kept_armed |= armed,
                None => have.push((span, guard, frame, armed)),
            }
        }
    }
}

/// Every action's second send to one mutation on one path.
pub(super) fn check(c: &Component) -> Vec<AnalyzeError> {
    let mut errors = Vec::new();
    for a in &c.actions {
        let mut assigns = BTreeSet::new();
        assigned(&a.body, &mut assigns);
        let mut walk = Walk {
            c,
            action: &a.name,
            frame: None,
            arms: 0,
            assigns,
            errors: Vec::new(),
        };
        walk.block(&a.body, &mut Guard::new(), &mut BTreeMap::new());
        // A callee expanded at a call repeats its own refusal there.
        for e in walk.errors {
            if !errors.iter().any(|x: &AnalyzeError| x.span == e.span) {
                errors.push(e);
            }
        }
    }
    errors
}

/// A send or an assignment of a mutation on some path: where, the guard it
/// was made under, and the call it is made in.
type Made<'a> = BTreeMap<&'a str, Vec<(Span, Guard, Frame<'a>)>>;

struct Both<'a> {
    c: &'a Component,
    action: &'a str,
    frame: Frame<'a>,
    /// One warning per mutation an action.
    said: BTreeSet<&'a str>,
    out: Vec<AnalyzeError>,
}

impl<'a> Both<'a> {
    fn block(
        &mut self,
        stmts: &'a [Stmt],
        guard: &mut Guard,
        sent: &mut Made<'a>,
        assigned: &mut Made<'a>,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::Send { target, span, .. } => {
                    if self.nonqueued(target) {
                        self.meet(target, *span, guard, assigned, "sends", "assigns");
                        sent.entry(target)
                            .or_default()
                            .push((*span, guard.clone(), self.frame));
                    }
                    moved(target, guard, sent);
                    moved(target, guard, assigned);
                }
                Stmt::Assign { target, span, .. } => {
                    if self.nonqueued(target) {
                        self.meet(target, *span, guard, sent, "assigns", "sends");
                        assigned.entry(target).or_default().push((
                            *span,
                            guard.clone(),
                            self.frame,
                        ));
                    }
                    moved(target, guard, sent);
                    moved(target, guard, assigned);
                }
                Stmt::Call {
                    action, body, span, ..
                } => {
                    let outer = self.frame.replace((action, *span));
                    self.block(body, guard, sent, assigned);
                    self.frame = outer;
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    let mut inner = guard.clone();
                    inner.extend(facts(cond));
                    let (mut other_sent, mut other_assigned) = (sent.clone(), assigned.clone());
                    let mut outer = guard.clone();
                    self.block(then, &mut inner, sent, assigned);
                    self.block(otherwise, &mut outer, &mut other_sent, &mut other_assigned);
                    union(sent, other_sent);
                    union(assigned, other_assigned);
                    guard.retain(|f| inner.contains(f) && outer.contains(f));
                }
                Stmt::Match { some, none, .. } => {
                    let (mut other_sent, mut other_assigned) = (sent.clone(), assigned.clone());
                    let (mut a, mut b) = (guard.clone(), guard.clone());
                    self.block(&some.1, &mut a, sent, assigned);
                    self.block(none, &mut b, &mut other_sent, &mut other_assigned);
                    union(sent, other_sent);
                    union(assigned, other_assigned);
                    guard.retain(|f| a.contains(f) && b.contains(f));
                }
                Stmt::Let { .. } | Stmt::Command { .. } | Stmt::Refresh { .. } => {}
            }
        }
    }

    fn nonqueued(&self, name: &str) -> bool {
        self.c.mutations.iter().any(|m| m.name == name && !m.queue)
    }

    /// `target` is sent (or assigned) here, after being assigned (or sent)
    /// on a path here: say what the runner does with both, once an action.
    fn meet(
        &mut self,
        target: &'a str,
        at: Span,
        guard: &Guard,
        earlier: &Made<'a>,
        this: &str,
        that: &str,
    ) {
        let Some((first, _, frame)) = earlier
            .get(target)
            .and_then(|v| v.iter().find(|(_, g, _)| !exclusive(g, guard)))
        else {
            return;
        };
        if !self.said.insert(target) {
            return;
        }
        let action = contract_syntax::inline::calls::shown(self.action);
        let (first_where, this_where) = (
            made_in(action, *frame, *first),
            made_in(action, self.frame, at),
        );
        self.out.push(AnalyzeError {
            id: "analyze-send-assigned",
            message: format!(
                "`{action}` both sends `{target}` and assigns it on one path: it {that} {first_where} and {this} {this_where}. A send is asked when the action runs; once the writes land, a request still in flight for a mutation the action assigned is forgotten and its reply dropped, and `{target}` keeps the assigned value (LLP 1016 D5), whichever line comes first. To clear an old result before a retry, keep the send and hide the result while `pending({target})`; to sign out, forget the sign-in mutation here and send the logout through a mutation of its own; to dismiss an answer, assign alone"
            ),
            span: at,
            related: vec![Related {
                span: *first,
                note: format!("`{target}` is {} here", if that == "sends" { "sent" } else { "assigned" }),
            }],
        });
    }
}

/// Forget what conditions said of `name`, now that it changed.
fn moved(name: &str, guard: &mut Guard, made: &mut Made<'_>) {
    guard.retain(|f| !about(name)(f));
    for v in made.values_mut() {
        for (_, g, _) in v.iter_mut() {
            g.retain(|f| !about(name)(f));
        }
    }
}

/// The union of two arms' sends or assignments.
fn union<'a>(into: &mut Made<'a>, from: Made<'a>) {
    for (target, made) in from {
        let have = into.entry(target).or_default();
        for (span, guard, frame) in made {
            match have.iter_mut().find(|(s, _, f)| *s == span && *f == frame) {
                Some((_, kept, _)) => kept.retain(|f| guard.contains(f)),
                None => have.push((span, guard, frame)),
            }
        }
    }
}

/// @ref LLP 1035.005.002 C1 — every action that both sends a nonqueued
/// mutation and assigns it on one path, through calls too, in either order:
/// a warning, once a mutation an action. In 1,029 bake-off builds no action
/// did; the one in the apps is Weird Castle's logout, which means it.
pub(super) fn send_assigned(c: &Component) -> Vec<AnalyzeError> {
    let mut out: Vec<AnalyzeError> = Vec::new();
    for a in &c.actions {
        let mut walk = Both {
            c,
            action: &a.name,
            frame: None,
            said: BTreeSet::new(),
            out: Vec::new(),
        };
        walk.block(
            &a.body,
            &mut Guard::new(),
            &mut BTreeMap::new(),
            &mut BTreeMap::new(),
        );
        for e in walk.out {
            if !out.iter().any(|x| x.span == e.span) {
                out.push(e);
            }
        }
    }
    out
}
