//! Bots: they see with raycasts, react after a delay, aim with an error that
//! shrinks while they track, strafe at a preferred range, duck behind cover to
//! reload or when hurt, and jump when the capsule stalls against an edge.
//! A brain only produces an `Intent`; the player's movement rules apply.
use crate::arena;
use crate::fighter::{self, Fighter, Intent, EYE};
use crate::weapons::Weapon;
use exact_game::math::{atan2, sqrt};
use exact_game::*;
use exact_game_physics as physics;

/// How often (in ticks) a bot re-checks sight; staggered by slot.
pub const SIGHT_EVERY: u64 = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Plan {
    #[default]
    Hunt,
    Fight,
    Cover,
}

#[derive(Clone, Debug, Default, Component)]
pub struct Brain {
    /// 0 (training dummy) to 1 (sharp).
    pub skill: f32,
    pub dummy: bool,
    pub plan: Plan,
    pub target: Option<u32>,
    pub visible: bool,
    pub last_seen: Vec3,
    pub react_at: f32,
    pub aim_error: Vec2,
    pub strafe: f32,
    pub strafe_until: f32,
    pub range: f32,
    pub goal: Vec3,
    pub cover_until: f32,
    pub stalled: f32,
    pub wander_until: f32,
    /// A short obstacle detour, held across ticks instead of alternating sides.
    pub detour: Vec3,
    pub detour_until: f32,
    pub head_bias: bool,
}

/// What a brain knows about one fighter this tick.
#[derive(Clone, Copy, Debug)]
pub struct Seen {
    pub entity: Entity,
    pub slot: u32,
    pub at: Vec3,
    pub alive: bool,
    pub hp: f32,
    pub velocity: Vec3,
}

pub fn snapshot(w: &World) -> Vec<Seen> {
    w.query::<(&Fighter, &Transform, &physics::CapsuleController)>()
        .iter()
        .map(|(e, (f, t, c))| Seen {
            entity: e,
            slot: f.slot,
            at: t.position,
            alive: f.alive,
            hp: f.hp,
            velocity: c.velocity,
        })
        .collect()
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    atan2(-d.x, -d.z)
}
fn pitch_to(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    atan2(d.y, sqrt(d.x * d.x + d.z * d.z))
}

/// Can `from` (an eye) see `target`'s capsule? One hitscan ray; the first
/// thing it reaches must be the target.
pub fn sees(w: &World, own: u32, from: Vec3, target: &Seen) -> bool {
    let to = target.at + Vec3::new(0.0, 0.35, 0.0);
    let d = to - from;
    crate::weapons::hitscan(w, from, d, d.length() + 1.0, own)
        .is_some_and(|h| h.fighter == Some(target.entity))
}

