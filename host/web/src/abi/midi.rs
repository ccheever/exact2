//! MIDI's export (LLP 1125 D6–D8): midi-glue.js's reports and the agent's
//! MIDI requests, as JSON in the input buffer.

use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// A MIDI message in the input buffer (`Host::midi`); returns the batch
    /// length.
    pub fn midi(&mut self, len: usize) -> u32 {
        let Ok(text) = std::str::from_utf8(&self.input[..len.min(self.input.len())]) else {
            return self.emit(exact_runner::agent::error("midi: invalid UTF-8"));
        };
        let text = text.to_owned();
        let out = self.host.as_mut().map_or_else(
            || exact_runner::agent::error("not booted"),
            |h| h.midi(&text),
        );
        self.emit(out)
    }
}

/// MIDI's export, invoked inside [`crate::host!`].
#[macro_export]
macro_rules! midi_exports {
    () => {
        /// A MIDI message in the input buffer (LLP 1125 D6–D8): the
        /// glue's ports and inputs, or the agent's; returns the batch length.
        #[no_mangle]
        pub extern "C" fn exact_midi(len: u32) -> u32 {
            EXACT_BRIDGE.with(|b| b.borrow_mut().midi(len as usize))
        }
    };
}
