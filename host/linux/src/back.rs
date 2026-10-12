//! The platform's own Back on the host (LLP 1115 D5; LLP 1115.001 D6): the
//! router's `back`, written by the runner as one commit.
use super::*;

impl<D: DataSource> Host<D> {
    /// The platform's own Back from the selected visit `id`, at `now_ms`:
    /// what a route with no authored Back control gets under a navigation
    /// root with no `navigate` handler ([`Runner::host_back`]). Whether the
    /// router popped: `false` when `id` is not the selected top, or is its
    /// stack's root. A refused commit is the error, and the tree is
    /// untouched.
    pub fn host_back(&mut self, id: u64, now_ms: f64) -> Result<bool, String> {
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.host_back(id) {
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                match self.commit(&[Timed { at_ms, receipt }], None) {
                    None => Ok(true),
                    Some(error) => Err(error),
                }
            }
            Ok(None) => Ok(false),
            Err(e) => Err(format!("host back: {e:?}")),
        }
    }
}
