//! Clock in (LLP 1125 D9): a sequence whose `play` names `clock=<input>`
//! follows that input's MIDI clock instead of keeping its own time. The
//! input's Timing Clock, Start, Continue, Stop and Song Position messages
//! arrive with their timestamps in the host's input deliveries (batched by
//! the arm, at most every [`BATCH`]), one recorded runner call each, so the
//! record is a function of those deliveries and the clock: a replay
//! reproduces jitter, missing ticks and stops, and between deliveries one
//! `advance(60_000)` equals sixty `advance(1_000)`s.
//!
//! - **Tempo** comes from a window of the last [`WINDOW`] tick stamps,
//!   euxy's estimator moved where it can be replayed: `(n − 1)` ticks over
//!   the window's span, taken only on a packet-leading tick (one stamped
//!   later than the tick before: ticks CoreMIDI coalesced into one packet
//!   share a stamp) once [`MIN_TICKS`] are in it. A stamp earlier than the
//!   last, or more than [`GAP`] after it, empties the window. Until the
//!   first estimate the tempo is `bpm=` (120 when omitted).
//! - **Position counts ticks** (rulings #11): the `k`th tick after Start is
//!   beat `from + k / 24`. Start puts beat `from` at the tick it predicts
//!   next (the last tick plus one interval, when one came within [`GAP`],
//!   and never before Start itself), else at Start's own time (rulings
//!   #1); Continue resumes at the Song Position received since the last
//!   Stop, else where that Stop held it, else at beat 0, with no count-in
//!   (rulings #12). Song Position while playing is ignored and journaled.
//! - **The anchor** is re-taken at the end of each run of ticks: the last
//!   tick's time, its beat, and the tempo. Hits are issued under one
//!   segment up to [`LOOKAHEAD`] past the delivery (more by a track's
//!   negative offset). A new anchor that moves any hit still revocable —
//!   from [`HOLD`] past the delivery (more by that offset) to where the
//!   hits are issued — by more than [`DRIFT`] becomes the segment, and
//!   every hit from the band's start is retracted (`by: clock`) and issued
//!   again under it; one within [`DRIFT`] everywhere leaves the hits as
//!   they are. Retracting the whole band, not only from the first beat
//!   that drifts (rulings #5), keeps every revocable hit timed by the one
//!   segment, so a retraction by time takes exactly the beats it re-issues.
//!   Nothing within the hold moves.
//! - **Stop** silences the sequence as its gate turning false would (D4) and
//!   leaves it `waiting` with reason `clock`, still following (rulings #3);
//!   the app hears Stop as a `midimessage` and ends its take itself. No
//!   timeout ends a lost clock (rulings #14): the hits simply run out.
//! - **`exactTransport()`** says `clock: "midi"`; its tempo is rounded to
//!   0.1 and its tempo and anchor are published again only when the state
//!   or reason changes, the tempo moves by 0.5 or more, or the published
//!   line puts the latest anchor more than 1/48 of a beat off (rulings #7).
//!   While waiting its tempo is the device's.
//!
//! An edit while following starts no segment (`bpm=` is the prior only); an
//! edit, or Start again, in the hold after a new anchor can play a hit at
//! that boundary twice or not at all within the drift: a declared limit.
//!
//! @ref LLP 1125 D9

use super::{Fill, Out, Segment, State, Transport};
use crate::agent::num;
use crate::midi::HOLD;
use crate::sound::By;
use exact_plan::Plan;
use std::collections::VecDeque;

/// Tick stamps the tempo window holds (two beats).
pub const WINDOW: usize = 48;
/// Ticks the window needs before it says a tempo.
pub const MIN_TICKS: usize = 13;
/// A gap between ticks, ms, past which the window starts over (a tick at
/// 5 bpm).
pub const GAP: f64 = 500.0;
/// The longest an arm holds clock ticks before it delivers them, ms.
pub const BATCH: f64 = 50.0;
/// How far past a delivery the hits are issued, ms: the batch interval
/// plus the hold.
pub const LOOKAHEAD: f64 = BATCH + HOLD;
/// How far a revocable hit may drift under a new anchor before the hits
/// are issued again, ms.
pub const DRIFT: f64 = 2.0;

/// What one delivery's messages said, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Heard {
    /// Start (`cont` false) or Continue: beat `beat` falls at `at`.
    Start { at: f64, beat: f64, cont: bool },
    /// Stop.
    Stop,
    /// A run of ticks ended: the anchor it gives.
    Anchor(Segment),
}

/// The input a sequence follows, and what its clock has said.
#[derive(Clone, Debug)]
pub struct Follow {
    /// The input's id.
    pub input: String,
    stamps: VecDeque<f64>,
    /// The window's tempo, once it said one.
    pub estimate: Option<f64>,
    /// `bpm=`: the tempo before the first estimate.
    pub(super) prior: f64,
    /// `from=`: where Start puts the clock.
    pub(super) from: f64,
    /// Started (or continued) and not stopped since.
    pub running: bool,
    /// The beat of the first tick after Start or Continue.
    base: f64,
    /// Ticks since Start or Continue.
    pub next: f64,
    /// Song Position since the last Stop, in beats.
    spp: Option<f64>,
    /// Where the last Stop held the clock, in beats.
    pub held: Option<f64>,
    last_tick: Option<f64>,
    /// Ticks ever heard.
    pub ticks: u64,
    /// The latest anchor or start, for `exactTransport`'s answer.
    pub(super) seen: Option<Segment>,
}

