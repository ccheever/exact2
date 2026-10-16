//! The transport (LLP 1125 D2–D5): a `sequence` declares what should play,
//! as a value, and the runner expands it into the voice table
//! (`crate::sound`) up to [`LOOKAHEAD`] ahead of its clock, on a wake of its
//! own that runs no app action and makes no commit. The record is a pure
//! function of the sequence's history and the clock, so the driver sees
//! every hit with no audio, and one `advance(60_000)` equals sixty
//! `advance(1_000)`s (the seek rule, LLP 1092 D10).
//!
//! - **Segments.** Beat `b` falls at `at + (b − beat) × 60000 / bpm` in the
//!   segment `(at, beat, bpm)` that holds it. Arming starts one at the
//!   commit's time and beat `from`; a new `bpm` starts one at the edit's
//!   time and the beat reached then, so tempo moves without a jump in phase.
//! - **Tracks** loop at their own length, `loops` times or forever (0), so
//!   tracks of different lengths run against each other with no common
//!   loop. Steps at one time play in track order, then list order; the
//!   count-in's (`countIn=`, beats from `from`) before every track's.
//! - **Windows.** Each fill issues the hits whose time is in
//!   `[issued_to, at + LOOKAHEAD)`, so consecutive windows partition the
//!   timeline and every hit is issued once, whatever the wakes. A fill is
//!   due again [`REFILL`] before its window ends. When [`WAITING`] voices
//!   already wait it stops early, at that hit's time (remembering how many
//!   hits at that time it issued), and wakes when the first waiting voice
//!   starts: nothing is dropped.
//! - **Edits.** Arguments that differ from the last ones read, compared by
//!   value, retract every voice the sequence issued that has not started
//!   (`edit`) and expand the new value from the commit's time. Voices
//!   already sounding play on.
//! - **What is not played** is dropped and journaled once per value read: a
//!   step outside its track (or its count-in), an undeclared sound, an
//!   unknown haptic kind, a non-finite gain, rate or pan, a track whose
//!   length is not positive, whose `loops` is not a whole number, or which
//!   would loop more than [`LOOPS_PER_SECOND`] times a second (the bound
//!   that keeps a fill's work finite). A `bpm` that is not a positive
//!   number, or a `from` that is not finite, stops the sequence (reason
//!   `bpm`).
//! - **One plays at a time.** A sequence plays when its gate turns true, or
//!   its key changes, while none plays; a second arming while one plays is
//!   journaled and does not play. One stopped by its end, the background or
//!   a bad tempo stays stopped until its gate turns true again or its key
//!   changes.
//!
//! A step's MIDI fields (`note`, `velocity`, `channel`, `length`) and a
//! track's `offset` and `output` are read and type-checked but play nothing
//! until MIDI comes in (LLP 1125 stage 3).
//! @ref LLP 1125 D2–D5

use crate::agent::num;
use crate::sound::{Added, Sounds, WAITING};
use exact_plan::{ActionsId, Plan, Value};
use std::collections::VecDeque;

/// Reserved resource source, answered by the runner.
pub const SOURCE: &str = "exactTransport";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &["state", "reason", "bpm", "anchorAt", "anchorBeat", "clock"];
/// How far ahead of its clock the runner issues a sequence's hits, ms.
pub const LOOKAHEAD: f64 = 1000.0;
/// A fill is due when this much of its window is left, ms: a host whose
/// main thread stalls for up to this long loses no step.
pub const REFILL: f64 = 500.0;
/// Segments the record keeps (D5).
pub const SEGMENTS: usize = 1024;
/// The most times a track may loop in a second.
pub const LOOPS_PER_SECOND: f64 = 1000.0;
/// The farthest `from` may be from beat 0, and the shortest a track may be,
/// in beats: with them a loop's index stays below 2^53, where every whole
/// number is exact, for thousands of years of play (a fill steps it by 1).
pub const FROM_LIMIT: f64 = 2147483648.0;
/// See [`FROM_LIMIT`]: 2^-20 of a beat.
pub const SHORTEST: f64 = 1.0 / 1048576.0;
/// A loop index past which a fill stops the sequence (2^52): a beat kept
/// across an edit (a long track at a high tempo, then a short one) can
/// reach it where `from` and the length alone cannot, and past 2^53 a loop
/// index no longer steps by 1.
pub const EXACT: f64 = 4503599627370496.0;
/// A step's `haptic` kinds: `haptic()`'s (LLP 1077 D14).
pub const HAPTIC_KINDS: [&str; 10] = [
    "selection",
    "impact",
    "impact-light",
    "impact-medium",
    "impact-heavy",
    "impact-soft",
    "impact-rigid",
    "success",
    "warning",
    "error",
];

