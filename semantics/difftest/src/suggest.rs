//! Literal properties supported by the observed scripts, never proofs.

use crate::property::{changed, number, Step};
use contract_syntax::{PropertyDecl, PropertyForm, PropertyKind, PropertyLine, PropertyTest, Span};
use exact_plan::{Plan, TypeKind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Default)]
struct Values {
    kind: Option<TypeKind>,
    enum_values: BTreeSet<String>,
    seen: BTreeSet<String>,
    writers: BTreeSet<String>,
    not_some: bool,
    after_boot: usize,
    broken_stays: BTreeSet<String>,
    became: BTreeSet<String>,
}
#[derive(Default)]
struct Frame {
    count: usize,
    changed: BTreeSet<String>,
}
#[derive(Default)]
pub(crate) struct Suggestions {
    values: BTreeMap<String, Values>,
    frames: BTreeMap<String, Frame>,
    scripts: usize,
    steps: usize,
}
impl Suggestions {
    pub fn new(plan: &Plan, file: &Path) -> Self {
        let names: Vec<_> = plan
            .slots
            .iter()
            .filter(|s| s.owner.is_none())
            .map(|s| (plan.str(s.name), s.ty))
            .chain(plan.derives.iter().map(|d| (plan.str(d.name), d.ty)))
            .filter(|(n, _)| {
                n.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            })
            .collect();
        let properties: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(i, (name, _))| PropertyDecl {
                span: Span::point(i as u32 + 1, 1),
                kind: PropertyKind::Always,
                name: name.to_string(),
                lines: vec![PropertyLine {
                    span: Span::point(i as u32 + 1, 1),
                    form: PropertyForm::Hold {
                        name: name.to_string(),
                        test: PropertyTest::Whole,
                    },
                }],
            })
            .collect();
        let refused: BTreeSet<_> = contract::check_properties_path(file, &properties)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.span.line)
            .collect();
        let values = names
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !refused.contains(&(*i as u32 + 1)))
            .map(|(_, (name, ty))| {
                let row = plan.type_(ty);
                let enum_values = if row.kind == TypeKind::String {
                    row.fields
                        .iter()
                        .map(|f| crate::observe::quote(plan.str(plan.field(f).name)))
                        .collect()
                } else {
                    BTreeSet::new()
                };
                (
                    name.to_string(),
                    Values {
                        kind: Some(row.kind),
                        enum_values,
                        ..Values::default()
                    },
                )
            })
            .collect();
        Self {
            values,
            ..Self::default()
        }
    }

    pub fn observe(&mut self, observation: &[String]) {
        let steps: Vec<_> = crate::chunks(observation)
            .into_iter()
            .map(Step::new)
            .collect();
        self.scripts += 1;
        self.steps += steps.len();
        for (name, values) in &mut self.values {
            let mut seen = BTreeSet::<String>::new();
            for (i, step) in steps.iter().enumerate().filter(|(_, s)| s.config) {
                let Some(value) = step.value(name) else {
                    continue;
                };
                values.seen.insert(value.to_string());
                if i > 0 {
                    values.after_boot += 1;
                    values.not_some |= !value.starts_with("some(");
                }
                for old in &seen {
                    if old != value {
                        values.broken_stays.insert(old.clone());
                    }
                }
                if seen.iter().any(|old| old != value) {
                    values.became.insert(value.to_string());
                }
                seen.insert(value.to_string());
            }
        }
        for (i, step) in steps.iter().enumerate().skip(1) {
            let previous = &steps[i - 1];
            if step.input() {
                let [("handler", action, "ok")] = step.ran.as_slice() else {
                    continue;
                };
                let Some(land) = steps.get(i + 1).filter(|s| s.label == "land") else {
                    continue;
                };
                if !previous.config
                    || !step.config
                    || step.omitted
                    || !land.config
                    || land.omitted
                    || !land
                        .ran
                        .iter()
                        .all(|(k, _, _)| matches!(*k, "then" | "queue"))
                {
                    continue;
                }
                self.frame(action, previous, land);
            } else if step.label.starts_with("clock +")
                && previous.config
                && step.config
                && !step.omitted
            {
                self.frame("", previous, step);
            }
        }
    }

    fn frame(&mut self, action: &str, before: &Step<'_>, after: &Step<'_>) {
        let frame = self.frames.entry(action.to_string()).or_default();
        frame.count += 1;
        frame.changed.extend(changed(Some(before), after));
        for (name, values) in &mut self.values {
            if before.value(name) != after.value(name) {
                values.writers.insert(action.to_string());
            }
        }
    }

    pub fn blocks(&self, existing: &[PropertyDecl]) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut add = |block: String| {
            let Ok(parsed) = contract::properties(&block) else {
                return;
            };
            let candidate = &parsed[0];
            // Compare predicates and their scope, not the author's title or source spans.
            if candidate.lines.iter().all(|line| {
                existing.iter().any(|p| {
                    p.kind == candidate.kind
                        && p.lines.iter().any(|l| same_form(&l.form, &line.form))
                })
            }) {
                return;
            }
            blocks.push(block);
        };
        for (name, v) in &self.values {
            let scalar = matches!(v.kind, Some(TypeKind::Number | TypeKind::String));
            let small = !v.seen.is_empty() && v.seen.len() <= 6;
            if scalar
                && small
                && (v.seen.len() >= 2 || v.writers.iter().filter(|w| !w.is_empty()).count() >= 2)
                && (v.enum_values.is_empty() || v.seen != v.enum_values)
            {
                let literals: Option<Vec<_>> = v.seen.iter().map(|s| literal(s)).collect();
                if let Some(literals) = literals {
                    add(format!(
                        "always \"{name} is one of {} values\"\n  expect state {name} in {}\n",
                        v.seen.len(),
                        literals.join(" ")
                    ));
                }
            }
            if v.kind == Some(TypeKind::Number)
                && v.seen.len() > 1
                && v.seen
                    .iter()
                    .all(|s| number(s).is_some_and(|n| n.is_finite() && n.fract() == 0.0))
            {
                add(format!(
                    "always \"{name} is a whole number\"\n  expect state {name} whole\n"
                ));
            }
            if v.kind == Some(TypeKind::Option) && v.after_boot > 0 && !v.not_some {
                add(format!(
                    "always \"{name} is never none\"\n  expect state {name} in some\n"
                ));
            }
            if v.kind == Some(TypeKind::Bool) || (scalar && small) {
                for value in v.became.difference(&v.broken_stays) {
                    if let Some(value) = literal(value) {
                        let title = crate::observe::quote(&format!("{name} stays {value}"));
                        add(format!(
                            "always {title}\n  expect state {name} stays {value}\n"
                        ));
                    }
                }
            }
        }
        for (action, f) in &self.frames {
            if f.count < 2 {
                continue;
            }
            let names = f.changed.iter().cloned().collect::<Vec<_>>().join(" ");
            let writes = if names.is_empty() {
                "writes none".into()
            } else {
                format!("only writes {names}")
            };
            let desc = if names.is_empty() { "none" } else { &names };
            let (scope, subject) = if action.is_empty() {
                ("clock".into(), "the clock")
            } else {
                (crate::observe::quote(action), action.as_str())
            };
            let title = crate::observe::quote(&format!("{subject} writes only {desc}"));
            add(format!("after {scope} {title}\n  {writes}\n"));
        }
        blocks
    }

    pub fn print(&self, existing: &[PropertyDecl]) {
        println!(
            "Observed over {} scripts and {} steps, not proved; keep the ones the app means:\n",
            self.scripts, self.steps
        );
        for block in self.blocks(existing) {
            println!("{block}");
        }
    }
}
fn literal(value: &str) -> Option<String> {
    if let Some(n) = number(value) {
        let text = exact_runner::stdlib::format_number(n);
        return (n.is_finite()
            && text
                .parse::<f64>()
                .is_ok_and(|v| v.to_bits() == n.to_bits()))
        .then_some(text);
    }
    if value.starts_with('"') || matches!(value, "none" | "some" | "true" | "false") {
        Some(value.to_string())
    } else {
        None
    }
}
fn same_form(a: &PropertyForm, b: &PropertyForm) -> bool {
    match (a, b) {
        (PropertyForm::OnlyWrites(a), PropertyForm::OnlyWrites(b)) => {
            a.iter().collect::<BTreeSet<_>>() == b.iter().collect::<BTreeSet<_>>()
        }
        (
            PropertyForm::Hold {
                name: a,
                test: PropertyTest::In(av),
            },
            PropertyForm::Hold {
                name: b,
                test: PropertyTest::In(bv),
            },
        ) => a == b && av.len() == bv.len() && av.iter().all(|v| bv.contains(v)),
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(values: &[(&str, &str, &str)]) -> Vec<String> {
        values
            .iter()
            .enumerate()
            .flat_map(|(i, (n, ready, answer))| {
                vec![
                    if i == 0 {
                        "== boot".into()
                    } else {
                        "== clock +1".into()
                    },
                    "outcome ok".into(),
                    format!("slot n {n}"),
                    format!("slot ready {ready}"),
                    format!("slot answer {answer}"),
                ]
            })
            .collect()
    }
    fn search() -> Suggestions {
        Suggestions {
            values: [
                ("n", TypeKind::Number),
                ("ready", TypeKind::Bool),
                ("answer", TypeKind::Option),
            ]
            .into_iter()
            .map(|(name, kind)| {
                (
                    name.into(),
                    Values {
                        kind: Some(kind),
                        ..Values::default()
                    },
                )
            })
            .collect(),
            ..Suggestions::default()
        }
    }
    #[test]
    fn printed_numbers_must_represent_the_observed_bits() {
        assert_eq!(literal("n8000000000000000"), None);
        assert_eq!(literal("n3fb999999999999a"), Some("0.1".into()));
    }
    #[test]
    fn stays_sets_whole_options_and_frames_obey_their_evidence() {
        let mut s = search();
        let lines = steps(&[
            ("n0000000000000000", "false", "some(true)"),
            ("n3ff0000000000000", "true", "some(false)"),
            ("n4000000000000000", "true", "some(true)"),
        ]);
        s.observe(&lines);
        let blocks = s.blocks(&[]).join("\n");
        assert!(blocks.contains("state ready stays true"));
        assert!(!blocks.contains("state ready in"));
        assert!(blocks.contains("state n whole"));
        assert!(blocks.contains("state n in 0 1 2"));
        assert!(blocks.contains("state answer in some"));
        assert!(blocks.contains("only writes answer n ready"));
        assert!(
            crate::property::evaluate(&contract::properties(&blocks).unwrap(), &lines).passed()
        );
        s.observe(&steps(&[
            ("n3ff8000000000000", "true", "none"),
            ("n4000000000000000", "false", "none"),
        ]));
        let blocks = s.blocks(&[]).join("\n");
        assert!(!blocks.contains("state ready stays true"));
        assert!(!blocks.contains("state n whole"));
        assert!(!blocks.contains("state answer in some"));
    }
    #[test]
    fn an_action_frame_includes_the_queue_slot_and_requires_two_eligible_pairs() {
        let mut s = search();
        let lines = [
            "== boot",
            "outcome ok",
            "slot answer none",
            "== tap \"send\"",
            "outcome ok",
            "ran handler send ok",
            "slot answer none",
            "== land",
            "outcome ok",
            "ran queue answer ok",
            "ran then landed ok",
            "slot answer some(true)",
        ]
        .map(str::to_string);
        s.observe(&lines);
        assert!(!s
            .blocks(&[])
            .iter()
            .any(|b| b.starts_with("after \"send\"")));
        s.observe(&lines);
        assert!(s
            .blocks(&[])
            .iter()
            .any(|b| b.contains("after \"send\"") && b.contains("only writes answer")));
        let mut excluded = search();
        let mut lines = lines.to_vec();
        lines.insert(6, crate::observe::OMITTED_COMMIT.into());
        excluded.observe(&lines);
        excluded.observe(&lines);
        assert!(!excluded
            .blocks(&[])
            .iter()
            .any(|b| b.starts_with("after \"send\"")));
    }
}
