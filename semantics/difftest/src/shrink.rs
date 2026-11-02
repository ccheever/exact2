//! Delete delivery groups while preserving a runner finding or a two-engine divergence.

use crate::{
    observe,
    oracle::Oracle,
    script::{Case, Event},
};
use exact_plan::Plan;

pub(crate) struct Shrunk {
    pub case: Case,
    pub observation: Vec<String>,
    pub kept: Vec<usize>,
    pub candidates: usize,
}

pub(crate) fn runner(
    plan: &Plan,
    case: &Case,
    observation: &[String],
    predicate: impl Fn(&[String]) -> bool,
    mut seen: impl FnMut(&[String]),
) -> Shrunk {
    let mut best = Shrunk {
        case: case.clone(),
        observation: observation.to_vec(),
        kept: (0..case.events.len()).collect(),
        candidates: 0,
    };
    let mut width = groups(&best.case.events).len().max(1);
    loop {
        let before = best.case.events.len();
        let mut start = 0;
        while start < groups(&best.case.events).len() {
            let ranges = groups(&best.case.events);
            let range = ranges[start].start..ranges[(start + width).min(ranges.len()) - 1].end;
            let mut candidate = best.case.clone();
            candidate.events.drain(range.clone());
            let mut kept = best.kept.clone();
            kept.drain(range);
            let (lines, indices) = normalize(plan, &mut candidate);
            let kept = indices.into_iter().map(|i| kept[i]).collect();
            best.candidates += 1;
            seen(&lines);
            if predicate(&lines) {
                best.case = candidate;
                best.kept = kept;
                best.observation = lines;
            } else {
                start += width;
            }
        }
        if width == 1 {
            if before == best.case.events.len() {
                break;
            }
            continue;
        }
        width = width.div_ceil(2);
    }
    best
}

pub(crate) fn divergence(
    case: &Case,
    tag: &str,
    mut seen: impl FnMut(&[String]),
) -> Result<(Case, usize), String> {
    let mut best = case.clone();
    let mut count = 0;
    let mut width = groups(&best.events).len().max(1);
    loop {
        let before = best.events.len();
        let mut start = 0;
        while start < groups(&best.events).len() {
            let mut candidate = best.clone();
            let ranges = groups(&best.events);
            let range = ranges[start].start..ranges[(start + width).min(ranges.len()) - 1].end;
            candidate.events.drain(range);
            if let Ok(plan) = crate::compile(&candidate) {
                normalize(&plan, &mut candidate);
            }
            let outcome = crate::check(vec![candidate], 1, tag)?.remove(0);
            count += 1;
            if let Some((rust, _)) = &outcome.observations {
                seen(rust);
            }
            if matches!(outcome.verdict, crate::Verdict::Diverge { .. }) {
                best = outcome.case;
            } else {
                start += width;
            }
        }
        if width == 1 {
            if before == best.events.len() {
                break;
            }
            continue;
        }
        width = width.div_ceil(2);
    }
    Ok((best, count))
}

/// An input, its landing and its automatic clocks are one deletion unit.
fn groups(events: &[Event]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let start = i;
        i += 1;
        if matches!(events[start], Event::Tap(_) | Event::Type(..))
            && events.get(i) == Some(&Event::InputLand)
        {
            i += 1;
        }
        while matches!(events.get(i), Some(Event::TestClock(_))) {
            i += 1;
        }
        out.push(start..i);
    }
    out
}

fn normalize(plan: &Plan, case: &mut Case) -> (Vec<String>, Vec<usize>) {
    let (mut driver, mut lines) = observe::Driver::new(plan.clone(), Oracle::for_case(plan, case));
    let mut events = Vec::new();
    let mut kept = Vec::new();
    let mut skip = false;
    for (i, event) in case.events.iter().enumerate() {
        if skip && matches!(event, Event::InputLand | Event::TestClock(_)) {
            continue;
        }
        let step = driver.step(event);
        skip = matches!(event, Event::Tap(_) | Event::Type(..)) && !crate::settle::delivered(&step);
        lines.extend(step);
        events.push(event.clone());
        kept.push(i);
    }
    case.events = events;
    (lines, kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losing_an_input_keeps_the_authored_data_clock_after_its_landing() {
        let source = "component App\n  view\n    text \"hi\"\ntest \"missing\"\n  tap \"missing\"\n  clock data\n";
        let mut case = crate::script::scripted("missing", source, None)
            .unwrap()
            .remove(0)
            .case;
        let expected = vec![Event::Tap("missing".into()), Event::Land];
        assert_eq!(case.events, expected);
        assert_eq!(groups(&case.events), [0..1, 1..2]);
        let plan = crate::compile(&case).unwrap();
        normalize(&plan, &mut case);
        assert_eq!(case.events, expected);
        case.events
            .splice(1..1, [Event::InputLand, Event::TestClock(1.0)]);
        let (lines, kept) = normalize(&plan, &mut case);
        assert_eq!(case.events, expected);
        assert_eq!(kept, [0, 3]);
        assert_eq!(crate::chunks(&lines).last().unwrap()[0], "== land");
    }
}
