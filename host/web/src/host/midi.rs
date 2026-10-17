//! MIDI from the page (LLP 1125 D6–D8): what midi-glue.js reports of the
//! ports and the inputs, and the agent's `prefer midi.*` and `tap midi`,
//! each in one batch.

use super::super::*;

impl<D: DataSource> Host<D> {
    /// A MIDI message from the page, in one batch: the glue's
    /// (`{"ports":…}`, `{"messages":…}`, `exact_runner::Runner::midi_host`)
    /// or the agent's (`exact_runner::Runner::midi_agent`); a refusal is
    /// the batch's error.
    pub fn midi(&mut self, json: &str) -> String {
        let (receipts, error) = match self.runner.midi_agent(json) {
            Some((receipts, reply)) => (receipts, reply.err()),
            None => match self.runner.midi_host(json) {
                Ok((receipts, e)) => (receipts, e.map(|e| format!("midi: {e:?}"))),
                Err(e) => (Vec::new(), Some(e)),
            },
        };
        let at_ms = self.now_ms;
        let timed: Vec<Timed> = receipts
            .into_iter()
            .map(|receipt| Timed { at_ms, receipt })
            .collect();
        self.batch_for(&timed, error.as_deref())
    }

    /// The batch of a host that replaces `old` (a boot, a new generation):
    /// the old runner's last outputs go first, every voice ended and every
    /// note it turned on given its note-off now, clock out's Stop sent, so
    /// nothing is left sounding when the new boot's `midi` op lets go of what
    /// the output held (LLP 1125 D4).
    pub fn ending(old: Option<Self>, batch: String) -> String {
        let Some(mut old) = old else {
            return batch;
        };
        old.runner.end_sounds();
        let mut last = Batch::new();
        last.timed(&mut old.runner, false);
        let ops = last.into_ops();
        match batch.strip_prefix("{\"ops\":[") {
            Some(rest) if !ops.is_empty() => {
                let sep = if rest.starts_with(']') { "" } else { "," };
                format!("{{\"ops\":[{}{sep}{rest}", ops.join(","))
            }
            _ => batch,
        }
    }
}