/// Think for one bot: returns its intent for this tick.
pub fn think(w: &World, e: Entity, all: &[Seen], covers: &[Vec3]) -> Intent {
    let now = w.seconds() as f32;
    let dt = w.dt();
    let me = *all
        .iter()
        .find(|s| s.entity == e)
        .expect("bot is a fighter");
    let (own, yaw, pitch, weapon, ammo, reloading) = {
        let f = w.require::<Fighter>(e);
        let ammo = match f.weapon {
            Weapon::Rifle => f.rifle_ammo,
            Weapon::Rocket => f.rocket_ammo,
            Weapon::Knife => 1,
        };
        (
            f.bit(),
            f.yaw,
            f.pitch,
            f.weapon,
            ammo,
            f.reload_until > 0.0,
        )
    };
    let mut b = w.require_mut::<Brain>(e);
    let mut intent = Intent::default();
    if !me.alive || b.dummy {
        return intent;
    }
    let eye = me.at + Vec3::new(0.0, EYE, 0.0);
    // Sight: re-check every few ticks (rays are the expensive part), staggered.
    if (w.tick() + me.slot as u64).is_multiple_of(SIGHT_EVERY) {
        let mut best: Option<(f32, u32, bool)> = None;
        for other in all.iter().filter(|o| o.slot != me.slot && o.alive) {
            let d = (other.at - me.at).length();
            let keep = b.target == Some(other.slot);
            if best.is_some_and(|(bd, _, _)| bd <= d && !keep) {
                continue;
            }
            if sees(w, own, eye, other) {
                best = Some((if keep { d * 0.7 } else { d }, other.slot, true));
            }
        }
        let was = b.visible;
        match best {
            Some((_, slot, _)) => {
                if !was || b.target != Some(slot) {
                    // Acquisition: a reaction delay and a fresh aim error.
                    b.react_at = now + 0.32 - 0.14 * b.skill;
                    let wide = 0.22 - 0.12 * b.skill;
                    b.aim_error = Vec2::new(w.rand(-wide..wide), w.rand(-wide..wide) * 0.5);
                    b.head_bias = w.chance(0.15 + 0.35 * b.skill);
                }
                b.target = Some(slot);
                b.visible = true;
                b.wander_until = now + 5.0;
                b.plan = if b.plan == Plan::Cover {
                    Plan::Cover
                } else {
                    Plan::Fight
                };
            }
            None => {
                b.visible = false;
                if b.plan == Plan::Fight {
                    b.plan = Plan::Hunt;
                }
            }
        }
    }
    let target = b
        .target
        .and_then(|slot| all.iter().find(|s| s.slot == slot && s.alive).copied());
    if target.is_none() {
        b.target = None;
        b.visible = false;
    }
    if let Some(t) = target.filter(|_| b.visible) {
        b.last_seen = t.at;
    }
    // Hurt or reloading in the open: find cover out of the target's sight.
    let hurt = me.hp < 40.0;
    if b.plan != Plan::Cover && b.visible && (reloading || hurt) && now >= b.cover_until {
        if let Some(t) = target {
            let threat = t.at + Vec3::new(0.0, EYE, 0.0);
            let mut best: Option<(f32, Vec3)> = None;
            for c in covers {
                let d = (*c - me.at).length();
                if d > 14.0 || best.is_some_and(|(bd, _)| bd <= d) {
                    continue;
                }
                let to = *c + Vec3::new(0.0, EYE, 0.0) - threat;
                let hidden = physics::raycast(w, threat, to, to.length(), arena::WORLD)
                    .is_some_and(|h| h.distance < to.length() - 0.2);
                if hidden {
                    best = Some((d, *c));
                }
            }
            if let Some((_, c)) = best {
                b.plan = Plan::Cover;
                b.goal = c;
                b.cover_until = now + if hurt { 3.0 } else { 2.4 };
            }
        }
    }
    if b.plan == Plan::Cover && now >= b.cover_until {
        b.plan = if b.visible { Plan::Fight } else { Plan::Hunt };
        b.cover_until = now + 2.0;
    }
    // Aim: approach the target with a turn-rate limit; the error decays as it tracks.
    if let Some(t) = target.filter(|_| b.visible) {
        let lead = t.velocity * (0.08 + 0.1 * (1.0 - b.skill));
        let aim_at = if weapon == Weapon::Rocket {
            t.at + Vec3::new(0.0, -0.75, 0.0) + t.velocity * ((t.at - me.at).length() / 42.0)
        } else if b.head_bias {
            t.at + Vec3::new(0.0, 0.68, 0.0) + lead
        } else {
            t.at + Vec3::new(0.0, 0.2, 0.0) + lead
        };
        let decay = 1.0 - (2.0 + 2.5 * b.skill) * dt;
        b.aim_error *= decay.max(0.0);
        let floor = 0.012 + 0.03 * (1.0 - b.skill);
        let wobble = Vec2::new(w.rand(-floor..floor), w.rand(-floor..floor));
        let want_yaw = yaw_to(eye, aim_at) + b.aim_error.x + wobble.x;
        let want_pitch = pitch_to(eye, aim_at) + b.aim_error.y + wobble.y;
        let rate = (5.0 + 7.0 * b.skill) * dt;
        let dy = exact_game::math::wrap_angle(want_yaw - yaw);
        let dp = want_pitch - pitch;
        intent.yaw = dy.clamp(-rate, rate);
        intent.pitch = dp.clamp(-rate, rate);
        let distance = (t.at - me.at).length();
        let on_target = dy.abs() < 0.06 && dp.abs() < 0.08;
        intent.fire = now >= b.react_at && on_target && b.plan != Plan::Cover && ammo > 0;
        // Weapon choice by range: knife up close, rockets at mid range on a whim.
        intent.switch = if distance < 3.2 {
            Some(Weapon::Knife)
        } else if weapon == Weapon::Knife || (weapon == Weapon::Rocket && ammo == 0) {
            Some(Weapon::Rifle)
        } else if weapon == Weapon::Rifle && (6.0..22.0).contains(&distance) && w.chance(0.25 * dt)
        {
            Some(Weapon::Rocket)
        } else {
            None
        };
        if weapon == Weapon::Rifle && ammo < 8 && b.plan == Plan::Cover {
            intent.reload = true;
        }
    } else {
        intent.pitch = (-pitch).clamp(-2.0 * dt, 2.0 * dt);
        if ammo < 20 && weapon == Weapon::Rifle {
            intent.reload = true;
        }
        if weapon != Weapon::Rifle {
            intent.switch = Some(Weapon::Rifle);
        }
    }
    // The planner asks to start a normal reload once; it does not repeatedly
    // press the timing control while that reload is in progress.
    intent.reload &= !reloading;
    // Movement: a world-space wish, converted to the stick's yaw-local frame.
    let mut wish = Vec3::ZERO;
    match (b.plan, target) {
        (Plan::Cover, _) => {
            let to = b.goal - me.at;
            if Vec3::new(to.x, 0.0, to.z).length() > 0.6 {
                wish = Vec3::new(to.x, 0.0, to.z).normalize();
                intent.sprint = true;
            }
        }
        (Plan::Fight, Some(t)) => {
            if now >= b.strafe_until {
                b.strafe = if w.chance(0.5) { 1.0 } else { -1.0 };
                b.strafe_until = now + w.rand(0.45..1.3);
                b.range = w.rand(8.0..15.0);
                if w.chance(0.12 + 0.2 * b.skill) {
                    intent.jump = true;
                }
            }
            let to = t.at - me.at;
            let flat = Vec3::new(to.x, 0.0, to.z);
            let d = flat.length().max(0.01);
            let toward = flat / d;
            let side = Vec3::new(-toward.z, 0.0, toward.x) * b.strafe;
            let close = if weapon == Weapon::Knife {
                1.0
            } else {
                ((d - b.range) / 4.0).clamp(-1.0, 1.0)
            };
            wish = (side + toward * close).normalize_or_zero();
            if weapon == Weapon::Knife {
                intent.sprint = true;
                intent.slide = d < 7.0 && w.chance(1.5 * dt);
            }
        }
        _ => {
            // Hunt toward where the target was, or wander between cover points.
            let goal = if target.is_some() {
                b.last_seen
            } else {
                b.goal
            };
            if (goal - me.at).length() < 2.0 || now >= b.wander_until {
                // Once the last sighting is searched, commit to a new place.
                // Keeping target here overwrote that choice with last_seen on
                // the next tick, or picked a different cover point every tick.
                b.target = None;
                let next: Vec<_> = covers
                    .iter()
                    .copied()
                    .filter(|p| (*p - me.at).length() > 3.0)
                    .collect();
                b.goal = *w.pick(&next).unwrap_or(&Vec3::ZERO);
                b.wander_until = now + w.rand(3.0..6.0);
            } else if target.is_some() {
                b.goal = goal;
            }
            let to = b.goal - me.at;
            wish = Vec3::new(to.x, 0.0, to.z).normalize_or_zero();
            intent.sprint = true;
            intent.slide = w.chance(0.4 * dt);
        }
    }
    if b.plan != Plan::Fight && wish.length_squared() > 0.0 {
        let to = b.goal - me.at;
        let distance = Vec3::new(to.x, 0.0, to.z).length();
        wish = steer(w, me.at, wish, distance, now, &mut b);
    }
    if !b.visible && wish.length_squared() > 0.0 {
        let dy = exact_game::math::wrap_angle(yaw_to(me.at, me.at + wish) - yaw);
        intent.yaw = dy.clamp(-4.0 * dt, 4.0 * dt);
    }
    // Dress a wound only after reaching cover, using the player's action and
    // interruption rules. Holding still also avoids spending the channel moving.
    if b.plan == Plan::Cover
        && !b.visible
        && wish.length_squared() == 0.0
        && me.hp <= fighter::MAX_HP - fighter::BANDAGE_HEAL
        && w.require::<Fighter>(e).can_bandage()
    {
        intent = Intent {
            bandage: true,
            yaw: intent.yaw,
            pitch: intent.pitch,
            ..Intent::default()
        };
        b.cover_until = b.cover_until.max(now + fighter::BANDAGE_TIME + dt);
    }
    // Stuck against an edge: hop (crates are jumpable) and flip the strafe.
    let moving = Vec3::new(me.velocity.x, 0.0, me.velocity.z).length();
    if wish.length() > 0.5 && moving < 1.5 {
        b.stalled += dt;
    } else {
        b.stalled = 0.0;
    }
    if b.stalled > 0.25 {
        intent.jump = true;
        b.strafe = -b.strafe;
        b.stalled = 0.0;
    }
    let fwd = fighter::forward(yaw + intent.yaw);
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    intent.stick = Vec3::new(wish.dot(right), 0.0, -wish.dot(fwd));
    intent
}

