//! The two players: running, the racket's swing as transforms, where a stroke
//! can meet the ball, what it does to it, and how the far player gets there.
use crate::ball::{self, Flight, Stroke, Touch, HALF_LENGTH};
use crate::brain::Intent;
use crate::rules::Side;
use exact_game::{math, Component, Data, Quat, Vec2, Vec3};

/// Ticks from pressing a stroke to the racket meeting the ball (0.158 s).
pub const CONTACT: u64 = 19;
/// A whole swing, recovery included (0.55 s).
pub const SWING: u64 = 66;
const ACCEL: f32 = 22.0;
/// Players plant their feet: stopping or turning is twice as quick as starting.
const BRAKE: f32 = 45.0;
pub const NEAR_SPEED: f32 = 6.0;
pub const FAR_SPEED: f32 = 5.6;
/// The far player's split step: it starts running this long after your contact.
pub const REACTION: f32 = 0.2;
/// Ideal contact: this far in front of the body, along the facing direction.
const SWEET: f32 = 0.35;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Hand {
    #[default]
    Forehand,
    Backhand,
    Serve,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub struct Swing {
    pub hand: Hand,
    pub start: u64,
    /// The contact tick was evaluated (hit, mishit or whiff).
    pub done: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Player {
    pub side: Side,
    pub vel: Vec3,
    pub swing: Option<Swing>,
    /// Far player: where it is running to.
    pub goal: Vec3,
    /// Far player: the planned contact tick (0 for none) and stroke.
    pub contact: u64,
    pub hand: Hand,
    /// The planned contact needs a lunge.
    pub stretch: bool,
    /// Far player: the split step before it reacts to your shot (a tick).
    pub react: u64,
}

impl Player {
    /// Local right in world x: +1 near (facing -z), -1 far (facing +z).
    pub fn right(&self) -> f32 {
        self.side.half()
    }
    pub fn swinging(&self, tick: u64) -> bool {
        self.swing.is_some_and(|s| tick < s.start + SWING)
    }
}

/// Accelerate toward `want` (a unit-clamped world direction) and move,
/// staying on your own half.
pub fn run(p: &mut Player, at: &mut Vec3, want: Vec3, speed: f32, dt: f32) {
    let target = want * speed;
    let rate = if target.dot(p.vel) <= 0.0 {
        BRAKE
    } else {
        ACCEL
    };
    p.vel += (target - p.vel).clamp_length_max(rate * dt);
    *at += p.vel * dt;
    let half = p.side.half();
    let (near, far) = (0.45, HALF_LENGTH + 4.5);
    let z = (at.z * half).clamp(near, far) * half;
    if z != at.z {
        at.z = z;
        p.vel.z = 0.0;
    }
    if at.x.abs() > 7.5 {
        at.x = at.x.clamp(-7.5, 7.5);
        p.vel.x = 0.0;
    }
}

/// Head for a point and stop on it without overshooting.
pub fn seek(p: &mut Player, at: &mut Vec3, goal: Vec3, speed: f32, dt: f32) {
    let delta = Vec3::new(goal.x - at.x, 0.0, goal.z - at.z);
    let distance = delta.length();
    // The speed that still brakes to zero at the goal.
    let brake = math::sqrt(2.0 * ACCEL * distance);
    let want = if distance < 0.02 {
        Vec3::ZERO
    } else {
        delta / distance * (brake.min(speed) / speed)
    };
    run(p, at, want, speed, dt);
}

/// Where the ball sits relative to a player: (right, forward, height).
pub fn relative(p: &Player, body: Vec3, ball: Vec3) -> Vec3 {
    let d = ball - body;
    Vec3::new(d.x * p.right(), -d.z * p.side.half(), ball.y)
}

/// The contact a swing makes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Meet {
    Whiff,
    /// Wrong side, jammed, or badly timed: a weak, wild ball.
    Mishit,
    /// Timing from -1 (late: pushed) to +1 (early: pulled).
    Clean(f32),
}

pub fn meet(hand: Hand, rel: Vec3) -> Meet {
    let (right, forward, height) = (rel.x, rel.y, rel.z);
    if right.abs() > 1.75 || !(-1.0..=2.2).contains(&forward) || !(0.05..=2.5).contains(&height) {
        return Meet::Whiff;
    }
    let side_ok = match hand {
        Hand::Forehand => right >= 0.15,
        Hand::Backhand => right <= -0.15,
        Hand::Serve => true,
    };
    let e = if forward > SWEET {
        (forward - SWEET) / 1.85
    } else {
        (forward - SWEET) / 1.35
    };
    if !side_ok || e.abs() > 0.8 {
        Meet::Mishit
    } else {
        Meet::Clean(e)
    }
}

/// A launch before scatter: where to land, how, and how far it may stray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub target: Vec2,
    pub stroke: Stroke,
    /// Radius of the landing scatter, metres.
    pub error: f32,
    /// Bound of the launch-angle error, radians: nets and long balls.
    pub wobble: f32,
}

