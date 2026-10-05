//! Tennis's rules are a Contract program, `../rules/rules.contract`, run inside
//! the tick (LLP 1046.009 §3.3): scoring, the serve, faults, lets and the
//! point's flow. Its slots live in the world's saved [`Rules`] resource; this
//! module raises its events and reads it as the kernels need.
use exact_game::{Data, Resource, World, WorldId};
use exact_game_rules::{Program, Rules};
use std::rc::Rc;

/// The compiled program (logic/build.rs compiles it at build).
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rules.plan"));

thread_local! {
    static COMPILED: Rc<Program> = Rc::new(Program::new(PLAN).expect("the compiled rules program"));
    /// A program posted to a running world (`rules:<hex>`), by world id: the
    /// development reload. A restored world, or a fresh module, runs `PLAN`.
    static POSTED: std::cell::RefCell<Vec<(WorldId, Rc<Program>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The program this world runs.
pub fn program(w: &World) -> Rc<Program> {
    let id = w.id();
    POSTED
        .with(|p| {
            p.borrow()
                .iter()
                .find(|(w, _)| *w == id)
                .map(|(_, p)| p.clone())
        })
        .unwrap_or_else(|| COMPILED.with(Rc::clone))
}

/// Run a new program in this world from now on, carrying its slots by name.
pub fn install(w: &World, bytes: &[u8]) -> Result<exact_game_rules::Adopted, String> {
    let next = Rc::new(Program::new(bytes)?);
    let (rules, adopted) = next.adopt(&w.resource::<Rules>())?;
    *w.resource_mut::<Rules>() = rules;
    let id = w.id();
    POSTED.with(|p| {
        let mut p = p.borrow_mut();
        p.retain(|(w, _)| *w != id);
        p.push((id, next));
    });
    Ok(adopted)
}

/// A posted program's bytes, from the hex a `rules:` message carries.
pub fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return Err("rules: a posted program is hex".into());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| format!("rules: {e}")))
        .collect()
}

/// Near is you (positive z, facing -z); Far is Jev (negative z, facing +z).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Side {
    #[default]
    Near,
    Far,
}
impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Near => Side::Far,
            Side::Far => Side::Near,
        }
    }
    /// The sign of z on this side's half of the court.
    pub fn half(self) -> f32 {
        match self {
            Side::Near => 1.0,
            Side::Far => -1.0,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Side::Near => "You",
            Side::Far => "Jev",
        }
    }
    pub fn of_z(z: f32) -> Side {
        if z >= 0.0 {
            Side::Near
        } else {
            Side::Far
        }
    }
    fn of(near: bool) -> Side {
        if near {
            Side::Near
        } else {
            Side::Far
        }
    }
}

/// Where the point is: the program's `phase`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// The server has the ball in hand.
    #[default]
    Ready,
    /// The ball is in the air above the server.
    Toss,
    /// The ball is live.
    Rally,
    /// The point is decided; the call shows until the next point.
    Dead,
    /// Someone has won the match.
    Over,
}

/// The tick the current phase began: the kernels' clock for the toss, the
/// serve's timing and the dead time. The rules have no clock of their own.
#[derive(Clone, Debug, Default, Resource)]
pub struct PhaseClock {
    pub since: u64,
}

/// The rules as the kernels read them, one tick at a time.
#[derive(Clone, Debug, Default)]
pub struct Match {
    pub games: [u32; 2],
    pub points: [u32; 2],
    pub server: Side,
    /// The first serve of this point has faulted.
    pub fault: bool,
    pub phase: Phase,
    /// Tick the phase began.
    pub since: u64,
    /// The umpire's last call, shown on the HUD.
    pub call: String,
    pub winner: Option<Side>,
    /// Strokes in the current rally, serve included.
    pub rally: u32,
    pub points_played: u32,
    pub longest_rally: u32,
    /// The ball in play: who hit it last, whether it is a serve, its bounces
    /// since, and whether it clipped the tape.
    pub hitter: Side,
    pub serve: bool,
    pub bounces: u32,
    pub cord: bool,
    deuce: bool,
    labels: (String, String),
    pressure: String,
    first_to: u32,
    pub prompt: String,
    pub dead_seconds: f32,
    /// The last point decided: to whom, and how (`Out`, `Ace` …).
    pub point_to: Side,
    pub point_call: String,
}