/// What the transport is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Nothing plays.
    Stopped,
    /// Beats below 0: the count-in.
    CountIn,
    /// Beat 0 and on.
    Playing,
}

impl State {
    /// Its word in `exactTransport().state`.
    pub fn word(self) -> &'static str {
        match self {
            State::Stopped => "stopped",
            State::CountIn => "countin",
            State::Playing => "playing",
        }
    }
}

/// A stretch of one tempo: beat `beat` falls at `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Runner time, ms.
    pub at: f64,
    /// The beat at `at`.
    pub beat: f64,
    /// Beats a minute.
    pub bpm: f64,
}

impl Segment {
    /// When beat `b` falls.
    pub fn time(&self, b: f64) -> f64 {
        self.at + (b - self.beat) * 60000.0 / self.bpm
    }

    /// The beat at time `t`.
    pub fn beat_at(&self, t: f64) -> f64 {
        self.beat + (t - self.at) * self.bpm / 60000.0
    }
}

/// One step as played: a sample's fields as `playSound` takes them, a
/// haptic, or both.
#[derive(Clone, Debug)]
struct Step {
    beat: f64,
    src: String,
    gain: f64,
    group: String,
    playback_rate: f64,
    pan: f64,
    haptic: String,
}

#[derive(Clone, Debug)]
struct Track {
    length: f64,
    loops: f64,
    steps: Vec<Step>,
}

/// A `play(...)` value as read.
#[derive(Clone, Debug)]
struct Play {
    tracks: Vec<Track>,
    bpm: f64,
    from: f64,
    count_in: Vec<Step>,
}

/// One hit of a window: its time, its track (0 the count-in) and step.
struct Hit {
    at: f64,
    track: usize,
    step: usize,
}

/// A number field.
fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => *n,
        _ => f64::NAN,
    }
}

/// A string field.
fn text(v: Option<&Value>) -> String {
    v.filter(|v| v.is_str())
        .map(|v| v.text().to_string())
        .unwrap_or_default()
}

/// A list's or a record's items.
fn items(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::List(i) | Value::Record(i)) => i,
        _ => &[],
    }
}

/// Two values the same: numbers bit for bit (so `NaN` is itself), a shared
/// list at once.
pub(crate) fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.to_bits() == y.to_bits(),
        (Value::List(x), Value::List(y)) | (Value::Record(x), Value::Record(y)) => {
            exact_plan::Items::ptr_eq(x, y)
                || (x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| same(a, b)))
        }
        (Value::Option(x), Value::Option(y)) => match (x, y) {
            (Some(x), Some(y)) => same(x, y),
            (None, None) => true,
            _ => false,
        },
        _ => a == b,
    }
}

