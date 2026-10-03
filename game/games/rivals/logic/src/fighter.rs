//! Fighters: one capsule each, steered by the player's input or a bot's brain.
//! Walk, sprint, jump, slide and knockback all feed one horizontal wish velocity
//! into the physics capsule; gravity and jumps own its vertical velocity.
use crate::weapons::{Weapon, RIFLE_MAG, ROCKET_MAG};
use exact_game::math::{cos, sin};
use exact_game::*;
use exact_game_physics::{self as physics, CapsuleController, Collider, Shape};

pub const RADIUS: f32 = 0.35;
pub const HEIGHT: f32 = 1.8;
/// Eye above the capsule centre (feet are 0.9 below it).
pub const EYE: f32 = 0.7;
pub const SLIDE_EYE: f32 = 0.1;
/// A ray hitting the capsule above this height (from its centre) is a headshot.
pub const HEAD_FROM: f32 = 0.5;
pub const MAX_HP: f32 = 100.0;
/// The view never pitches past this, up or down.
pub const PITCH_LIMIT: f32 = 1.45;
pub const WALK: f32 = 7.0;
pub const SPRINT: f32 = 10.0;
pub const GRAVITY: f32 = 22.0;
pub const JUMP_SPEED: f32 = 7.6;
const GROUND_ACCEL: f32 = 70.0;
const AIR_ACCEL: f32 = 16.0;
const SLIDE_BOOST: f32 = 4.0;
const SLIDE_TIME: f32 = 0.8;
const SLIDE_COOLDOWN: f32 = 0.9;
const KNOCK_DECAY: f32 = 3.5;

/// What one fighter asks for in one tick. Player input and bot brains both
/// produce this, so bots move with exactly the player's rules.
#[derive(Clone, Copy, Debug, Default)]
pub struct Intent {
    /// Local stick: x right, z back (as `Input::stick_xz`).
    pub stick: Vec3,
    pub sprint: bool,
    pub jump: bool,
    pub slide: bool,
    pub fire: bool,
    /// Aim down sights: a narrower view, a tighter spread, a slower walk.
    pub aim: bool,
    pub reload: bool,
    pub inspect: bool,
    pub switch: Option<Weapon>,
    /// Look change in radians this tick.
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Debug, Default, Component)]
pub struct Fighter {
    /// Collision layer index (bit `slot`); 1 is the player.
    pub slot: u32,
    pub label: String,
    pub bot: bool,
    pub hp: f32,
    pub alive: bool,
    pub respawn_at: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub weapon: Weapon,
    pub rifle_ammo: u32,
    pub rocket_ammo: u32,
    pub reload_until: f32,
    pub next_shot: f32,
    pub bloom: f32,
    pub kick: f32,
    /// View climb from recoil still to recover.
    pub climb: f32,
    pub swing_at: f32,
    pub shot_at: f32,
    pub inspect_at: f32,
    pub knock: Vec3,
    /// Input-driven horizontal velocity; knockback adds on top of it.
    pub planar: Vec3,
    pub slide_until: f32,
    pub slide_ready: f32,
    pub slide_dir: Vec3,
    pub eye: f32,
    pub aiming: bool,
    pub kills: u32,
    pub deaths: u32,
    pub rounds: u32,
    pub last_attacker: u32,
    pub shots: u32,
    pub hits: u32,
    pub headshots: u32,
}
impl Fighter {
    pub fn bit(&self) -> u32 {
        1 << self.slot
    }
    pub fn sliding(&self, now: f32) -> bool {
        now < self.slide_until
    }
    pub fn reset_loadout(&mut self) {
        self.hp = MAX_HP;
        self.alive = true;
        self.weapon = Weapon::Rifle;
        self.rifle_ammo = RIFLE_MAG;
        self.rocket_ammo = ROCKET_MAG;
        self.reload_until = 0.0;
        self.bloom = 0.0;
        self.kick = 0.0;
        self.shot_at = -10.0;
        self.inspect_at = -10.0;
        self.climb = 0.0;
        self.knock = Vec3::ZERO;
        self.planar = Vec3::ZERO;
        self.slide_until = 0.0;
        self.eye = EYE;
    }
}

/// Horizontal forward for a yaw; yaw 0 looks down -Z.
pub fn forward(yaw: f32) -> Vec3 {
    Vec3::new(-sin(yaw), 0.0, -cos(yaw))
}
/// Unit view direction for a yaw and pitch (positive pitch looks up).
pub fn view(yaw: f32, pitch: f32) -> Vec3 {
    let (sp, cp) = (sin(pitch), cos(pitch));
    Vec3::new(-sin(yaw) * cp, sp, -cos(yaw) * cp)
}
pub fn look(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
}

/// Spawn a fighter root (physics capsule) and, for a bot, its soldier model in
/// team colour `look` (the player is a camera: no body to draw).
pub fn spawn(w: &mut World, slot: u32, label: &str, bot: bool, look: Option<usize>) -> Entity {
    let mut f = Fighter {
        slot,
        label: label.into(),
        bot,
        ..Fighter::default()
    };
    f.reset_loadout();
    let root = w.spawn_named(
        label.to_string(),
        (
            Transform::at(0.0, 0.92, 0.0),
            CapsuleController {
                radius: RADIUS,
                height: HEIGHT,
                step: 0.35,
                ..CapsuleController::default()
            },
            Collider {
                shape: Shape::Capsule {
                    radius: RADIUS,
                    height: HEIGHT,
                },
                sensor: true,
                layer: 1 << slot,
                // Fighters block one another as well as the arena.
                mask: u32::MAX,
                ..Collider::default()
            },
            f,
        ),
    );
    if let Some(look) = look {
        w.spawn_named(
            format!("{label}-model"),
            (
                Parent(root),
                Transform::default(),
                Mesh::asset(format!("soldier{look}.model")),
                Animator::new([State::blend(
                    "move",
                    Blend::across([(0.0, "Idle"), (WALK, "Run")]).parameter("speed"),
                )]),
            ),
        );
    }
    root
}

