//! The posture and the segment counts (LLP 1076 D4, D6): the web's wasm
//! host answers `exactViewport`'s three fold fields; the browser resolves
//! the `env(viewport-segment-*)` lengths itself.

use super::{Host, Timed};
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// The device's posture and the segment counts (LLP 1076 D4, D6), the
    /// `exactViewport` fields re-answered in one batch.
    pub fn set_fold(&mut self, fold: exact_runner::Fold) -> String {
        match self.runner.set_fold(fold) {
            Ok(Some(receipt)) => {
                let receipts = [Timed {
                    at_ms: self.now_ms,
                    receipt,
                }];
                self.batch_for(&receipts, None)
            }
            Ok(None) => self.batch_for(&[], None),
            Err(e) => self.batch_for(&[], Some(&format!("segments: {e:?}"))),
        }
    }
}
