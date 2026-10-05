//! Tennis against Jev: forehands on J, backhands on K, first to four games.
//! The world owns the court, the ball and both players' bodies; the rules are
//! a Contract program it runs (rules.rs, ../rules/rules.contract). Jev
//! (through Contract's `jev` resource) only chooses the far player's intent.
pub mod ball;
pub mod brain;
mod court;
pub mod players;
pub mod rules;

use ball::{Flight, Touch, HALF_LENGTH, SERVICE_LINE};
use brain::{Brain, Intent, Scouting, Situation};
use exact_game::audio::{self, AudioListener};
use exact_game::*;
use players::{Hand, Meet, Player, CONTACT};
use rules::{raise, Match, Phase, Side};

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    /// Play the deterministic fallback policy without asking Jev.
    pub offline: bool,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
    /// Jev's latest answer from the `jev` resource, applied only when its id
    /// matches the open question (brain::receive).
    #[live]
    pub plan: String,
}

/// The ball's physics. Who hit it, its bounces and whether it is a serve are
/// the rules' (`Match::hitter`, `bounces`, `serve`, `cord`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Ball {
    pub flight: Flight,
    /// Moving under physics (tossed, live or dribbling after the point).
    pub live: bool,
    /// Predicted first bounce of a ball coming at you.
    pub mark: Option<Vec3>,
}

/// What the HUD reads through `exactSurface("world")` besides the rules'
/// own slots and derives (`rules::publish`): Jev's side.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Hud {
    pub jev: String,
    pub jev_state: String,
    pub jev_status: String,
    /// The open question for Jev, or "" — the `jev` resource's argument.
    pub ask: String,
    pub asked: u32,
    pub on_time: u32,
    pub late: u32,
}

const TOSS_UP: f32 = 5.6;

pub struct Tennis;
impl Game for Tennis {
    const ID: &'static str = "tennis";
    const HZ: u32 = 120;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("forehand", &["KeyJ"])
            .button("backhand", &["KeyK"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        court::court(w);
        court::sounds(w);
        rules::boot(w);
        w.insert_resource(Brain {
            offline: args.offline,
            state: if args.offline {
                "offline".into()
            } else {
                "idle".into()
            },
            ..Brain::default()
        });
        w.insert_resource(Scouting::default());
        for side in [Side::Near, Side::Far] {
            court::player(w, side);
        }
        w.spawn_named(
            "ball",
            (
                Transform::default(),
                Mesh::sphere(0.055),
                Material::rgb(0.72, 0.9, 0.06)
                    .emissive(0.25, 0.32, 0.01)
                    .rough(0.8),
                Ball::default(),
            ),
        );
        w.spawn_named(
            "marker",
            (
                Transform::at(0.0, -1.0, 0.0).with_scale(Vec3::new(1.0, 1.0, 1.0)),
                Mesh::cylinder(0.2, 0.004),
                Material::glow([0.9, 0.85, 0.3]),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(0.0, 9.5, 24.5).looking_at(Vec3::new(0.0, 0.0, -1.0), Vec3::Y),
                Camera {
                    fov_y_degrees: 40.0,
                    ..Camera::default()
                },
                AudioListener,
            ),
        );
        next_point(w);
        publish(w);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        rules::settle(w);
        // A development reload: a new rules program, carried by slot name.
        for message in input.messages() {
            if let Some(hex) = message.strip_prefix("rules:") {
                match rules::decode_hex(hex).and_then(|bytes| rules::install(w, &bytes)) {
                    Ok(adopted) => w.log(format_args!("rules: new program ({adopted:?})")),
                    Err(e) => w.log(format_args!("rules: kept the running program: {e}")),
                }
            }
        }
        brain::receive(w, &args.plan);
        near(w, input);
        far(w);
        fly(w);
        let m = Match::of(w);
        if m.phase == Phase::Dead && (w.tick() - m.since) as f32 * w.dt() >= m.dead_seconds {
            next_point(w);
        }
        animate(w);
        follow_camera(w);
        publish(w);
        audio::step(w);
    }
}

fn name(side: Side) -> &'static str {
    if side == Side::Near {
        "near"
    } else {
        "far"
    }
}
fn load(w: &World, side: Side) -> (Player, Vec3) {
    (
        *w.require::<Player>(name(side)),
        w.require::<Transform>(name(side)).position,
    )
}
fn store(w: &World, p: Player, at: Vec3) {
    *w.require_mut::<Player>(name(p.side)) = p;
    w.require_mut::<Transform>(name(p.side)).position = at;
}
fn ball(w: &World) -> Ball {
    *w.require::<Ball>("ball")
}
fn set_ball(w: &World, b: Ball) {
    *w.require_mut::<Ball>("ball") = b;
    w.require_mut::<Transform>("ball").position = b.flight.pos;
}
fn seconds(w: &World, since: u64) -> f32 {
    (w.tick() - since) as f32 * w.dt()
}
fn sound(w: &World, name: &str, at: Vec3, gain: f32) {
    w.play(name).at_point(at).gain(gain).start();
}

