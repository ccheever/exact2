//! Literal properties over the observation shared by the runner and Lean.

use contract_syntax::{
    PropertyDecl, PropertyForm as F, PropertyKind as K, PropertyTest as T, PropertyValue as V,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Count {
    pub checked: usize,
    pub counted: usize,
    pub false_: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failure {
    pub property: usize,
    pub line: usize,
    pub step: usize,
    pub observed: String,
    pub previous: Option<String>,
}
#[derive(Default)]
pub(crate) struct Report {
    pub counts: Vec<Vec<Count>>,
    pub failures: Vec<Failure>,
    pub judged: BTreeMap<(usize, usize, usize), bool>,
}
impl Report {
    pub fn new(properties: &[PropertyDecl]) -> Self {
        Self {
            counts: properties
                .iter()
                .map(|p| vec![Count::default(); p.lines.len().max(1)])
                .collect(),
            failures: Vec::new(),
            judged: BTreeMap::new(),
        }
    }
    pub fn merge(&mut self, other: &Self) {
        for (a, b) in self
            .counts
            .iter_mut()
            .flatten()
            .zip(other.counts.iter().flatten())
        {
            a.checked += b.checked;
            a.counted += b.counted;
            a.false_ += b.false_;
        }
    }
    pub fn passed(&self) -> bool {
        self.counts.iter().flatten().all(|c| c.false_ == 0)
    }
}

pub(crate) struct Step<'a> {
    pub(crate) lines: &'a [String],
    pub(crate) label: &'a str,
    pub(crate) ran: Vec<(&'a str, &'a str, &'a str)>,
    pub(crate) config: bool,
    pub(crate) omitted: bool,
}
impl<'a> Step<'a> {
    pub(crate) fn new(lines: &'a [String]) -> Self {
        Self {
            lines,
            label: lines
                .first()
                .and_then(|l| l.strip_prefix("== "))
                .unwrap_or(""),
            ran: lines
                .iter()
                .filter_map(|l| {
                    let mut w = l.strip_prefix("ran ")?.split_whitespace();
                    Some((w.next()?, w.next()?, w.next()?))
                })
                .collect(),
            config: lines.iter().any(|l| {
                ["slot ", "derive ", "resource ", "command ", "view "]
                    .iter()
                    .any(|p| l.starts_with(p))
            }),
            omitted: lines.iter().any(|l| l == crate::observe::OMITTED_COMMIT),
        }
    }
    pub(crate) fn input(&self) -> bool {
        self.label.starts_with("tap ") || self.label.starts_with("type ")
    }
    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| {
            l.strip_prefix(&format!("slot {name} "))
                .or_else(|| l.strip_prefix(&format!("derive {name} ")))
        })
    }
    fn line(&self, name: &str) -> String {
        self.lines
            .iter()
            .find(|l| {
                l.starts_with(&format!("slot {name} ")) || l.starts_with(&format!("derive {name} "))
            })
            .cloned()
            .unwrap_or_else(|| format!("slot {name} <not printed>"))
    }
}