impl Play {
    /// Read `play`'s arguments, journaling each step or track dropped; a
    /// tempo that is none is the refusal's text.
    fn read(
        plan: &Plan,
        name: &str,
        args: &[Value],
        log: &mut Vec<String>,
    ) -> Result<Play, String> {
        let bpm = number(args.get(1));
        if !(bpm.is_finite() && bpm > 0.0) {
            return Err(format!("bpm {} is not a tempo", num(bpm)));
        }
        let from = number(args.get(2));
        if !from.is_finite() {
            return Err("from is not a number".into());
        }
        if from.abs() > FROM_LIMIT {
            return Err(format!("from {} is beyond 2^31 beats", num(from)));
        }
        let mut tracks = Vec::new();
        for (i, track) in items(args.first()).iter().enumerate() {
            let f = items(Some(track));
            let (length, loops) = (number(f.first()), number(f.get(1)));
            let label = format!("sequence {name}: track {}", i + 1);
            let why = if !(length.is_finite() && length > 0.0) {
                Some(format!(
                    "its length {} is not a positive number of beats",
                    num(length)
                ))
            } else if !(loops.is_finite() && loops >= 0.0 && loops.fract() == 0.0) {
                Some(format!("loops {} is not a whole number", num(loops)))
            } else if length < SHORTEST {
                Some(format!("its length {} is under 2^-20 beats", num(length)))
            } else if bpm / 60.0 / length > LOOPS_PER_SECOND {
                Some(format!(
                    "it would loop more than {} times a second",
                    num(LOOPS_PER_SECOND)
                ))
            } else {
                None
            };
            if let Some(why) = why {
                log.push(format!("{label} dropped: {why}"));
                continue;
            }
            let steps = items(f.get(4))
                .iter()
                .enumerate()
                .filter_map(|(j, s)| step(plan, s, length, &format!("{label} step {}", j + 1), log))
                .collect();
            tracks.push(Track {
                length,
                loops,
                steps,
            });
        }
        let count_in = if from < 0.0 {
            items(args.get(3))
                .iter()
                .enumerate()
                .filter_map(|(j, s)| {
                    step(
                        plan,
                        s,
                        -from,
                        &format!("sequence {name}: count-in step {}", j + 1),
                        log,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(Play {
            tracks,
            bpm,
            from,
            count_in,
        })
    }

    /// The same play, numbers bit for bit (as transport.js's `Object.is`).
    fn same(&self, other: &Play) -> bool {
        let n = |a: f64, b: f64| a.to_bits() == b.to_bits();
        let steps = |a: &[Step], b: &[Step]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    n(a.beat, b.beat)
                        && n(a.gain, b.gain)
                        && n(a.playback_rate, b.playback_rate)
                        && n(a.pan, b.pan)
                        && a.src == b.src
                        && a.group == b.group
                        && a.haptic == b.haptic
                })
        };
        n(self.bpm, other.bpm)
            && n(self.from, other.from)
            && steps(&self.count_in, &other.count_in)
            && self.tracks.len() == other.tracks.len()
            && self.tracks.iter().zip(&other.tracks).all(|(a, b)| {
                n(a.length, b.length) && n(a.loops, b.loops) && steps(&a.steps, &b.steps)
            })
    }

    /// The beat the last finite track ends at, when every track is finite.
    fn end_beat(&self) -> Option<f64> {
        if self.tracks.iter().any(|t| t.loops == 0.0) {
            return None;
        }
        Some(
            self.tracks
                .iter()
                .map(|t| t.loops * t.length)
                .fold(0.0, f64::max),
        )
    }

    /// The hits whose time is in `[from, to)` under `seg`, in order.
    fn hits(&self, seg: &Segment, from: f64, to: f64) -> Vec<Hit> {
        let mut out = Vec::new();
        for (j, s) in self.count_in.iter().enumerate() {
            let at = seg.time(self.from + s.beat);
            if from <= at && at < to {
                out.push(Hit {
                    at,
                    track: 0,
                    step: j,
                });
            }
        }
        let (b0, b1) = (seg.beat_at(from), seg.beat_at(to));
        for (i, t) in self.tracks.iter().enumerate() {
            // A loop either side, so a time the division rounds is still seen.
            let mut k = ((b0 / t.length).floor() - 1.0).max(0.0);
            let mut last = (b1 / t.length).ceil() + 1.0;
            if t.loops > 0.0 {
                last = last.min(t.loops - 1.0);
            }
            while k <= last {
                for (j, s) in t.steps.iter().enumerate() {
                    let at = seg.time(k * t.length + s.beat);
                    if from <= at && at < to {
                        out.push(Hit {
                            at,
                            track: i + 1,
                            step: j,
                        });
                    }
                }
                k += 1.0;
            }
        }
        out.sort_by(|a, b| {
            // As transport.js compares them: `-0` is `0`.
            a.at.partial_cmp(&b.at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.track.cmp(&b.track))
                .then(a.step.cmp(&b.step))
        });
        out
    }

    fn step(&self, hit: &Hit) -> &Step {
        match hit.track {
            0 => &self.count_in[hit.step],
            i => &self.tracks[i - 1].steps[hit.step],
        }
    }
}

/// One step read from its record, or why not (journaled); a step with no
/// sample and no haptic plays nothing here and is left out unsaid.
fn step(plan: &Plan, v: &Value, length: f64, label: &str, log: &mut Vec<String>) -> Option<Step> {
    let f = items(Some(v));
    let beat = number(f.first());
    let src = text(f.get(1));
    let haptic = text(f.get(8));
    let (gain, playback_rate, pan) = (
        number(f.get(2)),
        f.get(9).map_or(1.0, |v| number(Some(v))),
        f.get(10).map_or(0.0, |v| number(Some(v))),
    );
    let why = if !(beat.is_finite() && beat >= 0.0 && beat < length) {
        Some(format!(
            "beat {} is outside [0, {})",
            num(beat),
            num(length)
        ))
    } else if !src.is_empty() && !plan.sounds.iter().any(|r| plan.str(r.src) == src) {
        Some(format!("{src:?} is not a declared sound"))
    } else if !haptic.is_empty() && !HAPTIC_KINDS.contains(&haptic.as_str()) {
        Some(format!("{haptic:?} is not a haptic kind"))
    } else if !src.is_empty() && !gain.is_finite() {
        Some("gain is not a number".into())
    } else if !src.is_empty() && !playback_rate.is_finite() {
        Some("playbackRate is not a number".into())
    } else if !src.is_empty() && !pan.is_finite() {
        Some("pan is not a number".into())
    } else {
        None
    };
    if let Some(why) = why {
        log.push(format!("{label} dropped: {why}"));
        return None;
    }
    if src.is_empty() && haptic.is_empty() {
        return None;
    }
    Some(Step {
        beat,
        src,
        gain,
        group: text(f.get(3)),
        playback_rate,
        pan,
        haptic,
    })
}

/// What one sequence's gate step read: its gate, its key's text, and (with
/// the gate true) its six `play` arguments.
pub(crate) struct Seen {
    pub(crate) on: bool,
    pub(crate) key: Option<String>,
    pub(crate) args: Vec<Value>,
}

/// What `exactTransport()` answers, by field.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// `stopped`, `countin` or `playing`.
    pub state: State,
    /// Why it last stopped: `gate`, `ended`, `background` or `bpm`; empty
    /// while it plays, or before it ever did.
    pub reason: &'static str,
    /// The tempo, beats a minute (0 before anything played).
    pub bpm: f64,
    /// The current segment's start, runner ms.
    pub anchor_at: f64,
    /// The beat at `anchor_at`.
    pub anchor_beat: f64,
}

