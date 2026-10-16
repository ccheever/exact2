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
//! - **Length.** The file's, divided by the voice's `playbackRate`: pitch
//!   and duration move together, as on tape (LLP 1125 D10), so a group's
//!   cut and the bound read the end the speaker will reach.
//! - **Groups** are monophonic by start time: a voice ends at the start of
//!   the next voice in its group, a tie going to the later call (D4).
//! - **The bound.** At most [`LIVE`] voices sound at any voice's start,
//!   after its cuts, and at most [`WAITING`] wait to start (LLP 1125 D3);
//!   another is dropped and journaled.
//! - **A sequence's voices** (LLP 1125 D3) are the same voices, issued by
//!   `runner/src/transport.rs` a second ahead: an edit retracts the ones not
//!   yet started (`edit`), its stop ends them as `stopSounds()` would, and
//!   `stopSounds` stops the sounding ones and leaves those still to start,
//!   which are the sequence's next steps (D4). A step's haptic is a timed
//!   [`SoundOp::Haptic`], recorded beside the voices.
//! - **The record** keeps the last [`RECORD`] voices for the driver (D10).

use crate::agent::num;
use exact_plan::{Plan, Value};
use std::collections::VecDeque;

/// Voices sounding at any voice's start (LLP 1096 §9 Q2; LLP 1125 D3).
pub const LIVE: usize = 32;
/// Voices issued and not yet started (LLP 1125 D3): another tap that would
/// wait is dropped; a sequence stops expanding there instead.
pub const WAITING: usize = 256;
/// Voices the record keeps for `expect sound` (D5).
pub const RECORD: usize = 1024;
/// Haptics the record keeps (LLP 1125 D5).
pub const HAPTICS: usize = 1024;
/// The range of `playbackRate=` (LLP 1125 D10); a computed one is clamped to it.
pub const PLAYBACK_RATE: std::ops::RangeInclusive<f64> = 0.25..=4.0;

/// What the output does: the table's changes, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum SoundOp {
    /// Start voice `id` of the plan's sound `sound` at runner time `at`
    /// (ms), times `gain`, read at `playback_rate` and placed at `pan`; it
    /// plays to the file's end unless an `End` comes.
    Play {
        /// The voice.
        id: u64,
        /// Its row in the plan's `sounds` table.
        sound: u32,
        /// When it starts, runner milliseconds.
        at: f64,
        /// Its linear gain, 0–1.
        gain: f64,
        /// Web Audio's `AudioBufferSourceNode.playbackRate`, 0.25–4.
        playback_rate: f64,
        /// Web Audio's `StereoPannerNode.pan`, −1 (left) to 1 (right).
        pan: f64,
    },
    /// Voice `id` now ends at runner time `at`, earlier than it did.
    End {
        /// The voice.
        id: u64,
        /// When it ends, runner milliseconds.
        at: f64,
    },
    /// Play haptic `kind` (LLP 1077 D14) at runner time `at`: a sequence
    /// step's (LLP 1125 D3), played on the main thread at its time.
    Haptic {
        /// The haptic, counted from 1 per boot apart from the voices.
        id: u64,
        /// Its kind, `haptic()`'s word.
        kind: String,
        /// When, runner milliseconds.
        at: f64,
    },
    /// Haptic `id` is retracted: an edit or a stop came before its time.
    Unhaptic {
        /// The haptic.
        id: u64,
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
    /// Its sequence was edited before it started (LLP 1125 D3): it never
    /// sounds, and the new value is expanded in its place.
    Edit,
    /// Its sequence follows a MIDI clock whose new tempo or phase moved its
    /// time (LLP 1125 D9): it never sounds, and is issued again at the new
    /// time.
    Clock,
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
            By::Edit => "edit",
            By::Clock => "clock",
        }
    }

    /// Whether a voice that ended this way ever sounds.
    fn sounds(self) -> bool {
        !matches!(self, By::Cut | By::Cancelled | By::Edit | By::Clock)
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
    /// Its playback rate, clamped to [`PLAYBACK_RATE`]; 1 is the file's own.
    pub playback_rate: f64,
    /// Its pan, clamped to −1–1; 0 is centred.
    pub pan: f64,
    /// When it ends: its start plus its length, or earlier.
    pub ends: f64,
    /// How it ends.
    pub by: By,
    /// A sequence issued it (LLP 1125 D3).
    pub seq: bool,
    /// The step a sequence issued it for, before its track's offset (LLP
    /// 1125 D7); its own start otherwise.
    step_at: f64,
    natural: f64,
}

