//! Jev plans, the engine executes. The world asks by publishing a numbered
//! situation; Contract's `jev` resource carries it to the gateway; the answer
//! returns as the live `plan` argument. Only an answer whose id matches the
//! open question is applied, and applying copies it — every probability — into
//! this saved resource and samples it with the world's RNG, so a run replays
//! exactly from its inputs. When the far player must swing and no answer has
//! arrived, a deterministic fallback plays and the HUD says so.
//! @ref game/diaries/003-tennis.md "Design: Jev plans, the engine executes"
use crate::rules::{Match, Side};
use exact_game::{json, math, Data, Resource, World};
use std::collections::BTreeMap;

/// One question Jev answers: its options' probabilities, as returned.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Choice {
    pub choice: String,
    pub confidence: f64,
    pub p: BTreeMap<String, f64>,
}

/// A scored question: the expected label index out of `levels` labels.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Score {
    pub score: f64,
    pub levels: u32,
    pub p: BTreeMap<String, f64>,
}

/// The data source's normalized reply, delivered as the `plan` argument.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Answer {
    pub id: u32,
    pub ok: bool,
    pub error: String,
    pub shot: Choice,
    pub target: Choice,
    pub aggression: Score,
    /// Probability that the far player should come to the net.
    pub approach: f64,
}

/// Where the intent came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Source {
    #[default]
    Jev,
    /// The answer missed the far player's commit point.
    Late,
    /// The request failed; the reason is in the journal.
    Unavailable,
    /// The player chose the offline opponent.
    Offline,
}

/// What the far player will do with its next stroke.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Intent {
    pub id: u32,
    pub serve: bool,
    pub shot: String,
    pub target: String,
    /// 0 (very safe) ..= 1 (all out).
    pub aggression: f32,
    pub approach: bool,
    /// Jev's probability for the sampled shot; 1 for a fallback.
    pub p: f32,
    pub source: Source,
}

/// A question in flight.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Ask {
    pub id: u32,
    pub serve: bool,
    /// Tick it was published.
    pub asked: u64,
    /// The far player already had to swing without it.
    pub committed: bool,
    /// The published request: `{"id":…,"kind":…,"state":{…}}`.
    pub json: String,
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Stats {
    pub asked: u32,
    pub on_time: u32,
    /// Committed without an answer (the fallback played).
    pub late: u32,
    /// Answers that arrived after their commit point.
    pub late_arrivals: u32,
    pub errors: u32,
    /// Answer latency in world milliseconds, for answers that arrived.
    pub latency_ms: Vec<u32>,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct Brain {
    pub offline: bool,
    pub next: u32,
    pub asking: Option<Ask>,
    /// The applied answer, waiting for the far player's commit.
    pub ready: Option<Intent>,
    /// The copied answer behind `ready`, probabilities included.
    pub answer: Option<Answer>,
    /// Last answer id seen in the plan argument.
    pub seen: u32,
    pub last: Option<Intent>,
    /// idle, thinking, ready, late, unavailable, offline.
    pub state: String,
    /// The last decision the far player played, shown until the next one.
    pub line: String,
    /// What Jev is doing now: "Jev thinking…", "plan ready in 0.54 s", "".
    pub status: String,
    pub stats: Stats,
}

/// The near player's habits, counted from rally history (never from input
/// timing), which Jev sees so it can pick on a weak side.
#[derive(Clone, Debug, Default, Resource)]
pub struct Scouting {
    pub forehands: u32,
    pub backhands: u32,
    pub forehand_errors: u32,
    pub backhand_errors: u32,
    pub crosscourt: u32,
    pub down_the_line: u32,
    pub net_approaches: u32,
    pub first_serves: u32,
    pub first_serves_in: u32,
    pub double_faults: u32,
    pub recent: Vec<String>,
}

impl Scouting {
    pub fn weaker(&self) -> &'static str {
        // Laplace-smoothed error rates; ties favour attacking the backhand.
        let rate = |e: u32, n: u32| (e as f32 + 1.0) / (n as f32 + 2.0);
        if rate(self.forehand_errors, self.forehands) > rate(self.backhand_errors, self.backhands) {
            "forehand"
        } else {
            "backhand"
        }
    }
    pub fn remember(&mut self, line: String) {
        self.recent.push(line);
        if self.recent.len() > 4 {
            self.recent.remove(0);
        }
    }
}

