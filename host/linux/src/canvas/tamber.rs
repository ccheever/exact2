//! The canvas host's answer to Android's `ExactTamber`.

use super::CanvasHost;
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// What `ExactTamber(view)` asks: [`crate::Host::tamber_json`].
    pub fn tamber_json(&mut self, request: &str) -> String {
        self.p.host_mut().tamber_json(request)
    }
}