/// Where the server's hand holds the ball.
fn hand(side: Side, at: Vec3) -> Vec3 {
    Vec3::new(at.x + 0.3 * side.half(), 1.1, at.z - 0.35 * side.half())
}

/// Reset for the next point (or the second serve), and ask Jev for its serve.
fn next_point(w: &mut World) {
    if Match::of(w).phase == Phase::Dead && raise(w, "next").after.phase == Phase::Over {
        return;
    }
    let m = Match::of(w);
    let deuce = m.deuce_court();
    let server = m.server;
    let receiver = server.other();
    // The server stands right of centre in the deuce court; the receiver diagonally opposite.
    let serve_at = Vec3::new(
        server.half() * if deuce { 0.9 } else { -0.9 },
        0.0,
        server.half() * (HALF_LENGTH + 0.25),
    );
    let receive_at = Vec3::new(
        receiver.half() * if deuce { 2.6 } else { -2.6 },
        0.0,
        receiver.half() * (HALF_LENGTH + 0.7),
    );
    for (side, at) in [(server, serve_at), (receiver, receive_at)] {
        let e = w.named(name(side)).unwrap();
        let rotation = w.require::<Transform>(e).rotation;
        w.teleport(
            e,
            Transform {
                position: at,
                rotation,
                scale: Vec3::ONE,
            },
        );
        *w.require_mut::<Player>(e) = Player {
            side,
            goal: at,
            ..Player::default()
        };
    }
    let e = w.named("ball").unwrap();
    let held = hand(server, serve_at);
    w.teleport(e, Transform::at(held.x, held.y, held.z));
    *w.require_mut::<Ball>(e) = Ball {
        flight: Flight {
            pos: held,
            ..Flight::default()
        },
        ..Ball::default()
    };
    if server == Side::Far {
        let second = m.fault;
        let incoming = format!(
            "you are about to hit your {} serve from the {} court; the receiver waits {}",
            if second { "second" } else { "first" },
            if deuce { "deuce" } else { "ad" },
            place(receive_at, receiver)
        );
        brain::ask(
            w,
            true,
            situation(
                w,
                if second {
                    "your second serve"
                } else {
                    "your first serve"
                },
                incoming,
            ),
        );
    }
}

/// Describe a spot from that player's own point of view.
fn place(at: Vec3, side: Side) -> String {
    let lateral = at.x * side.half();
    let depth = at.z * side.half() - HALF_LENGTH;
    let across = if lateral.abs() < 0.4 {
        "in the middle".to_string()
    } else {
        format!(
            "{:.1} m to the {} side of centre",
            lateral.abs(),
            if lateral > 0.0 {
                "forehand"
            } else {
                "backhand"
            }
        )
    };
    let deep = if depth > 0.3 {
        format!("{depth:.1} m behind the baseline")
    } else if depth > -1.5 {
        "on the baseline".into()
    } else if depth > -(HALF_LENGTH - SERVICE_LINE) {
        "in the forecourt".into()
    } else {
        "at the net".into()
    };
    format!("{across}, {deep}")
}