impl Answer {
    /// One declared field's value.
    pub fn field(&self, name: &str) -> Option<Value> {
        Some(match name {
            "state" => Value::str(self.state.word()),
            "reason" => Value::str(self.reason),
            "bpm" => Value::Number(self.bpm),
            "anchorAt" => Value::Number(self.anchor_at),
            "anchorBeat" => Value::Number(self.anchor_beat),
            "clock" => Value::str("internal"),
            _ => return None,
        })
    }
}

/// The transport record and what it owes the runner's clock.
pub struct Transport {
    /// Each sequence's gate and key as the last commit that stood read them.
    gates: Vec<(bool, Option<String>)>,
    /// The sequence playing, by its plan row.
    owner: Option<usize>,
    /// The last sequence that played, for the agent.
    last: Option<usize>,
    raw: Vec<Value>,
    play: Option<Play>,
    segments: VecDeque<Segment>,
    recorded: u64,
    state: State,
    reason: &'static str,
    issued_to: f64,
    /// Hits at `issued_to` a fill already issued (it stopped at the cap).
    skip: usize,
    refill_due: f64,
    phase_due: f64,
    ended_due: f64,
    /// When the `exactTransport` readers are answered again.
    pub(crate) answer_due: f64,
    /// The resources reading `exactTransport`.
    pub(crate) readers: Vec<usize>,
    /// The gate step's reading, applied once its commit stands.
    pub(crate) pending: Option<Vec<Seen>>,
    /// A background stop the commit of `set_page`'s call that stands
    /// applies; the call clears it if none does.
    pub(crate) suspend: bool,
}

