//! The commit's sound commands, which the runner's voice table takes once
//! the commit stands (LLP 1096 D5).

use super::{DataSource, Runner};

impl<D: DataSource> Runner<D> {
    /// The voice table takes the commit's `playSound`, `playSounds` and
    /// `stopSounds` once it stands, in the order they were issued; the
    /// commands stay for the host, which skips them (LLP 1096 D5). A
    /// `reload` ends every live voice first.
    pub(super) fn apply_sounds(&mut self, from: usize) {
        let commands = &self.commands[from.min(self.commands.len())..];
        let calls: Vec<_> = commands
            .iter()
            .filter_map(|c| crate::sound::Call::of(&c.name, &c.args))
            .collect();
        let mut lines = Vec::new();
        if !calls.is_empty() {
            self.sounds
                .apply(&self.plan, self.now_ms, &calls, &mut lines);
        }
        if commands.iter().any(|c| c.name == "reload") {
            if !self.plan.sounds.is_empty() {
                self.sounds.reload(self.now_ms, &mut lines);
            }
            // Every note the queue turned on ends, handed now (LLP 1125 D4).
            self.midi.reload(self.now_ms, &mut lines);
        }
        // `sendMIDI` and `requestMIDIAccess` (LLP 1125 D6, D7): the queue's.
        let seek = !self.clock_live();
        for c in commands {
            match c.name.as_str() {
                "sendMIDI" => self.midi.send(&c.args, self.now_ms, &mut lines),
                "requestMIDIAccess" => self.midi.request(self.now_ms, seek, &mut lines),
                _ => {}
            }
        }
        lines.into_iter().for_each(|l| self.log(l));
    }
}