fn situation(w: &World, decide: &str, incoming: String) -> Situation {
    let m = Match::of(w);
    let near_at = w.require::<Transform>("near").position;
    let far_at = w.require::<Transform>("far").position;
    let scouting = w.resource::<Scouting>();
    Situation {
        you_are: "Jev, the far-side player in a singles tennis match against a human".into(),
        decide: decide.into(),
        score: brain::score_line(&m),
        pressure: m.pressure().into(),
        incoming,
        you: place(far_at, Side::Far),
        opponent: place(near_at, Side::Near),
        opponent_x_m: brain::dm(near_at.x),
        opponent_depth_m: brain::dm(near_at.z - HALF_LENGTH),
        rally_shots: m.rally,
        tendencies: brain::tendencies(&scouting),
        recent_points: scouting.recent.clone(),
    }
}

/// Your side: run, toss, swing, and meet the ball on the contact tick.
fn near(w: &mut World, input: &Input) {
    let (mut p, mut at) = load(w, Side::Near);
    let m = Match::of(w);
    let tick = w.tick();
    let serving = m.server == Side::Near && matches!(m.phase, Phase::Ready | Phase::Toss);
    let stroke = if input.pressed("forehand") {
        Some(Hand::Forehand)
    } else if input.pressed("backhand") {
        Some(Hand::Backhand)
    } else {
        None
    };
    // Movement: a server may only shuffle along the baseline before the toss.
    let mut want = input.stick_xz("move");
    if serving {
        want = if m.phase == Phase::Ready {
            Vec3::new(want.x, 0.0, 0.0)
        } else {
            Vec3::ZERO
        };
    }
    let speed = if p.swinging(tick) {
        players::NEAR_SPEED * 0.35
    } else {
        players::NEAR_SPEED
    };
    players::run(&mut p, &mut at, want, speed, w.dt());
    if serving {
        let s = if m.deuce_court() { 1.0 } else { -1.0 };
        at.x = (at.x * s).clamp(0.2, 4.0) * s;
    }
    match (stroke, m.phase) {
        (Some(_), Phase::Ready) if serving => {
            store(w, p, at);
            toss(w, Side::Near);
            let incoming = format!(
                "the opponent is tossing for a {} serve to your {} court; you stand {}",
                if m.fault { "second" } else { "first" },
                if m.deuce_court() { "deuce" } else { "ad" },
                place(w.require::<Transform>("far").position, Side::Far)
            );
            brain::ask(w, false, situation(w, "your return of serve", incoming));
            return;
        }
        (Some(_), Phase::Toss) if serving && p.swing.is_none_or(|s| s.done) => {
            p.swing = Some(players::Swing {
                hand: Hand::Serve,
                start: tick,
                done: false,
            });
        }
        (Some(hand), _) if !serving && p.swing.is_none_or(|s| tick >= s.start + CONTACT + 8) => {
            p.swing = Some(players::Swing {
                hand,
                start: tick,
                done: false,
            });
        }
        _ => {}
    }
    store(w, p, at);
    let Some(mut swing) = p.swing.filter(|s| !s.done && tick >= s.start + CONTACT) else {
        return;
    };
    swing.done = true;
    p.swing = Some(swing);
    store(w, p, at);
    let b = ball(w);
    let m = Match::of(w);
    if swing.hand == Hand::Serve {
        if m.phase == Phase::Toss && b.flight.pos.y >= 1.6 {
            let apex = TOSS_UP / 9.81;
            let quality = (1.0 - (seconds(w, m.since) - apex).abs() / 0.4).clamp(0.0, 1.0);
            let shot = players::near_serve(m.deuce_court(), m.fault, quality, input.stick("move"));
            if !m.fault {
                w.resource_mut::<Scouting>().first_serves += 1;
            }
            strike(w, Side::Near, b.flight.pos, shot, true);
        } else {
            sound(w, "swish", at, 1.0);
        }
        return;
    }
    let returnable = m.phase == Phase::Rally
        && m.hitter == Side::Far
        && m.bounces < 2
        && (!m.serve || m.bounces == 1);
    let rel = players::relative(&p, at, b.flight.pos);
    let meet = if returnable {
        players::meet(swing.hand, rel)
    } else {
        Meet::Whiff
    };
    if meet == Meet::Whiff {
        sound(w, "swish", at, 1.0);
        if returnable {
            w.log(format_args!(
                "near whiff {:?}: ball {:.2} m right, {:.2} m in front, {:.2} m high",
                swing.hand, rel.x, rel.y, rel.z
            ));
        }
        return;
    }
    let shot = players::near_shot(swing.hand, meet, b.flight.pos, input.stick("move"));
    {
        let mut s = w.resource_mut::<Scouting>();
        if swing.hand == Hand::Forehand {
            s.forehands += 1;
        } else {
            s.backhands += 1;
        }
        let x = b.flight.pos.x;
        if x.abs() > 1.0 && shot.target.x * x < 0.0 {
            s.crosscourt += 1;
        } else if x.abs() > 1.0 && shot.target.x.abs() > 1.5 {
            s.down_the_line += 1;
        }
        if at.z < SERVICE_LINE {
            s.net_approaches += 1;
        }
    }
    w.log(format_args!(
        "near {:?} {}",
        swing.hand,
        match meet {
            Meet::Clean(e) => format!("clean timing {e:+.2}"),
            _ => "mishit".into(),
        }
    ));
    strike(w, Side::Near, b.flight.pos, shot, false);
}