impl Transport {
    /// The transport for `plan`: stopped.
    pub fn new(plan: &Plan) -> Transport {
        Transport {
            gates: vec![(false, None); plan.sequences.len()],
            owner: None,
            last: None,
            raw: Vec::new(),
            play: None,
            segments: VecDeque::new(),
            recorded: 0,
            state: State::Stopped,
            reason: "",
            issued_to: 0.0,
            skip: 0,
            refill_due: f64::INFINITY,
            phase_due: f64::INFINITY,
            ended_due: f64::INFINITY,
            answer_due: f64::INFINITY,
            readers: (0..plan.resources.len())
                .filter(|&i| plan.str(plan.resources[i].source) == SOURCE)
                .collect(),
            pending: None,
            suspend: false,
        }
    }

    /// What `exactTransport()` answers now.
    pub fn answer(&self) -> Answer {
        let seg = self.segments.back().copied().unwrap_or(Segment {
            at: 0.0,
            beat: 0.0,
            bpm: 0.0,
        });
        Answer {
            state: self.state,
            reason: self.reason,
            bpm: seg.bpm,
            anchor_at: seg.at,
            anchor_beat: seg.beat,
        }
    }

    /// Whether a sequence plays (its count-in included).
    pub fn playing(&self) -> bool {
        self.owner.is_some()
    }

    /// The last sequence that played, by its plan row.
    pub fn sequence(&self) -> Option<usize> {
        self.last
    }

    /// Where the issued hits end, ms.
    pub fn issued_to(&self) -> f64 {
        self.issued_to
    }

    /// The record's segments, oldest first, and how many it ever held.
    pub fn segments(
        &self,
    ) -> (
        impl DoubleEndedIterator<Item = &Segment> + ExactSizeIterator,
        u64,
    ) {
        (self.segments.iter(), self.recorded)
    }

    /// The soonest of its own wakes: a fill or the count-in's end.
    pub fn wake_due(&self) -> f64 {
        self.refill_due.min(self.phase_due)
    }

    /// When the last finite track ends.
    pub fn ended_due(&self) -> f64 {
        self.ended_due
    }