/// The near player's groundstroke. Timing pulls crosscourt or pushes down the
/// line; the stick at contact nudges direction (x) and depth or spin (y):
/// forward drives flatter and deeper, back slices.
pub fn near_shot(hand: Hand, meet: Meet, contact: Vec3, stick: Vec2) -> Shot {
    let e = match meet {
        Meet::Clean(e) => e,
        _ => {
            return Shot {
                target: Vec2::new(contact.x * 0.5, -6.5),
                stroke: Stroke {
                    speed: 12.0,
                    rpm: 300.0,
                    ..Stroke::default()
                },
                error: 3.5,
                wobble: 0.05,
            }
        }
    };
    let (speed, rpm, depth) = if stick.y < -0.5 {
        (19.0, -1700.0, 8.4)
    } else if stick.y > 0.5 {
        (29.0, 1100.0, 10.0)
    } else if hand == Hand::Forehand {
        (26.0, 2100.0, 9.2)
    } else {
        (24.5, 1800.0, 9.0)
    };
    let quality = 1.0 - e * e;
    let pull = if hand == Hand::Backhand {
        e * 3.0
    } else {
        -e * 3.0
    };
    let x = (contact.x * 0.5 + pull + stick.x * 1.8).clamp(-5.5, 5.5);
    Shot {
        target: Vec2::new(x, -depth),
        stroke: Stroke {
            speed: speed * (0.8 + 0.2 * quality),
            rpm,
            ..Stroke::default()
        },
        error: 0.35 + 1.4 * e * e,
        wobble: 0.006 + 0.03 * e * e,
    }
}

/// The near player's serve: the stick aims within the box; timing the hit to
/// the toss's apex decides pace and accuracy. Second serves kick.
pub fn near_serve(deuce: bool, second: bool, quality: f32, stick: Vec2) -> Shot {
    // The far receiver's deuce box is x < 0 (their right).
    let box_x = if deuce { -2.0 } else { 2.0 };
    let x = (box_x + stick.x * 1.6).clamp(-4.6, 4.6);
    let target = Vec2::new(x, -(5.4 + stick.y * 0.7));
    let miss = 1.0 - quality;
    if second {
        Shot {
            target,
            stroke: Stroke {
                speed: 31.0 + 4.0 * quality,
                rpm: 3000.0,
                ..Stroke::default()
            },
            error: 0.2 + 0.6 * miss * miss,
            wobble: 0.003 + 0.006 * miss,
        }
    } else {
        Shot {
            target,
            stroke: Stroke {
                speed: 37.0 + 12.0 * quality,
                rpm: 900.0,
                ..Stroke::default()
            },
            error: 0.3 + 1.5 * miss * miss,
            wobble: 0.004 + 0.012 * miss,
        }
    }
}

