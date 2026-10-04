//! Cases: a program and the events a host delivers to it.
//!
//! A corpus case is a `.contract` file with `test` blocks (LLP 1017 P7):
//! each block's `tap`, `type` and `clock +ms` steps are the events, and its
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
}

/// A program and its script.
#[derive(Debug, Clone)]
pub struct Case {
    /// Where it came from: a file and test name, or a seed.
    pub name: String,
    /// The Contract source.
    pub source: String,
    /// The events, in order.
    pub events: Vec<Event>,
    /// The file it was read from, when it resolves `use`s.
    pub path: Option<std::path::PathBuf>,
}

/// A corpus case: a [`Case`] whose script also holds expectations.
#[derive(Debug, Clone)]
pub struct Scripted {
    /// The case (its `events` are the script's events alone).
    pub case: Case,
    /// Events and expectations interleaved, as written.
    pub items: Vec<Item>,
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
        }
    }

    /// The event as a `Contract.Observe.Event` term.
    pub fn lean(&self) -> String {
        use contract::lean::string;
        match self {
            Event::Tap(t) => format!("(.tap {})", string(t)),
            Event::Type(t, s) => format!("(.change {} {})", string(t), string(s)),
            Event::Clock(ms) => format!("(.clock (Float.ofBits 0x{:016x}))", ms.to_bits()),
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
        Expr::EmptyList(_) => "[]".into(),
        _ => return None,
    })
}

/// Every `test` block of a corpus file, as a scripted case. A step the
/// differential run cannot deliver (a hover, a key, a screenshot, an
/// absolute clock) is refused by name.
pub fn scripted(
    file: &str,
    source: &str,
    path: Option<&std::path::Path>,
) -> Result<Vec<Scripted>, String> {
    let tests = contract::tests(source).map_err(|e| format!("{file}: {e}"))?;
    let mut out = Vec::new();
    for t in tests {
        let mut items = Vec::new();
        for step in &t.steps {
            items.push(match step {
                Step::Tap {
                    target,
                    form: contract_syntax::TapForm::Press,
                    ..
                } => Item::Event(Event::Tap(target.clone())),
                Step::Type {
                    target,
                    text,
                    append: false,
                    ..
                } => Item::Event(Event::Type(target.clone(), text.clone())),
                Step::Clock { arg, .. } => match arg.strip_prefix('+').map(str::parse::<f64>) {
                    Some(Ok(ms)) if ms.is_finite() && ms >= 0.0 => Item::Event(Event::Clock(ms)),
                    _ => {
                        return Err(format!(
                            "{file}: test {:?}: `clock {arg}`: only `clock +ms` is delivered",
                            t.name
                        ))
                    }
                },
                Step::ExpectState { name, value, .. } => match literal(value) {
                    Some(v) => Item::Expect(Expect::State(name.clone(), v)),
                    None => {
                        return Err(format!(
                            "{file}: test {:?}: `expect state {name}` needs a literal",
                            t.name
                        ))
                    }
                },
                Step::ExpectText { target, value, .. } => {
                    Item::Expect(Expect::Text(target.clone(), value.clone()))
                }
                Step::ExpectTree {
                    target, present, ..
                } => Item::Expect(Expect::Tree(target.clone(), *present)),
                other => {
                    return Err(format!(
                    "{file}: test {:?}: a step the differential run does not deliver: {other:?}",
                    t.name
                ))
                }
            });
        }
        let events = items
            .iter()
            .filter_map(|i| match i {
                Item::Event(e) => Some(e.clone()),
                Item::Expect(_) => None,
            })
            .collect();
        out.push(Scripted {
            case: Case {
                name: format!("{file}: {}", t.name),
                source: source.to_string(),
                events,
                path: path.map(std::path::Path::to_path_buf),
            },
            items,
        });
    }
    Ok(out)
}
