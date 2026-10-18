//! The window's app modes (LLP 1121 D5): the agent's `prefer mode`, which
//! the glue says as the runner's `setMode`.

use super::*;

// The live table (LLP 1121 stage 4), beside the modes it sets rows as.
#[path = "live.rs"]
mod live;
// A node's state looks as page rules (LLP 1121 D8, stage 8 B).
#[path = "looks.rs"]
pub(crate) mod looks;
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

    /// Whether [`Host::set_modes`] would take `pins`, changing nothing: the
    /// window's modes, or the refusal.
    pub fn check_modes(&mut self, pins: &[(&str, &str)]) -> String {
        match self.runner.check_modes(pins) {
            Ok(()) => format!("{{\"modes\":{}}}", self.runner.modes_json()),
            Err(e) => exact_runner::agent::error(&format!("mode: {e:?}")),
        }
    }
}
