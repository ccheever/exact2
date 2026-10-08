//! The lanes' bookkeeping: which turn each asks with, and giving up on a
//! request whose reply is not coming.

use super::*;

impl Model {
    /// Requests out too long fail, so their lanes ask again. A reply that
    /// comes after all is for an old turn, and `lib.rs` drops it.
    pub(super) fn give_up_on_overdue(&mut self, now: f64) {
        // The long poll holds 25 s, plus the server's settle and the relay;
        // the first (`since` 0) is answered at once, so waiting that long
        // for it is only "Connecting…" on the screen.
        let limit = if self.since == 0 { 10_000.0 } else { 40_000.0 };
        if self.poll.overdue(now, limit) {
            self.poll.inflight = false;
            self.poll_failed();
        }
        if self.transcript.overdue(now, 30_000.0) {
            self.transcript_done(Err("No answer from the Mac.".into()));
        }
        // An interrupting send waits on the server (up to 35 s) for the turn
        // to stop before it answers.
        if self.send.overdue(now, 45_000.0) {
            self.send_done(Err(
                "No answer from the Mac; the message may not have arrived.".into(),
            ));
        }
        if self.probe.overdue(now, 15_000.0) {
            self.probe_done(false);
        }
        if self.refresh.overdue(now, 30_000.0) {
            self.refresh_done();
        }
        if self.reads.overdue(now, 30_000.0) {
            self.reads_done(Err(0));
        }
        if self.models.overdue(now, 30_000.0) {
            self.models_done(Err("No answer.".into()));
        }
        // A launch prepares the account and starts the agent: up to 90 s.
        if self.launch.overdue(now, 100_000.0) {
            self.launch_done(Err(
                "No answer from the machine; the session may have started.".into(),
            ));
        }
        if self.report.overdue(now, 30_000.0) {
            self.report_done(false);
        }
        if self.buzz.overdue(now, 5_000.0) {
            self.haptic_done();
        }
    }

    /// The turn `source`'s lane is on: a reply asked with another is stale.
    pub fn lane_turn(&self, source: &str) -> Option<u64> {
        Some(match source {
            "poll" => self.poll.turn,
            "transcript" => self.transcript.turn,
            "send" => self.send.turn,
            "probe" => self.probe.turn,
            "resync" => self.refresh.turn,
            "markRead" => self.reads.turn,
            "haptic" => self.buzz.turn,
            "report" => self.report.turn,
            "models" => self.models.turn,
            "launch" => self.launch.turn,
            _ => return None,
        })
    }
}