/// The far player has just struck: ask Jev now for its next shot, so the
/// question has both balls' flight time (~2.4 s) before its commit point.
/// Asking when you strike leaves ~1.1 s; the live check measured 19% late.
/// @ref game/diaries/003-tennis.md "Design: Jev plans, the engine executes"
fn ask_next(w: &World, intent: &Intent) {
    let flight = ball(w).flight;
    let hint = *w.require::<Player>("near");
    let aim = match intent.target.as_str() {
        "forehand" | "backhand" | "body" => format!("their {}", intent.target),
        "open_court" => "the open court".into(),
        "t" => "the T".into(),
        other => other.into(),
    };
    let kind = if intent.serve { " serve" } else { "" };
    let shot = format!("a {}{kind} aimed at {aim}", intent.shot.replace('_', " "));
    let incoming = match ball::landing(flight, w.dt(), 4.0) {
        Some(p) if ball::in_court(p) && p.z > 0.0 => format!(
            "you just hit {shot}; it bounces {:.1} m inside the opponent's baseline and they will return it from {}",
            HALF_LENGTH - p.z,
            if hint.contact > 0 { place(hint.goal, Side::Near) } else { "out of reach".into() }
        ),
        Some(_) => format!("you just hit {shot}, but it looks like it is going out"),
        None => format!("you just hit {shot}, but it looks like it will not clear the net"),
    };
    brain::ask(
        w,
        false,
        situation(
            w,
            "your next shot, after the opponent returns this one",
            incoming,
        ),
    );
}

/// Put the ball in the air above the server.
fn toss(w: &World, side: Side) {
    let at = w.require::<Transform>(name(side)).position;
    let pos = hand(side, at);
    set_ball(
        w,
        Ball {
            flight: Flight {
                pos,
                vel: Vec3::new(0.0, TOSS_UP, 0.0),
                spin: Vec3::ZERO,
            },
            live: true,
            ..Ball::default()
        },
    );
    debug_assert_eq!(Match::of(w).server, side, "only the server tosses");
    raise(w, "toss");
}

