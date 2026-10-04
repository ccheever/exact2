//! @ref LLP 1092 D9 — a gated task's gate and key are read at commits, not
//! as the clock moves: one that reads `now()`, directly, through a derive
//! or through a `fn` body, is refused (`analyze-task-gate-clock`).

use super::AnalyzeError;
use contract_syntax::{Component, Expr, FnDecl, TemplatePart};
use std::collections::BTreeSet;

/// Every gate or key of `c`'s tasks that reads the clock.
pub(super) fn check(c: &Component, fns: &[FnDecl]) -> Vec<AnalyzeError> {
    let mut errors = Vec::new();
    for t in &c.tasks {
        for (what, e) in [("gate", &t.gate), ("key", &t.key)] {
            let Some(e) = e else { continue };
            let mut seen = BTreeSet::new();
            if let Some(at) = clock(e, c, fns, &mut seen) {
                errors.push(AnalyzeError {
                    id: "analyze-task-gate-clock",
                    message: format!(
                        "`task {}`'s {what} reads `now()`{}. A gate is read at commits, not as the clock moves, so an `after` gated on `now()` cannot be dropped before it fires, and an `every` stops up to an interval late. Gate on state (`toast != \"\"`) and let `after(5000, …)` measure the time",
                        t.name,
                        match at {
                            Through::Direct => String::new(),
                            Through::Derive(d) => format!(" through `{d}`"),
                            Through::Fn(f) => format!(" through `fn {f}`"),
                        }
                    ),
                    span: e.span(),
                    related: Vec::new(),
                });
            }
        }
    }
    errors
}

enum Through {
    Direct,
    Derive(String),
    Fn(String),
}

/// Where `e` reads the clock, if it does: `now()` itself, or the first
/// derive or `fn` whose value does.
fn clock<'a>(
    e: &'a Expr,
    c: &'a Component,
    fns: &'a [FnDecl],
    seen: &mut BTreeSet<&'a str>,
) -> Option<Through> {
    let via = |name: &'a str, seen: &mut BTreeSet<&'a str>| -> Option<Through> {
        if !seen.insert(name) {
            return None;
        }
        if let Some(d) = c.derives.iter().find(|d| d.name == name) {
            return clock(&d.expr, c, fns, seen).map(|_| Through::Derive(shown(name)));
        }
        None
    };
    match e {
        Expr::Call(name, args, _) if name == "now" && args.is_empty() => Some(Through::Direct),
        Expr::Call(name, args, _) => {
            if let Some(f) = fns.iter().find(|f| &f.name == name) {
                if seen.insert(&f.name) && clock(&f.body, c, fns, seen).is_some() {
                    return Some(Through::Fn(shown(name)));
                }
            }
            args.iter().find_map(|a| clock(a, c, fns, seen))
        }
        Expr::Ident(name, _) => via(name, seen),
        Expr::Template(parts, _) => parts.iter().find_map(|p| match p {
            TemplatePart::Expr(e) => clock(e, c, fns, seen),
            TemplatePart::Text(_) => None,
        }),
        Expr::Some(a, _)
        | Expr::Member(a, _, _)
        | Expr::NamedArg(_, a, _)
        | Expr::Unary(_, a, _)
        | Expr::Typed(a, _, _) => clock(a, c, fns, seen),
        Expr::Arrow { body, .. } => clock(body, c, fns, seen),
        Expr::Binary(_, a, b, _) => clock(a, c, fns, seen).or_else(|| clock(b, c, fns, seen)),
        Expr::Let { value, body, .. } => {
            clock(value, c, fns, seen).or_else(|| clock(body, c, fns, seen))
        }
        Expr::Ternary(a, b, d, _) => clock(a, c, fns, seen)
            .or_else(|| clock(b, c, fns, seen))
            .or_else(|| clock(d, c, fns, seen)),
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => clock(subject, c, fns, seen)
            .or_else(|| clock(some, c, fns, seen))
            .or_else(|| clock(none, c, fns, seen)),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::EmptyList(_) => {
            None
        }
    }
}

/// A name as written: a lifted child's `name#N` without its suffix.
fn shown(name: &str) -> String {
    name.split('#').next().unwrap_or(name).to_string()
}