/// The far player's groundstroke from an intent, aimed at the near half.
/// `pressure` (0 easy .. 1 hard) comes from the incoming pace, an awkward
/// contact height and a stretch; aggression and pressure both cost accuracy.
pub fn far_shot(i: &Intent, opponent: Vec3, stretched: bool, pressure: f32) -> Shot {
    let a = i.aggression;
    let side = 3.0 + 0.6 * a;
    let x = match i.target.as_str() {
        "forehand" => side,
        "backhand" => -side,
        "body" => opponent.x.clamp(-3.6, 3.6),
        _ => {
            if opponent.x >= 0.0 {
                -side
            } else {
                side
            }
        }
    };
    let (depth, mut stroke) = match i.shot.as_str() {
        "flat" => (
            7.4 + 3.0 * a,
            Stroke {
                speed: 25.0 + 10.0 * a,
                rpm: 700.0,
                ..Stroke::default()
            },
        ),
        "slice" => (
            7.4 + 2.6 * a,
            Stroke {
                speed: 17.0 + 4.0 * a,
                rpm: -1700.0,
                ..Stroke::default()
            },
        ),
        "drop_shot" => (
            2.2 + 0.8 * a,
            Stroke {
                speed: 11.0 + 2.0 * a,
                rpm: -2300.0,
                ..Stroke::default()
            },
        ),
        "lob" => (
            9.6 + 1.2 * a,
            Stroke {
                speed: 15.0 + 3.0 * a,
                rpm: 2200.0,
                lob: true,
                ..Stroke::default()
            },
        ),
        _ => (
            7.4 + 3.0 * a,
            Stroke {
                speed: 21.0 + 9.0 * a,
                rpm: 1900.0 + 900.0 * a,
                ..Stroke::default()
            },
        ),
    };
    let mut error = 0.9 + 0.6 * a * a;
    if matches!(i.shot.as_str(), "lob" | "drop_shot") {
        error += 0.3;
    }
    if stretched {
        stroke.speed *= 0.75;
        error += 1.0;
    }
    Shot {
        target: Vec2::new(x, depth),
        stroke,
        error: error + 1.0 * pressure,
        wobble: 0.012 + 0.012 * a + 0.016 * pressure + if stretched { 0.012 } else { 0.0 },
    }
}

/// How hard the incoming ball is to handle, 0..~1.5.
pub fn pressure(ball: Flight, contact_offset: f32) -> f32 {
    let pace = ((ball.vel.length() - 16.0) / 14.0).max(0.0);
    let height = if ball.pos.y > 1.5 {
        (ball.pos.y - 1.5) * 0.6
    } else if ball.pos.y < 0.45 {
        (0.45 - ball.pos.y) * 1.5
    } else {
        0.0
    };
    pace + height + 0.5 * contact_offset
}

/// The far player's serve into the near receiver's box.
pub fn far_serve(i: &Intent, deuce: bool, second: bool, receiver: Vec3) -> Shot {
    let a = if second {
        i.aggression * 0.5
    } else {
        i.aggression
    };
    // The near receiver's deuce box is x > 0 (their right).
    let s = if deuce { 1.0 } else { -1.0 };
    let x = match i.target.as_str() {
        "wide" => s * (3.25 + 0.5 * a),
        "t" => s * (0.55 - 0.3 * a),
        _ => (receiver.x * s).clamp(0.8, 3.4) * s,
    };
    let target = Vec2::new(x, 5.2 + 0.9 * a);
    let stroke = match (second, i.shot.as_str()) {
        (true, _) | (false, "kick") => Stroke {
            speed: 33.0 + 5.0 * a,
            rpm: 3000.0,
            ..Stroke::default()
        },
        (false, "slice") => Stroke {
            speed: 39.0 + 6.0 * a,
            rpm: 900.0,
            side: s * 1400.0,
            ..Stroke::default()
        },
        _ => Stroke {
            speed: 44.0 + 8.0 * a,
            rpm: 700.0,
            ..Stroke::default()
        },
    };
    let error = if second {
        0.15 + 0.4 * a * a
    } else {
        0.22 + 0.9 * a * a
    };
    Shot {
        target,
        stroke,
        error,
        wobble: 0.003 + 0.008 * a,
    }
}

/// A planned contact for the far player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Ticks from now.
    pub ticks: u64,
    pub ball: Vec3,
    pub hand: Hand,
    pub stand: Vec3,
    pub stretch: bool,
}

/// Distance a runner starting from rest covers in `t` seconds.
fn reach(t: f32, speed: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else if t < speed / ACCEL {
        0.5 * ACCEL * t * t
    } else {
        speed * t - speed * speed / (2.0 * ACCEL)
    }
}

