//! `lower-count-plural` (LLP 1116.004 D4): a number written straight
//! before a plural noun, `${n} cards`, reads "1 cards" when the count is
//! one. Round after round two or three graded apps showed one ("1 cards",
//! "1 spots left", "1 people"), from builders who never looked the idiom
//! up, so the compiler says it where it is written.
//!
//! The number is the type checker's: a string (`${name} votes`) is never
//! named, nor anything it cannot type. A template under a choice made on
//! the count (`n == 1 ? … : …`, `when n == 1`, `if n != 1`) is the
//! author's choice already; a unit's abbreviation (`lbs`, `mins`) is the
//! same for one, `%` is no word, and a total after `of` (`${done} of
//! ${total} cards`) is rarely one.

use crate::LowerError;
use contract_syntax::{BinOp, Expr, Node, Span, Stmt, TemplatePart, UnOp};
use contract_types::{infer, Checked, Ref, Scope, Shapes, Ty};
use std::collections::BTreeSet;

/// Plurals that do not end in `s`.
const IRREGULAR: &[(&str, &str)] = &[
    ("people", "person"),
    ("children", "child"),
    ("men", "man"),
    ("women", "woman"),
    ("feet", "foot"),
    ("teeth", "tooth"),
    ("mice", "mouse"),
    ("geese", "goose"),
];

/// Words ending in `s` that are not a plural noun after a number, and the
/// abbreviations a unit keeps for one (`1 lbs` is how it is written).
const NOT_PLURAL: &[&str] = &[
    "was", "has", "does", "yes", "its", "always", "perhaps", "whereas", "thus", "lbs", "hrs",
    "mins", "secs", "yrs", "pts", "fps", "kbps", "mbps", "gbps", "ozs", "kgs", "pcs",
];

/// The plural noun a template's text opens with after a space, and its
/// singular: `" cards left"` is `cards`, `card`.
fn plural_after(text: &str) -> Option<(String, String)> {
    let word: String = text
        .strip_prefix(' ')?
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let lower = word.to_ascii_lowercase();
    if let Some((_, one)) = IRREGULAR.iter().find(|(many, _)| *many == lower) {
        return Some((word, (*one).to_string()));
    }
    let plural = lower.len() >= 3
        && lower.ends_with('s')
        && !["ss", "us", "is"].iter().any(|end| lower.ends_with(end))
        && !NOT_PLURAL.contains(&lower.as_str());
    if !plural {
        return None;
    }
    let one = if let Some(stem) = word.strip_suffix("ies") {
        format!("{stem}y")
    } else if ["ches", "shes", "sses", "xes", "zes"]
        .iter()
        .any(|end| lower.ends_with(end))
    {
        word[..word.len() - 2].to_string()
    } else {
        word[..word.len() - 1].to_string()
    };
    Some((word, one))
}

/// Whether a condition chooses on a count being one: a comparison with 1
/// (`n == 1`, `n > 1`), or `n < 2`, `n >= 2`, through `and`, `or`, `not`.
fn on_one(cond: &Expr) -> bool {
    let is = |e: &Expr, v: f64| matches!(e, Expr::Number(n, _) if *n == v);
    match cond {
        Expr::Binary(BinOp::And | BinOp::Or, a, b, _) => on_one(a) || on_one(b),
        Expr::Binary(
            op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge),
            a,
            b,
            _,
        ) => {
            is(a, 1.0)
                || is(b, 1.0)
                || (matches!(op, BinOp::Lt | BinOp::Ge) && is(b, 2.0))
                || (matches!(op, BinOp::Gt | BinOp::Le) && is(a, 2.0))
        }
        Expr::Unary(UnOp::Not, inner, _) | Expr::Typed(inner, _, _) => on_one(inner),
        _ => false,
    }
}

struct Counts<'a> {
    shapes: &'a Shapes,
    /// A component used twice is inlined twice: one line for its source.
    seen: BTreeSet<(u32, u32, u32, u32)>,
    out: Vec<LowerError>,
}