impl Voice {
    /// Sounding or waiting at `t`.
    fn live(&self, t: f64) -> bool {
        self.by.sounds() && self.ends > t
    }

    /// Sounding at `t`: started and not yet ended.
    fn sounding(&self, t: f64) -> bool {
        self.by.sounds() && self.at <= t && self.ends > t
    }

    /// Issued and not yet started at `t`.
    fn waits(&self, t: f64) -> bool {
        self.by.sounds() && self.at > t
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

/// One timed haptic (LLP 1125 D3), as the record keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Haptic {
    /// Counts up from 1 per boot.
    pub id: u64,
    /// Its kind.
    pub kind: String,
    /// When it plays.
    pub at: f64,
    /// Why it was retracted before its time, if it was: `edit`, `clock`
    /// (LLP 1125 D9), or `cancelled` (a stop or a reload).
    pub by: Option<&'static str>,
    /// The step it is for, before its track's offset.
    step_at: f64,
}

impl Haptic {
    /// `waiting` or `played` at `t`, or why it was retracted.
    pub fn state(&self, t: f64) -> &'static str {
        match self.by {
            Some(by) => by,
            None if t < self.at => "waiting",
            None => "played",
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
    haptics: VecDeque<Haptic>,
    next_haptic: u64,
}

/// What became of a sequence step's sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Added {
    /// Issued, or dropped and journaled (a bad field, the bound).
    Done,
    /// [`WAITING`] voices already wait: the sequence stops expanding here.
    Full,
}

/// Whether a step at `step_at` is one a retraction `by` from `from` takes:
/// at or after it, but a clock's only after it (LLP 1125 D9: `from` is the
/// hold's edge, and a note at it is handed already).
pub(crate) fn after(step_at: f64, from: f64, by: &str) -> bool {
    if by == "clock" {
        step_at > from
    } else {
        step_at >= from
    }
}

/// Journal a dropped voice.
fn dropped(log: &mut Vec<String>, line: String) -> Added {
    log.push(line);
    Added::Done
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

    /// The haptics' record, oldest first.
    pub fn haptics(&self) -> impl DoubleEndedIterator<Item = &Haptic> + ExactSizeIterator {
        self.haptics.iter()
    }

    /// Voices sounding or waiting at `t`.
    pub fn live(&self, t: f64) -> usize {
        self.voices.iter().filter(|v| v.live(t)).count()
    }

    /// Voices issued and not yet started at `t`.
    pub fn waiting(&self, t: f64) -> usize {
        self.voices.iter().filter(|v| v.waits(t)).count()
    }

    /// The earliest start after `t` of a voice still to start.
    pub(crate) fn next_start(&self, t: f64) -> Option<f64> {
        self.voices
            .iter()
            .filter(|v| v.waits(t))
            .map(|v| v.at)
            .reduce(f64::min)
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
        self.batch(|s| {
            for call in calls {
                match call {
                    Call::Play(args) => {
                        s.play(plan, t, args, None, log);
                    }
                    Call::Hits(Value::List(items)) => {
                        for item in items.iter() {
                            match item {
                                Value::Record(f) => {
                                    s.play(plan, t, &f[..], None, log);
                                }
                                _ => log.push("sound dropped: a hit is not a record".into()),
                            }
                        }
                    }
                    Call::Hits(_) => log.push("sound dropped: playSounds takes a list".into()),
                    Call::Stop(group) => {
                        let group = given(*group).map(|g| g.text().to_string());
                        s.stop(t, group.as_deref(), log);
                    }
                }
            }
        });
    }

    /// Every live voice ends now: the runner is starting over (D5); every
    /// haptic still to come is retracted.
    pub(crate) fn reload(&mut self, t: f64, log: &mut Vec<String>) {
        self.batch(|s| {
            let n = s.end_live(t, None, false);
            log.push(format!("sounds stopped: {n} (reload)"));
        });
        self.retract_haptics(t, f64::NEG_INFINITY, "cancelled", log);
    }

    /// Changes made by `f`, whose ops for the output are queued once, at the
    /// end: a commit's calls, or a sequence's changes at one instant.
    pub(crate) fn batch<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let before: Vec<(u64, f64)> = self.voices.iter().map(|v| (v.id, v.ends)).collect();
        let first_new = self.next + 1;
        let out = f(self);
        self.queue(before, first_new);
        out
    }