/// Look ahead with the same capsule width and head height as the fighter.
/// The feet are lifted 4 cm so the support floor is not an obstacle. Only the
/// static arena participates; this does not reveal another fighter's position.
fn steer(w: &World, at: Vec3, wish: Vec3, distance: f32, now: f32, brain: &mut Brain) -> Vec3 {
    let shape = physics::Shape::Capsule {
        radius: fighter::RADIUS,
        height: fighter::HEIGHT - 0.04,
    };
    let pose = Transform::at(at.x, at.y + 0.02, at.z);
    let ahead = distance.min(2.0);
    let clearance = |dir: Vec3| {
        physics::sweep(w, &shape, pose, dir * ahead, arena::WORLD).map_or(ahead, |hit| hit.distance)
    };
    if now < brain.detour_until && clearance(brain.detour) >= ahead.min(0.5) {
        return brain.detour;
    }
    if clearance(wish) >= ahead {
        brain.detour = Vec3::ZERO;
        brain.detour_until = 0.0;
        return wish;
    }
    let side = Vec3::new(-wish.z, 0.0, wish.x);
    let mut best = (f32::NEG_INFINITY, wish);
    for dir in [
        wish + side,
        wish - side,
        side,
        -side,
        -wish + side,
        -wish - side,
        -wish,
    ] {
        let dir = dir.normalize();
        let score = clearance(dir) * 2.0 + dir.dot(wish) + dir.dot(brain.detour) * 0.2;
        if score > best.0 {
            best = (score, dir);
        }
    }
    brain.detour = best.1;
    brain.detour_until = now + 0.3;
    brain.detour
}
