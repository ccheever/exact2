//! The window's app modes (LLP 1121 D5): the agent's `prefer mode`, which
//! the glue says as the runner's `setMode`.

use super::*;

impl<D: DataSource> Host<D> {
    /// An app mode (LLP 1121 D5, the agent's `prefer mode`): the window's
    /// slice set again in one batch, which paints.
    pub fn set_mode(&mut self, axis: &str, value: &str) -> String {
        let (receipts, error) = match self.runner.set_mode(axis, value) {
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
        self.batch_for(&receipts, error.as_deref())
    }
}