    /// One sequence step's sample at `t` (LLP 1125 D3), `args` as a
    /// `playSound`'s, for the step at `step_at`; [`Added::Full`] when
    /// [`WAITING`] voices already wait and this one would too. Inside
    /// [`Sounds::batch`].
    pub(crate) fn issue(
        &mut self,
        plan: &Plan,
        t: f64,
        args: &[Value],
        step_at: f64,
        log: &mut Vec<String>,
    ) -> Added {
        self.play(plan, t, args, Some(step_at), log)
    }

    /// One sequence step's haptic at `at`, for the step at `step_at`,
    /// issued at `t`, journaled and queued.
    pub(crate) fn issue_haptic(
        &mut self,
        kind: &str,
        at: f64,
        step_at: f64,
        t: f64,
        log: &mut Vec<String>,
    ) {
        self.next_haptic += 1;
        let id = self.next_haptic;
        log.push(format!("haptic {id} {kind} at {}", num(at)));
        self.ops.push(SoundOp::Haptic {
            id,
            kind: kind.to_string(),
            at,
        });
        self.haptics.push_back(Haptic {
            id,
            kind: kind.to_string(),
            at,
            by: None,
            step_at,
        });
        // Keep the last HAPTICS, letting the oldest played or retracted go
        // first: one still to come is kept for its retraction.
        while self.haptics.len() > HAPTICS {
            let Some(i) = self.haptics.iter().position(|h| h.by.is_some() || h.at < t) else {
                break;
            };
            self.haptics.remove(i);
        }
    }

    /// Haptics still waiting at `t` (`at >= t`) for a step at or after
    /// `from`, retracted with why; one line when any was.
    pub(crate) fn retract_haptics(
        &mut self,
        t: f64,
        from: f64,
        by: &'static str,
        log: &mut Vec<String>,
    ) {
        let mut n = 0;
        for h in self.haptics.iter_mut() {
            if h.by.is_none() && h.at >= t && after(h.step_at, from, by) {
                h.by = Some(by);
                self.ops.push(SoundOp::Unhaptic { id: h.id });
                n += 1;
            }
        }
        if n > 0 {
            log.push(format!("haptics retracted: {n} ({by})"));
        }
    }

    /// An edit at `t` (LLP 1125 D3; `from` is `t`, `by` [`By::Edit`]), or a
    /// clock's new anchor (D9; `by` [`By::Clock`]): every sequence voice not
    /// started by `t` for a step at or after `from` ends as `by` at its
    /// start, and so leaves the bound; so do its haptics still to come (a
    /// track's offset can put an earlier step's output after `from`: it
    /// stays, LLP 1125 D7). Inside [`Sounds::batch`].
    pub(crate) fn retract_seq(&mut self, t: f64, from: f64, by: By, log: &mut Vec<String>) {
        let mut n = 0;
        for v in self.voices.iter_mut() {
            if v.seq && v.by.sounds() && v.at >= t && after(v.step_at, from, by.word()) {
                (v.ends, v.by) = (v.at, by);
                n += 1;
            }
        }
        log.push(format!("sounds retracted: {n} ({})", by.word()));
        self.retract_haptics(t, from, by.word(), log);
    }

    /// The sequence stops at `t` (LLP 1125 D4): its voices end as
    /// `stopSounds()` ends them, and its haptics still to come are
    /// retracted. Inside [`Sounds::batch`].
    pub(crate) fn stop_seq(&mut self, t: f64, name: &str, log: &mut Vec<String>) {
        let mut n = 0;
        for v in self.voices.iter_mut() {
            if v.seq && v.live(t) {
                n += 1;
                if v.at < t {
                    (v.ends, v.by) = (t, By::Stop);
                } else {
                    (v.ends, v.by) = (v.at, By::Cancelled);
                }
            }
        }
        log.push(format!("sounds stopped: {n} (sequence {name})"));
        self.retract_haptics(t, f64::NEG_INFINITY, "cancelled", log);
    }

