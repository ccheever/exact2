//! Cases: a program and the events a host delivers to it.
//!
//! A corpus case is a `.contract` file with `test` blocks (LLP 1017 P7):
//! each block's inputs and clocks become primitive events, and its
//! `expect` lines are checked against the runner's observations. A random
//! case comes from [`crate::gen`].

use contract_syntax::{Expr, Step};

/// A host event, as both sides deliver it.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A press on the first element, in preorder, with this `testId`.
    Tap(String),
    /// A `change` carrying this text to the element with this `testId`.
    Type(String, String),
    /// Advance the clock by this many milliseconds, firing due timers.
    Clock(f64),
    /// A device mutation-clock advance, retained with its step when shrinking.
    TestClock(f64),
    /// Land armed mutation continuations and queued sends without firing timers.
    Land,
    /// The landing belonging to an input; printed and replayed as `land`.
    InputLand,
}

/// What a corpus test expects of the runner after the steps before it.
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// `expect state name == literal`: the observation line's value text.
    State(String, String),
    /// `expect text "id" == "value"`.
    Text(String, String),
    /// `expect tree has|missing "id"`.
    Tree(String, bool),
}

/// One step of a script: an event, or an expectation.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Deliver an event.
    Event(Event),
    /// Check the last observation.
    Expect(Expect, contract_syntax::Span),
    /// A host-only expectation still drains the device test clock.
    Observe(contract_syntax::Span),
    /// Expanded from live runner tasks before the case is compared.
    Settle { span: contract_syntax::Span },
}

/// A program and its script.
#[derive(Debug, Clone)]
pub struct Case {
    /// Where it came from: a file and test name, or a seed.
    pub name: String,
    /// An explicit oracle seed, retained when a case is renamed or shrunk.
    pub seed: Option<u64>,
    /// The Contract source.
    pub source: String,
    /// The events, in order.
    pub events: Vec<Event>,
    /// The file it was read from, when it resolves `use`s.
    pub path: Option<std::path::PathBuf>,
    /// Whether the device's store holds persisted states at launch
    /// (LLP 1116 D5, [`crate::oracle::Oracle::with_store`]); an authored
    /// test's starts empty.
    pub stored: bool,
    /// Whether the host builds every tab panel from boot on, as a page
    /// rendered ahead of its runtime does (LLP 1075.003 §9.14,
    /// `Runner::build_every_tab`); an authored test's panels wait.
    pub prerendered: bool,
    /// Fixed host facts supplied before boot.
    pub launch: exact_runner::time::Launch,
}

/// A corpus case: a [`Case`] whose script also holds expectations.
#[derive(Debug, Clone)]
pub struct Scripted {
    /// The case (its `events` are the script's events alone).
    pub case: Case,
    /// Events and expectations interleaved, as written.
    pub items: Vec<Item>,
    /// How far an authored block can be delivered.
    pub delivery: Delivery,
}

/// Delivery of an authored block under the synchronous oracle.
#[derive(Debug, Clone, Default)]
pub struct Delivery {
    /// Where delivery stopped: its authored location, kind and reason.
    pub stopped: Option<(contract_syntax::Span, String, String)>,
    /// Steps consumed before stopping, including expectations and ignored reads.
    pub taken: usize,
    /// The authored location of each delivered event.
    pub events: Vec<contract_syntax::Span>,
    /// Primitive events emitted for all settles in this block.
    pub settle_events: Option<usize>,
    /// Settles that still had due work after sixteen rounds.
    pub settle_bounds: usize,
    /// Advances issued by the device runner's mutation clock.
    pub mutation_events: usize,
    /// Mutation clocks that still issued requests after sixteen rounds.
    pub mutation_bounds: usize,
    /// The replayable expansion, including automatic clock steps.
    pub expansion: Option<std::path::PathBuf>,
}

impl Event {
    /// The step's label, exactly as `Contract.Observe.ScriptEvent.label` prints it.
    pub fn label(&self) -> String {
        match self {
            Event::Tap(t) => format!("tap {}", crate::observe::quote(t)),
            Event::Type(t, s) => format!(
                "type {} {}",
                crate::observe::quote(t),
                crate::observe::quote(s)
            ),
            Event::Clock(ms) | Event::TestClock(ms) => {
                format!("clock +{}", exact_runner::stdlib::format_number(*ms))
            }
            Event::Land | Event::InputLand => "land".into(),
        }
    }

