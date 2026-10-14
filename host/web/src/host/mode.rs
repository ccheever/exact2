//! The window's app modes (LLP 1121 D5): the agent's `prefer mode`, which
//! the glue says as the runner's `setMode`.

use super::*;

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
}