    /// Apply a gate step's reading at `t`, the commit that read it having
    /// stood: arm, restart, edit or stop, in plan order.
    pub(crate) fn stood(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        t: f64,
        live: bool,
        seen: Vec<Seen>,
        log: &mut Vec<String>,
    ) {
        for (i, s) in seen.into_iter().enumerate() {
            let (was_on, was_key) = std::mem::replace(&mut self.gates[i], (s.on, s.key.clone()));
            let rising = s.on && (!was_on || s.key != was_key);
            if self.owner == Some(i) {
                if !s.on {
                    self.stop(plan, sounds, t, "gate", log);
                } else if s.key != was_key {
                    sounds.batch(|so| so.retract_seq(t, log));
                    self.arm(plan, sounds, i, t, live, s.args, log);
                } else if self.raw.len() != s.args.len()
                    || !self.raw.iter().zip(&s.args).all(|(a, b)| same(a, b))
                {
                    self.edit(plan, sounds, t, live, s.args, log);
                }
            } else if rising {
                match self.owner {
                    Some(o) => log.push(format!(
                        "sequence {} did not play: sequence {} is playing",
                        plan.str(plan.sequences[i].name),
                        plan.str(plan.sequences[o].name)
                    )),
                    None => self.arm(plan, sounds, i, t, live, s.args, log),
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // the runner's parts a fill needs, and the arguments read
    fn arm(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        i: usize,
        t: f64,
        live: bool,
        args: Vec<Value>,
        log: &mut Vec<String>,
    ) {
        let name = plan.str(plan.sequences[i].name);
        self.last = Some(i);
        let play = match Play::read(plan, name, &args, log) {
            Ok(play) => play,
            Err(why) => {
                log.push(format!("sequence {name} did not play: {why}"));
                // A restart's: the one playing stops.
                self.stop(plan, sounds, t, "bpm", log);
                self.state = State::Stopped;
                self.reason = "bpm";
                return;
            }
        };
        log.push(format!(
            "sequence {name} starts at beat {}, bpm {}",
            num(play.from),
            num(play.bpm)
        ));
        self.owner = Some(i);
        self.raw = args;
        self.segment(Segment {
            at: t,
            beat: play.from,
            bpm: play.bpm,
        });
        self.state = if play.from < 0.0 {
            State::CountIn
        } else {
            State::Playing
        };
        self.reason = "";
        self.play = Some(play);
        self.issued_to = t;
        self.skip = 0;
        self.timing(t);
        self.fill(plan, sounds, t, live, log);
    }

    fn edit(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        t: f64,
        live: bool,
        args: Vec<Value>,
        log: &mut Vec<String>,
    ) {
        let i = self.owner.expect("an edit is the owner's");
        let name = plan.str(plan.sequences[i].name);
        let play = match Play::read(plan, name, &args, log) {
            Ok(play) => play,
            Err(why) => {
                log.push(format!("sequence {name}: {why}"));
                return self.stop(plan, sounds, t, "bpm", log);
            }
        };
        self.raw = args;
        if self.play.as_ref().is_some_and(|p| p.same(&play)) {
            return;
        }
        let seg = *self
            .segments
            .back()
            .expect("a playing sequence has a segment");
        if play.bpm.to_bits() != seg.bpm.to_bits() {
            log.push(format!("sequence {name} edited: bpm {}", num(play.bpm)));
            self.segment(Segment {
                at: t,
                beat: seg.beat_at(t),
                bpm: play.bpm,
            });
        } else {
            log.push(format!("sequence {name} edited"));
        }
        sounds.batch(|s| s.retract_seq(t, log));
        self.play = Some(play);
        self.issued_to = t;
        self.skip = 0;
        self.timing(t);
        self.fill(plan, sounds, t, live, log);
    }

    /// The count-in's end and the last track's, under the current segment.
    /// `t`: the arming's or the edit's time, before which nothing ends: a
    /// last track already past (`from` beyond it, or an edit that shortened
    /// it) ends at once, never back in time.
    fn timing(&mut self, t: f64) {
        let (Some(play), Some(seg)) = (&self.play, self.segments.back()) else {
            return;
        };
        self.phase_due = if self.state == State::CountIn {
            seg.time(0.0)
        } else {
            f64::INFINITY
        };
        self.ended_due = play
            .end_beat()
            .map_or(f64::INFINITY, |b| seg.time(b).max(t));
    }

    fn segment(&mut self, seg: Segment) {
        self.segments.push_back(seg);
        self.recorded += 1;
        if self.segments.len() > SEGMENTS {
            self.segments.pop_front();
        }
    }

    /// Stop at `t`, journaled with `reason`: the sequence's voices end as
    /// `stopSounds()` ends them (LLP 1125 D4).
    pub(crate) fn stop(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        t: f64,
        reason: &'static str,
        log: &mut Vec<String>,
    ) {
        let Some(i) = self.owner.take() else {
            return;
        };
        let name = plan.str(plan.sequences[i].name);
        log.push(format!("sequence {name} stopped ({reason})"));
        sounds.batch(|s| s.stop_seq(t, name, log));
        self.state = State::Stopped;
        self.reason = reason;
        self.play = None;
        self.raw.clear();
        self.refill_due = f64::INFINITY;
        self.phase_due = f64::INFINITY;
        self.ended_due = f64::INFINITY;
    }

    /// The host is about to suspend the app (LLP 1125 D4): stop as if the
    /// gate had turned false, with reason `background`.
    pub(crate) fn suspend(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        t: f64,
        log: &mut Vec<String>,
    ) {
        if self.owner.is_some() {
            self.stop(plan, sounds, t, "background", log);
        }
    }

    /// The `ended=` action of the sequence playing, if it has one.
    pub(crate) fn ended_action(&self, plan: &Plan) -> Option<ActionsId> {
        self.owner.and_then(|i| plan.sequences[i].ended)
    }

    /// The last finite track ended at `t`: stopped, with the `ended=`
    /// action to run, if any.
    pub(crate) fn end(&mut self, plan: &Plan, log: &mut Vec<String>) -> Option<ActionsId> {
        let i = self.owner.take()?;
        let row = &plan.sequences[i];
        log.push(format!("sequence {} ended", plan.str(row.name)));
        self.state = State::Stopped;
        self.reason = "ended";
        self.play = None;
        self.raw.clear();
        self.refill_due = f64::INFINITY;
        self.phase_due = f64::INFINITY;
        self.ended_due = f64::INFINITY;
        row.ended
    }

    /// Its own wake at `at`: the count-in ends, or a fill.
    pub(crate) fn wake(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        at: f64,
        live: bool,
        log: &mut Vec<String>,
    ) {
        if self.phase_due <= at {
            self.phase_due = f64::INFINITY;
            self.state = State::Playing;
        }
        if self.refill_due <= at {
            self.fill(plan, sounds, at, live, log);
        }
    }

    /// Issue the hits in `[issued_to, at + LOOKAHEAD)`. On a live clock a
    /// hit more than [`REFILL`] past (a stall's) is skipped, and said once;
    /// one less past starts now. Never on a seek.
    fn fill(
        &mut self,
        plan: &Plan,
        sounds: &mut Sounds,
        at: f64,
        live: bool,
        log: &mut Vec<String>,
    ) {
        let (Some(seg), Some(i)) = (self.segments.back().copied(), self.owner) else {
            return;
        };
        let end = seg.beat_at(at + LOOKAHEAD);
        let far = self.play.as_ref().is_some_and(|p| {
            p.tracks.iter().any(|t| {
                let k = (end / t.length).abs();
                k.is_nan() || k >= EXACT
            })
        });
        if far {
            let name = plan.str(plan.sequences[i].name);
            log.push(format!(
                "sequence {name}: beat {} is past exact loop counting",
                num(end)
            ));
            return self.stop(plan, sounds, at, "bpm", log);
        }
        let Some(play) = self.play.take() else {
            return;
        };
        let from = self.issued_to;
        let to = at + LOOKAHEAD;
        let mut hits = play.hits(&seg, from, to);
        let skip = std::mem::take(&mut self.skip);
        let skipped = hits.iter().take(skip).take_while(|h| h.at == from).count();
        hits.drain(..skipped);
        let mut late = 0;
        let full = sounds.batch(|s| {
            for (k, hit) in hits.iter().enumerate() {
                // Only a stall's: a wake's own lateness (the blocked hit's
                // wake runs just after its time) is no reason to skip.
                if live && hit.at < at - REFILL {
                    late += 1;
                    continue;
                }
                let step = play.step(hit);
                if !step.src.is_empty() {
                    let args = [
                        Value::str(&step.src),
                        Value::Number(hit.at),
                        Value::Number(step.gain),
                        Value::str(&step.group),
                        Value::Number(step.playback_rate),
                        Value::Number(step.pan),
                    ];
                    if s.issue(plan, at, &args, log) == Added::Full {
                        return Some(k);
                    }
                }
                if !step.haptic.is_empty() {
                    s.issue_haptic(&step.haptic, hit.at, at, log);
                }
            }
            None
        });
        if late > 0 {
            log.push(format!(
                "sequence {}: {late} steps late, skipped",
                plan.str(plan.sequences[i].name)
            ));
        }
        match full {
            Some(k) => {
                let t = hits[k].at;
                let before = hits[..k].iter().rev().take_while(|h| h.at == t).count();
                self.issued_to = t;
                self.skip = before + if t == from { skipped } else { 0 };
                // WAITING voices wait: the first to start frees a place,
                // and the blocked hit is due no later than its own time
                // (then it no longer waits: taps far ahead do not starve it).
                debug_assert!(sounds.waiting(at) >= WAITING);
                self.refill_due = sounds.next_start(at).map_or(t, |s| s.min(t));
            }
            None => {
                self.issued_to = to;
                self.refill_due = to - REFILL;
            }
        }
        self.play = Some(play);
    }
}
#[cfg(test)]
pub(crate) mod tests;