impl Counts<'_> {
    fn number(&self, e: &Expr, scope: &Scope) -> bool {
        matches!(infer(e, scope, self.shapes), Ok(Ty::Number))
    }

    fn template(&mut self, parts: &[TemplatePart], span: Span, scope: &Scope) {
        for (i, pair) in parts.windows(2).enumerate() {
            let [TemplatePart::Expr(x), TemplatePart::Text(text)] = pair else {
                continue;
            };
            // `${done} of ${total} cards`: the noun counts a total, which is
            // rarely one.
            if i > 0
                && matches!(&parts[i - 1], TemplatePart::Text(before) if before.ends_with(" of "))
            {
                continue;
            }
            let Some((many, one)) = plural_after(text) else {
                continue;
            };
            if !self.number(x, scope)
                || !self
                    .seen
                    .insert((span.source_id, span.line, span.col, span.end_col))
            {
                continue;
            }
            self.out.push(LowerError {
                id: "lower-count-plural",
                message: format!(
                    "this count reads \"1 {many}\" when it is 1: choose the noun by the number, `${{n}} ${{n == 1 ? \"{one}\" : \"{many}\"}}` with the count as `n`, or a `fn` that words every count (`contract vocab plural`)"
                ),
                span: x.span(),
            });
        }
    }

    /// Every template in `e`, with what its choices and bindings say.
    fn expr(&mut self, e: &Expr, scope: &Scope, chosen: bool) {
        match e {
            Expr::Template(parts, span) => {
                if !chosen {
                    self.template(parts, *span, scope);
                }
                for p in parts {
                    if let TemplatePart::Expr(x) = p {
                        self.expr(x, scope, chosen);
                    }
                }
            }
            Expr::Ternary(cond, yes, no, _) => {
                self.expr(cond, scope, chosen);
                let chosen = chosen || on_one(cond);
                self.expr(yes, scope, chosen);
                self.expr(no, scope, chosen);
            }
            Expr::Binary(_, a, b, _) => {
                self.expr(a, scope, chosen);
                self.expr(b, scope, chosen);
            }
            Expr::Unary(_, inner, _)
            | Expr::Some(inner, _)
            | Expr::NamedArg(_, inner, _)
            | Expr::Typed(inner, _, _)
            | Expr::Member(inner, _, _) => self.expr(inner, scope, chosen),
            Expr::Call(_, items, _) | Expr::List(items, _) => {
                for x in items {
                    self.expr(x, scope, chosen);
                }
            }
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => {
                self.expr(subject, scope, chosen);
                let bound = match infer(subject, scope, self.shapes) {
                    Ok(Ty::Option(t)) => *t,
                    _ => Ty::Unknown,
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Bound(0), bound)));
                self.expr(some, &inner, chosen);
                self.expr(none, scope, chosen);
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.expr(value, scope, chosen);
                let mut inner = scope.clone();
                let ty = infer(value, scope, self.shapes).unwrap_or(Ty::Unknown);
                inner.push(vec![(name.clone(), Ref::Bound(0), ty)]);
                self.expr(body, &inner, chosen);
            }
            Expr::Arrow { params, body, .. } => {
                let mut inner = scope.clone();
                inner.push(
                    params
                        .iter()
                        .map(|p| (p.clone(), Ref::Bound(0), Ty::Unknown))
                        .collect(),
                );
                self.expr(body, &inner, chosen);
            }
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {
            }
        }
    }

    fn nodes(&mut self, nodes: &[Node], scope: &Scope, chosen: bool) {
        for node in nodes {
            match node {
                Node::Element {
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    for e in positional.iter().chain(attrs.iter().map(|a| &a.value)) {
                        self.expr(e, scope, chosen);
                    }
                    self.nodes(children, scope, chosen);
                }
                Node::When {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond, scope, chosen);
                    let chosen = chosen || on_one(cond);
                    self.nodes(then, scope, chosen);
                    self.nodes(otherwise, scope, chosen);
                }
                Node::Each {
                    var,
                    index,
                    list,
                    body,
                    ..
                } => {
                    self.expr(list, scope, chosen);
                    let item = match infer(list, scope, self.shapes) {
                        Ok(Ty::List(t)) => *t,
                        _ => Ty::Unknown,
                    };
                    let mut inner = scope.clone();
                    inner.push_each(var, index.as_deref(), item);
                    self.nodes(body, &inner, chosen);
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject, scope, chosen);
                    let bound = match infer(subject, scope, self.shapes) {
                        Ok(Ty::Option(t)) => *t,
                        _ => Ty::Unknown,
                    };
                    let mut inner = scope.clone();
                    inner.push_region(Some((some.0.clone(), Ref::Bound(0), bound)));
                    self.nodes(&some.1, &inner, chosen);
                    let mut none_scope = scope.clone();
                    none_scope.push_region(None);
                    self.nodes(none, &none_scope, chosen);
                }
                Node::Use { args, children, .. } => {
                    for a in args {
                        self.expr(&a.value, scope, chosen);
                    }
                    self.nodes(children, scope, chosen);
                }
                Node::Children { .. } => {}
            }
        }
    }

    fn stmts(&mut self, body: &[Stmt], scope: &Scope, chosen: bool) {
        let mut scope = scope.clone();
        for s in body {
            match s {
                Stmt::Let { name, expr, .. } => {
                    self.expr(expr, &scope, chosen);
                    let ty = infer(expr, &scope, self.shapes).unwrap_or(Ty::Unknown);
                    scope.push(vec![(name.clone(), Ref::Bound(0), ty)]);
                }
                Stmt::Assign { expr, .. } => self.expr(expr, &scope, chosen),
                Stmt::Command { args, .. } | Stmt::Send { args, .. } => {
                    for a in args {
                        self.expr(a, &scope, chosen);
                    }
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond, &scope, chosen);
                    let chosen = chosen || on_one(cond);
                    self.stmts(then, &scope, chosen);
                    self.stmts(otherwise, &scope, chosen);
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject, &scope, chosen);
                    let bound = match infer(subject, &scope, self.shapes) {
                        Ok(Ty::Option(t)) => *t,
                        _ => Ty::Unknown,
                    };
                    let mut inner = scope.clone();
                    inner.push_region(Some((some.0.clone(), Ref::Bound(0), bound)));
                    self.stmts(&some.1, &inner, chosen);
                    self.stmts(none, &scope, chosen);
                }
                Stmt::Call { args, body, .. } => {
                    for a in args {
                        self.expr(a, &scope, chosen);
                    }
                    self.stmts(body, &scope, chosen);
                }
                Stmt::Refresh { .. } => {}
            }
        }
    }
}