impl Match {
    pub fn of(w: &World) -> Match {
        let r = w.resource::<Rules>();
        let r = program(w).derived(&r).unwrap_or_else(|e| panic!("{e}"));
        Match::read(&r, w.resource::<PhaseClock>().since)
    }

    pub fn read(r: &Rules, since: u64) -> Match {
        Match {
            games: [r.count("nearGames"), r.count("farGames")],
            points: [r.count("nearPoints"), r.count("farPoints")],
            server: Side::of(r.flag("nearServes")),
            fault: r.flag("fault"),
            phase: match r.text("phase") {
                "ready" => Phase::Ready,
                "toss" => Phase::Toss,
                "rally" => Phase::Rally,
                "dead" => Phase::Dead,
                "over" => Phase::Over,
                other => panic!("rules: unknown phase {other:?}"),
            },
            since,
            call: r.text("call").into(),
            winner: match r.text("winner") {
                "near" => Some(Side::Near),
                "far" => Some(Side::Far),
                _ => None,
            },
            rally: r.count("rally"),
            points_played: r.count("played"),
            longest_rally: r.count("longest"),
            hitter: Side::of(r.flag("nearHit")),
            serve: r.flag("serve"),
            bounces: r.count("bounces"),
            cord: r.flag("cord"),
            deuce: r.flag("deuceCourt"),
            labels: (r.text("nearLabel").into(), r.text("farLabel").into()),
            pressure: r.text("pressure").into(),
            first_to: r.count("firstTo"),
            prompt: r.text("prompt").into(),
            dead_seconds: r.num("deadSeconds") as f32,
            point_to: Side::of(r.text("pointTo") == "near"),
            point_call: r.text("pointCall").into(),
        }
    }

    /// Points alternate courts: deuce (the receiver's right) on even totals.
    pub fn deuce_court(&self) -> bool {
        self.deuce
    }
    /// Point labels for (you, Jev).
    pub fn labels(&self) -> (String, String) {
        self.labels.clone()
    }
    /// Break point, set point or match point, if any.
    pub fn pressure(&self) -> &str {
        &self.pressure
    }
    /// Games to win the match.
    pub fn first_to(&self) -> u32 {
        self.first_to
    }
}

/// The rules before the first point.
pub fn boot(w: &mut World) {
    let rules = program(w).boot().expect("the rules boot");
    w.insert_resource(rules);
    w.insert_resource(PhaseClock { since: w.tick() });
}

/// Settle a restored world's derives, which saves leave out.
pub fn settle(w: &World) {
    if w.resource::<Rules>().derived.is_empty() {
        program(w)
            .settle(&mut w.resource_mut::<Rules>())
            .unwrap_or_else(|e| panic!("{e}"));
    }
}

/// The rules' slots and derives, published under the program's own names:
/// the HUD (app.contract's `shape Hud`) reads the ones it shows as
/// rules.contract names them, and no Rust copy stands between.
pub fn publish(w: &World) {
    let r = w.resource::<Rules>();
    let r = program(w).derived(&r).unwrap_or_else(|e| panic!("{e}"));
    for (name, value) in r.slots.iter().chain(r.derived.iter()) {
        w.publish(name, value.clone());
    }
}

/// What one event did, for the world's side effects.
pub struct Raised {
    pub before: Match,
    pub after: Match,
}
impl Raised {
    /// The point it decided: to whom, and the umpire's word for how.
    pub fn point(&self) -> Option<(Side, &str)> {
        (self.after.points_played > self.before.points_played)
            .then_some((self.after.point_to, self.after.point_call.as_str()))
    }
}

/// Raise one of the program's events. The phase clock restarts when the
/// phase changes; a refused event changes nothing and panics, since the
/// kernels raise only declared events.
pub fn raise(w: &World, event: &str) -> Raised {
    let program = program(w);
    let since = w.resource::<PhaseClock>().since;
    let before = Match::read(&w.resource::<Rules>(), since);
    program
        .act(&mut w.resource_mut::<Rules>(), event)
        .unwrap_or_else(|e| panic!("{e}"));
    let mut after = Match::read(&w.resource::<Rules>(), since);
    if after.phase != before.phase {
        w.resource_mut::<PhaseClock>().since = w.tick();
        after.since = w.tick();
    }
    Raised { before, after }
}
