//! Refusal attribution and coverage from the runner's commit lines.

use crate::refusal_class;
use exact_plan::Plan;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct Census {
    classes: BTreeMap<&'static str, usize>,
    committed: BTreeSet<String>,
    attempted: BTreeSet<String>,
    tasks: BTreeSet<String>,
    steps: usize,
    multiple: usize,
    poisoned: usize,
    pub(crate) omitted: usize,
}

pub(crate) struct Finding {
    pub class: &'static str,
    pub at: usize,
    pub line: String,
    pub reason: String,
}

impl Census {
    pub fn observe(&mut self, lines: &[String]) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut compared = 0;
        for step in crate::chunks(lines) {
            self.steps += 1;
            self.omitted += usize::from(step.iter().any(|l| l == crate::observe::OMITTED_COMMIT));
            let commits = step.iter().filter(|l| l.starts_with("ran ")).count();
            self.multiple += usize::from(commits > 1);
            self.poisoned += usize::from(step.iter().any(|l| {
                (l.starts_with("outcome ") || l.starts_with("ran ")) && l.ends_with(" poisoned")
            }));
            for (i, line) in step.iter().enumerate() {
                if line.starts_with('#') {
                    continue;
                }
                let at = compared;
                compared += 1;
                let words: Vec<_> = line.split_whitespace().collect();
                if let ["ran", kind, name, result] = words.as_slice() {
                    if *kind != "queue" {
                        self.attempted.insert((*name).to_string());
                        if *result == "ok" {
                            self.committed.insert((*name).to_string());
                        }
                    }
                    if *kind == "task" {
                        self.tasks.insert((*name).to_string());
                    }
                }
                if !(line.starts_with("outcome ") || line.starts_with("ran "))
                    || !(line.ends_with(" refused") || line.ends_with(" poisoned"))
                {
                    continue;
                }
                let reason = step
                    .get(i + 1)
                    .and_then(|l| l.strip_prefix("# "))
                    .unwrap_or("");
                let class = refusal_class(reason);
                *self.classes.entry(class).or_default() += 1;
                // Aggregate outcomes count, but a commit's own line owns its
                // finding. Boot and argument refusals have no commit line.
                let attributed = line.starts_with("outcome ")
                    && step.windows(2).any(|pair| {
                        pair[0].starts_with("ran ")
                            && (pair[0].ends_with(" refused") || pair[0].ends_with(" poisoned"))
                            && pair[1].strip_prefix("# ") == Some(reason)
                    });
                if matches!(class, "app" | "type") && !attributed {
                    findings.push(Finding {
                        class,
                        at,
                        line: line.clone(),
                        reason: reason.into(),
                    });
                }
            }
        }
        findings
    }

    pub fn passed(&self) -> bool {
        self.classes.get("type").copied().unwrap_or(0) == 0
    }

    pub fn print(&self, plan: &Plan) {
        let declared: BTreeSet<_> = plan.actions.iter().map(|a| plan.str(a.name)).collect();
        let committed = declared
            .iter()
            .filter(|a| self.committed.contains(**a))
            .count();
        let never: Vec<_> = declared
            .iter()
            .copied()
            .filter(|a| !self.attempted.contains(*a))
            .collect();
        let uncommitted: Vec<_> = declared
            .iter()
            .copied()
            .filter(|a| !self.committed.contains(*a))
            .collect();
        let fired = plan
            .timers
            .iter()
            .filter(|t| self.tasks.contains(plan.str(plan.action(t.action).name)))
            .count();
        println!(
            "actions: committed {committed} of {}; never run: {}; never committed: {}",
            declared.len(),
            names(&never),
            names(&uncommitted)
        );
        println!(
            "clock actions fired: {fired} of {} tasks declared (by action name)",
            plan.timers.len()
        );
        println!(
            "steps: {} (including boot); {} with more than one ran line; {} poisoned",
            self.steps, self.multiple, self.poisoned
        );
        println!("steps with a commit the trace leaves out: {}", self.omitted);
        let counts: Vec<_> = [
            "world",
            "limit",
            "poisoned",
            "cycle",
            "value",
            "app",
            "type",
            "unclassified",
        ]
        .into_iter()
        .map(|c| format!("{c} {}", self.classes.get(c).unwrap_or(&0)))
        .collect();
        println!(
            "refusal lines (aggregate outcomes and commits): {}",
            counts.join(", ")
        );
    }
}

