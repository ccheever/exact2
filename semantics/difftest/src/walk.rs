//! Adaptive scripts over rendered controls, with replayable oracle seeds.

use crate::{
    observe::Driver,
    oracle::Oracle,
    rng::Rng,
    script::{Case, Event, Item, Scripted},
};
use contract_syntax::{
    lexer::{Lexer, TokenKind},
    Step, TestDecl,
};
use exact_plan::Plan;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct Inputs {
    authored: BTreeMap<String, Vec<String>>,
    literals: Vec<String>,
}
impl Inputs {
    pub fn new(source: &str, tests: impl Iterator<Item = TestDecl>) -> Self {
        let mut inputs = Self::default();
        for test in tests {
            for step in test.steps {
                if let Step::Type { target, text, .. } = step {
                    inputs.authored.entry(target).or_default().push(text);
                }
            }
        }
        if let Ok(tokens) = Lexer::tokenize(source, 1) {
            for token in tokens {
                if let TokenKind::Str(s) = token.kind {
                    if !inputs.literals.contains(&s) {
                        inputs.literals.push(s);
                    }
                }
            }
        }
        inputs
    }

    fn draw(&self, id: &str, visit: usize, rng: &mut Rng) -> String {
        let authored = self.authored.get(id).map_or(&[][..], Vec::as_slice);
        if let Some(s) = authored.get(visit) {
            return s.clone();
        }
        if let Some(s) = self.literals.get(visit.saturating_sub(authored.len())) {
            return s.clone();
        }
        crate::gen::input(rng)
    }
}

pub(crate) struct Walk {
    pub scripted: Scripted,
    pub observation: Vec<String>,
    pub rendered: BTreeSet<String>,
}

pub(crate) fn generate(
    plan: &Plan,
    base: &Case,
    prefix: Option<&Case>,
    seed: u64,
    index: usize,
    steps: usize,
    inputs: &Inputs,
) -> Walk {
    let start = prefix.unwrap_or(base);
    let from = prefix.map_or("boot", |c| c.name.as_str());
    let mut case = Case {
        name: format!("walk {} from {from:?}, seed 0x{seed:016x}", index + 1),
        seed: Some(prefix.map_or(seed, |c| c.seed.unwrap_or_else(|| crate::seed_of(&c.name)))),
        events: prefix.map_or_else(Vec::new, |c| c.events.clone()),
        ..start.clone()
    };
    let (mut driver, mut observation) = Driver::new(plan.clone(), Oracle::for_case(plan, &case));
    let mut rendered = BTreeSet::new();
    let mut tapped = BTreeSet::new();
    let mut visits = BTreeMap::<String, usize>::new();
    let record = |d: &Driver, rendered: &mut BTreeSet<String>| {
        for c in d.controls() {
            rendered.extend(c.press.into_iter().chain(c.input));
        }
    };
    record(&driver, &mut rendered);
    for event in &case.events {
        observation.extend(driver.step(event));
        if let Event::Tap(id) = event {
            tapped.insert(id.clone());
        }
        record(&driver, &mut rendered);
    }
    let mut clock = if prefix.is_some() {
        crate::settle::MutationClock::resume(&driver)
    } else {
        crate::settle::MutationClock::default()
    };
    let mut mutation_events = 0;
    let mut mutation_bounds = 0;
    let interval = plan
        .timers
        .iter()
        .map(|t| {
            if t.frame {
                1000.0 / 60.0
            } else {
                f64::from(t.interval_ms)
            }
        })
        .filter(|n| *n >= 0.0)
        .reduce(f64::min)
        .unwrap_or(1000.0);
    let mut rng = Rng::new(seed);
    let mut added = 0;
    while added < steps && driver.live() {
        let controls = driver.controls();
        let presses: Vec<_> = controls.iter().filter(|c| c.press.is_some()).collect();
        let fresh: Vec<_> = presses
            .iter()
            .copied()
            .filter(|c| !tapped.contains(&c.id))
            .collect();
        let types: Vec<_> = controls.iter().filter(|c| c.input.is_some()).collect();
        let paired = steps - added >= 2;
        let choice = rng.weighted(&[
            if paired && !presses.is_empty() { 6 } else { 0 },
            if paired && !types.is_empty() { 3 } else { 0 },
            2,
        ]);
        let event = match choice {
            0 => {
                let c = rng.pick(if fresh.is_empty() { &presses } else { &fresh });
                tapped.insert(c.id.clone());
                Event::Tap(c.id.clone())
            }
            1 => {
                let c = rng.pick(&types);
                let visit = visits.entry(c.id.clone()).or_default();
                let value = inputs.draw(&c.id, *visit, &mut rng);
                *visit += 1;
                Event::Type(c.id.clone(), value)
            }
            _ => Event::Clock(if rng.chance(1, 2) { interval } else { 60_000.0 }),
        };
        let input = matches!(event, Event::Tap(_) | Event::Type(..));
        let lines = driver.step(&event);
        let delivered = crate::settle::delivered(&lines);
        observation.extend(lines);
        case.events.push(event);
        added += 1;
        record(&driver, &mut rendered);
        if input && delivered && driver.live() {
            observation.extend(driver.step(&Event::InputLand));
            case.events.push(Event::InputLand);
            added += 1;
            record(&driver, &mut rendered);
        }
        let (events, lines, bounded) = clock.advance(&mut driver);
        mutation_events += events.len();
        mutation_bounds += usize::from(bounded);
        case.events.extend(events);
        observation.extend(lines);
        record(&driver, &mut rendered);
        if bounded {
            break;
        }
    }
    Walk {
        scripted: Scripted {
            items: case.events.iter().cloned().map(Item::Event).collect(),
            case,
            delivery: crate::script::Delivery {
                mutation_events,
                mutation_bounds,
                ..Default::default()
            },
        },
        observation,
        rendered,
    }
}
