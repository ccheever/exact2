//! MIDI's test steps (LLP 1125 §4): the driver's `prefer midi.*`, `tap midi`
//! and a read of `state.midi.sent`.

/// One MIDI test step.
#[derive(Debug, Clone, PartialEq)]
pub enum MidiStep {
    /// `prefer midi.state granted midi.outputs "synth" midi.inputs "keys"`:
    /// what a request finds under the agent, the ports by name.
    Prefer {
        /// `state` (`prompt`, `granted`, `denied`, `unavailable`), if named.
        state: Option<String>,
        /// The inputs, if named.
        inputs: Option<Vec<String>>,
        /// The outputs, if named.
        outputs: Option<Vec<String>>,
    },
    /// `tap midi "keys" 90 3c 64`: a message from that input now.
    Tap {
        /// The input, by id.
        input: String,
        /// The bytes.
        data: Vec<u8>,
    },
    /// `expect midi sent|missing "synth" 90 3c 64 [at N]`: a message the
    /// runner queued for that output (not taken back), or none.
    Expect {
        /// The output, by id.
        output: String,
        /// The bytes.
        data: Vec<u8>,
        /// When it is due, runner ms.
        at: Option<f64>,
        /// `sent`, or `missing`.
        present: bool,
    },
}
