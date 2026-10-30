//! Predicates kept while deleting events from runner findings.

use crate::{census::Census, property, script::Case};
use contract_syntax::PropertyDecl;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Predicate {
    Property(usize, usize),
    Refusal {
        class: &'static str,
        action: Option<String>,
    },
}
impl Predicate {
    pub fn holds(&self, properties: &[PropertyDecl], lines: &[String]) -> bool {
        match self {
            Self::Property(p, l) => property::evaluate(properties, lines)
                .failures
                .iter()
                .any(|f| f.property == *p && f.line == *l),
            Self::Refusal { class, action } => crate::chunks(lines).iter().any(|step| {
                step.iter().enumerate().any(|(i, line)| {
                    (line.starts_with("ran ") || line.starts_with("outcome "))
                        && (line.ends_with(" refused") || line.ends_with(" poisoned"))
                        && action
                            .as_ref()
                            .is_none_or(|a| action_of(line).as_ref() == Some(a))
                        && crate::refusal_class(
                            step.get(i + 1)
                                .and_then(|l| l.strip_prefix("# "))
                                .unwrap_or(""),
                        ) == *class
                })
            }),
        }
    }
}
pub(crate) fn predicates(properties: &[PropertyDecl], lines: &[String]) -> BTreeSet<Predicate> {
    property::evaluate(properties, lines)
        .failures
        .into_iter()
        .map(|f| Predicate::Property(f.property, f.line))
        .chain(
            Census::default()
                .observe(lines)
                .into_iter()
                .map(|f| Predicate::Refusal {
                    class: f.class,
                    action: action_of(&f.line),
                }),
        )
        .collect()
}
pub(crate) fn action_of(line: &str) -> Option<String> {
    line.strip_prefix("ran ")?
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
}
pub(crate) fn store(plan: &exact_plan::Plan, case: &Case) -> String {
    let oracle = crate::oracle::Oracle::for_case(plan, case);
    let entries: Vec<_> = oracle
        .stored
        .iter()
        .map(|(n, _, v)| format!("{n} {}", crate::observe::value(v)))
        .collect();
    if entries.is_empty() {
        "no entries (all slots omitted)".into()
    } else {
        entries.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_class_predicate_covers_counted_refusals_and_an_action_predicate_keeps_its_name() {
        let lines = [
            "== tap \"absent\"",
            "outcome refused",
            "# UnknownView(0)",
            "== tap \"go\"",
            "outcome refused",
            "ran handler go refused",
            "# Trap(UnwrapNone { pc: 0 })",
        ]
        .map(str::to_string);
        assert!(Predicate::Refusal {
            class: "world",
            action: None
        }
        .holds(&[], &lines));
        assert!(Predicate::Refusal {
            class: "app",
            action: Some("go".into())
        }
        .holds(&[], &lines));
        assert!(!Predicate::Refusal {
            class: "app",
            action: Some("other".into())
        }
        .holds(&[], &lines));
    }
}