    /// The event as a `Contract.Observe.ScriptEvent` term.
    pub fn lean(&self) -> String {
        use contract::lean::string;
        match self {
            Event::Tap(t) => format!("(.tap {})", string(t)),
            Event::Type(t, s) => format!("(.change {} {})", string(t), string(s)),
            Event::Clock(ms) | Event::TestClock(ms) => {
                format!("(.clock (F64.ofBits 0x{:016x}))", ms.to_bits())
            }
            Event::Land | Event::InputLand => "(.land)".into(),
        }
    }
}

/// The literal of an `expect state` line as an observation value prints.
fn literal(e: &Expr) -> Option<String> {
    use crate::observe::{number, quote};
    Some(match e {
        Expr::Number(n, _) => number(*n),
        Expr::Unary(contract_syntax::UnOp::Neg, inner, _) => match inner.as_ref() {
            Expr::Number(n, _) => number(-*n),
            _ => return None,
        },
        Expr::Str(s, _) => quote(s),
        Expr::Bool(b, _) => b.to_string(),
        Expr::None(_) => "none".into(),
        Expr::List(items, _) if items.is_empty() => "[]".into(),
        _ => return None,
    })
}

/// Every `test` block of a corpus file, as a scripted case. A step the
/// differential run cannot deliver ends that block's prefix.
pub fn scripted(
    file: &str,
    source: &str,
    path: Option<&std::path::Path>,
) -> Result<Vec<Scripted>, String> {
    let tests = contract::tests(source).map_err(|e| format!("{file}: {e}"))?;
    tests
        .iter()
        .map(|t| test_case(file, source, path, t, None))
        .collect()
}

/// One `test` block as a scripted case over the program `source`, which
/// may be another file's (an `app.test.contract` beside its
/// `app.contract`). Inputs include a following `land`; settles expand from
/// the runner's live tasks. `plan` reuses the caller's compilation when supplied.
pub fn test_case(
    file: &str,
    source: &str,
    path: Option<&std::path::Path>,
    t: &contract_syntax::TestDecl,
    plan: Option<&exact_plan::Plan>,
) -> Result<Scripted, String> {
    let mut items = Vec::new();
    let mut delivery = Delivery::default();
    let (mut epoch, mut zone) = (None, None);
    let mut leading = true;
    let mut launch_steps = 0;
    for step in &t.steps {
        if leading {
            match step {
                Step::Epoch { value, .. } if value != "now" => {
                    epoch = Some(value.as_str());
                    launch_steps += 1;
                    continue;
                }
                Step::TimeZone { zone: value, .. } => {
                    zone = Some(value.as_str());
                    launch_steps += 1;
                    continue;
                }
                _ => leading = false,
            }
        }
        let item = if matches!(step, Step::Clock { arg, .. } if arg == "settle") {
            Ok(Some(Item::Settle { span: step.span() }))
        } else {
            deliver(step)
        };
        let item = match item {
            Ok(item) => item,
            Err(why) => {
                delivery.stopped = Some((step.span(), step_kind(step), why));
                break;
            }
        };
        delivery.taken += 1;
        if let Some(item) = item {
            if matches!(item, Item::Event(_)) {
                delivery.events.push(step.span());
            }
            let input = matches!(item, Item::Event(Event::Tap(_) | Event::Type(_, _)));
            items.push(item);
            if input {
                items.push(Item::Event(Event::InputLand));
                delivery.events.push(step.span());
            }
        }
    }
    let events = items
        .iter()
        .filter_map(|i| match i {
            Item::Event(e) => Some(e.clone()),
            Item::Expect(..) | Item::Observe(_) | Item::Settle { .. } => None,
        })
        .collect();
    let mut script = Scripted {
        case: Case {
            seed: None,
            name: format!("{file}: {}", t.name),
            source: source.to_string(),
            events,
            path: path.map(std::path::Path::to_path_buf),
            stored: false,
            prerendered: false,
            launch: crate::launch::facts(epoch, zone)?,
        },
        items,
        delivery,
    };
    crate::settle::expand(&mut script, plan)?;
    if script.delivery.mutation_bounds > 0 {
        let span = script.delivery.stopped.as_ref().unwrap().0;
        script.delivery.taken = t
            .steps
            .iter()
            .take_while(|step| step.span().line <= span.line)
            .count()
            .saturating_sub(launch_steps);
    }
    Ok(script)
}