/// Predict the incoming ball exactly (the tick's own integrator) and choose
/// where `side` meets it: after its bounce at a comfortable height, or on the
/// volley when already at the net. None when it will be out, in the net, or
/// never cross — the far player lets those go, and so may you.
pub fn plan(
    side: Side,
    mut f: Flight,
    from: Vec3,
    at_net: bool,
    serve: bool,
    dt: f32,
) -> Option<Plan> {
    let body = Player {
        side,
        ..Player::default()
    };
    let half = side.half();
    let speed = if side == Side::Near {
        NEAR_SPEED
    } else {
        FAR_SPEED
    };
    let mut bounces = 0;
    let mut best: Option<(f32, Plan)> = None;
    for k in 1..(4.0 / dt) as u64 {
        match f.step(dt) {
            Touch::Net => return None,
            Touch::Bounce(at) => {
                bounces += 1;
                if bounces == 1 && (at.z * half < 0.0 || !ball::in_court(at)) {
                    return None;
                }
                if bounces == 2 {
                    break;
                }
            }
            _ => {}
        }
        let p = f.pos;
        let volley =
            bounces == 0 && at_net && !serve && p.z * half > 0.8 && (0.4..=2.1).contains(&p.y);
        let ground = bounces == 1 && (0.3..=1.5).contains(&p.y) && p.z * half < HALF_LENGTH + 3.8;
        if !volley && !ground {
            continue;
        }
        let comfortable = volley || (f.vel.y <= 0.0 && p.y <= 1.25) || (0.7..=1.3).contains(&p.y);
        let t = k as f32 * dt - if side == Side::Far { REACTION } else { 0.1 };
        let stand = |hand: Hand| {
            let right = if hand == Hand::Forehand { 0.8 } else { -0.8 };
            // Stand so the ball is `right` to the side and SWEET in front.
            Vec3::new(p.x - right * body.right(), 0.0, p.z + SWEET * half)
        };
        let (fh, bh) = (stand(Hand::Forehand), stand(Hand::Backhand));
        let d = |s: Vec3| Vec2::new(s.x - from.x, s.z - from.z).length();
        let hand = if d(fh) <= d(bh) + 0.5 {
            Hand::Forehand
        } else {
            Hand::Backhand
        };
        let spot = if hand == Hand::Forehand { fh } else { bh };
        let short = d(spot) - reach(t, speed);
        let candidate = Plan {
            ticks: k,
            ball: p,
            hand,
            stand: spot,
            stretch: short > 0.3,
        };
        if short <= 0.3 && comfortable {
            return Some(candidate);
        }
        if best.is_none_or(|(s, _)| short < s) {
            best = Some((short, candidate));
        }
    }
    best.map(|(_, plan)| plan)
}