/// Launch the ball with scatter drawn from the world's RNG.
fn strike(w: &World, side: Side, from: Vec3, shot: players::Shot, serve: bool) {
    let (u, v) = (w.rand(0.0f32..1.0), w.rand(0.0f32..1.0));
    let target = shot.target + ball::scatter(shot.error, u, v);
    let mut flight = ball::launch(from, target, shot.stroke, w.dt());
    // Launch-angle error: the aim was right, the racket face was not quite.
    flight.vel.y += flight.vel.length() * shot.wobble * w.rand(-1.0f32..1.0);
    let mark = if side == Side::Far {
        ball::landing(flight, w.dt(), 4.0)
    } else {
        None
    };
    set_ball(
        w,
        Ball {
            flight,
            live: true,
            mark,
        },
    );
    let (mut hitter, at) = load(w, side);
    hitter.contact = 0;
    store(w, hitter, at);
    receive_plan(w, side.other(), flight, serve);
    let raised = raise(w, if serve { "serveBall" } else { "hit" });
    debug_assert_eq!(raised.after.hitter, side, "the rules agree who struck");
    sound(w, "hit", from, 0.9);
}

/// Plan where the receiver meets a ball just struck toward it. The far player
/// runs there; for you it is only a hint an agent can read (`state world:near`).
fn receive_plan(w: &World, side: Side, flight: Flight, serve: bool) {
    let (mut p, at) = load(w, side);
    let at_net = at.z * side.half() < SERVICE_LINE + 0.5;
    match players::plan(side, flight, at, at_net, serve, w.dt()) {
        Some(plan) => {
            p.goal = plan.stand;
            p.contact = w.tick() + plan.ticks;
            p.react = w.tick() + (players::REACTION / w.dt()) as u64;
            p.hand = plan.hand;
            p.stretch = plan.stretch;
        }
        None => p.contact = 0,
    }
    store(w, p, at);
}

/// Jev's side: serve, run to the planned contact, commit, swing, recover.
fn far(w: &mut World) {
    let (mut p, mut at) = load(w, Side::Far);
    let m = Match::of(w);
    let tick = w.tick();
    let dt = w.dt();
    if m.server == Side::Far && m.phase == Phase::Ready {
        let waited = seconds(w, m.since);
        let ready = w.resource::<Brain>().ready.is_some();
        if (ready && waited >= 0.6)
            || waited >= 1.2
            || w.resource::<Brain>().offline && waited >= 0.8
        {
            let fallback = brain::fallback(w, true, m.fault, false, false);
            brain::commit(w, true, fallback);
            toss(w, Side::Far);
        }
        return;
    }
    if m.server == Side::Far && m.phase == Phase::Toss {
        let apex = (TOSS_UP / 9.81 / dt) as u64;
        if tick == m.since + apex - CONTACT {
            p.swing = Some(players::Swing {
                hand: Hand::Serve,
                start: tick,
                done: false,
            });
        }
        if tick == m.since + apex {
            let intent = w.resource::<Brain>().last.clone().unwrap_or_default();
            let receiver = w.require::<Transform>("near").position;
            let shot = players::far_serve(&intent, m.deuce_court(), m.fault, receiver);
            if let Some(s) = p.swing.as_mut() {
                s.done = true;
            }
            store(w, p, at);
            strike(w, Side::Far, ball(w).flight.pos, shot, true);
            ask_next(w, &intent);
            return;
        }
        store(w, p, at);
        return;
    }
    // Commit on the swing's first tick (or at once, for a ball faster than a swing).
    if p.contact > 0 && tick + CONTACT >= p.contact && p.swing.is_none_or(|s| s.done) {
        p.swing = Some(players::Swing {
            hand: p.hand,
            start: tick,
            done: false,
        });
        let near_at = w.require::<Transform>("near").position;
        let at_net = near_at.z < SERVICE_LINE;
        let fallback = brain::fallback(w, false, false, p.stretch, at_net);
        brain::commit(w, false, fallback);
    }
    if p.contact > 0 && tick >= p.contact {
        p.contact = 0;
        if let Some(s) = p.swing.as_mut() {
            s.done = true;
        }
        let b = ball(w);
        let rel = players::relative(&p, at, b.flight.pos);
        let lateral_ok =
            rel.x.abs() <= 1.9 && rel.x * if p.hand == Hand::Forehand { 1.0 } else { -1.0 } >= -0.2;
        let reach = lateral_ok && (-1.0..=1.6).contains(&rel.y) && rel.z <= 2.6;
        let lunge = !reach && Vec2::new(rel.x, rel.y).length() <= 2.4 && rel.z <= 2.6;
        let live = m.phase == Phase::Rally && m.hitter == Side::Near && m.bounces < 2;
        if live && (reach || lunge) {
            let intent = w.resource::<Brain>().last.clone().unwrap_or_default();
            let near_at = w.require::<Transform>("near").position;
            let off = (rel.x.abs() - 0.8).abs() + 0.5 * (rel.y - 0.35).abs();
            let pressure = players::pressure(b.flight, off);
            let shot = players::far_shot(&intent, near_at, p.stretch || lunge, pressure);
            store(w, p, at);
            strike(w, Side::Far, b.flight.pos, shot, false);
            ask_next(w, &intent);
            // Recover to the baseline, or follow the shot in when Jev said so.
            p.goal = if intent.approach {
                Vec3::new(shot.target.x * 0.3, 0.0, -(SERVICE_LINE - 2.8))
            } else {
                Vec3::new(-shot.target.x * 0.15, 0.0, -(HALF_LENGTH + 0.8))
            };
            w.log(format_args!(
                "far {:?} {} → {} ({:?})",
                p.hand, intent.shot, intent.target, intent.source
            ));
        } else {
            sound(w, "swish", at, 0.8);
            if live {
                w.log(format_args!(
                    "far misses: {:.2} m right, {:.2} m forward, {:.2} m high",
                    rel.x, rel.y, rel.z
                ));
            }
            p.goal = Vec3::new(0.0, 0.0, -(HALF_LENGTH + 0.8));
        }
    }
    let speed = if p.swinging(tick) {
        players::FAR_SPEED * 0.4
    } else {
        players::FAR_SPEED
    };
    let goal = p.goal;
    if tick < p.react {
        players::run(&mut p, &mut at, Vec3::ZERO, speed, dt);
    } else {
        players::seek(&mut p, &mut at, goal, speed, dt);
    }
    store(w, p, at);
}

