//! A `sequence` in the runner's commits and clock (LLP 1125 D2–D5): the
//! gate step reads each sequence's gate, key and `play` arguments over the
//! commit's settled state, inside its rollback; the transport takes them
//! once the commit stands, so a refused commit changes nothing that sounds.
//! Its own wakes (a fill, the count-in's end) run no action and make no
//! commit; its end runs `ended=` as a timer runs; and what `exactTransport`
//! answers is published by a commit of its own, as an answer's `then` is.

use super::*;
use crate::transport::{Answer, Seen, SOURCE};

impl<D: DataSource> Runner<D> {
    /// The gate step's part (LLP 1092 D8): each sequence's gate, its key's
    /// text while the gate holds, and its six arguments, read now and kept
    /// for [`Runner::transport_stood`]. A trap, or a key that is no key,
    /// refuses the commit.
    pub(super) fn sequence_step(&mut self) -> Result<(), RunnerError> {
        self.transport.pending = None;
        if self.plan.sequences.is_empty() {
            return Ok(());
        }
        let mut seen = Vec::with_capacity(self.plan.sequences.len());
        for row in self.plan.sequences.iter() {
            let on = self.eval(row.gate, &[], &[])? == Value::Bool(true);
            let key = if row.keyed && on {
                let value = self.eval(row.key, &[], &[])?;
                Some(
                    crate::instance::key_text(&value).ok_or_else(|| RunnerError::TaskKey {
                        task: self.plan.str(row.name).to_string(),
                    })?,
                )
            } else {
                None
            };
            let mut args = Vec::new();
            if on {
                for code in [
                    row.tracks,
                    row.bpm,
                    row.from,
                    row.count_in,
                    row.clock,
                    row.clock_out,
                ] {
                    args.push(self.eval(code, &[], &[])?);
                }
            }
            seen.push(Seen { on, key, args });
        }
        self.transport.pending = Some(seen);
        Ok(())
    }

    /// The commit that read the gates stood: arm, restart, edit or stop.
    /// A background stop the commit asked for (`set_page`) is applied here
    /// too, after the gates, so a refused commit leaves it unmade.
    pub(super) fn transport_stood(&mut self) {
        let suspend = std::mem::take(&mut self.transport.suspend);
        let seen = self.transport.pending.take();
        if seen.is_none() && !suspend {
            return;
        }
        let before = self.transport.answer();
        let (now, live) = (self.now_ms, self.clock_live());
        let mut lines = Vec::new();
        if let Some(seen) = seen {
            self.transport
                .stood(&self.plan, &mut self.sounds, now, live, seen, &mut lines);
        }
        if suspend {
            self.transport
                .suspend(&self.plan, &mut self.sounds, now, &mut lines);
        }
        self.transport_said(lines, before);
    }

    /// Journal what the transport said, and answer `exactTransport` again
    /// when what it answers changed.
    fn transport_said(&mut self, lines: Vec<String>, before: Answer) {
        lines.into_iter().for_each(|l| self.log(l));
        if !self.transport.readers.is_empty() && self.transport.answer() != before {
            self.transport.answer_due = self.transport.answer_due.min(self.now_ms);
        }
    }

    /// Its own wake at `at` (a fill, the count-in's end): no action, no
    /// commit, no receipt.
    pub(super) fn transport_wake(&mut self, at: f64) {
        let before = self.transport.answer();
        let live = self.clock_live();
        let mut lines = Vec::new();
        self.transport
            .wake(&self.plan, &mut self.sounds, at, live, &mut lines);
        self.transport_said(lines, before);
    }

    /// The last finite track ended at the clock's time: the `ended=`
    /// action, if any, to run as a timer's.
    pub(super) fn transport_end(&mut self) -> Option<ActionsId> {
        let before = self.transport.answer();
        let mut lines = Vec::new();
        let action = self.transport.end(&self.plan, &mut lines);
        self.transport_said(lines, before);
        action
    }

    /// The transport record, for `state.transport` and a test.
    pub fn transport(&self) -> &crate::transport::Transport {
        &self.transport
    }

    /// `exactTransport()`'s answer, by the declared fields.
    pub(super) fn transport_answer(&self, i: usize) -> Result<Value, DataError> {
        let row = &self.plan.resources[i];
        if row.args.len > 0 {
            return Err(DataError::BadArguments(format!(
                "{SOURCE} takes no arguments"
            )));
        }
        let ty = self.plan.type_(row.ty);
        if ty.kind != exact_plan::TypeKind::Record {
            return Err(DataError::Unavailable(format!(
                "{SOURCE} answers a record; this one is declared `{}`",
                self.plan.str(ty.name)
            )));
        }
        let answer = self.transport.answer();
        let mut fields = Vec::with_capacity(ty.fields.len as usize);
        for f in ty.fields.iter() {
            let name = self.plan.str(self.plan.field(f).name);
            fields.push(answer.field(name).ok_or_else(|| {
                DataError::Unavailable(format!("{SOURCE} has no field `{name}`"))
            })?);
        }
        Ok(Value::record(fields))
    }

    /// The voice table (LLP 1096 D5), for `state.sounds` and a test.
    pub fn sounds(&self) -> &crate::sound::Sounds {
        &self.sounds
    }

    /// What the output plays since the last take: a `Play` per new voice,
    /// an `End` per voice that now ends earlier (LLP 1096 D5), and a
    /// sequence's timed haptics (LLP 1125 D3).
    pub fn take_sounds(&mut self) -> Vec<crate::sound::SoundOp> {
        self.sounds.take()
    }

    /// End every live voice before this runner is replaced (a dev reload,
    /// LLP 1096 D5); the host drains `take_sounds` before the restart.
    pub fn end_sounds(&mut self) {
        if self.plan.sounds.is_empty() {
            return;
        }
        let mut lines = Vec::new();
        self.sounds.reload(self.now_ms, &mut lines);
        lines.into_iter().for_each(|l| self.log(l));
    }
}