/// `lower-count-plural`'s sites in the expanded root: its view, derives,
/// states and actions, and the file's `fn`s.
pub(super) fn count_plural(checked: &Checked<'_>) -> Vec<LowerError> {
    let root = &checked.expanded.root;
    let Some(types) = checked.types.components.first() else {
        return Vec::new();
    };
    let shapes = &checked.types.shapes;
    let mut counts = Counts {
        shapes,
        seen: BTreeSet::new(),
        out: Vec::new(),
    };
    let scope = checked.types.component_scope(root, types);
    for f in &checked.file.fns {
        let params = shapes
            .fns
            .get(&f.name)
            .map(|(p, _)| p.as_slice())
            .unwrap_or_default();
        let mut inner = Scope::default();
        inner.push(
            f.params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let ty = params.get(i).cloned().unwrap_or(Ty::Unknown);
                    (p.name.clone(), Ref::Param(i as u32), ty)
                })
                .collect(),
        );
        counts.expr(&f.body, &inner, false);
    }
    for b in root.states.iter().chain(&root.derives) {
        counts.expr(&b.expr, &scope, false);
    }
    for (i, a) in root.actions.iter().enumerate() {
        let params = types.actions.get(i).map(Vec::as_slice).unwrap_or_default();
        let mut inner = scope.clone();
        inner.enter_action();
        inner.push(
            a.params
                .iter()
                .enumerate()
                .map(|(j, p)| {
                    let ty = params.get(j).cloned().unwrap_or(Ty::Unknown);
                    (p.name.clone(), Ref::Param(j as u32), ty)
                })
                .collect(),
        );
        counts.stmts(&a.body, &inner, false);
    }
    counts.nodes(&root.view, &scope, false);
    let mut out = counts.out;
    out.sort_by_key(|e| (e.span.source_id, e.span.line, e.span.col));
    out
}
