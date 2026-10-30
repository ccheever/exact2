//! Replay the device runner's settle and mutation clocks (LLP 1151 D4.1).

use crate::{
    observe::Driver,
    script::{Event, Item, Scripted},
};

pub(crate) const HOST_WORLD: &str = "settle omits host waits on motion, scrolling and hatches, seeks to re-ask due times (the oracle schedules none), and advances to animation ends";

/// The counter is cumulative, including requests issued at boot.
#[derive(Default)]
pub(crate) struct MutationClock {
    cursor: u64,
}
impl MutationClock {
    pub fn resume(driver: &Driver) -> Self {
        Self {
            cursor: driver.mutation_requests(),
        }
    }

    pub fn advance(&mut self, driver: &mut Driver) -> (Vec<Event>, Vec<String>, bool) {
        let mut events = Vec::new();
        let mut lines = Vec::new();
        for _ in 0..16 {
            if !driver.live() {
                break;
            }
            let requests = driver.mutation_requests();
            let count = requests.saturating_sub(self.cursor);
            self.cursor = requests;
            if count == 0 {
                return (events, lines, false);
            }
            let event = Event::TestClock(count as f64);
            lines.extend(driver.step(&event));
            events.push(event);
        }
        let bounded = driver.live() && driver.mutation_requests() > self.cursor;
        (events, lines, bounded)
    }
}

pub(crate) fn delivered(lines: &[String]) -> bool {
    lines.iter().any(|l| l.starts_with("ran handler "))
}

