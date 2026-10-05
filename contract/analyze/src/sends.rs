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

/// The union of two arms' sends; a send made before the branch keeps only
/// what both arms still know of its path.
fn merge<'a>(into: &mut Sent<'a>, from: Sent<'a>) {
    for (target, sends) in from {
        let have = into.entry(target).or_default();
        for (span, guard, frame, armed) in sends {
            match have
                .iter_mut()
                .find(|(s, _, f, _)| *s == span && *f == frame)
            {
                Some((_, kept, _, _)) => kept.retain(|f| guard.contains(f)),
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