/// Racket pose as (yaw, roll, twist) in degrees for a swing `t` seconds old.
/// Yaw turns the arm about the shoulder (90 points it forward), roll lifts it,
/// twist turns the face. Keyframes are eased with smoothstep.
pub fn pose(hand: Option<Hand>, t: f32, tossing: bool) -> Quat {
    const READY: [f32; 3] = [90.0, -25.0, 0.0];
    let keys: &[(f32, [f32; 3])] = match hand {
        Some(Hand::Forehand) => &[
            (0.0, READY),
            (0.09, [-70.0, -10.0, 0.0]),
            (0.158, [10.0, -5.0, 0.0]),
            (0.3, [150.0, 10.0, 0.0]),
            (0.55, READY),
        ],
        Some(Hand::Backhand) => &[
            (0.0, READY),
            (0.09, [225.0, -10.0, 0.0]),
            (0.158, [170.0, -5.0, 0.0]),
            (0.3, [30.0, 10.0, 0.0]),
            (0.55, READY),
        ],
        Some(Hand::Serve) => &[
            (0.0, [90.0, 180.0, 90.0]),
            (0.08, [90.0, 250.0, 90.0]),
            (0.158, [90.0, 70.0, 90.0]),
            (0.3, [90.0, -60.0, 90.0]),
            (0.55, READY),
        ],
        None if tossing => &[(0.0, READY), (0.35, [90.0, 180.0, 90.0])],
        None => &[(0.0, READY)],
    };
    let mut angles = keys[keys.len() - 1].1;
    for pair in keys.windows(2) {
        let ((t0, a), (t1, b)) = (pair[0], pair[1]);
        if t < t1 {
            let s = math::smoothstep(0.0, 1.0, ((t - t0) / (t1 - t0)).clamp(0.0, 1.0));
            angles = [0, 1, 2].map(|i| math::lerp(a[i], b[i], s));
            break;
        }
    }
    let r = |deg: f32| deg.to_radians();
    Quat::from_rotation_y(r(angles[0]))
        * Quat::from_rotation_z(r(angles[1]))
        * Quat::from_rotation_x(r(angles[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroke_sides_and_timing() {
        let near = Player::default();
        let clean = |m: Meet| matches!(m, Meet::Clean(e) if e.abs() < 1e-3);
        let body = Vec3::new(0.0, 0.0, 10.0);
        // A ball 0.8 m to the right, 0.35 m in front: a clean forehand, a wrong-side backhand.
        let ball = relative(&near, body, Vec3::new(0.8, 1.0, 10.0 - 0.35));
        assert!(clean(meet(Hand::Forehand, ball)));
        assert_eq!(meet(Hand::Backhand, ball), Meet::Mishit);
        let left = relative(&near, body, Vec3::new(-0.8, 1.0, 9.0));
        assert!(
            matches!(meet(Hand::Backhand, left), Meet::Clean(e) if e > 0.0),
            "early"
        );
        assert_eq!(
            meet(
                Hand::Forehand,
                relative(&near, body, Vec3::new(3.0, 1.0, 10.0))
            ),
            Meet::Whiff
        );
        let far = Player {
            side: Side::Far,
            ..Player::default()
        };
        let far_body = Vec3::new(0.0, 0.0, -10.0);
        // The far player's right is -x; its front is +z.
        assert!(clean(meet(
            Hand::Forehand,
            relative(&far, far_body, Vec3::new(-0.8, 1.0, -9.65))
        )));
    }

    #[test]
    fn early_pulls_crosscourt_late_pushes_down_the_line() {
        let contact = Vec3::new(2.0, 1.0, 10.0);
        let early = near_shot(Hand::Forehand, Meet::Clean(0.6), contact, Vec2::ZERO);
        let late = near_shot(Hand::Forehand, Meet::Clean(-0.6), contact, Vec2::ZERO);
        assert!(early.target.x < 0.0 && late.target.x > contact.x);
        let slice = near_shot(
            Hand::Backhand,
            Meet::Clean(0.0),
            contact,
            Vec2::new(0.0, -1.0),
        );
        assert!(slice.stroke.rpm < 0.0);
    }
}

#[cfg(test)]
mod error_rates {
    use super::*;
    use crate::brain::Intent;

    /// Fraction of far-player shots that net or land out, over a grid of draws.
    fn misses(shot: &str, aggression: f32, pressure: f32) -> f32 {
        let intent = Intent {
            shot: shot.into(),
            target: "backhand".into(),
            aggression,
            ..Intent::default()
        };
        let base = far_shot(&intent, Vec3::new(0.5, 0.0, 12.0), false, pressure);
        let from = Vec3::new(0.3, 0.9, -11.5);
        let (mut bad, mut n) = (0, 0);
        for i in 0..12 {
            for j in 0..12 {
                let (u, v) = ((i as f32 + 0.5) / 12.0, (j as f32 + 0.5) / 12.0);
                let target = base.target + ball::scatter(base.error, u, v);
                let mut f = ball::launch(from, target, base.stroke, 1.0 / 120.0);
                let wobble = base.wobble * (2.0 * ((i * 12 + j) as f32 / 143.0) - 1.0);
                f.vel.y += f.vel.length() * wobble;
                n += 1;
                match ball::landing(f, 1.0 / 120.0, 4.0) {
                    Some(at) if at.z > 0.0 && ball::in_court(at) => {}
                    _ => bad += 1,
                }
            }
        }
        bad as f32 / n as f32
    }

    #[test]
    fn aggression_and_pressure_cost_accuracy() {
        let calm = misses("drive", 0.45, 0.0);
        let bold = misses("flat", 1.0, 0.0);
        let hurried = misses("drive", 0.45, 1.0);
        eprintln!("miss rates: calm {calm:.3} bold {bold:.3} hurried {hurried:.3}");
        assert!((0.02..0.15).contains(&calm), "calm {calm}");
        assert!(bold > calm && (0.15..0.5).contains(&bold), "bold {bold}");
        assert!(hurried > calm, "hurried {hurried}");
    }
}