fn deliver(step: &Step) -> Result<Option<Item>, String> {
    if !delivered_kind(step.kind()) {
        return Err(format!(
            "{} is not delivered by the synchronous oracle",
            step_kind(step)
        ));
    }
    Ok(Some(match step {
        Step::Tap {
            target,
            form: contract_syntax::TapForm::Press,
            modifiers,
            ..
        } if modifiers.is_empty() => Item::Event(Event::Tap(target.clone())),
        Step::Type {
            target,
            text,
            append: false,
            ..
        } => Item::Event(Event::Type(target.clone(), text.clone())),
        Step::Clock { arg, .. } if arg == "data" => Item::Event(Event::Land),
        Step::Clock { arg, .. } => match arg.strip_prefix('+').map(str::parse::<f64>) {
            Some(Ok(ms)) if ms.is_finite() && ms >= 0.0 => Item::Event(Event::Clock(ms)),
            _ => return Err("only clock +ms, data and settle are delivered".into()),
        },
        Step::ExpectState { name, value, .. } => match literal(value) {
            Some(v) => Item::Expect(Expect::State(name.clone(), v), step.span()),
            None => return Err(format!("expect state {name} needs a literal")),
        },
        Step::ExpectText { target, value, .. } => {
            Item::Expect(Expect::Text(target.clone(), value.clone()), step.span())
        }
        Step::ExpectTree {
            target, present, ..
        } => Item::Expect(Expect::Tree(target.clone(), *present), step.span()),
        // The voice table is not in Lean (LLP 1096 D11): its commands
        // are, and those are compared; a step that reads the table is
        // the runner's tests' and conformance's.
        Step::ExpectSound { .. } => Item::Observe(step.span()),
        // Displayed wake-lock holders have no Lean oracle (LLP 1125 D11).
        Step::ExpectWakelock { .. } => Item::Observe(step.span()),
        // A control's `disabled` is read from the host's tree (LLP 1116.010 D3), which Lean has no model of.
        Step::ExpectDisabled { .. } => Item::Observe(step.span()),
        // The media session is the host's (LLP 1098 D12): an assertion on
        // it has no oracle, and a test that only asserts it still runs.
        // Its `tap` form is not a press, so it reaches `other` and the
        // block stops at that step.
        Step::ExpectMediaSession { .. } | Step::ExpectMediaSessionAction { .. } => {
            Item::Observe(step.span())
        }
        other => {
            return Err(format!(
                "{} is not delivered by the synchronous oracle",
                step_kind(other)
            ))
        }
    }))
}

fn delivered_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Tap"
            | "Type"
            | "Clock"
            | "ExpectState"
            | "ExpectText"
            | "ExpectTree"
            | "ExpectSound"
            | "ExpectWakelock"
            | "ExpectDisabled"
            | "ExpectMediaSession"
            | "ExpectMediaSessionAction"
    )
}

/// The parser's kinds minus those delivered here; conditional forms are named separately.
pub(crate) fn undeliverable_kinds() -> String {
    Step::KINDS
        .iter()
        .filter(|kind| !delivered_kind(kind))
        .map(|kind| kind_name(kind))
        .collect::<Vec<_>>()
        .join(", ")
}

fn kind_name(kind: &str) -> String {
    let mut out = String::new();
    for (i, c) in kind.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    if out == "key" {
        "type key".into()
    } else {
        out
    }
}

