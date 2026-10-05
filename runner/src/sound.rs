//! The voice table (LLP 1096 D3–D5): what `playSound`, `playSounds` and
//! `stopSounds` scheduled, when, and how each voice ends — kept by the
//! runner, so it is the same on every host and under the driver's clock.
//!
//! A commit's sound commands are applied here once the commit stands, in
//! the order they were issued; the authored commands still go to the host
//! through `take_commands` (what difftest observes), and every host skips
//! them as the runner's own. What an output plays is [`SoundOp`]s: a
//! `Play` for each new voice and an `End` for each voice whose end moved
//! earlier, drained with `take_sounds`.
//!
//! - **Start.** `max(at, t)`, `t` the commit's time: a timer's commit is its
//!   due time, so a hit planned at `t` lands on the grid (D3).
//! - **Groups** are monophonic by start time: a voice ends at the start of
//!   the next voice in its group, a tie going to the later call (D4).
//! - **The bound.** At most [`LIVE`] voices sound or wait at a commit's
//!   time, after its cuts; another is dropped and journaled.
//! - **The record** keeps the last [`RECORD`] voices for the driver (D10).

use crate::agent::num;
use exact_plan::{Plan, Value};
use std::collections::VecDeque;

/// Voices sounding or waiting at once (LLP 1096 §9 Q2).
pub const LIVE: usize = 32;
/// Voices the record keeps for `expect sound` (D5).
pub const RECORD: usize = 1024;

/// What the output does: the table's changes, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum SoundOp {
    /// Start voice `id` of the plan's sound `sound` at runner time `at`
    /// (ms), times `gain`; it plays to the file's end unless an `End` comes.
    Play {
        /// The voice.
        id: u64,
        /// Its row in the plan's `sounds` table.
        sound: u32,
        /// When it starts, runner milliseconds.
        at: f64,
        /// Its linear gain, 0–1.
        gain: f64,
    },
    /// Voice `id` now ends at runner time `at`, earlier than it did.
    End {
        /// The voice.
        id: u64,
        /// When it ends, runner milliseconds.
        at: f64,
    },
}

/// How a voice ended, or will (D4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    /// Its natural end.
    End,
    /// A later voice in its group cut it after it started.
    Group,
    /// A later voice in its group cut it before it started: it never sounds.
    Cut,
    /// `stopSounds` ended it while it sounded.
    Stop,
    /// `stopSounds` came at or before its start: it never sounds.
    Cancelled,
}

impl By {
    /// Its word in `state.sounds` and `expect sound … by`.
    pub fn word(self) -> &'static str {
        match self {
            By::End => "end",
            By::Group => "group",
            By::Cut => "cut",
            By::Stop => "stop",
            By::Cancelled => "cancelled",
        }
    }

    /// Whether a voice that ended this way ever sounds.
    fn sounds(self) -> bool {
        !matches!(self, By::Cut | By::Cancelled)
    }
}

/// One voice, as the record keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    /// Counts up from 1 per boot.
    pub id: u64,
    /// Its row in the plan's `sounds` table.
    pub sound: u32,
    /// The declared path.
    pub src: String,
    /// Its effective start, `max(at, t)`.
    pub at: f64,
    /// Its gain, clamped to 0–1.
    pub gain: f64,
    /// Its group; empty is none.
    pub group: String,
    /// When it ends: its start plus its length, or earlier.
    pub ends: f64,
    /// How it ends.
    pub by: By,
    natural: f64,
}

impl Voice {
    /// Sounding or waiting at `t`.
    fn live(&self, t: f64) -> bool {
        self.by.sounds() && self.ends > t
    }

    /// `waiting`, `sounding` or `ended` at `t`.
    pub fn state(&self, t: f64) -> &'static str {
        if !self.by.sounds() || t >= self.ends {
            "ended"
        } else if t < self.at {
            "waiting"
        } else {
            "sounding"
        }
    }
}

/// The table: the record and the ops not yet taken.
#[derive(Default)]
pub struct Sounds {
    voices: VecDeque<Voice>,
    next: u64,
    /// Voices let go from the record, and the latest start among them.
    evicted: u64,
    evicted_through: f64,
    ops: Vec<SoundOp>,
}