pub(crate) fn evaluate(properties: &[PropertyDecl], observation: &[String]) -> Report {
    let steps: Vec<_> = crate::chunks(observation)
        .into_iter()
        .map(Step::new)
        .collect();
    let mut report = Report::new(properties);
    for (pi, p) in properties.iter().enumerate() {
        if p.kind == K::NeverRefuses {
            for (si, step) in steps.iter().enumerate() {
                for (kind, name, outcome) in &step.ran {
                    if *name != p.name {
                        continue;
                    }
                    report.counts[pi][0].checked += 1;
                    report
                        .judged
                        .entry((pi, 0, si))
                        .and_modify(|held| *held &= *outcome != "refused")
                        .or_insert(*outcome != "refused");
                    if *outcome == "refused" {
                        report.counts[pi][0].false_ += 1;
                        report.failures.push(Failure {
                            property: pi,
                            line: 0,
                            step: si,
                            observed: format!("ran {kind} {name} {outcome}"),
                            previous: None,
                        });
                    }
                }
            }
            continue;
        }
        for (si, step) in steps.iter().enumerate() {
            let previous = si.checked_sub(1).map(|k| &steps[k]);
            let (old, current, commands, eligible, at) = match &p.kind {
                K::Always => (previous, step, vec![step], step.config, si),
                K::After(action) => {
                    if !step.input()
                        || !step
                            .ran
                            .iter()
                            .any(|(k, a, _)| *k == "handler" && *a == action)
                    {
                        continue;
                    }
                    let land = steps.get(si + 1).filter(|s| s.label == "land");
                    let eligible = step.ran.as_slice() == [("handler", action.as_str(), "ok")]
                        && !step.omitted
                        && step.config
                        && previous.is_some_and(|s| s.config)
                        && land.is_some_and(|s| {
                            s.config
                                && !s.omitted
                                && s.ran.iter().all(|(k, _, _)| matches!(*k, "then" | "queue"))
                        });
                    (
                        previous,
                        land.unwrap_or(step),
                        vec![step, land.unwrap_or(step)],
                        eligible,
                        if land.is_some() { si + 1 } else { si },
                    )
                }
                K::Clock => {
                    if !(step.label.starts_with("clock +")
                        || (step.label == "land" && !previous.is_some_and(Step::input)))
                    {
                        continue;
                    }
                    (
                        previous,
                        step,
                        vec![step],
                        step.config && !step.omitted && previous.is_some_and(|s| s.config),
                        si,
                    )
                }
                K::NeverRefuses => unreachable!(),
            };
            for (li, line) in p.lines.iter().enumerate() {
                let count = &mut report.counts[pi][li];
                let needs_previous = matches!(
                    &line.form,
                    F::Hold {
                        test: T::Stays(_),
                        ..
                    }
                ) && si > 0;
                if !eligible || (needs_previous && !old.is_some_and(|s| s.config)) {
                    count.counted += 1;
                    continue;
                }
                count.checked += 1;
                let held = holds(&line.form, old, current, &commands);
                report.judged.insert((pi, li, at), held);
                if !held {
                    count.false_ += 1;
                    let (observed, previous) = match &line.form {
                        F::Hold { name, .. } => (current.line(name), old.map(|s| s.line(name))),
                        F::OnlyWrites(_) => {
                            let changed = changed(old, current);
                            (
                                changed
                                    .iter()
                                    .map(|n| current.line(n))
                                    .collect::<Vec<_>>()
                                    .join("; "),
                                old.map(|s| {
                                    changed
                                        .iter()
                                        .map(|n| s.line(n))
                                        .collect::<Vec<_>>()
                                        .join("; ")
                                }),
                            )
                        }
                        F::Command { .. } => (
                            commands
                                .iter()
                                .flat_map(|s| s.lines.iter())
                                .filter(|l| l.starts_with("command "))
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("; "),
                            None,
                        ),
                    };
                    report.failures.push(Failure {
                        property: pi,
                        line: li,
                        step: at,
                        observed,
                        previous,
                    });
                }
            }
        }
    }
    report
}

