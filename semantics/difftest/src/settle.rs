//! Expand the driver's settle from the runner's live task deadlines (LLP 1148 D4.1).

use crate::script::{Event, Item, Scripted};

pub(crate) fn expand(script: &mut Scripted, plan: Option<&exact_plan::Plan>) -> Result<(), String> {
    if !script
        .items
        .iter()
        .any(|i| matches!(i, Item::Settle { .. }))
    {
        return Ok(());
    }
    let plan = match plan {
        Some(plan) => plan.clone(),
        None => crate::compile(&script.case)?,
    };
    let oracle = crate::oracle::Oracle::for_case(&plan, &script.case);
    let mut runner = crate::observe::boot(plan, oracle).ok();
    let spans = std::mem::take(&mut script.delivery.events);
    let mut spans = spans.into_iter();
    let mut expanded = Vec::new();
    let mut count = 0;
    for item in std::mem::take(&mut script.items) {
        match item {
            Item::Event(e) => {
                if let Some(r) = &mut runner {
                    let _ = crate::observe::apply(r, &e);
                }
                expanded.push(Item::Event(e));
                if let Some(span) = spans.next() {
                    script.delivery.events.push(span);
                }
            }
            Item::Expect(e) => expanded.push(Item::Expect(e)),
            Item::Settle { span, taken } => {
                let mut emit =
                    |e: Event, runner: &mut Option<exact_runner::Runner<crate::oracle::Oracle>>| {
                        if let Some(r) = runner {
                            let _ = crate::observe::apply(r, &e);
                        }
                        expanded.push(Item::Event(e));
                        script.delivery.events.push(span);
                        count += 1;
                    };
                emit(Event::Clock(0.0), &mut runner);
                for _ in 0..16 {
                    let Some(r) = &runner else { break };
                    if r.is_poisoned() {
                        break;
                    }
                    let Some(at) = soon(r).iter().map(|(_, at)| *at).reduce(f64::max) else {
                        break;
                    };
                    emit(Event::Clock(at - r.now_ms()), &mut runner);
                    emit(Event::Clock(0.0), &mut runner);
                }
                let due = runner.as_ref().map(soon).unwrap_or_default();
                if !due.is_empty() {
                    let names = due
                        .iter()
                        .map(|(name, at)| format!("{name} at {at} ms"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    script.delivery.stopped = Some((
                        span,
                        "clock settle".into(),
                        format!("tasks still due after sixteen rounds: {names}"),
                    ));
                    script.delivery.taken = taken;
                    break;
                }
            }
        }
    }
    script.items = expanded;
    script.case.events = script
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Event(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    script.delivery.settle_events = Some(count);
    let text = script
        .case
        .events
        .iter()
        .map(Event::label)
        .collect::<Vec<_>>()
        .join("\n");
    let dir = crate::work_dir().join("expansions");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let key = crate::seed_of(&format!(
        "{}\n{}\n{text}",
        script.case.name, script.case.source
    ));
    let path = dir.join(format!("{key:016x}.events"));
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    script.delivery.expansion = Some(path);
    Ok(())
}

fn soon(r: &exact_runner::Runner<crate::oracle::Oracle>) -> Vec<(String, f64)> {
    let now = r.now_ms();
    r.tasks()
        .into_iter()
        .filter_map(|(name, at)| {
            at.filter(|at| at.is_finite() && now < *at && *at <= now + 1.0)
                .map(|at| (name, at))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::script::{self, Event};

    #[test]
    fn expansion_uses_live_gates_and_keeps_the_authored_locations() {
        let s = script::scripted(
            "settle",
            include_str!("../../corpus/properties/settle.contract"),
            None,
        )
        .unwrap()
        .remove(0);
        crate::expectations(&s).unwrap();
        assert_eq!(s.delivery.settle_events, Some(4));
        assert_eq!(
            &s.case.events[s.case.events.len() - 3..],
            &[Event::Clock(0.0), Event::Clock(1.0), Event::Clock(0.0)]
        );
        assert_eq!(s.delivery.events.len(), s.case.events.len());
        let spans = &s.delivery.events[s.delivery.events.len() - 3..];
        assert!(spans.iter().all(|s| *s == spans[0]));
        let text = std::fs::read_to_string(s.delivery.expansion.as_ref().unwrap()).unwrap();
        assert_eq!(
            text,
            s.case
                .events
                .iter()
                .map(Event::label)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn repeating_tasks_stop_the_prefix_after_sixteen_rounds() {
        let source = "component App\n  state n = 0\n  action tick\n    n = n + 1\n  task repeat mount\n    every(1, tick)\n  view\n    text \"hi\"\n\ntest \"bound\"\n  expect state n == 0\n  clock settle\n  expect state n == 999\n  clock +100\n";
        let s = script::scripted("bound", source, None).unwrap().remove(0);
        assert_eq!(s.delivery.settle_events, Some(33));
        assert_eq!(s.delivery.taken, 1);
        assert!(s
            .delivery
            .stopped
            .as_ref()
            .unwrap()
            .2
            .contains("repeat at 17 ms"));
        assert_eq!(s.case.events.len(), 33);
        crate::expectations(&s).unwrap();
    }
}
