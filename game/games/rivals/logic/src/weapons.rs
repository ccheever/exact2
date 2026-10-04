//! Three weapons: a hitscan rifle with recoil and spread, a rocket launcher
//! whose rockets fly as swept rays and explode with splash and knockback, and a
//! knife. All damage goes through `damage`, which the round rules read.
use crate::arena;
use crate::fighter::{self, Fighter, HEAD_FROM};
use exact_game::*;
use exact_game_physics::{self as physics, Shape};

pub const RIFLE_MAG: u32 = 30;
pub const ROCKET_MAG: u32 = 2;
const RIFLE_INTERVAL: f32 = 0.1;
const RIFLE_BODY: f32 = 20.0;
const HEAD_MULTIPLIER: f32 = 1.8;
const RIFLE_RANGE: f32 = 150.0;
const RIFLE_RELOAD: f32 = 1.6;
const ROCKET_INTERVAL: f32 = 0.8;
const ROCKET_RELOAD: f32 = 2.2;
pub const ROCKET_SPEED: f32 = 42.0;
const ROCKET_LIFE: f32 = 4.0;
const ROCKET_DIRECT: f32 = 35.0;
pub const SPLASH_RADIUS: f32 = 4.5;
const SPLASH_DAMAGE: f32 = 75.0;
const SELF_SPLASH: f32 = 0.35;
const KNOCKBACK: f32 = 16.0;
const KNIFE_INTERVAL: f32 = 0.55;
const KNIFE_RANGE: f32 = 2.4;
const KNIFE_DAMAGE: f32 = 45.0;
const BACKSTAB: f32 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Weapon {
    #[default]
    Rifle,
    Rocket,
    Knife,
}
impl Weapon {
    pub fn name(self) -> &'static str {
        match self {
            Weapon::Rifle => "Assault rifle",
            Weapon::Rocket => "Rocket launcher",
            Weapon::Knife => "Knife",
        }
    }
    /// Movement-speed multiplier while holding this weapon.
    pub fn speed(self) -> f32 {
        match self {
            Weapon::Rifle => 1.0,
            Weapon::Rocket => 0.92,
            Weapon::Knife => 1.15,
        }
    }
}

/// A rocket in flight. Each tick it sweeps a ray over its whole step, so no
/// speed can tunnel through a wall or a capsule.
#[derive(Clone, Debug, Default, Component)]
pub struct Rocket {
    pub owner: u32,
    pub velocity: Vec3,
    pub born: f32,
}

/// A short-lived cosmetic: tracers, impacts and explosions.
#[derive(Clone, Debug, Default, Component)]
pub struct Effect {
    pub until: f32,
    pub grow: f32,
}

/// One landed hit, for hit markers, damage numbers and the round rules.
#[derive(Clone, Debug, Default, Data)]
pub struct Damage {
    pub attacker: u32,
    pub victim: u32,
    pub amount: f32,
    pub head: bool,
    pub weapon: Weapon,
    pub killed: bool,
    /// Where this shot, swing or explosion came from when it hit.
    pub origin: Vec3,
}

/// Fighter slot behind a ray or overlap hit, if the entity is a fighter.
fn fighter_slot(w: &World, e: Entity) -> Option<u32> {
    w.get::<Fighter>(e).map(|f| f.slot)
}
/// Every fighter's root by slot, in entity order.
pub fn roots(w: &World) -> Vec<(Entity, u32)> {
    w.query::<&Fighter>()
        .iter()
        .map(|(e, f)| (e, f.slot))
        .collect()
}
fn root_of(w: &World, slot: u32) -> Option<Entity> {
    roots(w)
        .into_iter()
        .find(|(_, s)| *s == slot)
        .map(|(e, _)| e)
}
/// Layer mask of every fighter (bits 1..).
pub fn fighters_mask(w: &World) -> u32 {
    roots(w).iter().fold(0, |m, (_, s)| m | (1 << s))
}

/// What a hitscan ray hits first: a wall or a living fighter's capsule.
/// Dead fighters leave their collision layer, so rays pass through them.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub fighter: Option<Entity>,
    pub distance: f32,
    pub point: Vec3,
}
pub fn hitscan(w: &World, origin: Vec3, dir: Vec3, range: f32, own: u32) -> Option<Shot> {
    physics::raycast(w, origin, dir, range, !own).map(|h| Shot {
        fighter: w.has::<Fighter>(h.entity).then_some(h.entity),
        distance: h.distance,
        point: h.point,
    })
}

