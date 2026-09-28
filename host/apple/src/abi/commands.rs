//! The session's command-side calls on the bridge: a command that shows
//! system UI about to run (`share`, LLP 1069.003; `saveFile`, LLP 1069.010)
//! and a host line into the runner's journal.
use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// A command the session is about to run (`exact_command`).
    pub fn command(&mut self, len: usize) -> u32 {
        let request = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        let out = match self.host.as_mut() {
            Some(h) => h.command(&request),
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
