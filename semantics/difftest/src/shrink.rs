//! Runner-only event deletion with the finding's predicate held fixed.

use crate::{observe, oracle::Oracle, script::Case};
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
    let mut width = best.kept.len().max(1);
    loop {
        let before = best.case.events.len();
        let mut start = 0;
        while start < best.case.events.len() {
            let end = (start + width).min(best.case.events.len());
            let mut candidate = best.case.clone();
            candidate.events.drain(start..end);
            let oracle = Oracle::for_case(plan, &candidate);
            let (lines, _) = observe::run(plan.clone(), oracle, &candidate.events);
            best.candidates += 1;
            seen(&lines);
            if predicate(&lines) {
                best.case = candidate;
                best.kept.drain(start..end);
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
    let mut width = best.events.len().max(1);
    loop {
        let before = best.events.len();
        let mut start = 0;
        while start < best.events.len() {
            let mut candidate = best.clone();
            let end = (start + width).min(candidate.events.len());
            candidate.events.drain(start..end);
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
