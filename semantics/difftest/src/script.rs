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
    /// Land armed mutation continuations and queued sends without firing timers.
    Land,
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
    Expect(Expect),
    /// Expanded from live runner tasks before the case is compared.
    Settle {
        span: contract_syntax::Span,
        taken: usize,
    },
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
    /// The first step not delivered: its location, kind and reason.
    pub stopped: Option<(contract_syntax::Span, String, String)>,
    /// Steps consumed before stopping, including expectations and ignored reads.
    pub taken: usize,
    /// The authored location of each delivered event.
    pub events: Vec<contract_syntax::Span>,
    /// Primitive events emitted for all settles in this block.
    pub settle_events: Option<usize>,
    /// The replayable expansion, when the block contains a settle.
    pub expansion: Option<std::path::PathBuf>,
}

impl Event {
    /// The step's label, exactly as `Contract.Observe.Event.label` prints it.
    pub fn label(&self) -> String {
        match self {
            Event::Tap(t) => format!("tap {}", crate::observe::quote(t)),
            Event::Type(t, s) => format!(
                "type {} {}",
                crate::observe::quote(t),
                crate::observe::quote(s)
            ),
            Event::Clock(ms) => format!("clock +{}", exact_runner::stdlib::format_number(*ms)),
            Event::Land => "land".into(),
        }
    }

    /// The event as a `Contract.Observe.ScriptEvent` term.
    pub fn lean(&self) -> String {
        use contract::lean::string;
        match self {
            Event::Tap(t) => format!("(.tap {})", string(t)),
            Event::Type(t, s) => format!("(.change {} {})", string(t), string(s)),
            Event::Clock(ms) => format!("(.clock (F64.ofBits 0x{:016x}))", ms.to_bits()),
            Event::Land => "(.land)".into(),
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
    for step in &t.steps {
        if leading {
            match step {
                Step::Epoch { value, .. } if value != "now" => {
                    epoch = Some(value.as_str());
                    continue;
                }
                Step::TimeZone { zone: value, .. } => {
                    zone = Some(value.as_str());
                    continue;
                }
                _ => leading = false,
            }
        }
        let item = if matches!(step, Step::Clock { arg, .. } if arg == "settle") {
            Ok(Some(Item::Settle {
                span: step.span(),
                taken: delivery.taken,
            }))
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
                items.push(Item::Event(Event::Land));
                delivery.events.push(step.span());
            }
        }
    }
    let events = items
        .iter()
        .filter_map(|i| match i {
            Item::Event(e) => Some(e.clone()),
            Item::Expect(_) | Item::Settle { .. } => None,
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
    Ok(script)
}

fn deliver(step: &Step) -> Result<Option<Item>, String> {
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
            Some(v) => Item::Expect(Expect::State(name.clone(), v)),
            None => return Err(format!("expect state {name} needs a literal")),
        },
        Step::ExpectText { target, value, .. } => {
            Item::Expect(Expect::Text(target.clone(), value.clone()))
        }
        Step::ExpectTree {
            target, present, ..
        } => Item::Expect(Expect::Tree(target.clone(), *present)),
        // The voice table is not in Lean (LLP 1096 D11): its commands
        // are, and those are compared; a step that reads the table is
        // the runner's tests' and conformance's.
        Step::ExpectSound { .. } | Step::Midi { .. } => return Ok(None),
        // Displayed wake-lock holders have no Lean oracle (LLP 1125 D11).
        Step::ExpectWakelock { .. } => return Ok(None),
        // A control's `disabled` is read from the host's tree (LLP 1116.010 D3), which Lean has no model of.
        Step::ExpectDisabled { .. } => return Ok(None),
        // The media session is the host's (LLP 1098 D12): an assertion on
        // it has no oracle, and a test that only asserts it still runs.
        // Its `tap` form is not a press, so it reaches `other` and the
        // block stops at that step.
        Step::ExpectMediaSession { .. } | Step::ExpectMediaSessionAction { .. } => return Ok(None),
        other => {
            return Err(format!(
                "{} is not delivered by the synchronous oracle",
                step_kind(other)
            ))
        }
    }))
}

fn step_kind(step: &Step) -> String {
    match step {
        Step::Clock { arg, .. } => format!("clock {}", arg.split_whitespace().next().unwrap_or("")),
        Step::Key { .. } => "type key".into(),
        Step::FailFetch { .. } => "fail fetch".into(),
        Step::PassFetch { .. } => "pass fetch".into(),
        Step::Tap { .. } => "tap".into(),
        Step::ExpectState { .. } => "expect state".into(),
        other => format!("{other:?}")
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "component App\n  state n = 0\n  action add\n    n = n + 1\n  view\n    button \"add\" testId=\"add\" press=add\n";

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
                Event::Land,
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
            ("tap \"add\" hover", "tap"),
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