impl Follow {
    pub(super) fn new(input: &str, prior: f64, from: f64) -> Follow {
        Follow {
            input: input.to_string(),
            stamps: VecDeque::new(),
            estimate: None,
            prior,
            from,
            running: false,
            base: 0.0,
            next: 0.0,
            spp: None,
            held: None,
            last_tick: None,
            ticks: 0,
            seen: None,
        }
    }

    /// Tick stamps in the window.
    pub fn window(&self) -> usize {
        self.stamps.len()
    }

    /// The last tick's stamp.
    pub fn last_tick(&self) -> Option<f64> {
        self.last_tick
    }

    /// The tempo: the estimate, else the prior.
    pub fn tempo(&self) -> f64 {
        self.estimate.unwrap_or(self.prior)
    }

    /// One tick's stamp into the window (euxy's `engine.ts`, the order of
    /// its operations kept, as transport.js keeps it).
    fn stamp(&mut self, at: f64) {
        let prev = self.stamps.back().copied();
        if prev.is_some_and(|p| at < p || at - p > GAP) {
            self.stamps.clear();
        }
        self.stamps.push_back(at);
        if self.stamps.len() > WINDOW {
            let mut removed = self.stamps.pop_front();
            // The window's first tick stays packet-leading.
            while self.stamps.len() > 1 && self.stamps.front().copied() == removed {
                removed = self.stamps.pop_front();
            }
        }
        let n = self.stamps.len();
        let elapsed = at - self.stamps[0];
        let leading = prev.is_none_or(|p| at > p);
        if leading && n >= MIN_TICKS && elapsed > 0.0 {
            self.estimate = Some(60000.0 * (n - 1) as f64 / (elapsed * 24.0));
        }
    }

    /// The anchor at the last tick.
    fn anchor(&self) -> Segment {
        Segment {
            at: self.last_tick.unwrap_or(0.0),
            beat: self.base + (self.next - 1.0) / 24.0,
            bpm: self.tempo(),
        }
    }

    /// One delivery's clock messages, `(stamp, bytes)` each, in arrival
    /// order: what they said, a run of ticks ending at each transport
    /// message and at the delivery's end.
    pub(super) fn feed(
        &mut self,
        msgs: &[(f64, Vec<u8>)],
        name: &str,
        log: &mut Vec<String>,
    ) -> Vec<Heard> {
        let (mut out, mut run) = (Vec::new(), false);
        for (at, data) in msgs {
            let at = *at;
            let status = data.first().copied().unwrap_or(0);
            if status == 0xF8 {
                self.stamp(at);
                self.ticks += 1;
                if self.running {
                    self.next += 1.0;
                    run = true;
                }
                self.last_tick = Some(at);
                continue;
            }
            if run && matches!(status, 0xFA..=0xFC) {
                run = false;
                out.push(Heard::Anchor(self.anchor()));
                self.seen = Some(self.anchor());
            }
            match status {
                0xFA | 0xFB => {
                    let cont = status == 0xFB;
                    let interval = 60000.0 / (24.0 * self.tempo());
                    let start = match self.last_tick {
                        Some(l) if at >= l && at - l <= GAP => (l + interval).max(at),
                        _ => at,
                    };
                    self.base = if cont {
                        self.spp.or(self.held).unwrap_or(0.0)
                    } else {
                        self.from
                    };
                    (self.next, self.running, self.spp, self.held) = (0.0, true, None, None);
                    out.push(Heard::Start {
                        at: start,
                        beat: self.base,
                        cont,
                    });
                    self.seen = Some(Segment {
                        at: start,
                        beat: self.base,
                        bpm: self.tempo(),
                    });
                }
                0xFC if self.running => {
                    self.held = Some(self.base + self.next / 24.0);
                    (self.running, self.spp) = (false, None);
                    out.push(Heard::Stop);
                }
                0xF2 if data.len() == 3 => {
                    if self.running {
                        log.push(format!(
                            "sequence {name}: song position ignored while playing"
                        ));
                    } else {
                        let sixteenths = (u32::from(data[2]) << 7) | u32::from(data[1]);
                        self.spp = Some(f64::from(sixteenths) / 4.0);
                    }
                }
                _ => {}
            }
        }
        if run {
            out.push(Heard::Anchor(self.anchor()));
            self.seen = Some(self.anchor());
        }
        out
    }
}

/// What `exactTransport()` last published while following (rulings #7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Published {
    pub(super) state: State,
    pub(super) reason: &'static str,
    pub(super) bpm: f64,
    pub(super) at: f64,
    pub(super) beat: f64,
}

/// A tempo to one decimal, as `exactTransport` says it.
fn tenth(bpm: f64) -> f64 {
    (bpm * 10.0).round() / 10.0
}