    fn queue(&mut self, before: Vec<(u64, f64)>, first_new: u64) {
        for v in &self.voices {
            if v.id >= first_new {
                self.ops.push(SoundOp::Play {
                    id: v.id,
                    sound: v.sound,
                    at: v.at,
                    gain: v.gain,
                    playback_rate: v.playback_rate,
                    pan: v.pan,
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

    /// One `playSound(src, at, gain, group, playbackRate, pan)`, or one
    /// hit's fields in the same order (a hit may stop after `group`); with
    /// its step's time when a sequence issues it.
    fn play(
        &mut self,
        plan: &Plan,
        t: f64,
        args: &[Value],
        step_at: Option<f64>,
        log: &mut Vec<String>,
    ) -> Added {
        let seq = step_at.is_some();
        let src = match args.first() {
            Some(v) if v.is_str() => v.text().to_string(),
            _ => return dropped(log, "sound dropped: the sound is not a string".into()),
        };
        let Some((sound, row)) = plan
            .sounds
            .iter()
            .enumerate()
            .find(|(_, row)| plan.str(row.src) == src)
        else {
            return dropped(
                log,
                format!("sound dropped: {src:?} is not a declared sound"),
            );
        };
        let number = |i: usize, default: f64| match given(args.get(i)) {
            None => Some(default),
            Some(Value::Number(n)) if n.is_finite() => Some(*n),
            Some(_) => None,
        };
        let Some(asked) = number(1, t) else {
            return dropped(log, format!("sound dropped: at is not a number ({src})"));
        };
        let Some(gain) = number(2, 1.0) else {
            return dropped(log, format!("sound dropped: gain is not a number ({src})"));
        };
        let group = given(args.get(3))
            .map(|g| g.text().to_string())
            .unwrap_or_default();
        let Some(playback_rate) = number(4, 1.0) else {
            return dropped(
                log,
                format!("sound dropped: playbackRate is not a number ({src})"),
            );
        };
        let Some(pan) = number(5, 0.0) else {
            return dropped(log, format!("sound dropped: pan is not a number ({src})"));
        };
        let playback_rate = playback_rate.clamp(*PLAYBACK_RATE.start(), *PLAYBACK_RATE.end());
        let pan = pan.clamp(-1.0, 1.0);
        let at = asked.max(t);
        // Another voice that would wait behind WAITING others: a tap's is
        // dropped; a sequence stops expanding there (LLP 1125 D3).
        if at > t && self.waiting(t) >= WAITING {
            if seq {
                return Added::Full;
            }
            return dropped(
                log,
                format!(
                    "sound dropped: {WAITING} voices wait ({src} at {})",
                    num(at)
                ),
            );
        }
        let length = 1000.0 * f64::from(row.frames) / f64::from(row.rate.max(1)) / playback_rate;
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
            playback_rate,
            pan,
            ends: at + length,
            by: By::End,
            seq,
            step_at: step_at.unwrap_or(at),
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
        if self.overfull(&voice) {
            for (w, (_, ends, by)) in self.voices.iter_mut().zip(&saved) {
                (w.ends, w.by) = (*ends, *by);
            }
            self.next -= 1;
            return dropped(
                log,
                format!(
                    "sound dropped: {LIVE} voices sound at its start ({} at {})",
                    voice.src,
                    num(at)
                ),
            );
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
        if voice.playback_rate != 1.0 {
            line.push_str(&format!(" playbackRate {}", num(voice.playback_rate)));
        }
        if voice.pan != 0.0 {
            line.push_str(&format!(" pan {}", num(voice.pan)));
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
        Added::Done
    }

    /// Whether `voice` would make more than [`LIVE`] voices sound at some
    /// voice's start (LLP 1125 D3): its own, or a later one's while it
    /// sounds. The count rises only at a start, so those are the instants
    /// to check, and the bound does not depend on the lookahead.
    fn overfull(&self, voice: &Voice) -> bool {
        if !voice.by.sounds() || voice.ends <= voice.at {
            return false;
        }
        let starts = self
            .voices
            .iter()
            .filter(|w| w.by.sounds() && w.at > voice.at && w.at < voice.ends)
            .map(|w| w.at);
        std::iter::once(voice.at)
            .chain(starts)
            .any(|s| self.voices.iter().filter(|w| w.sounding(s)).count() >= LIVE)
    }

    /// `stopSounds(group=)` at `t`: a sequence's voices still to start are
    /// its next steps, and play (LLP 1125 D4).
    fn stop(&mut self, t: f64, group: Option<&str>, log: &mut Vec<String>) {
        let n = self.end_live(t, group.filter(|g| !g.is_empty()), true);
        log.push(match group {
            Some(g) if !g.is_empty() => format!("sounds stopped: {n} (group {g})"),
            _ => format!("sounds stopped: {n}"),
        });
    }

    /// End every voice live at `t` (in `group`, when given): one sounding
    /// stops at `t`, one waiting is cancelled at its start — but a
    /// sequence's waiting one is kept when `seq_waits`.
    fn end_live(&mut self, t: f64, group: Option<&str>, seq_waits: bool) -> usize {
        let mut n = 0;
        for v in self.voices.iter_mut() {
            let kept = seq_waits && v.seq && v.at >= t;
            if v.live(t) && !kept && group.is_none_or(|g| v.group == g) {
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
