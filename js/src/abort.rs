//! An app's aborted fetches (door 23), by their tickets, for the runner to
//! cancel and resume; and the abort's test hooks.
use super::Module;

impl Module {
    /// The fetches the app aborted since the last take (door 23), by their
    /// tickets ([`exact_runner::Request::fetch`]): the runner cancels the
    /// request each names, whoever's it is now, and resumes its answer.
    pub fn take_aborted(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.host.aborted)
    }

    /// Requests the module recorded and has neither handed out nor
    /// withdrawn: a diagnostic (a fetch aborted unsent leaves none).
    pub fn unsent(&self) -> usize {
        self.host.requests.len()
    }

    /// An older host's door, for tests: `op` answers as a door without it
    /// does ("no op"), so the prelude's use of it can be shown harmless.
    #[doc(hidden)]
    pub fn refuse_door_op(&mut self, op: u32) {
        self.host.refused_ops.push(op);
    }
}
