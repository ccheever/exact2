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
//! - **MIDI** (LLP 1125 D7). A step whose `note` is not −1 on a track with
//!   an `output` is a note-on there and a note-off `duration` beats later
//!   (`crate::midi`); `clockOut=` sends clock to an output. A track's
//!   `offset` shifts its samples', notes' and haptics' times, never which
//!   hits a window holds: at the arming an output before the commit's time
//!   is played at it, at an edit it is skipped (it was played already), and
//!   on a live clock one past at a fill is late. So an edit of `offset`
//!   itself may play a step twice or not at all within the change's size,
//!   a declared limit.
//! @ref LLP 1125 D2–D5, D7

use crate::agent::num;
use crate::midi::{Midi, Queued, HOLD, TICKS_PER_SECOND};
use crate::sound::{Added, Sounds};
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
/// The farthest a track's `offset` may move it, ms.
pub const OFFSET_LIMIT: f64 = 500.0;
/// The farthest `from` may be from beat 0, and the shortest a track may be,
/// in beats: with them a loop's index stays below 2^53, where every whole
/// number is exact, for thousands of years of play (a fill steps it by 1).
pub const FROM_LIMIT: f64 = 2147483648.0;
/// See [`FROM_LIMIT`]: 2^-20 of a beat.
pub const SHORTEST: f64 = 1.0 / 1048576.0;
/// A loop index past which a fill drops a track that loops forever (or as
/// many times): a beat kept across an edit (a long track at a high tempo,
/// then a short one) can reach it where `from` and the length alone
/// cannot, and past 2^53 a loop index no longer steps by 1. A finite track
/// with fewer loops is never iterated there (its last loop is behind).
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
/// MIDI note (`note` −1 for none), a haptic, or any of them.
#[derive(Clone, Debug)]
struct Step {
    beat: f64,
    src: String,
    gain: f64,
    group: String,
    note: f64,
    velocity: f64,
    channel: f64,
    duration: f64,
    playback_rate: f64,
    pan: f64,
    haptic: String,
}

#[derive(Clone, Debug)]
struct Track {
    length: f64,
    loops: f64,
    offset: f64,
    output: String,
    steps: Vec<Step>,
}

/// A `play(...)` value as read.
#[derive(Clone, Debug)]
struct Play {
    tracks: Vec<Track>,
    bpm: f64,
    from: f64,
    count_in: Vec<Step>,
    clock_out: String,
}

/// One hit of a window: its time and beat, its track (0 the count-in) and
/// step.
struct Hit {
    at: f64,
    beat: f64,
    track: usize,
    step: usize,
}

/// How a fill treats an output before the time it runs at (the module's
/// rule for `offset`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill {
    /// The arming: played at the commit's time.
    Arm,
    /// An edit: skipped, it was played already.
    Edit,
    /// Its own wake: late on a live clock.
    Wake,
}

