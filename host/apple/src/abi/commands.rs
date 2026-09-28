//! The session's command-side calls on the bridge: a `share` about to run
//! (LLP 1069.003) and a host line into the runner's journal.
use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// A `share` the session is about to run (`exact_share`).
    pub fn share(&mut self, len: usize) -> u32 {
        let request = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        let out = match self.host.as_mut() {
            Some(h) => h.share(&request),
            None => exact_runner::agent::error("not booted"),
        };
        self.emit(out)
    }

    /// A host line into the runner's journal (`exact_log`).
    pub fn log(&mut self, len: usize) -> u32 {
        let line = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned();
        if let Some(h) = self.host.as_mut() {
            h.log(&line);
        }
        0
    }
}
