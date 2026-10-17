//! Sound and MIDI on Linux (LLP 1096 D9; LLP 1125 D1): the runner keeps the
//! voice table and the MIDI queue as the record, and this host plays and
//! sends nothing, so their ops are drained and dropped. Off the agent a
//! request finds no MIDI (`unavailable`); under it the runner answers one
//! itself, and the driver's `prefer midi.*` and `tap midi` are the runner's.

use super::Host;
use exact_runner::{DataSource, Timed};

impl<D: DataSource> Host<D> {
    /// The voice table's and the MIDI queue's ops since the last take,
    /// dropped: there is no output here.
    pub(super) fn drain_outputs(&mut self) {
        drop(self.runner.take_sounds());
        drop(self.runner.take_midi());
    }

    /// A host message for the MIDI queue (`exact_runner::Runner::midi_host`),
    /// committed; the refusal, if any.
    pub fn midi_host(&mut self, json: &str) -> Option<String> {
        match self.runner.midi_host(json) {
            Ok((receipts, error)) => {
                self.midi_commit(receipts, error.map(|e| format!("midi: {e:?}")))
            }
            Err(e) => Some(e),
        }
    }

    /// The agent's `prefer midi.*` and `tap midi` (LLP 1125 §4), committed:
    /// `None` for another request, else the reply or the refusal.
    pub fn midi_agent(&mut self, request: &str) -> Option<Result<String, String>> {
        let (receipts, reply) = self.runner.midi_agent(request)?;
        match self.midi_commit(receipts, None) {
            Some(e) => Some(Err(e)),
            None => Some(reply),
        }
    }

    fn midi_commit(
        &mut self,
        receipts: Vec<exact_kernel::CommitReceipt>,
        error: Option<String>,
    ) -> Option<String> {
        let at_ms = self.now_ms;
        let timed: Vec<Timed> = receipts
            .into_iter()
            .map(|receipt| Timed { at_ms, receipt })
            .collect();
        if timed.is_empty() {
            return error;
        }
        self.commit(&timed, error)
    }
}
