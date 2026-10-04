//! @ref LLP 1085 D8 (flashcards F4) — a mutation keeps one reply: a second
//! send forgets the first's in-flight reply (LLP 1016 D5), so two sends to
//! one mutation on one path through an action are refused, not warned of.
//!
//! The walk is path-sensitive, over the body's statements (`Action::effects`
//! flattens branches): each `if`/`match` arm starts from what the paths
//! into it may have sent, and the branch leaves their union, so exclusive
//! arms pass while an arm's send followed by another after the branch is
//! refused. Sequential `if`s whose conditions cannot both hold are
//! exclusive too — a key handler's `if k == "1" …` then `if k == "2" …` —
//! when each tests one name against literals (`k == "m" or k == "M"`, alone
//! or as a conjunct of an `and`), the literals differ, and nothing assigns
//! or sends that name between them.

use super::{AnalyzeError, Related};
use contract_syntax::{BinOp, Component, Expr, Span, Stmt};
use std::collections::{BTreeMap, BTreeSet};

/// What a path's enclosing conditions say: each tested name is one of these
/// literals.
type Guard = Vec<(String, BTreeSet<String>)>;

/// Two guards no one evaluation can satisfy both of.
fn exclusive(a: &Guard, b: &Guard) -> bool {
    a.iter().any(|(name, values)| {
        b.iter()
            .any(|(other, given)| name == other && values.is_disjoint(given))
    })
}

/// What `cond` being true says of the names it tests.
fn facts(cond: &Expr) -> Guard {
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

type Sent<'a> = BTreeMap<&'a str, Vec<(Span, Guard)>>;

/// Forget what conditions said of `name`, now that it changed.
fn changed(name: &str, guard: &mut Guard, sent: &mut Sent<'_>) {
    let about = |(tested, _): &(String, BTreeSet<String>)| tested.split('.').next() == Some(name);
    guard.retain(|f| !about(f));
    for sends in sent.values_mut() {
        for (_, g) in sends.iter_mut() {
            g.retain(|f| !about(f));
        }
    }
}

struct Walk<'a> {
    c: &'a Component,
    action: &'a str,
    errors: Vec<AnalyzeError>,
}

impl<'a> Walk<'a> {
    /// The block's statements on every path, from `guard` and what was sent
    /// before it; `guard` leaves holding what still holds after the block.
    fn block(&mut self, stmts: &'a [Stmt], guard: &mut Guard, sent: &mut Sent<'a>) {
        for stmt in stmts {
            match stmt {
                Stmt::Send { target, span, .. } => {
                    let earlier = sent.entry(target).or_default();
                    if let Some((first, _)) = earlier.iter().find(|(_, g)| !exclusive(g, guard)) {
                        let reaches = match self.c.mutations.iter().find(|m| &m.name == target) {
                            Some(m) => match &m.then {
                                Some((then, _)) => format!("`then {then}`"),
                                None => format!("`{target}`"),
                            },
                            None => format!("`{target}`"),
                        };
                        self.errors.push(AnalyzeError {
                            id: "analyze-send-twice",
                            message: format!(
                                "`{}` sends `{target}` twice; only the last send's reply reaches {reaches} (LLP 1016 D5). Send once, or use a mutation per request",
                                self.action
                            ),
                            span: *span,
                            related: vec![Related {
                                span: *first,
                                note: format!("`{target}` is sent here first"),
                            }],
                        });
                    }
                    earlier.push((*span, guard.clone()));
                    changed(target, guard, sent);
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
                    self.block(then, &mut inner, sent);
                    self.block(otherwise, &mut outer, &mut other);
                    merge(sent, other);
                    // What an arm changed no longer holds after the branch.
                    guard.retain(|f| inner.contains(f) && outer.contains(f));
                }
                Stmt::Match { some, none, .. } => {
                    let mut other = sent.clone();
                    let (mut a, mut b) = (guard.clone(), guard.clone());
                    self.block(&some.1, &mut a, sent);
                    self.block(none, &mut b, &mut other);
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
        for (span, guard) in sends {
            match have.iter_mut().find(|(s, _)| *s == span) {
                Some((_, kept)) => kept.retain(|f| guard.contains(f)),
                None => have.push((span, guard)),
            }
        }
    }
}

/// Every action's second send to one mutation on one path.
pub(super) fn check(c: &Component) -> Vec<AnalyzeError> {
    let mut errors = Vec::new();
    for a in &c.actions {
        let mut walk = Walk {
            c,
            action: &a.name,
            errors: Vec::new(),
        };
        walk.block(&a.body, &mut Guard::new(), &mut BTreeMap::new());
        errors.extend(walk.errors);
    }
    errors
}