fn names(names: &[&str]) -> String {
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world() {
        for reason in [
            "Data { resource: \"x\", error: Refused(\"no answer\") }",
            "Shape { resource: \"x\", why: \"number\" }",
            "NoHandler { view: 1, event: \"press\" }",
            "Arity { action: \"a\", expected: 1, actual: 0 }",
        ] {
            assert_eq!(refusal_class(reason), "world");
        }
    }

    #[test]
    fn limit() {
        for reason in [
            "Trap(IterationLimit { pc: 8 })",
            "StringTooLong { name: \"x\" }",
            "TimerFireLimit { limit: 4096 }",
            "QueueFull { mutation: \"save\" }",
        ] {
            assert_eq!(refusal_class(reason), "limit");
        }
    }

    #[test]
    fn poisoned() {
        assert_eq!(refusal_class("Poisoned"), "poisoned");
    }

    #[test]
    fn cycle() {
        assert_eq!(refusal_class("Cycle"), "cycle");
    }

    #[test]
    fn value() {
        for reason in [
            "NonFiniteValue { name: \"n\" }",
            "Trap(RangeCount { pc: 4, non_finite: true })",
            "Trap(PathParameter { pc: 8 })",
            "Router(\"invalid router\")",
        ] {
            assert_eq!(refusal_class(reason), "value");
        }
    }

    #[test]
    fn app() {
        for reason in [
            "Trap(UnwrapNone { pc: 0 })",
            "Trap(RangeCount { pc: 4, non_finite: false })",
            "router push refused: no route matches /missing",
        ] {
            assert_eq!(refusal_class(reason), "app");
        }
    }

    #[test]
    fn type_error() {
        for reason in [
            "SlotType { slot: \"x\" }",
            "DeriveType { derive: \"x\" }",
            "Trap(Arity { pc: 2, expected: 1 })",
            "ArgumentType { action: \"a\", param: \"x\" }",
        ] {
            assert_eq!(refusal_class(reason), "type");
        }
    }

    #[test]
    fn unknown_variants_stay_unclassified() {
        assert_eq!(
            refusal_class("FutureError { name: \"UnwrapNone\" }"),
            "unclassified"
        );
        assert_eq!(refusal_class("Trap(FutureError { pc: 0 })"), "unclassified");
    }

    #[test]
    fn a_type_refusal_fails_even_when_rendering_poisoned_the_step() {
        let lines = [
            "== tap \"go\"",
            "outcome poisoned",
            "# Trap(TypeMismatch { pc: 2, op: Add })",
            "ran handler go poisoned",
            "# Trap(TypeMismatch { pc: 2, op: Add })",
        ]
        .map(str::to_string);
        let mut census = Census::default();
        let findings = census.observe(&lines);
        assert!(!census.passed());
        assert_eq!(census.poisoned, 1);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, "type");
        assert_eq!(findings[0].line, "ran handler go poisoned");
    }

    #[test]
    fn due_refusal_belongs_to_the_task() {
        let lines = [
            "== tap \"go\"",
            "outcome refused",
            "# Trap(UnwrapNone { pc: 0 })",
            "ran task tick refused",
            "# Trap(UnwrapNone { pc: 0 })",
            "ran handler go ok",
        ]
        .map(str::to_string);
        let mut census = Census::default();
        let findings = census.observe(&lines);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, "ran task tick refused");
        assert_eq!(findings[0].at, 2);
        assert!(census.committed.contains("go"));
        assert!(!census.committed.contains("tick"));
        assert_eq!(census.multiple, 1);
        assert_eq!(census.classes["app"], 2);
    }
}