fn step_kind(step: &Step) -> String {
    match step {
        Step::Clock { arg, .. } => format!("clock {}", arg.split_whitespace().next().unwrap_or("")),
        Step::Tap {
            form, modifiers, ..
        } if *form != contract_syntax::TapForm::Press || !modifiers.is_empty() => {
            "tap (not a plain press)".into()
        }
        other => kind_name(other.kind()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "component App\n  state n = 0\n  action add\n    n = n + 1\n  view\n    button \"add\" testId=\"add\" press=add\n";

    #[test]
    fn land_printers_use_the_semantics_label() {
        assert_eq!(Event::Land.label(), "land");
        assert_eq!(Event::Land.lean(), "(.land)");
        assert_eq!(Event::InputLand.label(), Event::Land.label());
        assert_eq!(Event::InputLand.lean(), Event::Land.lean());
        // The corpus runs this term and compares the resulting label byte for byte.
        let s = scripted(
            "label",
            &format!("{APP}\ntest \"land\"\n  clock data\n"),
            None,
        )
        .unwrap()
        .remove(0);
        let plan = crate::compile(&s.case).unwrap();
        let (lines, _) = crate::observe::run(
            plan.clone(),
            crate::oracle::Oracle::for_case(&plan, &s.case),
            &s.case.events,
        );
        assert_eq!(crate::chunks(&lines)[1][0], "== land");
    }

    #[test]
    fn synchronous_clocks_keep_expectations_and_stop_at_the_first_unsupported_step() {
        let source = format!("{APP}\ntest \"prefix\"\n  clock data\n  tap \"add\"\n  clock settle\n  expect state n == 1\n  reload\n  tap \"add\"\n  expect state n == 2\n");
        let scripts = scripted("app.test.contract", &source, None).unwrap();
        let s = &scripts[0];
        assert_eq!(
            s.case.events,
            [
                Event::Land,
                Event::Tap("add".into()),
                Event::InputLand,
                Event::Clock(0.0)
            ]
        );
        assert_eq!(s.delivery.taken, 4);
        assert_eq!(s.delivery.settle_events, Some(1));
        let (span, kind, _) = s.delivery.stopped.as_ref().unwrap();
        assert_eq!((span.line, span.col, kind.as_str()), (13, 3, "reload"));
        assert_eq!(
            s.delivery.events.iter().map(|s| s.line).collect::<Vec<_>>(),
            [9, 10, 10, 11]
        );
        crate::expectations(s).unwrap();
        let source = source.replace("expect state n == 1", "expect state n == 7");
        assert!(
            crate::expectations(&scripted("app.test.contract", &source, None).unwrap()[0]).is_err()
        );
    }

    #[test]
    fn undeliverable_steps_keep_their_kind_and_location() {
        for (step, kind) in [
            ("request GET \"/items\"", "request"),
            ("clock until state n == 1 within 100", "clock until"),
            ("fail fetch \"https://example.test\"", "fail fetch"),
            ("pass fetch \"https://example.test\"", "pass fetch"),
            ("back", "back"),
            ("close", "close"),
            ("type \"add\" key \"Enter\"", "type key"),
            ("tap \"add\" hover", "tap (not a plain press)"),
            ("pick \"file\" cancel", "pick"),
            ("resize 800x600", "resize"),
            ("prefer color-scheme dark", "prefer"),
        ] {
            let source = format!("{APP}\ntest \"stops\"\n  {step}\n  tap \"add\"\n");
            let s = scripted("test", &source, None).unwrap().remove(0);
            assert_eq!(s.delivery.taken, 0, "{step}");
            assert!(s.case.events.is_empty(), "{step}");
            assert_eq!(s.delivery.stopped.as_ref().unwrap().1, kind, "{step}");
        }
    }
    #[test]
    fn fixed_launch_facts_precede_the_delivered_prefix() {
        let source = format!("{APP}\nepoch \"2026-10-22T18:00:00Z\"\ntime-zone \"America/Los_Angeles\"\ntest \"fixed\"\n  clock settle\n  clock until state n == 1 within 100\n");
        let s = scripted("launch", &source, None).unwrap().remove(0);
        assert_eq!(s.case.launch.time.unwrap().epoch_at_zero, 1792692000000.0);
        assert_eq!(s.case.launch.time.unwrap().utc_offset, -420.0);
        assert_eq!(s.delivery.settle_events, Some(1));
        assert_eq!(s.delivery.taken, 1);
        assert_eq!(s.delivery.stopped.unwrap().1, "clock until");
    }
}