/// Move the ball and raise what it touches as the rules' events.
fn fly(w: &mut World) {
    let mut b = ball(w);
    let m = Match::of(w);
    if m.phase == Phase::Ready {
        let server = w.require::<Transform>(name(m.server)).position;
        b.flight.pos = hand(m.server, server);
        set_ball(w, b);
        return;
    }
    if !b.live {
        return;
    }
    let touch = b.flight.step(w.dt());
    set_ball(w, b);
    match touch {
        Touch::Bounce(at) => sound(w, "bounce", at, (b.flight.vel.y * 0.12).clamp(0.2, 1.0)),
        Touch::Net => sound(w, "net", b.flight.pos, 1.0),
        Touch::Cord => sound(w, "net", b.flight.pos, 0.5),
        Touch::Air => {}
    }
    if m.phase == Phase::Toss && b.flight.vel.y < 0.0 && b.flight.pos.y < 1.0 {
        // Nobody hit the toss: catch it and toss again.
        w.log("toss dropped: serve again");
        raise(w, "dropToss");
        return;
    }
    if m.phase != Phase::Rally {
        return;
    }
    // Where a bounce landed, as the rules ask: the hitter's own half, the
    // receiver's service box on this point's court, the court, or out.
    let receiver = m.hitter.other();
    let event = match touch {
        Touch::Net => "net",
        Touch::Cord => "clip",
        Touch::Bounce(at) if Side::of_z(at.z) == m.hitter => "landOwn",
        Touch::Bounce(at) if ball::in_box(at, receiver.half(), m.deuce_court()) => "landBox",
        Touch::Bounce(at) if ball::in_court(at) => "landCourt",
        Touch::Bounce(_) => "landOut",
        Touch::Air => return,
    };
    let raised = raise(w, event);
    called(w, &raised);
}