/// What Jev is told. Human-readable where a sentence says more than numbers.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Situation {
    pub you_are: String,
    pub decide: String,
    pub score: String,
    pub pressure: String,
    pub incoming: String,
    pub you: String,
    pub opponent: String,
    pub opponent_x_m: f32,
    pub opponent_depth_m: f32,
    pub rally_shots: u32,
    pub tendencies: Tendencies,
    pub recent_points: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Tendencies {
    pub forehands: u32,
    pub forehand_errors: u32,
    pub backhands: u32,
    pub backhand_errors: u32,
    pub weaker_side: String,
    pub crosscourt: u32,
    pub down_the_line: u32,
    pub net_approaches: u32,
    pub first_serve_pct: u32,
    pub double_faults: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
struct Request {
    id: u32,
    kind: String,
    state: Situation,
}

pub fn tendencies(s: &Scouting) -> Tendencies {
    Tendencies {
        forehands: s.forehands,
        forehand_errors: s.forehand_errors,
        backhands: s.backhands,
        backhand_errors: s.backhand_errors,
        weaker_side: s.weaker().into(),
        crosscourt: s.crosscourt,
        down_the_line: s.down_the_line,
        net_approaches: s.net_approaches,
        first_serve_pct: (s.first_serves_in * 100)
            .checked_div(s.first_serves)
            .unwrap_or(0),
        double_faults: s.double_faults,
    }
}

/// Publishable score from the far player's point of view.
pub fn score_line(m: &Match) -> String {
    let (you, jev) = m.labels();
    format!(
        "games: Jev {} – opponent {} (first to {}); points: Jev {} – opponent {}; {} serving",
        m.games[1],
        m.games[0],
        crate::rules::FIRST_TO,
        jev,
        you,
        if m.server == Side::Far {
            "Jev"
        } else {
            "opponent"
        }
    )
}

/// Open a numbered question. Offline play never asks.
pub fn ask(w: &World, serve: bool, situation: Situation) {
    let mut brain = w.resource_mut::<Brain>();
    if brain.offline {
        brain.state = "offline".into();
        return;
    }
    brain.next += 1;
    let id = brain.next;
    let kind = if serve { "serve" } else { "rally" };
    let json = json::to_string(&Request {
        id,
        kind: kind.into(),
        state: situation,
    })
    .expect("finite situation");
    brain.asking = Some(Ask {
        id,
        serve,
        asked: w.tick(),
        committed: false,
        json,
    });
    brain.ready = None;
    brain.answer = None;
    brain.stats.asked += 1;
    brain.state = "thinking".into();
    brain.status = "Jev thinking…".into();
    w.log(format_args!("jev ask {id} {kind}"));
}

/// Read the plan argument; apply the matching answer exactly once.
pub fn receive(w: &World, plan: &str) {
    if plan.is_empty() {
        return;
    }
    let Ok(answer) = json::from_str::<Answer>(plan) else {
        return;
    };
    if answer.id == w.resource::<Brain>().seen {
        return;
    }
    let mut brain = w.resource_mut::<Brain>();
    brain.seen = answer.id;
    let Some(ask) = brain.asking.clone().filter(|a| a.id == answer.id) else {
        w.log(format_args!("jev answer {} ignored: stale", answer.id));
        return;
    };
    brain.asking = None;
    let ms = ((w.tick() - ask.asked) * 1000 / w.hz() as u64) as u32;
    brain.stats.latency_ms.push(ms);
    if ask.committed {
        brain.stats.late_arrivals += 1;
        w.log(format_args!(
            "jev answer {} arrived {ms} ms after asking, past its commit point: ignored",
            ask.id
        ));
        return;
    }
    if !answer.ok {
        brain.stats.errors += 1;
        brain.state = "unavailable".into();
        brain.status = format!("Jev unavailable — {}", answer.error);
        w.log(format_args!(
            "jev answer {} failed after {ms} ms: {}",
            ask.id, answer.error
        ));
        return;
    }
    drop(brain);
    let intent = sample(w, &answer, ask.serve);
    let mut brain = w.resource_mut::<Brain>();
    // The plan stays secret until the far player swings (commit shows it).
    brain.status = format!("plan ready in {:.2} s", ms as f32 / 1000.0);
    brain.state = "ready".into();
    w.log(format_args!(
        "jev answer {} in {ms} ms: {} {} → {} aggression {:.2} approach {}",
        ask.id,
        if ask.serve { "serve" } else { "shot" },
        intent.shot,
        intent.target,
        intent.aggression,
        intent.approach
    ));
    brain.answer = Some(answer);
    brain.ready = Some(intent);
}

/// Draw from a probability table in key order with the world's RNG.
fn draw(w: &World, choice: &Choice, allowed: &[&str]) -> (String, f32) {
    let total: f64 = choice
        .p
        .iter()
        .filter(|(k, _)| allowed.contains(&k.as_str()))
        .map(|(_, p)| p.max(0.0))
        .sum();
    if total > 0.0 {
        let mut u = w.rand(0.0..total);
        for (key, p) in choice
            .p
            .iter()
            .filter(|(k, _)| allowed.contains(&k.as_str()))
        {
            if u < p.max(0.0) {
                return (key.clone(), (p / total) as f32);
            }
            u -= p.max(0.0);
        }
    }
    let fallback = if allowed.contains(&choice.choice.as_str()) {
        choice.choice.clone()
    } else {
        allowed[0].into()
    };
    (fallback, 1.0)
}

pub const SHOTS: [&str; 5] = ["drive", "flat", "slice", "drop_shot", "lob"];
pub const TARGETS: [&str; 4] = ["forehand", "backhand", "body", "open_court"];
pub const SERVES: [&str; 3] = ["flat", "kick", "slice"];
pub const SERVE_TARGETS: [&str; 3] = ["wide", "body", "t"];

fn sample(w: &World, a: &Answer, serve: bool) -> Intent {
    let (shots, targets): (&[&str], &[&str]) = if serve {
        (&SERVES, &SERVE_TARGETS)
    } else {
        (&SHOTS, &TARGETS)
    };
    let (shot, p) = draw(w, &a.shot, shots);
    let (target, _) = draw(w, &a.target, targets);
    let levels = a.aggression.levels.max(2) as f64;
    let aggression = (a.aggression.score / (levels - 1.0)).clamp(0.0, 1.0) as f32;
    let approach = w.chance(a.approach.clamp(0.0, 1.0) as f32);
    Intent {
        id: a.id,
        serve,
        shot,
        target,
        aggression,
        approach,
        p,
        source: Source::Jev,
    }
}

/// The far player must swing now: Jev's plan if it is here, else the fallback.
pub fn commit(w: &World, serve: bool, fallback: Intent) -> Intent {
    let mut brain = w.resource_mut::<Brain>();
    if let Some(intent) = brain.ready.take().filter(|i| i.serve == serve) {
        brain.stats.on_time += 1;
        brain.line = describe("Jev", &intent);
        brain.status.clear();
        brain.last = Some(intent.clone());
        brain.state = "idle".into();
        return intent;
    }
    let mut intent = fallback;
    if brain.offline {
        intent.source = Source::Offline;
        brain.line = describe("Offline", &intent);
    } else if let Some(ask) = brain.asking.as_mut().filter(|a| !a.committed) {
        ask.committed = true;
        let (id, waited) = (ask.id, (w.tick() - ask.asked) * 1000 / w.hz() as u64);
        intent.id = id;
        brain.stats.late += 1;
        intent.source = Source::Late;
        brain.state = "late".into();
        brain.status = "late".into();
        brain.line = describe("Jev late → fallback", &intent);
        w.log(format_args!(
            "jev decision {id} late after {waited} ms: fallback {} → {}",
            intent.shot, intent.target
        ));
    } else {
        intent.source = Source::Unavailable;
        brain.line = describe("Fallback", &intent);
        w.log(format_args!(
            "jev unavailable: fallback {} → {}",
            intent.shot, intent.target
        ));
    }
    brain.last = Some(intent.clone());
    intent
}

pub fn describe(who: &str, i: &Intent) -> String {
    let shot = match (i.serve, i.shot.as_str()) {
        (true, "flat") => "flat serve",
        (true, "kick") => "kick serve",
        (true, _) => "slice serve",
        (false, "drop_shot") => "drop shot",
        (false, "flat") => "flat drive",
        (false, s) => s,
    };
    let target = match i.target.as_str() {
        "forehand" => "your forehand",
        "backhand" => "your backhand",
        "body" => "your body",
        "open_court" => "the open court",
        "wide" => "wide",
        "t" => "the T",
        _ => "the middle",
    };
    let net = if i.approach { " · to the net" } else { "" };
    if i.source == Source::Jev {
        format!("{who}: {shot} → {target} ({:.0}%){net}", i.p * 100.0)
    } else {
        format!("{who}: {shot} → {target}{net}")
    }
}

/// The deterministic policy used when Jev is late, unavailable or offline:
/// attack the weaker side, lob a net rusher, stay patient when stretched.
pub fn fallback(
    w: &World,
    serve: bool,
    second_serve: bool,
    stretched: bool,
    opponent_at_net: bool,
) -> Intent {
    let weak = w.resource::<Scouting>().weaker();
    let roll = w.rand(0.0f32..1.0);
    if serve {
        let (shot, aggression) = if second_serve {
            ("kick", 0.25)
        } else if roll < 0.5 {
            ("flat", 0.55)
        } else {
            ("slice", 0.5)
        };
        let target = SERVE_TARGETS[(w.rand(0.0f32..3.0) as usize).min(2)];
        return Intent {
            serve,
            shot: shot.into(),
            target: target.into(),
            aggression,
            p: 1.0,
            ..Intent::default()
        };
    }
    let (shot, target, aggression) = if opponent_at_net && roll < 0.5 {
        ("lob", "open_court", 0.35)
    } else if opponent_at_net {
        ("drive", "body", 0.5)
    } else if stretched {
        ("slice", weak, 0.2)
    } else if roll < 0.6 {
        ("drive", weak, 0.45)
    } else if roll < 0.85 {
        ("drive", "open_court", 0.5)
    } else {
        ("flat", weak, 0.6)
    };
    Intent {
        serve,
        shot: shot.into(),
        target: target.into(),
        aggression,
        p: 1.0,
        ..Intent::default()
    }
}

/// Round to a decimetre for Jev's eyes.
pub fn dm(x: f32) -> f32 {
    math::round(x * 10.0) / 10.0
}
