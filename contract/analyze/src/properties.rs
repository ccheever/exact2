//! Root property reads closed over derive and function bodies (LLP 1151 D1).

use super::AnalyzeError;
use contract_syntax::{Component, Expr, FnDecl, PropertyDecl, PropertyForm, TemplatePart};
use std::collections::BTreeSet;

/// Refuse unknown names and derives whose values depend on the oracle, clock or geometry.
/// `root` is the expanded component that the plan compiler uses.
pub fn check_properties(
    root: &Component,
    fns: &[FnDecl],
    properties: &[PropertyDecl],
) -> Vec<AnalyzeError> {
    let mut errors = Vec::new();
    for property in properties {
        for line in &property.lines {
            let names: Vec<&str> = match &line.form {
                PropertyForm::Hold { name, .. } => vec![name],
                PropertyForm::OnlyWrites(names) => names.iter().map(String::as_str).collect(),
                PropertyForm::Command { .. } => Vec::new(),
            };
            for name in names {
                let slot = root.states.iter().any(|s| s.name == name)
                    || root.mutations.iter().any(|m| m.name == name);
                if slot {
                    continue;
                }
                let mut cone = Cone {
                    root,
                    fns,
                    seen: BTreeSet::new(),
                };
                let reason = if let Some(d) = root.derives.iter().find(|d| d.name == name) {
                    if matches!(line.form, PropertyForm::OnlyWrites(_)) {
                        Some(format!(
                            "`{name}` is a derive; `only writes` names root slots"
                        ))
                    } else {
                        cone.expr(&d.expr, &BTreeSet::new()).map(|chain| format!("`{name}` -> {chain}: its value would be the oracle's or would change on a step that commits nothing"))
                    }
                } else if root.resources.iter().any(|r| r.name == name) {
                    Some(format!("`{name}` is a resource: its value would be the oracle's or would change on a step that commits nothing"))
                } else {
                    Some(format!("`{name}` is not a root state or derive"))
                };
                if let Some(message) = reason {
                    errors.push(AnalyzeError {
                        id: "analyze-property-read",
                        message,
                        span: line.span,
                        related: Vec::new(),
                    });
                }
            }
        }
    }
    errors
}

struct Cone<'a> {
    root: &'a Component,
    fns: &'a [FnDecl],
    seen: BTreeSet<(&'static str, String)>,
}
impl Cone<'_> {
    fn expr(&mut self, e: &Expr, bound: &BTreeSet<String>) -> Option<String> {
        match e {
            Expr::Ident(name, _) if !bound.contains(name) => {
                if self.root.resources.iter().any(|r| r.name == *name) {
                    return Some(format!("resource `{name}`"));
                }
                if self.seen.insert(("derive", name.clone())) {
                    if let Some(d) = self.root.derives.iter().find(|d| d.name == *name) {
                        return self
                            .expr(&d.expr, &BTreeSet::new())
                            .map(|s| format!("`{name}` -> {s}"));
                    }
                }
                None
            }
            Expr::Call(name, args, _) => {
                if let Some(f) = self.fns.iter().find(|f| f.name == *name) {
                    if self.seen.insert(("fn", name.clone())) {
                        let locals = f.params.iter().map(|p| p.name.clone()).collect();
                        if let Some(s) = self.expr(&f.body, &locals) {
                            return Some(format!("fn `{name}` -> {s}"));
                        }
                    }
                } else if matches!(
                    name.as_str(),
                    "pending" | "failed" | "failure" | "performanceNow" | "frame" | "measure"
                ) {
                    // The roster's date formatters read their arguments; exactTime is a resource.
                    return Some(format!("`{name}(…)`"));
                }
                args.iter().find_map(|a| self.expr(a, bound))
            }
            Expr::Some(a, _)
            | Expr::Member(a, _, _)
            | Expr::NamedArg(_, a, _)
            | Expr::Unary(_, a, _)
            | Expr::Typed(a, _, _) => self.expr(a, bound),
            Expr::Binary(_, a, b, _) => self.expr(a, bound).or_else(|| self.expr(b, bound)),
            Expr::Ternary(a, b, c, _) => self
                .expr(a, bound)
                .or_else(|| self.expr(b, bound))
                .or_else(|| self.expr(c, bound)),
            Expr::Let {
                name, value, body, ..
            } => self.expr(value, bound).or_else(|| {
                let mut locals = bound.clone();
                locals.insert(name.clone());
                self.expr(body, &locals)
            }),
            Expr::Arrow { params, body, .. } => {
                let mut locals = bound.clone();
                locals.extend(params.iter().cloned());
                self.expr(body, &locals)
            }
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => self
                .expr(subject, bound)
                .or_else(|| {
                    let mut locals = bound.clone();
                    locals.insert(var.clone());
                    self.expr(some, &locals)
                })
                .or_else(|| self.expr(none, bound)),
            Expr::Template(parts, _) => parts.iter().find_map(|p| match p {
                TemplatePart::Expr(e) => self.expr(e, bound),
                TemplatePart::Text(_) => None,
            }),
            Expr::List(items, _) => items.iter().find_map(|e| self.expr(e, bound)),
            Expr::Ident(..) | Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {
                None
            }
        }
    }
}