/// Place a fighter at a spawn, facing the arena's centre.
pub fn place(w: &mut World, e: Entity, spawn: [f32; 2]) {
    let yaw = exact_game::math::atan2(spawn[0], spawn[1]);
    w.teleport(e, Transform::at(spawn[0], 0.92, spawn[1]));
    {
        let mut c = w.require_mut::<CapsuleController>(e);
        c.velocity = Vec3::ZERO;
        c.grounded = false;
    }
    let mut f = w.require_mut::<Fighter>(e);
    f.yaw = yaw;
    f.pitch = 0.0;
    f.reset_loadout();
    let bit = f.bit();
    drop(f);
    w.require_mut::<Collider>(e).layer = bit;
}

/// Turn, then move one fighter through its capsule. Returns the step result.
pub fn step(w: &mut World, e: Entity, intent: &Intent) -> physics::CapsuleStep {
    let now = w.seconds() as f32;
    let dt = w.dt();
    let (velocity, alive) = {
        let mut f = w.require_mut::<Fighter>(e);
        let mut c = w.require_mut::<CapsuleController>(e);
        if intent.inspect && f.alive && f.reload_until == 0.0 {
            f.inspect_at = now;
        }
        f.aiming = f.alive && intent.aim && f.weapon != Weapon::Knife && !f.sliding(now);
        if f.alive {
            f.yaw = exact_game::math::wrap_angle(f.yaw + intent.yaw);
            f.pitch = (f.pitch + intent.pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        }
        let fwd = forward(f.yaw);
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let stick = if f.alive { intent.stick } else { Vec3::ZERO };
        let wish = (right * stick.x - fwd * stick.z).clamp_length_max(1.0);
        let forwardish = -stick.z > 0.3;
        let speed = if intent.sprint && forwardish && !f.aiming {
            SPRINT
        } else {
            WALK
        } * f.weapon.speed()
            * if f.aiming { 0.65 } else { 1.0 };
        let mut planar = f.planar;
        if f.alive && c.grounded && intent.slide && now >= f.slide_ready && planar.length() > 5.0 {
            f.slide_dir = planar.normalize();
            f.slide_until = now + SLIDE_TIME;
            f.slide_ready = now + SLIDE_COOLDOWN;
            planar = f.slide_dir * (planar.length().max(SPRINT) + SLIDE_BOOST);
        }
        if f.sliding(now) {
            // A slide keeps its line and bleeds speed; the stick steers a little.
            let left = (f.slide_until - now) / SLIDE_TIME;
            let target = f.slide_dir * (WALK + (SPRINT + SLIDE_BOOST - WALK) * left) + wish * 2.0;
            planar = planar + (target - planar) * (8.0 * dt).min(1.0);
            if intent.jump && c.grounded {
                f.slide_until = now;
            }
        } else {
            let accel = if c.grounded { GROUND_ACCEL } else { AIR_ACCEL };
            let target = wish * speed;
            let delta = target - planar;
            let step = accel * dt;
            planar = if delta.length() <= step {
                target
            } else {
                planar + delta * (step / delta.length())
            };
        }
        if f.alive && intent.jump && c.grounded {
            c.velocity.y = JUMP_SPEED;
        }
        c.velocity.y -= GRAVITY * dt;
        let eye = if f.sliding(now) { SLIDE_EYE } else { EYE };
        f.eye += (eye - f.eye) * (14.0 * dt).min(1.0);
        let decay = 1.0 - (KNOCK_DECAY * dt).min(1.0);
        let knock = f.knock;
        f.knock = if c.grounded && c.velocity.y <= 0.0 {
            knock * (decay * decay)
        } else {
            knock * decay
        };
        f.bloom = (f.bloom - 0.12 * dt).max(0.0);
        f.kick = (f.kick - 6.0 * dt).max(0.0);
        // Recoil settles back once the trigger rests.
        if now >= f.next_shot + 0.05 && f.climb > 0.0 {
            let back = f.climb.min(0.6 * dt);
            f.climb -= back;
            f.pitch -= back;
        }
        f.planar = planar;
        (planar + knock, f.alive)
    };
    let mut result = physics::capsule(w, e).step(velocity);
    // Knockback is game-owned; a wall that stopped the capsule absorbs it.
    let achieved = result.displacement / dt;
    let mut f = w.require_mut::<Fighter>(e);
    if f.knock.length_squared() > 0.0
        && achieved.length_squared() < velocity.length_squared() * 0.25
    {
        f.knock = Vec3::ZERO;
    }
    if !alive {
        result.displacement = Vec3::ZERO;
    }
    result
}

/// Turn and animate a bot's soldier (a child: the capsule root stays upright),
/// hidden while dead. Speed drives the idle–run blend.
pub fn pose(w: &mut World, label: &str, yaw: f32, speed: f32, alive: bool) {
    let name = format!("{label}-model");
    if let Some(mut t) = w.get_mut::<Transform>(name.as_str()) {
        let r = Quat::from_rotation_y(yaw);
        if t.rotation != r {
            t.rotation = r;
        }
    }
    if let Some(mut a) = w.get_mut::<Animator>(name.as_str()) {
        a.set("speed", speed);
    }
    let shown = w.get::<Visible>(name.as_str()).is_none_or(|v| v.0);
    if let Some(e) = w.named(&name).filter(|_| shown != alive) {
        w.insert(e, Visible(alive));
    }
}