pub(crate) fn expand(script: &mut Scripted, plan: Option<&exact_plan::Plan>) -> Result<(), String> {
    let plan = match plan {
        Some(plan) => plan.clone(),
        None => crate::compile(&script.case)?,
    };
    let oracle = crate::oracle::Oracle::for_case(&plan, &script.case);
    let (mut driver, _) = Driver::new(plan, oracle);
    let spans = std::mem::take(&mut script.delivery.events);
    let mut spans = spans.into_iter();
    let mut expanded = Vec::new();
    let mut clock = MutationClock::default();
    let mut skip_land = false;
    let mut count = 0;
    let mut settles = false;
    let mut origins = std::collections::BTreeMap::new();
    let mut items = std::mem::take(&mut script.items).into_iter().peekable();
    while let Some(item) = items.next() {
        let last_span;
        match item {
            Item::Event(e) => {
                last_span = spans.next().unwrap_or_default();
                if skip_land && e == Event::InputLand {
                    skip_land = false;
                    continue;
                }
                let input = matches!(e, Event::Tap(_) | Event::Type(..));
                let lines = driver.step(&e);
                skip_land = input && !delivered(&lines);
                expanded.push(Item::Event(e));
                script.delivery.events.push(last_span);
                // The device lands its input before advancing the test clock.
                if input
                    && !skip_land
                    && matches!(items.peek(), Some(Item::Event(Event::InputLand)))
                {
                    continue;
                }
            }
            Item::Expect(e, span) => {
                last_span = span;
                expanded.push(Item::Expect(e, span));
            }
            Item::Observe(span) => {
                last_span = span;
                expanded.push(Item::Observe(span));
            }
            Item::Until {
                path,
                literal,
                within,
                span,
            } => {
                last_span = span;
                origins.insert(
                    script.delivery.events.len(),
                    format!("# clock until state {path} == {literal} within {within}"),
                );
                let lines = driver.step(&Event::Land);
                expanded.push(Item::Event(Event::Land));
                script.delivery.events.push(span);
                let want = match &literal {
                    serde_json::Value::String(s) => crate::observe::quote(s),
                    serde_json::Value::Number(n) => crate::observe::number(n.as_f64().unwrap()),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "none".into(),
                    _ => unreachable!("the test parser accepts only scalar clock-until literals"),
                };
                let got = crate::script::state_value(&lines, &path);
                if got != Some(want.as_str()) {
                    let got = got.unwrap_or("<not printed>");
                    let shown = got
                        .strip_prefix('n')
                        .filter(|hex| hex.len() == 16)
                        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
                        .map_or_else(
                            || got.to_string(),
                            |bits| exact_runner::stdlib::format_number(f64::from_bits(bits)),
                        );
                    script.delivery.stopped = Some((
                        span,
                        "clock until (not met)".into(),
                        format!("clock until: {path} is {shown}, not {literal}; no answer is pending in this world"),
                    ));
                    break;
                }
                script.delivery.until_met += 1;
            }
            Item::Settle { span } => {
                last_span = span;
                settles = true;
                let emit = |e: Event,
                            driver: &mut Driver,
                            expanded: &mut Vec<Item>,
                            script: &mut Scripted| {
                    driver.step(&e);
                    expanded.push(Item::Event(e));
                    script.delivery.events.push(span);
                };
                emit(Event::Clock(0.0), &mut driver, &mut expanded, script);
                count += 1;
                for _ in 0..16 {
                    let Some(delta) = driver.soon() else {
                        break;
                    };
                    emit(Event::Clock(delta), &mut driver, &mut expanded, script);
                    emit(Event::Clock(0.0), &mut driver, &mut expanded, script);
                    count += 2;
                }
                script.delivery.settle_bounds += usize::from(driver.soon().is_some());
            }
        }
        let (events, _, bounded) = clock.advance(&mut driver);
        script.delivery.mutation_events += events.len();
        script.delivery.mutation_bounds += usize::from(bounded);
        script
            .delivery
            .events
            .extend(std::iter::repeat_n(last_span, events.len()));
        expanded.extend(events.into_iter().map(Item::Event));
        if bounded {
            script.delivery.stopped = Some((
                last_span,
                "clock (mutation bound)".into(),
                "mutations kept issuing requests across 16 automatic clock steps".into(),
            ));
            break;
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
    script.delivery.settle_events = settles.then_some(count);
    if settles || script.delivery.mutation_events > 0 || !origins.is_empty() {
        let text = script
            .case
            .events
            .iter()
            .enumerate()
            .map(|(i, event)| match origins.get(&i) {
                Some(origin) => format!("{origin}\n{}", event.label()),
                None => event.label(),
            })
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
    }
    Ok(())
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
    fn repeating_tasks_count_the_bound_and_continue() {
        let source = "component App\n  state n = 0\n  action tick\n    n = n + 1\n  task repeat mount\n    every(1, tick)\n  view\n    text \"hi\"\n\ntest \"bound\"\n  expect state n == 0\n  clock settle\n  expect state n == 16\n  clock +100\n";
        let s = script::scripted("bound", source, None).unwrap().remove(0);
        assert_eq!(s.delivery.settle_events, Some(33));
        assert_eq!(s.delivery.taken, 4);
        assert!(s.delivery.stopped.is_none());
        assert_eq!(s.delivery.settle_bounds, 1);
        assert_eq!(s.case.events.len(), 34);
        crate::expectations(&s).unwrap();
    }
    #[test]
    fn mutation_clock_fires_after_one_and_repeats_for_requests_the_advance_issues() {
        let source = "shape Ack\n  value: number\ncomponent App\n  state armed = false\n  state done = false\n  mutation saved as shape Ack\n  action start\n    armed = true\n    send saved = save()\n  action finish\n    done = true\n    send saved = save()\n  task later when armed\n    after(1, finish)\n  view\n    button \"start\" testId=\"start\" press=start\ntest \"mutation clock\"\n  tap \"start\"\n  expect state done == true\n";
        let script = script::scripted("mutations", source, None)
            .unwrap()
            .remove(0);
        assert_eq!(
            script.case.events,
            [
                Event::Tap("start".into()),
                Event::InputLand,
                Event::TestClock(1.0),
                Event::TestClock(1.0)
            ]
        );
        assert_eq!(script.delivery.mutation_events, 2);
        crate::expectations(&script).unwrap();
    }

    #[test]
    fn a_missing_input_has_no_landing() {
        let source = "component App\n  view\n    text \"hi\" testId=\"label\"\ntest \"missing\"\n  tap \"missing\"\n  tap \"label\"\n";
        let script = script::scripted("missing", source, None).unwrap().remove(0);
        assert_eq!(
            script.case.events,
            [Event::Tap("missing".into()), Event::Tap("label".into())]
        );
    }

    #[test]
    fn a_mutation_clock_bound_stops_before_the_next_authored_step() {
        let source = "shape Ack\n  value: number\ncomponent App\n  mutation saved as shape Ack\n  action tick\n    send saved = save()\n  task repeat mount\n    every(1, tick)\n  view\n    text \"hi\"\ntest \"bound\"\n  clock +1\n  clock +100\n";
        let script = script::scripted("bound", source, None).unwrap().remove(0);
        assert_eq!(script.delivery.mutation_events, 16);
        assert_eq!(script.delivery.mutation_bounds, 1);
        assert_eq!(script.delivery.taken, 1);
        assert_eq!(script.case.events.len(), 17);
        let (span, kind, _) = script.delivery.stopped.unwrap();
        assert_eq!(span.line, 12);
        assert_eq!(kind, "clock (mutation bound)");
    }
}