impl Transport {
    /// The input the playing sequence follows, while it does.
    pub fn clock_input(&self) -> Option<&str> {
        self.follow.as_ref().map(|f| f.input.as_str())
    }

    /// What the followed clock has said, while a sequence follows one.
    pub fn follower(&self) -> Option<&Follow> {
        self.follow.as_ref()
    }

    /// The farthest a track's offset moves a hit earlier, ms (0 or more).
    fn early(&self) -> f64 {
        self.play.as_ref().map_or(0.0, |p| {
            p.tracks.iter().map(|t| -t.offset).fold(0.0, f64::max)
        })
    }

    /// One delivery's clock messages from the followed input at runner time
    /// `now` (LLP 1125 D9): Start and Continue start a segment, Stop
    /// silences, each run of ticks re-anchors, then the hits are issued to
    /// [`LOOKAHEAD`] past the delivery.
    pub(crate) fn follow(
        &mut self,
        plan: &Plan,
        out: &mut Out<'_>,
        now: f64,
        live: bool,
        msgs: &[(f64, Vec<u8>)],
        log: &mut Vec<String>,
    ) {
        let (Some(i), Some(f)) = (self.owner, self.follow.as_mut()) else {
            return;
        };
        let name = plan.str(plan.sequences[i].name);
        let heard = f.feed(msgs, name, log);
        let t = msgs.iter().map(|m| m.0).fold(now, f64::max);
        let early = self.early();
        self.clock_to = t + LOOKAHEAD + early;
        for h in heard {
            match h {
                Heard::Start { at, beat, cont } => {
                    let word = if cont { "continue" } else { "start" };
                    log.push(format!(
                        "sequence {name}: clock {word} at {}, beat {}",
                        num(at),
                        num(beat)
                    ));
                    if self.state != State::Waiting {
                        // Start again while playing: what was issued for later goes.
                        out.sounds.batch(|s| s.retract_seq(t, t, By::Clock, log));
                        out.midi.retract_seq(t, t, "clock", log);
                    }
                    let bpm = self.follow.as_ref().map_or(0.0, Follow::tempo);
                    self.segment(Segment { at, beat, bpm });
                    self.state = if beat < 0.0 {
                        State::CountIn
                    } else {
                        State::Playing
                    };
                    self.reason = "";
                    (self.issued_to, self.skip, self.issued) = (at, 0, (beat, 0));
                }
                Heard::Stop => {
                    self.silence(plan, out, t, "clock", log);
                    self.state = State::Waiting;
                }
                Heard::Anchor(seg) if self.state != State::Waiting => {
                    self.anchor(out, t, seg, early, name, log);
                }
                Heard::Anchor(_) => {}
            }
        }
        if self.state != State::Waiting {
            self.timing(t);
            self.fill(plan, out, t, live, Fill::Clock, log);
        }
    }

    /// A run of ticks gave `new` at `t`: kept out while every revocable hit
    /// is within [`DRIFT`] of it, else the segment, the band re-issued.
    fn anchor(
        &mut self,
        out: &mut Out<'_>,
        t: f64,
        new: Segment,
        early: f64,
        name: &str,
        log: &mut Vec<String>,
    ) {
        let Some(old) = self.segments.back().copied() else {
            return;
        };
        let band = old.beat_at(t + HOLD + early);
        let issued = self.issued.0;
        let far = issued.max(old.beat_at(self.clock_to));
        let drift = |b: f64| (new.time(b) - old.time(b)).abs();
        if drift(band) <= DRIFT && drift(far) <= DRIFT {
            return;
        }
        log.push(format!(
            "sequence {name} re-anchored: beat {} at {}, bpm {}",
            num(new.beat),
            num(new.at),
            num(new.bpm)
        ));
        let from = if issued > band {
            let from = old.time(band);
            out.sounds.batch(|s| s.retract_seq(t, from, By::Clock, log));
            out.midi.retract_seq(t, from, "clock", log);
            self.issued = (band, 0);
            self.skip = 0;
            band
        } else {
            issued
        };
        self.segment(new);
        self.issued_to = new.time(from);
    }

    /// `exactTransport`'s answer while following, published again by
    /// rulings #7's hysteresis; called after every change.
    pub(crate) fn publish(&mut self) {
        if !self.clocked {
            return;
        }
        let p = self.published;
        let (bpm, seen) = match &self.follow {
            Some(f) => (tenth(f.tempo()), f.seen),
            None => (p.bpm, None),
        };
        let line = Segment {
            at: p.at,
            beat: p.beat,
            bpm: p.bpm,
        };
        let off =
            seen.is_some_and(|a| p.bpm <= 0.0 || (line.beat_at(a.at) - a.beat).abs() > 1.0 / 48.0);
        if (self.state, self.reason) != (p.state, p.reason) || (bpm - p.bpm).abs() >= 0.5 || off {
            self.published = Published {
                state: self.state,
                reason: self.reason,
                bpm,
                at: seen.map_or(p.at, |a| a.at),
                beat: seen.map_or(p.beat, |a| a.beat),
            };
        }
    }
}