/// One authored command, as the runner received it.
pub(crate) enum Call<'a> {
    Play(&'a [Value]),
    Hits(&'a Value),
    Stop(Option<&'a Value>),
}

impl<'a> Call<'a> {
    /// The command's call, when it is one of the three.
    pub(crate) fn of(name: &str, args: &'a [Value]) -> Option<Call<'a>> {
        match name {
            "playSound" => Some(Call::Play(args)),
            "playSounds" => args.first().map(Call::Hits),
            "stopSounds" => Some(Call::Stop(args.first())),
            _ => None,
        }
    }
}

/// A command argument that may be `none` (left to its default).
fn given(v: Option<&Value>) -> Option<&Value> {
    match v {
        None | Some(Value::Option(None)) => None,
        Some(Value::Option(Some(inner))) => Some(inner),
        Some(v) => Some(v),
    }
}

impl Sounds {
    /// The ops since the last take.
    pub fn take(&mut self) -> Vec<SoundOp> {
        std::mem::take(&mut self.ops)
    }

    /// The record, oldest first.
    pub fn voices(&self) -> impl DoubleEndedIterator<Item = &Voice> + ExactSizeIterator {
        self.voices.iter()
    }

    /// Voices sounding or waiting at `t`.
    pub fn live(&self, t: f64) -> usize {
        self.voices.iter().filter(|v| v.live(t)).count()
    }

    /// How many voices the record let go, and the latest start among them:
    /// a voice asked for at or before it may no longer be recorded.
    pub fn evicted(&self) -> (u64, f64) {
        (self.evicted, self.evicted_through)
    }

    /// Apply one commit's calls at its time `t`, in order, journaling each
    /// change into `log`. The ops for the output are queued once, at the
    /// end: a voice gets one `Play`, and an `End` only if it ends earlier
    /// than its file does.
    pub(crate) fn apply(&mut self, plan: &Plan, t: f64, calls: &[Call<'_>], log: &mut Vec<String>) {
        let before: Vec<(u64, f64)> = self.voices.iter().map(|v| (v.id, v.ends)).collect();
        let first_new = self.next + 1;
        for call in calls {
            match call {
                Call::Play(args) => self.play(plan, t, args, log),
                Call::Hits(Value::List(items)) => {
                    for item in items.iter() {
                        match item {
                            Value::Record(f) => self.play(plan, t, &f[..], log),
                            _ => log.push("sound dropped: a hit is not a record".into()),
                        }
                    }
                }
                Call::Hits(_) => log.push("sound dropped: playSounds takes a list".into()),
                Call::Stop(group) => {
                    let group = given(*group).map(|g| g.text().to_string());
                    self.stop(t, group.as_deref(), log);
                }
            }
        }
        self.queue(before, first_new);
    }

    /// Every live voice ends now: the runner is starting over (D5).
    pub(crate) fn reload(&mut self, t: f64, log: &mut Vec<String>) {
        let before: Vec<(u64, f64)> = self.voices.iter().map(|v| (v.id, v.ends)).collect();
        let n = self.end_live(t, None);
        log.push(format!("sounds stopped: {n} (reload)"));
        self.queue(before, self.next + 1);
    }

    fn queue(&mut self, before: Vec<(u64, f64)>, first_new: u64) {
        for v in &self.voices {
            if v.id >= first_new {
                self.ops.push(SoundOp::Play {
                    id: v.id,
                    sound: v.sound,
                    at: v.at,
                    gain: v.gain,
                });
                if v.ends < v.natural {
                    self.ops.push(SoundOp::End {
                        id: v.id,
                        at: v.ends,
                    });
                }
            } else if let Some((_, was)) = before.iter().find(|(id, _)| *id == v.id) {
                if v.ends < *was {
                    self.ops.push(SoundOp::End {
                        id: v.id,
                        at: v.ends,
                    });
                }
            }
        }
    }

    /// One `playSound(src, at, gain, group)`, or one hit's fields in the
    /// same order.
    fn play(&mut self, plan: &Plan, t: f64, args: &[Value], log: &mut Vec<String>) {
        let src = match args.first() {
            Some(v) if v.is_str() => v.text().to_string(),
            _ => return log.push("sound dropped: the sound is not a string".into()),
        };
        let Some((sound, row)) = plan
            .sounds
            .iter()
            .enumerate()
            .find(|(_, row)| plan.str(row.src) == src)
        else {
            return log.push(format!("sound dropped: {src:?} is not a declared sound"));
        };
        let number = |i: usize, default: f64| match given(args.get(i)) {
            None => Some(default),
            Some(Value::Number(n)) if n.is_finite() => Some(*n),
            Some(_) => None,
        };
        let Some(asked) = number(1, t) else {
            return log.push(format!("sound dropped: at is not a number ({src})"));
        };
        let Some(gain) = number(2, 1.0) else {
            return log.push(format!("sound dropped: gain is not a number ({src})"));
        };
        let group = given(args.get(3))
            .map(|g| g.text().to_string())
            .unwrap_or_default();
        let at = asked.max(t);
        let length = 1000.0 * f64::from(row.frames) / f64::from(row.rate.max(1));
        let saved: Vec<(u64, f64, By)> = self.voices.iter().map(|v| (v.id, v.ends, v.by)).collect();
        self.next += 1;
        let id = self.next;
        let mut voice = Voice {
            id,
            sound: sound as u32,
            src,
            at,
            gain: gain.clamp(0.0, 1.0),
            group,
            ends: at + length,
            by: By::End,
            natural: at + length,
        };
        let mut cuts = Vec::new();
        if !voice.group.is_empty() {
            // The next voice in the group that sounds ends this one; this
            // one ends every earlier voice still sounding at its start (a
            // tie goes to this, the later call).
            if let Some(next) = self
                .voices
                .iter()
                .filter(|w| w.group == voice.group && w.by.sounds() && w.at > at)
                .map(|w| w.at)
                .reduce(f64::min)
            {
                if next < voice.ends {
                    voice.ends = next;
                    voice.by = By::Group;
                    cuts.push((id, next));
                }
            }
            for w in self.voices.iter_mut() {
                if w.group == voice.group && w.by.sounds() && w.at <= at && w.ends > at {
                    w.ends = at;
                    w.by = if w.at < at { By::Group } else { By::Cut };
                    cuts.push((w.id, at));
                }
            }
        }
        if self.live(t) + usize::from(voice.live(t)) > LIVE {
            for (w, (_, ends, by)) in self.voices.iter_mut().zip(&saved) {
                (w.ends, w.by) = (*ends, *by);
            }
            self.next -= 1;
            return log.push(format!(
                "sound dropped: {LIVE} voices sound or wait ({} at {})",
                voice.src,
                num(at)
            ));
        }
        let mut line = format!(
            "sound {id} {} at {} gain {}",
            voice.src,
            num(at),
            num(voice.gain)
        );
        if !voice.group.is_empty() {
            line.push_str(&format!(" group {}", voice.group));
        }
        if asked < t {
            line.push_str(&format!(" (asked {})", num(asked)));
        }
        log.push(line);
        let group = voice.group.clone();
        self.voices.push_back(voice);
        cuts.sort_by_key(|(id, _)| *id);
        for (cut, ends) in cuts {
            log.push(format!("sound {cut} ends at {} (group {group})", num(ends)));
        }
        self.trim(t);
    }

    /// `stopSounds(group=)` at `t`.
    fn stop(&mut self, t: f64, group: Option<&str>, log: &mut Vec<String>) {
        let n = self.end_live(t, group.filter(|g| !g.is_empty()));
        log.push(match group {
            Some(g) if !g.is_empty() => format!("sounds stopped: {n} (group {g})"),
            _ => format!("sounds stopped: {n}"),
        });
    }

    /// End every voice live at `t` (in `group`, when given): one sounding
    /// stops at `t`, one waiting is cancelled at its start.
    fn end_live(&mut self, t: f64, group: Option<&str>) -> usize {
        let mut n = 0;
        for v in self.voices.iter_mut() {
            if v.live(t) && group.is_none_or(|g| v.group == g) {
                n += 1;
                if v.at < t {
                    (v.ends, v.by) = (t, By::Stop);
                } else {
                    (v.ends, v.by) = (v.at, By::Cancelled);
                }
            }
        }
        n
    }

    /// Keep the last [`RECORD`] voices, letting the oldest that is no
    /// longer live go first: a live one is still needed for its group.
    fn trim(&mut self, t: f64) {
        while self.voices.len() > RECORD {
            let Some(i) = self.voices.iter().position(|v| !v.live(t)) else {
                return;
            };
            let gone = self.voices.remove(i).expect("a position in the record");
            self.evicted += 1;
            self.evicted_through = self.evicted_through.max(gone.at);
        }
    }
}

#[cfg(test)]
mod tests;
