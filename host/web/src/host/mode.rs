//! The window's app modes (LLP 1121 D5): the agent's `prefer mode`, which
//! the glue says as the runner's `setMode`.

use super::*;

// The live table (LLP 1121 stage 4), beside the modes it sets rows as.
#[path = "live.rs"]
mod live;
// A node's state looks as page rules (LLP 1121 D8, stage 8 B).
#[path = "looks.rs"]
pub(crate) mod looks;
// What the page has of each view.
#[path = "mirror.rs"]
pub(crate) mod mirror;
// MIDI's messages from the page (LLP 1125 D6–D8), told as a fact is.
#[path = "midi.rs"]
mod midi;

impl<D: DataSource> Host<D> {
    /// App modes (LLP 1121 D5, the agent's `prefer mode`): the window's
    /// slice set again in one batch, which paints; the batch says the
    /// window's modes after it.
    pub fn set_modes(&mut self, pins: &[(&str, &str)]) -> String {
        let (receipts, error) = match self.runner.set_modes(pins) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("mode: {e:?}"))),
        };
        let batch = self.batch_for(&receipts, error.as_deref());
        // An object, as every batch is: the modes go first.
        format!("{{\"modes\":{},{}", self.runner.modes_json(), &batch[1..])
    }

    /// [`Bridge::hold_facts`](crate::Bridge::hold_facts)'s request: held,
    /// handed back (a batch), or asked; each answer with `asked`, what the
    /// app last asked of the scheme, each axis and the root size.
    pub fn hold_facts(&mut self, text: &str) -> String {
        let (receipts, error) = match text {
            "?" => (vec![], None),
            "-" => match self.runner.hold_facts(None) {
                Ok(Some(receipt)) => (
                    vec![Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }],
                    None,
                ),
                Ok(None) => (vec![], None),
                Err(e) => (vec![], Some(format!("hold: {e:?}"))),
            },
            _ => {
                let (flags, pins) = text.split_once(':').unwrap_or((text, ""));
                let pins: Option<Vec<(&str, &str)>> = pins
                    .split(',')
                    .filter(|p| !p.is_empty())
                    .map(|p| p.split_once('='))
                    .collect();
                match pins {
                    None => (vec![], Some("hold: write <flags>:axis=value,…".to_owned())),
                    Some(pins) => match self.runner.hold_facts(Some((
                        flags.contains('s'),
                        &pins,
                        flags.contains('r'),
                    ))) {
                        // (A takeover or a release commits: its rows reach the page.)
                        Ok(Some(receipt)) => (
                            vec![Timed {
                                at_ms: self.now_ms,
                                receipt,
                            }],
                            None,
                        ),
                        Ok(None) => (vec![], None),
                        Err(e) => (vec![], Some(format!("hold: {e:?}"))),
                    },
                }
            }
        };
        let asked = self.runner.asked();
        let mut said = String::from("{\"asked\":{");
        let mut first = true;
        let mut field = |name: &str, said: &mut String| {
            if !first {
                said.push(',');
            }
            first = false;
            exact_runner::agent::quote(name, said);
            said.push(':');
        };
        if let Some(s) = &asked.scheme {
            field("scheme", &mut said);
            exact_runner::agent::untyped_json(s, &mut said);
        }
        if !asked.modes.is_empty() {
            field("mode", &mut said);
            said.push('{');
            for (i, (a, v)) in asked.modes.iter().enumerate() {
                if i > 0 {
                    said.push(',');
                }
                exact_runner::agent::quote(a, &mut said);
                said.push(':');
                exact_runner::agent::quote(v, &mut said);
            }
            said.push('}');
        }
        if let Some(r) = &asked.root {
            field("rootFontSize", &mut said);
            exact_runner::agent::untyped_json(r, &mut said);
        }
        said.push_str("},");
        let batch = self.batch_for(&receipts, error.as_deref());
        format!("{said}{}", &batch[1..])
    }

    /// Whether [`Host::set_modes`] would take `pins`, changing nothing: the
    /// window's modes, or the refusal.
    pub fn check_modes(&mut self, pins: &[(&str, &str)]) -> String {
        match self.runner.check_modes(pins) {
            Ok(()) => format!("{{\"modes\":{}}}", self.runner.modes_json()),
            Err(e) => exact_runner::agent::error(&format!("mode: {e:?}")),
        }
    }
}
