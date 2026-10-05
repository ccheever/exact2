//! The tables: the arena and every number the fight is tuned by, in one typed
//! file, `rivals.level.json` (LLP 1046.009 §3.1). These types are its schema;
//! the bake checks it against them and `check` below, and the world reads it
//! each tick rather than copying it at spawn, so an edit in the dev loop
//! reaches a running match. The art generator, the proof and the HUD read the
//! same file. What a fighter saved from an old value — a reload's deadline, a
//! magazine already filled — keeps it until it is next set.
use exact_game::asset::{Checked, Level};
use exact_game::*;
use std::sync::Arc;

pub const NAME: &str = "rivals.level.json";
pub const LEVEL: Level = Level::checked::<Tables>(NAME);

/// The tables as delivered; declared, so they arrive before setup.
pub fn of(w: &World) -> Arc<Tables> {
    w.shared_level::<Tables>(NAME)
        .expect("rivals.level.json arrives before setup")
}
/// The tables a look reads.
pub fn presented(p: &Present<'_>) -> Arc<Tables> {
    p.shared_level::<Tables>(NAME)
        .expect("rivals.level.json arrives before setup")
}

#[derive(Clone, Debug, Default, Data)]
pub struct Tables {
    pub arena: Arena,
    pub fighter: Moves,
    pub rifle: Rifle,
    pub rocket: Launcher,
    pub knife: Knife,
    /// The green part of a reload, as fractions of it: a second press there
    /// fills the magazine at once.
    pub quick_reload: [f32; 2],
    pub round: Rules,
}

/// The walled square, its cover, the floodlight pylons and the spawns. The
/// colliders, the classic look's boxes and the art pass's model are all built
/// from these blocks, so what you see and what you hit cannot drift apart.
#[derive(Clone, Debug, Default, Data)]
pub struct Arena {
    /// Half the arena's side, inside the walls.
    pub half: f32,
    /// Floodlight pylons in the corners, `[x, z]`.
    pub lights: Vec<[f32; 2]>,
    /// Box centres and sizes; the first four are the perimeter walls.
    pub blocks: Vec<Block>,
    /// `[x, z]`, facing the middle; duels start at the first two.
    pub spawns: Vec<[f32; 2]>,
}
#[derive(Clone, Debug, Default, Data)]
pub struct Block {
    /// wall, deck, ramp, slab, crate or low: its colour and its dressing.
    pub kind: String,
    pub at: [f32; 3],
    pub size: [f32; 3],
    /// Radians about x: a ramp.
    pub tilt: f32,
}

/// Movement and health, metres and seconds.
#[derive(Clone, Debug, Default, Data)]
pub struct Moves {
    pub max_hp: f32,
    pub walk: f32,
    pub sprint: f32,
    /// Walk multiplier while aiming down sights.
    pub aim_speed: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub ground_accel: f32,
    pub air_accel: f32,
    pub slide_boost: f32,
    pub slide_time: f32,
    pub slide_cooldown: f32,
    pub knock_decay: f32,
    pub bandage_heal: f32,
    pub bandage_time: f32,
    /// Walk multiplier while dressing a wound.
    pub bandage_speed: f32,
}

/// Hitscan: `body` a hit, times `head` to the head.
#[derive(Clone, Debug, Default, Data)]
pub struct Rifle {
    pub mag: u32,
    pub interval: f32,
    pub reload: f32,
    pub move_speed: f32,
    pub body: f32,
    pub head: f32,
    pub range: f32,
}
/// Rockets fly at `velocity` as swept rays and burst for splash and knockback.
#[derive(Clone, Debug, Default, Data)]
pub struct Launcher {
    pub mag: u32,
    pub interval: f32,
    pub reload: f32,
    pub move_speed: f32,
    pub velocity: f32,
    pub life: f32,
    pub direct: f32,
    pub splash_radius: f32,
    pub splash_damage: f32,
    /// The share of splash a rocket's owner takes.
    pub self_splash: f32,
    pub knockback: f32,
}
#[derive(Clone, Debug, Default, Data)]
pub struct Knife {
    pub interval: f32,
    pub move_speed: f32,
    pub range: f32,
    pub damage: f32,
    pub backstab: f32,
}
/// Rounds: kills to win (Mayhem's 24 bots need more), and the waits.
#[derive(Clone, Debug, Default, Data)]
pub struct Rules {
    pub respawn: f32,
    pub over: f32,
    pub kills: u32,
    pub mayhem_kills: u32,
}

/// The most fighters a match holds: the player and 24 bots.
pub const FIGHTERS: usize = 25;