/// Apply damage; returns the landed hit (after death checks) for the caller to log.
pub fn damage(
    w: &World,
    attacker: u32,
    victim: Entity,
    amount: f32,
    head: bool,
    weapon: Weapon,
    origin: Vec3,
) -> Option<Damage> {
    let mut f = w.get_mut::<Fighter>(victim)?;
    if !f.alive || amount <= 0.0 {
        return None;
    }
    let amount = amount.min(f.hp);
    f.hp -= amount;
    if f.slot != attacker {
        f.last_attacker = attacker;
    }
    let killed = f.hp <= 0.0;
    if killed {
        f.alive = false;
        // The body leaves play: shots, splash and movement pass through it.
        if let Some(mut c) = w.get_mut::<exact_game_physics::Collider>(victim) {
            c.layer = 0;
        }
    }
    Some(Damage {
        attacker,
        victim: f.slot,
        amount,
        head,
        weapon,
        killed,
        origin,
    })
}

/// Reload, switch and fire for one fighter. Returns damage dealt this tick.
pub fn act(w: &mut World, e: Entity, intent: &fighter::Intent, origin: Vec3) -> Vec<Damage> {
    let now = w.seconds() as f32;
    let mut out = Vec::new();
    let (weapon, yaw, pitch, bloom, slot, moving, aiming) = {
        let mut f = w.require_mut::<Fighter>(e);
        if !f.alive {
            return out;
        }
        if let Some(next) = intent.switch.filter(|n| *n != f.weapon) {
            f.weapon = next;
            f.reload_until = 0.0;
            f.next_shot = f.next_shot.max(now + 0.25);
        }
        let reloading = f.reload_until > 0.0;
        if reloading && now >= f.reload_until {
            f.reload_until = 0.0;
            match f.weapon {
                Weapon::Rifle => f.rifle_ammo = RIFLE_MAG,
                Weapon::Rocket => f.rocket_ammo = ROCKET_MAG,
                Weapon::Knife => {}
            }
        }
        let (ammo, full, time) = match f.weapon {
            Weapon::Rifle => (f.rifle_ammo, RIFLE_MAG, RIFLE_RELOAD),
            Weapon::Rocket => (f.rocket_ammo, ROCKET_MAG, ROCKET_RELOAD),
            Weapon::Knife => (1, 1, 0.0),
        };
        if f.reload_until == 0.0 && ((intent.reload && ammo < full) || (ammo == 0 && intent.fire)) {
            f.reload_until = now + time;
        }
        if !intent.fire || f.reload_until > 0.0 || now < f.next_shot || ammo == 0 {
            return out;
        }
        f.next_shot = now
            + match f.weapon {
                Weapon::Rifle => RIFLE_INTERVAL,
                Weapon::Rocket => ROCKET_INTERVAL,
                Weapon::Knife => KNIFE_INTERVAL,
            };
        f.shots += 1;
        let moving = f.planar.length() > 2.0;
        (f.weapon, f.yaw, f.pitch, f.bloom, f.slot, moving, f.aiming)
    };
    let own = 1u32 << slot;
    match weapon {
        Weapon::Rifle => {
            // Spread from bloom (sustained fire) and movement; recoil kicks the view.
            let cone = (0.003 + bloom + if moving { 0.012 } else { 0.0 })
                * if aiming { 0.35 } else { 1.0 };
            let (a, r) = (w.rand(0.0..std::f32::consts::TAU), w.rand(0.0f32..1.0));
            let r = cone * exact_game::math::sqrt(r);
            let dir = fighter::view(
                yaw + r * exact_game::math::cos(a),
                pitch + r * exact_game::math::sin(a),
            );
            let hit = hitscan(w, origin, dir, RIFLE_RANGE, own);
            let end = hit.map_or(origin + dir * RIFLE_RANGE, |h| h.point);
            if let Some((victim, point)) = hit.and_then(|h| h.fighter.map(|e| (e, h.point))) {
                let centre = w.require::<Transform>(victim).position;
                let head = point.y - centre.y >= HEAD_FROM;
                let amount = RIFLE_BODY * if head { HEAD_MULTIPLIER } else { 1.0 };
                out.extend(damage(w, slot, victim, amount, head, Weapon::Rifle, origin));
            }
            {
                let mut f = w.require_mut::<Fighter>(e);
                f.rifle_ammo -= 1;
                f.bloom = (f.bloom + 0.006).min(0.045);
                f.kick = (f.kick + 0.35).min(1.0);
                let climb = 0.011f32.min(fighter::PITCH_LIMIT - f.pitch).max(0.0);
                f.pitch += climb;
                f.climb = (f.climb + climb).min(0.2);
                let sway = w.rand(-0.004f32..0.004);
                f.yaw += sway;
            }
            tracer(w, origin + dir * 0.6 + Vec3::new(0.0, -0.12, 0.0), end, now);
        }
        Weapon::Rocket => {
            let dir = fighter::view(yaw, pitch);
            w.require_mut::<Fighter>(e).rocket_ammo -= 1;
            w.require_mut::<Fighter>(e).kick = 1.0;
            // The muzzle is half a metre out; a wall closer than that takes the blast.
            match physics::raycast(w, origin, dir, 0.5, !own) {
                Some(h) => out.extend(explode(w, slot, h.point - dir * 0.05, Some(h.entity))),
                None => {
                    spawn_rocket(w, slot, origin + dir * 0.5, dir * ROCKET_SPEED, now);
                }
            }
        }
        Weapon::Knife => {
            w.require_mut::<Fighter>(e).swing_at = now;
            let dir = fighter::view(yaw, pitch);
            let victim = hitscan(w, origin, dir, KNIFE_RANGE, own)
                .and_then(|h| h.fighter)
                .or_else(|| {
                    // A forgiving sweep: knives should not need pixel aim.
                    physics::sweep(
                        w,
                        &Shape::Sphere { radius: 0.35 },
                        Transform::at(origin.x, origin.y - 0.3, origin.z),
                        dir * KNIFE_RANGE,
                        !own & !arena::WORLD,
                    )
                    .map(|h| h.entity)
                    .filter(|e| fighter_slot(w, *e).is_some())
                });
            if let Some(victim) = victim {
                let victim_yaw = w.require::<Fighter>(victim).yaw;
                let behind = fighter::forward(victim_yaw).dot(fighter::forward(yaw)) > 0.5;
                let amount = if behind { BACKSTAB } else { KNIFE_DAMAGE };
                out.extend(damage(
                    w,
                    slot,
                    victim,
                    amount,
                    false,
                    Weapon::Knife,
                    origin,
                ));
            }
        }
    }
    out
}