fn literal(v: &V) -> String {
    match v {
        V::Number(n) => crate::observe::number(*n),
        V::String(s) => crate::observe::quote(s),
        V::Bool(b) => b.to_string(),
        V::None => "none".into(),
        V::Some => "some".into(),
        V::EmptyList => "[]".into(),
    }
}
pub(crate) fn number(value: &str) -> Option<f64> {
    let hex = value.strip_prefix('n')?;
    (hex.len() == 16)
        .then(|| u64::from_str_radix(hex, 16).ok().map(f64::from_bits))
        .flatten()
}
pub(crate) fn changed(old: Option<&Step<'_>>, current: &Step<'_>) -> BTreeSet<String> {
    let slots = |s: &Step<'_>| -> BTreeMap<String, String> {
        s.lines
            .iter()
            .filter_map(|l| {
                l.strip_prefix("slot ")?
                    .split_once(' ')
                    .map(|(n, v)| (n.into(), v.into()))
            })
            .collect()
    };
    let before = old.map(slots).unwrap_or_default();
    let after = slots(current);
    before
        .keys()
        .chain(after.keys())
        .filter(|n| before.get(*n) != after.get(*n))
        .cloned()
        .collect()
}
fn holds(form: &F, old: Option<&Step<'_>>, current: &Step<'_>, commands: &[&Step<'_>]) -> bool {
    match form {
        F::Hold { name, test } => {
            let Some(value) = current.value(name) else {
                return false;
            };
            let before = old.and_then(|s| s.value(name));
            match test {
                T::Equal(v) => value == literal(v),
                T::Old => before == Some(value),
                T::Add(n) => before
                    .and_then(number)
                    .zip(number(value))
                    .is_some_and(|(a, b)| a + n == b),
                T::Subtract(n) => before
                    .and_then(number)
                    .zip(number(value))
                    .is_some_and(|(a, b)| a - n == b),
                T::NotOld => matches!(
                    (before, value),
                    (Some("true"), "false") | (Some("false"), "true")
                ),
                T::In(vs) => vs.iter().any(|v| {
                    if *v == V::Some {
                        value.starts_with("some(") && value.ends_with(')')
                    } else {
                        value == literal(v)
                    }
                }),
                T::AtLeast(n) => number(value).is_some_and(|v| v >= *n),
                T::AtMost(n) => number(value).is_some_and(|v| v <= *n),
                T::Whole => number(value).is_some_and(|v| v.is_finite() && v.fract() == 0.0),
                T::Stays(v) => before != Some(literal(v).as_str()) || value == literal(v),
            }
        }
        F::OnlyWrites(names) => {
            old.is_some() && changed(old, current).iter().all(|n| names.contains(n))
        }
        F::Command { name, args } => {
            let line = format!(
                "command {name}{}",
                args.iter()
                    .map(|v| format!(" {}", literal(v)))
                    .collect::<String>()
            );
            commands.iter().any(|s| s.lines.contains(&line))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, text: &str) -> Report {
        evaluate(
            &contract::properties(source).unwrap(),
            &text.lines().map(str::to_string).collect::<Vec<_>>(),
        )
    }
    fn n(x: f64) -> String {
        crate::observe::number(x)
    }
    fn pair(before: &str, input: &str, land: &str) -> String {
        format!("== boot\noutcome ok\n{before}\n== tap \"button\"\noutcome ok\nran handler act ok\n{input}\n== land\noutcome ok\n{land}\n")
    }

    #[test]
    fn literals_membership_bounds_and_whole_read_slots_and_derives() {
        let source = r#"always "values"
  expect state a == -2
  expect state b == "two words"
  expect state c == true
  expect state d == none
  expect state e == []
  expect state f in none some
  expect state a in 1 -2 3
  expect state a >= -2
  expect state a <= -2
  expect state a whole
"#;
        let text = format!("== boot\noutcome ok\nderive a {}\nslot b \"two words\"\nslot c true\nslot d none\nslot e []\nslot f some({{\"answer\",[]}})\n", n(-2.0));
        let r = run(source, &text);
        assert!(r.passed());
        assert!(r.counts[0].iter().all(|c| c.checked == 1));
        for bad in [n(-2.5), n(f64::NAN), n(f64::INFINITY), "\"-2\"".into()] {
            let r = run(source, &text.replace(&n(-2.0), &bad));
            assert!(!r.passed(), "{bad}");
        }
        assert!(!run(
            source,
            &text.replace("some({\"answer\",[]})", "\"some(answer)\"")
        )
        .passed());
    }

    #[test]
    fn after_reads_before_the_input_and_after_the_landing() {
        let source = r#"after "act" "changes"
  expect state a == old + 1
  expect state b == old - -2
  expect state c == not old
  expect state d == old
  expect state e == "landed"
  only writes a b c e queued
  command focus "done"
  command mark -2 true none []
"#;
        let before = format!("slot a {}\nslot b {}\nslot c false\nderive d \"same\"\nslot e \"before\"\nslot queued none", n(4.0), n(-3.0));
        let input = format!("{before}\ncommand focus \"done\"");
        let land = format!("ran then received ok\nran queue queued ok\nslot a {}\nslot b {}\nslot c true\nderive d \"same\"\nslot e \"landed\"\nslot queued some(true)\ncommand mark {} true none []", n(5.0), n(-1.0), n(-2.0));
        let text = pair(&before, &input, &land);
        let r = run(source, &text);
        assert!(r.passed(), "{:?}", r.failures);
        assert!(r.counts[0].iter().all(|c| c.checked == 1 && c.counted == 0));
        assert!(!run(
            source,
            &text.replace("slot e \"landed\"", "slot e \"wrong\"")
        )
        .passed());
        assert!(!run(
            source,
            &text.replace("command focus \"done\"", "command focus \"done later\"")
        )
        .passed());
        assert!(!run(
            source,
            &text.replace(
                "slot queued some(true)",
                "slot extra true\nslot queued some(true)"
            )
        )
        .passed());
    }

    #[test]
    fn adding_one_at_two_to_the_fifty_third_rounds_to_the_same_double() {
        let source = "after \"act\" \"rounded\"\n  expect state a == old + 1\n";
        let slot = format!("slot a {}", n(9007199254740992.0));
        let text = pair(&slot, &slot, &slot);
        assert!(run(source, &text).passed());
        let changed = text.rfind(&slot).unwrap();
        let text = format!("{}slot a {}\n", &text[..changed], n(9007199254740994.0));
        assert!(!run(source, &text).passed());
    }

    #[test]
    fn extra_commits_missing_landings_and_omitted_commits_are_counted_per_line() {
        let source = "after \"act\" \"alone\"\n  writes none\n  expect state a == old\n";
        let slot = "slot a true";
        for (input, land) in [
            ("ran task due ok\nslot a true", slot),
            (slot, "ran handler other ok\nslot a true"),
            (
                "# a commit the semantics has no branch for\nslot a true",
                slot,
            ),
            (
                slot,
                "# a commit the semantics has no branch for\nslot a true",
            ),
        ] {
            let r = run(source, &pair(slot, input, land));
            assert!(r.passed());
            assert!(r.counts[0].iter().all(|c| c.checked == 0 && c.counted == 1));
        }
        let r = run(source, &format!("== boot\noutcome ok\n{slot}\n== tap \"button\"\noutcome ok\nran handler act ok\n{slot}\n"));
        assert_eq!(r.counts[0][0].counted, 1);
        assert_eq!(run(source, &pair(slot, slot, slot)).counts[0][0].checked, 1);
    }

    #[test]
    fn clock_checks_all_commits_but_an_inputs_landing_belongs_to_its_pair() {
        let source =
            "after clock \"clock\"\n  only writes result queued\n  expect state a == old\n";
        let text = "== boot\noutcome ok\nslot a true\nslot result none\nslot queued none\n== tap \"button\"\noutcome ok\nran handler act ok\nslot a true\n== land\noutcome ok\nran then done ok\nslot a false\n== clock +5\noutcome ok\nran task tick ok\nran task tick ok\nran queue queued ok\nslot a false\nslot result some(true)\nslot queued some(false)\n== land\noutcome ok\nran then done ok\nslot a false\nslot result some(true)\nslot queued some(false)\n";
        let r = run(source, text);
        assert!(r.passed(), "{:?}", r.failures);
        assert_eq!(r.counts[0][0].checked, 2);
        let r = run(
            source,
            &text.replace(
                "== clock +5\n",
                "== clock +5\n# a commit the semantics has no branch for\n",
            ),
        );
        assert_eq!(r.counts[0][0].counted, 1);
    }

    #[test]
    fn stays_checks_adjacent_configurations_and_counts_poisoned_gaps() {
        let source = "always \"ready\"\n  expect state ready stays true\n";
        let text = "== boot\noutcome ok\nslot ready false\n== clock +1\noutcome ok\nslot ready true\n== clock +1\noutcome ok\nslot ready true\n";
        assert!(run(source, text).passed());
        let r = run(
            source,
            &format!("{text}== clock +1\noutcome ok\nslot ready false\n"),
        );
        assert_eq!(r.failures.len(), 1);
        assert_eq!(r.failures[0].step, 3);
        let r = run(
            source,
            &format!(
                "{text}== clock +1\noutcome poisoned\n== clock +1\noutcome ok\nslot ready false\n"
            ),
        );
        assert_eq!(r.counts[0][0].counted, 2);
        assert!(r.passed());
    }

    #[test]
    fn never_refuses_attributes_a_due_then_instead_of_the_tap() {
        let source = "never refuses \"due\"\nnever refuses \"act\"\nnever refuses \"unseen\"\n";
        let text = "== boot\noutcome ok\nslot a true\n== tap \"button\"\noutcome refused\nran then due refused\nran handler act ok\nslot a true\n";
        let r = run(source, text);
        assert_eq!(r.failures.len(), 1);
        assert_eq!(r.failures[0].observed, "ran then due refused");
        assert_eq!(r.counts[1][0].checked, 1);
        assert_eq!(r.counts[2][0].checked, 0);
    }

    #[test]
    fn the_same_line_can_hold_on_only_one_observation() {
        let source = "after \"act\" \"adds one\"\n  expect state a == old + 1\n";
        let one = format!("slot a {}", n(1.0));
        let two = format!("slot a {}", n(2.0));
        let r = run(source, &pair(&one, &two, &two));
        let l = run(source, &pair(&two, &two, &two));
        assert!(r.judged[&(0, 0, 2)]);
        assert!(!l.judged[&(0, 0, 2)]);
        assert_eq!(l.failures[0].previous.as_deref(), Some(two.as_str()));
    }

    #[test]
    fn writes_none_compares_whole_slot_text_including_queue_slots() {
        let source = "after \"act\" \"frame\"\n  writes none\n";
        let before = "slot queued none\nslot rec {true,[]}";
        assert!(run(source, &pair(before, before, before)).passed());
        assert!(!run(
            source,
            &pair(before, before, "slot queued some(true)\nslot rec {true,[]}")
        )
        .passed());
        assert!(!run(
            source,
            &pair(before, before, "slot queued none\nslot rec {false,[]}")
        )
        .passed());
    }
}