impl Checked for Tables {
    /// What the types cannot say. A missing number reads as 0, so every
    /// quantity that must be positive is checked here.
    fn check(&self) -> Result<(), String> {
        let positive = |name: &str, v: f32| {
            if v.is_finite() && v > 0.0 {
                Ok(())
            } else {
                Err(format!("{name} must be a positive number, not {v}"))
            }
        };
        let f = &self.fighter;
        for (name, v) in [
            ("fighter.max_hp", f.max_hp),
            ("fighter.walk", f.walk),
            ("fighter.sprint", f.sprint),
            ("fighter.aim_speed", f.aim_speed),
            ("fighter.gravity", f.gravity),
            ("fighter.jump_speed", f.jump_speed),
            ("fighter.ground_accel", f.ground_accel),
            ("fighter.air_accel", f.air_accel),
            ("fighter.slide_time", f.slide_time),
            ("fighter.knock_decay", f.knock_decay),
            ("fighter.bandage_time", f.bandage_time),
            ("fighter.bandage_speed", f.bandage_speed),
            ("rifle.interval", self.rifle.interval),
            ("rifle.reload", self.rifle.reload),
            ("rifle.move_speed", self.rifle.move_speed),
            ("rifle.body", self.rifle.body),
            ("rifle.range", self.rifle.range),
            ("rocket.interval", self.rocket.interval),
            ("rocket.reload", self.rocket.reload),
            ("rocket.move_speed", self.rocket.move_speed),
            ("rocket.velocity", self.rocket.velocity),
            ("rocket.life", self.rocket.life),
            ("rocket.splash_radius", self.rocket.splash_radius),
            ("knife.interval", self.knife.interval),
            ("knife.move_speed", self.knife.move_speed),
            ("knife.range", self.knife.range),
            ("knife.damage", self.knife.damage),
            ("round.respawn", self.round.respawn),
            ("round.over", self.round.over),
            ("arena.half", self.arena.half),
        ] {
            positive(name, v)?;
        }
        if f.sprint < f.walk {
            return Err("fighter.sprint is slower than fighter.walk".into());
        }
        if self.rifle.mag == 0 || self.rocket.mag == 0 {
            return Err("rifle.mag and rocket.mag hold at least one round".into());
        }
        if self.round.kills == 0 || self.round.mayhem_kills == 0 {
            return Err("round.kills and round.mayhem_kills are at least 1".into());
        }
        let [from, to] = self.quick_reload;
        if !(0.0 < from && from < to && to < 1.0) {
            return Err(format!(
                "quick_reload [{from}, {to}] is not a window inside the reload (0 < from < to < 1)"
            ));
        }
        let a = &self.arena;
        if a.blocks.len() < 4 || a.blocks[..4].iter().any(|b| b.kind != "wall") {
            return Err("arena.blocks start with the four perimeter walls".into());
        }
        for (i, b) in a.blocks.iter().enumerate() {
            if !matches!(
                b.kind.as_str(),
                "wall" | "deck" | "ramp" | "slab" | "crate" | "low"
            ) {
                return Err(format!(
                    "arena.blocks[{i}].kind `{}` is not a block kind",
                    b.kind
                ));
            }
            if b.size.iter().any(|s| !(s.is_finite() && *s > 0.0)) {
                return Err(format!("arena.blocks[{i}].size must be positive"));
            }
        }
        if a.spawns.len() < FIGHTERS {
            return Err(format!(
                "arena.spawns has {}; a match of {FIGHTERS} fighters needs one each",
                a.spawns.len()
            ));
        }
        for (i, [x, z]) in a.spawns.iter().enumerate() {
            if x.abs() > a.half - 1.0 || z.abs() > a.half - 1.0 {
                return Err(format!("arena.spawns[{i}] [{x}, {z}] is outside the arena"));
            }
            if let Some(j) = a.spawns[..i].iter().position(|s| s == &[*x, *z]) {
                return Err(format!("arena.spawns[{i}] repeats arena.spawns[{j}]"));
            }
            // A capsule (0.35 m) placed in a block's footprint stands on it,
            // or slides off it (a ramp).
            let inside = |b: &&Block| {
                (x - b.at[0]).abs() < b.size[0] / 2.0 + 0.35
                    && (z - b.at[2]).abs() < b.size[2] / 2.0 + 0.35
            };
            if let Some(b) = a.blocks.iter().find(inside) {
                return Err(format!(
                    "arena.spawns[{i}] [{x}, {z}] stands in a {} block at {:?}",
                    b.kind, b.at
                ));
            }
        }
        Ok(())
    }
}