pub fn spawn_rocket(w: &mut World, owner: u32, at: Vec3, velocity: Vec3, now: f32) -> Entity {
    let mut t = Transform::at(at.x, at.y, at.z);
    t.rotation = Quat::from_rotation_arc(Vec3::Y, velocity.normalize());
    w.spawn((
        t,
        Mesh::cylinder(0.09, 0.5),
        Material {
            color: [0.9, 0.9, 0.9, 1.0],
            ..Material::glow([4.0, 1.6, 0.3])
        },
        Rocket {
            owner,
            velocity,
            born: now,
        },
    ))
}

/// Fly every rocket one tick: sweep all rays first, then write poses, so the
/// shared query scene is built once per tick rather than once per rocket.
pub fn fly(w: &mut World) -> Vec<Damage> {
    let now = w.seconds() as f32;
    let dt = w.dt();
    let rockets: Vec<(Entity, Rocket, Vec3)> = w
        .query::<(&Rocket, &Transform)>()
        .iter()
        .map(|(e, (r, t))| (e, r.clone(), t.position))
        .collect();
    let mut moves = Vec::new();
    let mut blasts = Vec::new();
    {
        let q = physics::queries(w);
        for (e, r, at) in &rockets {
            let step = r.velocity * dt;
            let hit = q.raycast(*at, step, step.length(), !(1u32 << r.owner));
            match hit {
                Some(h) => blasts.push((
                    *e,
                    r.owner,
                    h.point - step.normalize() * 0.05,
                    Some(h.entity),
                )),
                None if now - r.born > ROCKET_LIFE || (*at + step).y < -5.0 => {
                    blasts.push((*e, r.owner, *at, None))
                }
                None => moves.push((*e, *at + step)),
            }
        }
    }
    for (e, to) in moves {
        w.require_mut::<Transform>(e).position = to;
    }
    let mut out = Vec::new();
    for (e, owner, at, direct) in blasts {
        w.despawn(e);
        out.extend(explode(w, owner, at, direct));
    }
    out
}