/// The world's side of what the rules decided: the log, sounds and what Jev
/// learns of your game.
fn called(w: &World, r: &rules::Raised) {
    let (b, a) = (&r.before, &r.after);
    if a.bounces == 1 && b.bounces == 0 && a.serve && a.hitter == Side::Near && !a.fault {
        w.resource_mut::<Scouting>().first_serves_in += 1;
    }
    if a.phase == Phase::Dead && a.call == "Let" {
        w.log("let: the serve clipped the tape");
    }
    if a.fault && !b.fault {
        w.log(format_args!("fault: {}'s first serve", b.server.name()));
    }
    if let Some((to, how)) = r.point() {
        point(w, b, to, how, &a.call);
    }
}

/// Remember how a point went and announce it.
fn point(w: &World, before: &Match, to: Side, call: &str, line: &str) {
    let hitter = before.hitter;
    {
        let mut s = w.resource_mut::<Scouting>();
        if call == "Double fault" && before.server == Side::Near {
            s.double_faults += 1;
        }
        // The near player's lost point, charged to the stroke that erred or,
        // for a ball that got past, to the side it passed on.
        if to == Side::Far && call != "Double fault" {
            let backhand = if hitter == Side::Near {
                w.require::<Player>("near")
                    .swing
                    .is_some_and(|s| s.hand == Hand::Backhand)
            } else {
                ball(w).flight.pos.x < w.require::<Transform>("near").position.x
            };
            if backhand {
                s.backhand_errors += 1;
            } else {
                s.forehand_errors += 1;
            }
        }
        let who = if hitter == Side::Near {
            "your"
        } else {
            "Jev's"
        };
        s.remember(format!("{} won the point: {call} ({who} shot)", to.name()));
    }
    w.log(format_args!("point {}: {line}", to.name()));
    let at = w.require::<Transform>("camera").position;
    sound(w, "point", at, 0.8);
    if call == "Winner" || call == "Ace" {
        sound(w, "crowd", Vec3::new(0.0, 3.0, 0.0), 1.0);
    }
}

/// Racket poses from each swing, the landing marker, and the ball in hand.
fn animate(w: &World) {
    let tick = w.tick();
    let m = Match::of(w);
    for side in [Side::Near, Side::Far] {
        let p = *w.require::<Player>(name(side));
        let tossing = m.phase == Phase::Toss && m.server == side;
        let (hand, t) = match p.swing {
            Some(s) if tick < s.start + players::SWING => {
                (Some(s.hand), (tick - s.start) as f32 * w.dt())
            }
            _ => (None, if tossing { seconds(w, m.since) } else { 0.0 }),
        };
        w.require_mut::<Transform>(format!("{}-arm", name(side)).as_str())
            .rotation = players::pose(hand, t, tossing);
    }
    let b = ball(w);
    let show = b
        .mark
        .filter(|_| m.phase == Phase::Rally && m.hitter == Side::Far && m.bounces == 0);
    let mut marker = w.require_mut::<Transform>("marker");
    marker.position = show.map_or(Vec3::new(0.0, -1.0, 0.0), |at| Vec3::new(at.x, 0.006, at.z));
}

/// Behind your baseline, raised, following you from side to side.
fn follow_camera(w: &World) {
    let x = w.require::<Transform>("near").position.x;
    let mut cam = w.require_mut::<Transform>("camera");
    let eased = math::ease(cam.position.x, x * 0.55, 0.35, w.dt());
    if eased != cam.position.x {
        *cam =
            Transform::at(eased, 9.5, 24.5).looking_at(Vec3::new(eased * 0.5, 0.0, -1.0), Vec3::Y);
    }
}

fn publish(w: &World) {
    rules::publish(w);
    let brain = w.resource::<Brain>();
    w.publish_record(&Hud {
        jev: brain.line.clone(),
        jev_state: brain.state.clone(),
        jev_status: brain.status.clone(),
        ask: brain
            .asking
            .as_ref()
            .map(|a| a.json.clone())
            .unwrap_or_default(),
        asked: brain.stats.asked,
        on_time: brain.stats.on_time,
        late: brain.stats.late,
    });
}

/// For tests and proofs: the intent the far player last executed.
pub fn last_intent(w: &World) -> Option<Intent> {
    w.resource::<Brain>().last.clone()
}