/// The outputs a fill issues into: the voice table and the MIDI queue.
pub(crate) struct Out<'a> {
    pub(crate) sounds: &'a mut Sounds,
    pub(crate) midi: &'a mut Midi,
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
            let (offset, output) = (f.get(2).map_or(0.0, |v| number(Some(v))), text(f.get(3)));
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
            } else if !(offset.is_finite() && offset.abs() <= OFFSET_LIMIT) {
                Some(format!(
                    "its offset {} is not within {} ms",
                    num(offset),
                    num(OFFSET_LIMIT)
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
                .filter_map(|(j, s)| {
                    let label = format!("{label} step {}", j + 1);
                    step(plan, s, length, &output, &label, log)
                })
                .collect();
            tracks.push(Track {
                length,
                loops,
                offset,
                output,
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
                        "",
                        &format!("sequence {name}: count-in step {}", j + 1),
                        log,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut clock_out = text(args.get(5));
        if !clock_out.is_empty() && bpm * 24.0 / 60.0 > TICKS_PER_SECOND {
            log.push(format!(
                "sequence {name}: clock out dropped: bpm {} is more than {} ticks a second",
                num(bpm),
                num(TICKS_PER_SECOND)
            ));
            clock_out.clear();
        }
        Ok(Play {
            tracks,
            bpm,
            from,
            count_in,
            clock_out,
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
                        && n(a.note, b.note)
                        && n(a.velocity, b.velocity)
                        && n(a.channel, b.channel)
                        && n(a.duration, b.duration)
                        && n(a.playback_rate, b.playback_rate)
                        && n(a.pan, b.pan)
                        && a.src == b.src
                        && a.group == b.group
                        && a.haptic == b.haptic
                })
        };
        n(self.bpm, other.bpm)
            && n(self.from, other.from)
            && self.clock_out == other.clock_out
            && steps(&self.count_in, &other.count_in)
            && self.tracks.len() == other.tracks.len()
            && self.tracks.iter().zip(&other.tracks).all(|(a, b)| {
                n(a.length, b.length)
                    && n(a.loops, b.loops)
                    && n(a.offset, b.offset)
                    && a.output == b.output
                    && steps(&a.steps, &b.steps)
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
            let beat = self.from + s.beat;
            let at = seg.time(beat);
            if from <= at && at < to {
                out.push(Hit {
                    at,
                    beat,
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
                    let beat = k * t.length + s.beat;
                    let at = seg.time(beat);
                    if from <= at && at < to {
                        out.push(Hit {
                            at,
                            beat,
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

    /// A hit's track's offset and output (none for the count-in).
    fn track(&self, hit: &Hit) -> (f64, &str) {
        match hit.track {
            0 => (0.0, ""),
            i => (self.tracks[i - 1].offset, &self.tracks[i - 1].output),
        }
    }
}

/// One step read from its record, or why not (journaled); a step with no
/// sample, no note its track sends (`output`) and no haptic plays nothing
/// here and is left out unsaid.
fn step(
    plan: &Plan,
    v: &Value,
    length: f64,
    output: &str,
    label: &str,
    log: &mut Vec<String>,
) -> Option<Step> {
    let f = items(Some(v));
    let beat = number(f.first());
    let src = text(f.get(1));
    let haptic = text(f.get(8));
    let (note, velocity, channel, duration) = (
        f.get(4).map_or(-1.0, |v| number(Some(v))),
        number(f.get(5)),
        number(f.get(6)),
        number(f.get(7)),
    );
    let whole = |n: f64, low: f64, high: f64| n.fract() == 0.0 && (low..=high).contains(&n);
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
    } else if note != -1.0 && !whole(note, 0.0, 127.0) {
        Some(format!("note {} is not -1 or a MIDI note 0–127", num(note)))
    } else if note != -1.0 && !whole(velocity, 1.0, 127.0) {
        Some(format!("velocity {} is not 1–127", num(velocity)))
    } else if note != -1.0 && !whole(channel, 0.0, 15.0) {
        Some(format!("channel {} is not 0–15", num(channel)))
    } else if note != -1.0 && !(duration.is_finite() && duration > 0.0) {
        Some(format!(
            "duration {} is not a positive number of beats",
            num(duration)
        ))
    } else {
        None
    };
    if let Some(why) = why {
        log.push(format!("{label} dropped: {why}"));
        return None;
    }
    if src.is_empty() && haptic.is_empty() && (note == -1.0 || output.is_empty()) {
        return None;
    }
    Some(Step {
        beat,
        src,
        gain,
        group: text(f.get(3)),
        note: if output.is_empty() { -1.0 } else { note },
        velocity,
        channel,
        duration,
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
    /// Notes the value read could not send were journaled.
    unsent_told: bool,
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
            unsent_told: false,
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

    /// The beat at `t` while a sequence plays, by the segment that holds
    /// it; −1 while stopped (LLP 1125 D8, rulings #13).
    pub fn beat_at(&self, t: f64) -> f64 {
        if self.owner.is_none() {
            return -1.0;
        }
        self.segments
            .iter()
            .rev()
            .find(|g| g.at <= t)
            .or(self.segments.front())
            .map_or(-1.0, |g| g.beat_at(t))
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
        out: &mut Out<'_>,
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
                    self.stop(plan, out, t, "gate", log);
                } else if s.key != was_key {
                    // A restart: Stop, then Start at its new beat 0 (rulings #9).
                    out.sounds.batch(|so| so.retract_seq(t, log));
                    out.midi.retract_seq(t, log);
                    out.midi.end_seq(t, log);
                    self.arm(plan, out, i, t, live, s.args, log);
                } else if self.raw.len() != s.args.len()
                    || !self.raw.iter().zip(&s.args).all(|(a, b)| same(a, b))
                {
                    self.edit(plan, out, t, live, s.args, log);
                }
            } else if rising {
                match self.owner {
                    Some(o) => log.push(format!(
                        "sequence {} did not play: sequence {} is playing",
                        plan.str(plan.sequences[i].name),
                        plan.str(plan.sequences[o].name)
                    )),
                    None => self.arm(plan, out, i, t, live, s.args, log),
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // the runner's parts a fill needs, and the arguments read
    fn arm(
        &mut self,
        plan: &Plan,
        out: &mut Out<'_>,
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
                self.stop(plan, out, t, "bpm", log);
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
        out.midi
            .clock_to(&play.clock_out, (play.from * 24.0).ceil(), t, log);
        self.play = Some(play);
        self.issued_to = t;
        self.skip = 0;
        self.unsent_told = false;
        self.timing(t);
        self.fill(plan, out, t, live, Fill::Arm, log);
    }

    fn edit(
        &mut self,
        plan: &Plan,
        out: &mut Out<'_>,
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
                return self.stop(plan, out, t, "bpm", log);
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
        out.sounds.batch(|s| s.retract_seq(t, log));
        out.midi.retract_seq(t, log);
        let first = (seg.beat_at(t) * 24.0).ceil();
        out.midi.clock_to(&play.clock_out, first, t, log);
        self.play = Some(play);
        self.issued_to = t;
        self.skip = 0;
        self.unsent_told = false;
        self.timing(t);
        self.fill(plan, out, t, live, Fill::Edit, log);
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
        out: &mut Out<'_>,
        t: f64,
        reason: &'static str,
        log: &mut Vec<String>,
    ) {
        let Some(i) = self.owner.take() else {
            return;
        };
        let name = plan.str(plan.sequences[i].name);
        log.push(format!("sequence {name} stopped ({reason})"));
        out.sounds.batch(|s| s.stop_seq(t, name, log));
        out.midi.stop_seq(t, log);
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
        out: &mut Out<'_>,
        t: f64,
        log: &mut Vec<String>,
    ) {
        if self.owner.is_some() {
            self.stop(plan, out, t, "background", log);
        }
    }

    /// The `ended=` action of the sequence playing, if it has one.
    pub(crate) fn ended_action(&self, plan: &Plan) -> Option<ActionsId> {
        self.owner.and_then(|i| plan.sequences[i].ended)
    }

    /// The last finite track ended at `t`: stopped, clock out sent Stop,
    /// with the `ended=` action to run, if any.
    pub(crate) fn end(
        &mut self,
        plan: &Plan,
        midi: &mut Midi,
        t: f64,
        log: &mut Vec<String>,
    ) -> Option<ActionsId> {
        let i = self.owner.take()?;
        let row = &plan.sequences[i];
        log.push(format!("sequence {} ended", plan.str(row.name)));
        midi.end_seq(t, log);
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
        out: &mut Out<'_>,
        at: f64,
        live: bool,
        log: &mut Vec<String>,
    ) {
        if self.phase_due <= at {
            self.phase_due = f64::INFINITY;
            self.state = State::Playing;
        }
        if self.refill_due <= at {
            self.fill(plan, out, at, live, Fill::Wake, log);
        }
    }

    /// Issue the hits in `[issued_to, at + LOOKAHEAD)`, and clock out's
    /// ticks before its end. On a live clock an output more than [`REFILL`]
    /// past at its own wake (a stall's) is skipped, and said once; one less
    /// past starts now. Never on a seek. `how` places an output before `at`
    /// (the module's rule).
    fn fill(
        &mut self,
        plan: &Plan,
        out: &mut Out<'_>,
        at: f64,
        live: bool,
        how: Fill,
        log: &mut Vec<String>,
    ) {
        let (Some(seg), Some(i)) = (self.segments.back().copied(), self.owner) else {
            return;
        };
        let end = seg.beat_at(at + LOOKAHEAD);
        let name = plan.str(plan.sequences[i].name);
        let mut dropped = false;
        if let Some(play) = self.play.as_mut() {
            play.tracks.retain(|t| {
                let k = (end / t.length).abs();
                let far = (t.loops == 0.0 || t.loops - 1.0 >= EXACT) && (k.is_nan() || k >= EXACT);
                if far {
                    log.push(format!(
                        "sequence {name}: a track dropped: beat {} is past exact loop counting",
                        num(end)
                    ));
                    dropped = true;
                }
                !far
            });
        }
        // What is left may now end: every track finite, or none.
        if dropped {
            self.timing(at);
        }
        let Some(play) = self.play.take() else {
            return;
        };
        let from = self.issued_to;
        let to = at + LOOKAHEAD;
        let late_before = if live { at } else { f64::NEG_INFINITY };
        let end = play.end_beat();
        out.midi
            .ticks(|b| seg.time(b), to, end, late_before, at, log);
        let mut hits = play.hits(&seg, from, to);
        let skip = std::mem::take(&mut self.skip);
        let skipped = hits.iter().take(skip).take_while(|h| h.at == from).count();
        hits.drain(..skipped);
        let (mut late, mut unsent) = (0, None);
        let midi = &mut *out.midi;
        let full = out.sounds.batch(|s| {
            for (k, hit) in hits.iter().enumerate() {
                let (offset, output) = play.track(hit);
                // Where an output before `at` goes: played now at the arming,
                // skipped at an edit, late at a stalled live wake.
                let place = |when: f64, late: &mut usize| -> Option<f64> {
                    if when >= at {
                        Some(when)
                    } else if how == Fill::Arm {
                        Some(at)
                    } else if how == Fill::Edit {
                        None
                    } else if live && when < at - REFILL {
                        *late += 1;
                        None
                    } else {
                        Some(when)
                    }
                };
                let step = play.step(hit);
                let when = hit.at + offset;
                let mut counted = late;
                if !step.src.is_empty() {
                    if let Some(when) = place(when, &mut counted) {
                        let args = [
                            Value::str(&step.src),
                            Value::Number(when),
                            Value::Number(step.gain),
                            Value::str(&step.group),
                            Value::Number(step.playback_rate),
                            Value::Number(step.pan),
                        ];
                        if s.issue(plan, at, &args, hit.at, log) == Added::Full {
                            return Some(k);
                        }
                    }
                }
                if step.note >= 0.0 {
                    let off = seg.time(hit.beat + step.duration) + offset;
                    // At an edit a note within the hold was handed already.
                    let skip = how == Fill::Edit && when - HOLD <= at;
                    match place(when, &mut counted).filter(|_| !skip) {
                        Some(on) if midi.can_send(output) => {
                            let status = 0x90 | step.channel as u8;
                            let data = [status, step.note as u8, step.velocity as u8];
                            let queued = midi.note(output, data, on, off.max(on), hit.at, at, log);
                            if queued == Queued::Full {
                                return Some(k);
                            }
                        }
                        Some(_) => {
                            unsent.get_or_insert(output.to_string());
                        }
                        None => {}
                    }
                }
                if !step.haptic.is_empty() {
                    if let Some(when) = place(when, &mut counted) {
                        s.issue_haptic(&step.haptic, when, hit.at, at, log);
                    }
                }
                // A hit is late once, however many of its outputs were.
                late += usize::from(counted > late);
            }
            None
        });
        if late > 0 {
            log.push(format!("sequence {name}: {late} steps late, skipped"));
        }
        if let Some(output) = unsent.filter(|_| !self.unsent_told) {
            self.unsent_told = true;
            let ports = out.midi.ports();
            let why = if ports.access != crate::midi::Access::Granted {
                format!("access is {}", ports.access.word())
            } else {
                format!("{output} is not a connected output")
            };
            log.push(format!("sequence {name}: notes not sent: {why}"));
        }
        match full {
            Some(k) => {
                let t = hits[k].at;
                let before = hits[..k].iter().rev().take_while(|h| h.at == t).count();
                self.issued_to = t;
                self.skip = before + if t == from { skipped } else { 0 };
                // WAITING voices or QUEUED messages wait: the first to start
                // or be handed frees a place, and the blocked hit is due no
                // later than its own time (then it no longer waits: taps far
                // ahead do not starve it).
                let freed = [out.sounds.next_start(at), out.midi.next_handed(at)];
                self.refill_due = freed.into_iter().flatten().fold(t, f64::min);
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