/// Splash: overlap a sphere against fighters, then check each one's line of
/// sight to the blast so cover protects. Knockback pushes away from the centre.
pub fn explode(w: &mut World, owner: u32, at: Vec3, direct: Option<Entity>) -> Vec<Damage> {
    let now = w.seconds() as f32;
    let mask = fighters_mask(w);
    let caught = physics::overlap(
        w,
        &Shape::Sphere {
            radius: SPLASH_RADIUS,
        },
        Transform::at(at.x, at.y, at.z),
        mask,
    );
    let mut out = Vec::new();
    for victim in caught {
        let Some(slot) = fighter_slot(w, victim) else {
            continue;
        };
        let centre = w.require::<Transform>(victim).position;
        let to = centre - at;
        let distance = to.length();
        let blocked = distance > 0.5
            && physics::raycast(w, at, to, distance, arena::WORLD)
                .is_some_and(|h| h.distance < distance - fighter::RADIUS);
        if blocked {
            continue;
        }
        let falloff = (1.0 - (distance - 0.8).max(0.0) / SPLASH_RADIUS).clamp(0.0, 1.0);
        let mut amount = SPLASH_DAMAGE * falloff;
        if direct == Some(victim) {
            amount += ROCKET_DIRECT;
        }
        if slot == owner {
            amount *= SELF_SPLASH;
        }
        let push = if distance > 0.01 {
            to / distance
        } else {
            Vec3::Y
        };
        {
            let mut f = w.require_mut::<Fighter>(victim);
            if f.alive {
                let kick = push * KNOCKBACK * (0.35 + 0.65 * falloff);
                f.knock += Vec3::new(kick.x, 0.0, kick.z);
                let mut c = w.require_mut::<exact_game_physics::CapsuleController>(victim);
                // Lift replaces (never adds to) a jump: a rocket jump clears the deck.
                c.velocity.y = c.velocity.y.max(kick.y.max(0.0) * 0.6 + 2.5 * falloff);
            }
        }
        out.extend(damage(w, owner, victim, amount, false, Weapon::Rocket, at));
    }
    w.spawn((
        Transform::at(at.x, at.y, at.z).with_scale(0.4),
        Mesh::sphere(1.0),
        Material {
            color: [1.0, 0.6, 0.2, 0.9],
            ..Material::glow([6.0, 2.4, 0.5])
        },
        Effect {
            until: now + 0.25,
            grow: SPLASH_RADIUS * 3.0,
        },
        Ambient,
    ));
    out
}

fn tracer(w: &mut World, from: Vec3, to: Vec3, now: f32) {
    let length = (to - from).length();
    if length < 0.05 {
        return;
    }
    let mut t = Transform::at(
        (from.x + to.x) / 2.0,
        (from.y + to.y) / 2.0,
        (from.z + to.z) / 2.0,
    );
    t.rotation = Quat::from_rotation_arc(Vec3::Z, (to - from) / length);
    t.scale = Vec3::new(1.0, 1.0, length);
    w.spawn((
        t,
        Mesh::cuboid(Vec3::new(0.02, 0.02, 1.0)),
        Material {
            color: [1.0, 0.9, 0.5, 1.0],
            ..Material::glow([5.0, 4.0, 1.5])
        },
        Effect {
            until: now + 0.05,
            grow: 0.0,
        },
        Ambient,
    ));
    w.spawn((
        Transform::at(to.x, to.y, to.z),
        Mesh::sphere(0.06),
        Material {
            color: [1.0, 0.8, 0.4, 1.0],
            ..Material::glow([5.0, 3.0, 1.0])
        },
        Effect {
            until: now + 0.08,
            grow: 0.0,
        },
        Ambient,
    ));
}

/// Grow and expire cosmetics.
pub fn effects(w: &mut World) {
    let now = w.seconds() as f32;
    let dt = w.dt();
    let mut dead = Vec::new();
    for (e, (fx, t)) in w.query::<(&Effect, &mut Transform)>().iter() {
        if now >= fx.until {
            dead.push(e);
        } else if fx.grow > 0.0 {
            t.scale += Vec3::splat(fx.grow * dt);
        }
    }
    for e in dead {
        w.despawn(e);
    }
}

/// Root entity of a slot (for the round rules).
pub fn fighter_root(w: &World, slot: u32) -> Option<Entity> {
    root_of(w, slot)
}
