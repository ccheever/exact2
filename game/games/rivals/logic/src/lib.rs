//! Rivals: a fast first-person arena duel against bots. First to five kills
//! takes the round. Mouse (or arrow keys) looks, WASD moves, Shift sprints,
//! Space jumps, C slides; click or F fires; 1/2/3 pick rifle, rockets, knife.
pub mod arena;
pub mod bots;
pub mod fighter;
pub mod round;
pub mod weapons;

use bots::Brain;
use exact_game::*;
use fighter::{Fighter, Intent};
use round::Round;
use weapons::{Effect, Rocket, Weapon};

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    /// Bots in the arena: 1 is a duel, more is free-for-all (up to 24).
    pub bots: u32,
    /// Bot skill, 0 to 1; 0 picks the default.
    pub skill: f32,
    /// Training range: bots stand still and never fire.
    pub range: bool,
    /// The night arena: moonlight, floodlights and glowing trim.
    pub night: bool,
    /// Radians of turn per canvas point of mouse movement; 0 picks the default.
    #[live]
    pub sensitivity: f32,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
}
impl Options {
    pub fn bot_count(&self) -> u32 {
        self.bots.clamp(1, 24)
    }
}

pub const DEFAULT_SENSITIVITY: f32 = 0.0025;
const KEY_TURN: f32 = 2.4;
/// Every model and sky the arena loads before setup (art-src/gen.mjs makes them).
pub const ASSETS: &[&str] = &[
    "arena.model",
    "rifle.model",
    "mag.model",
    "launcher.model",
    "knife.model",
    "rocket.model",
    "flash.model",
    "soldier0.model",
    "soldier1.model",
    "soldier2.model",
    "soldier3.model",
    "soldier4.model",
    "soldier5.model",
    "soldier6.model",
    "soldier7.model",
    "sky.tex",
    "sky_night.tex",
];

#[derive(Clone, Debug, Default, Data)]
pub struct Hud {
    pub hp: u32,
    pub weapon: String,
    pub slot: u32,
    pub ammo: u32,
    pub mag: u32,
    pub reloading: bool,
    pub kills: u32,
    pub deaths: u32,
    pub rival_kills: u32,
    pub rival: String,
    pub to_win: u32,
    pub round: u32,
    pub you_rounds: u32,
    pub rival_rounds: u32,
    pub alive: bool,
    pub respawn: u32,
    pub killed_by: String,
    pub marker: String,
    pub hurt: bool,
    pub over: bool,
    pub winner: String,
    pub you_won: bool,
    pub sliding: bool,
    pub sprinting: bool,
    pub accuracy: u32,
    pub feed: Vec<round::Feed>,
    pub marks: Vec<round::Mark>,
}

pub struct Rivals;
impl Game for Rivals {
    const ID: &'static str = "rivals";
    const HZ: u32 = 120;
    const ASSETS: &'static [&'static str] = ASSETS;
    type Args = Options;
    fn actions() -> Actions {
        actions()
    }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        register(w);
    }
    fn setup(w: &mut World, args: &Options) {
        setup(w, args);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        tick(w, input, args);
    }
}

/// The same game at other fixed rates, for the tick-rate experiments.
pub mod rates {
    use super::*;
    macro_rules! at_rate {
        ($name:ident, $hz:expr) => {
            pub struct $name;
            impl Game for $name {
                const ID: &'static str = "rivals";
                const HZ: u32 = $hz;
                const ASSETS: &'static [&'static str] = ASSETS;
                type Args = Options;
                fn actions() -> Actions {
                    actions()
                }
                fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
                    register(w);
                }
                fn setup(w: &mut World, args: &Options) {
                    setup(w, args);
                }
                fn paused(args: &Options) -> bool {
                    args.paused
                }
                fn tick(w: &mut World, input: &Input, args: &Options) {
                    tick(w, input, args);
                }
            }
        };
    }
    at_rate!(Rivals30, 30);
    at_rate!(Rivals60, 60);
    at_rate!(Rivals240, 240);
}

pub fn actions() -> Actions {
    Actions::new()
        .stick("move", Stick::wasd())
        .stick(
            "look",
            Stick::keys("ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"),
        )
        .button("jump", &["Space"])
        .button("sprint", &["ShiftLeft", "ShiftRight"])
        .button("slide", &["KeyC", "ControlLeft"])
        .button("fire", &["KeyF", "MouseLeft"])
        .button("aim", &["MouseRight"])
        .button("reload", &["KeyR"])
        .button("rifle", &["Digit1"])
        .button("rocket", &["Digit2"])
        .button("knife", &["Digit3"])
        .button("inspect", &["KeyT"])
}

pub fn register(w: &mut World) {
    exact_game_physics::register(w);
    w.register::<Fighter>()
        .register::<Brain>()
        .register::<Rocket>()
        .register::<Effect>()
        .register::<Ambient>()
        .register::<Emitter>()
        .register::<PointLight>()
        .register::<Animator>()
        .register::<animation::Layers>();
    w.register_resource::<Round>();
}

pub fn setup(w: &mut World, args: &Options) {
    w.reseed(args.seed);
    register(w);
    arena::build(w, args.night);
    let player = fighter::spawn(w, 1, "player", false, None);
    fighter::place(w, player, arena::SPAWNS[0]);
    let count = args.bot_count();
    let skill = if args.skill > 0.0 {
        args.skill.min(1.0)
    } else {
        0.55
    };
    for i in 0..count {
        let label = format!("bot-{}", i + 1);
        let e = fighter::spawn(w, i + 2, &label, true, Some(i as usize % 8));
        let spawn = if args.range {
            [-2.0 + 2.0 * (i % 3) as f32, 12.0 - 3.0 * (i / 3) as f32]
        } else if count == 1 {
            arena::SPAWNS[1]
        } else {
            arena::SPAWNS[(i as usize + 1) % arena::SPAWNS.len()]
        };
        fighter::place(w, e, spawn);
        if args.range {
            // Dummies face the player's spawn.
            w.require_mut::<Fighter>(e).yaw = std::f32::consts::PI;
        }
        w.insert(
            e,
            Brain {
                skill,
                dummy: args.range,
                goal: Vec3::ZERO,
                range: 11.0,
                ..Brain::default()
            },
        );
    }
    let camera = w.spawn_named(
        "camera",
        (
            Transform::at(0.0, 1.6, 18.0),
            Camera {
                fov_y_degrees: 74.0,
                near: 0.05,
                far: 300.0,
                ..Camera::default()
            },
        ),
    );
    viewmodel(w, camera);
    w.insert_resource(Round {
        number: 1,
        ..Round::default()
    });
    camera_follow(w, args);
    publish(w, args);
}

/// Weapons are modelled at life size and drawn smaller, nearer: as an FPS does.
const VIEW_SCALE: f32 = 0.72;

/// First-person weapon models, children of the camera, drawn in the viewmodel
/// layer so they never sink into walls. Each knows how it animates.
fn viewmodel(w: &mut World, camera: Entity) {
    use Part::*;
    let rifle = Vec3::new(0.12, -0.15, -0.5);
    let launcher = Vec3::new(0.15, -0.15, -0.42);
    let parts = [
        ("vm-rifle", Weapon::Rifle, Body, rifle, "rifle.model"),
        (
            "vm-rifle-mag",
            Weapon::Rifle,
            Mag,
            rifle + Vec3::new(0.0, -0.035, -0.14) * VIEW_SCALE,
            "mag.model",
        ),
        (
            "vm-rifle-flash",
            Weapon::Rifle,
            Flash,
            rifle + Vec3::new(0.0, 0.012, -0.57) * VIEW_SCALE,
            "flash.model",
        ),
        (
            "vm-rocket",
            Weapon::Rocket,
            Body,
            launcher,
            "launcher.model",
        ),
        (
            "vm-rocket-flash",
            Weapon::Rocket,
            Flash,
            launcher + Vec3::new(0.0, 0.0, -0.64) * VIEW_SCALE,
            "flash.model",
        ),
        (
            "vm-knife",
            Weapon::Knife,
            Body,
            Vec3::new(0.19, -0.19, -0.3),
            "knife.model",
        ),
    ];
    for (name, weapon, part, at, model) in parts {
        w.spawn_named(
            name,
            (
                Parent(camera),
                Transform::at(at.x, at.y, at.z).with_scale(VIEW_SCALE),
                Mesh::asset(model),
                Visible(weapon == Weapon::Rifle && part != Flash),
                ViewModel,
                WeaponPart {
                    weapon,
                    part,
                    rest: at,
                },
            ),
        );
    }
    // The muzzle's light: off until a shot.
    w.spawn_named(
        "vm-light",
        (
            Parent(camera),
            Transform::at(0.15, -0.1, -0.9),
            PointLight {
                color: [1.0, 0.7, 0.35],
                intensity: 0.0,
                range: 9.0,
            },
        ),
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Part {
    #[default]
    Body,
    Mag,
    Flash,
}
/// One part of a first-person weapon model and where it rests.
#[derive(Clone, Debug, Default, Component)]
pub struct WeaponPart {
    pub weapon: Weapon,
    pub part: Part,
    pub rest: Vec3,
}

/// The player's intent: input actions plus the mouse's accumulated delta.
pub fn player_intent(w: &World, input: &Input, args: &Options) -> Intent {
    let dt = w.dt();
    let sens = if args.sensitivity > 0.0 {
        args.sensitivity
    } else {
        DEFAULT_SENSITIVITY
    };
    let mut yaw = 0.0;
    let mut pitch = 0.0;
    if let Some(p) = input.pointer() {
        yaw -= p.delta.x * sens;
        pitch -= p.delta.y * sens;
    }
    let keys = input.stick("look");
    yaw -= keys.x * KEY_TURN * dt;
    pitch += keys.y * KEY_TURN * 0.6 * dt;
    let switch = if input.pressed("rifle") {
        Some(Weapon::Rifle)
    } else if input.pressed("rocket") {
        Some(Weapon::Rocket)
    } else if input.pressed("knife") {
        Some(Weapon::Knife)
    } else {
        let wheel = input.wheel().y;
        let current = w.require::<Fighter>("player").weapon;
        let order = [Weapon::Rifle, Weapon::Rocket, Weapon::Knife];
        let i = order.iter().position(|o| *o == current).unwrap_or(0);
        if wheel > 0.0 {
            Some(order[(i + 1) % 3])
        } else if wheel < 0.0 {
            Some(order[(i + 2) % 3])
        } else {
            None
        }
    };
    Intent {
        stick: input.stick_xz("move"),
        sprint: input.held("sprint"),
        jump: input.pressed("jump"),
        slide: input.pressed("slide"),
        fire: input.held("fire") || input.pressed("fire"),
        aim: input.held("aim"),
        reload: input.pressed("reload"),
        inspect: input.pressed("inspect"),
        switch,
        yaw,
        pitch,
    }
}

pub fn tick(w: &mut World, input: &Input, args: &Options) {
    let now = w.seconds() as f32;
    let over_until = w.resource::<Round>().over_until;
    if over_until > 0.0 {
        if now >= over_until {
            w.resource_mut::<Round>().over_until = 0.0;
            round::next_round(w, args.bot_count() == 1);
        } else {
            mouse_look(w, args, false);
            weapons::effects(w);
            publish(w, args);
            return;
        }
    }
    // Decide: the player from input, bots from what they can see.
    let mut intents: Vec<(Entity, Intent)> = Vec::new();
    let player = w.named("player").expect("player");
    intents.push((player, player_intent(w, input, args)));
    let seen = bots::snapshot(w);
    let covers = arena::cover_points();
    let brains: Vec<Entity> = w.query::<&Brain>().iter().map(|(e, _)| e).collect();
    for e in brains {
        intents.push((e, bots::think(w, e, &seen, &covers)));
    }
    intents.sort_by_key(|(e, _)| *e);
    // Move everyone, then let everyone act on the moved world: no fighter's
    // shot sees a half-stepped arena, and the query scene is rebuilt once.
    for (e, intent) in &intents {
        fighter::step(w, *e, intent);
    }
    let mut hits = Vec::new();
    for (e, intent) in &intents {
        let (at, eye) = {
            let f = w.require::<Fighter>(*e);
            (w.require::<Transform>(*e).position, f.eye)
        };
        hits.extend(weapons::act(w, *e, intent, at + Vec3::new(0.0, eye, 0.0)));
    }
    hits.extend(weapons::fly(w));
    // Bots hit this tick flinch: an additive layer over their run or idle.
    let flinched: Vec<String> = hits
        .iter()
        .filter(|h| !h.killed)
        .filter_map(|h| weapons::fighter_root(w, h.victim))
        .filter_map(|e| {
            w.get::<Fighter>(e)
                .filter(|f| f.bot)
                .map(|f| f.label.clone())
        })
        .collect();
    round::score(w, hits);
    round::respawn(w);
    round::check_win(w);
    for s in bots::snapshot(w) {
        let f = w.require::<Fighter>(s.entity).clone();
        if f.bot {
            fighter::pose(w, &f.label, f.yaw, f.planar.length(), f.alive);
        }
    }
    for victim in flinched {
        if let Some(e) = w.named(&format!("{victim}-model")) {
            w.insert(
                e,
                animation::Layers(vec![animation::Layer::new(
                    Animation::play("Flinch").once(),
                )
                .additive()]),
            );
        }
    }
    animation::step(w);
    emitter::step(w);
    camera_follow(w, args);
    weapons::effects(w);
    publish(w, args);
}

/// While the tick turns the camera by the mouse, the renderer may too: between
/// ticks `MouseLook` turns the drawn camera by motion no tick has shown yet.
/// Dead, or between rounds, the mouse turns nothing, so neither may the renderer.
fn mouse_look(w: &mut World, args: &Options, on: bool) {
    let e = w.named("camera").expect("camera");
    let sens = if args.sensitivity > 0.0 {
        args.sensitivity
    } else {
        DEFAULT_SENSITIVITY
    };
    let look = MouseLook {
        yaw_per_point: -sens,
        pitch_per_point: -sens,
        pitch_limit: fighter::PITCH_LIMIT,
    };
    let current = w.get::<MouseLook>(e).map(|l| *l);
    match (on, current) {
        (true, Some(l)) if l == look => {}
        (true, _) => {
            w.insert(e, look);
        }
        (false, Some(_)) => {
            w.remove::<MouseLook>(e);
        }
        (false, None) => {}
    }
}

/// The camera sits at the player's eye. The weapon bobs with your stride, kicks
/// with recoil, tilts and drops its magazine to reload, turns over to inspect,
/// comes to the centre when aiming, and flashes at the muzzle with each shot.
pub fn camera_follow(w: &mut World, args: &Options) {
    use std::f32::consts::PI;
    let now = w.seconds() as f32;
    let (at, f) = {
        let e = w.named("player").expect("player");
        (
            w.require::<Transform>(e).position,
            w.require::<Fighter>(e).clone(),
        )
    };
    let lift = if f.alive { 0.0 } else { 1.2 };
    let mut t = Transform::at(at.x, at.y + f.eye + lift, at.z);
    t.rotation = fighter::look(f.yaw, if f.alive { f.pitch } else { -0.5 });
    *w.require_mut::<Transform>("camera") = t;
    let fov = if f.aiming { 52.0 } else { 74.0 };
    if w.require::<Camera>("camera").fov_y_degrees != fov {
        w.require_mut::<Camera>("camera").fov_y_degrees = fov;
    }
    mouse_look(w, args, f.alive);
    let speed = (f.planar.length() / fighter::SPRINT).min(1.0);
    let bob = Vec3::new(
        0.006 * math::cos(now * 7.0),
        0.009 * math::sin(now * 14.0),
        0.0,
    ) * speed;
    let swing = (now - f.swing_at).clamp(0.0, 0.3) / 0.3;
    let reload = if f.reload_until > now {
        1.0 - (f.reload_until - now) / weapons::reload_time(f.weapon)
    } else {
        1.0
    };
    let inspect = ((now - f.inspect_at) / 1.6).clamp(0.0, 1.0);
    let flashing = now - f.shot_at < 0.04 && f.alive;
    for (_, (vm, t, visible)) in w
        .query::<(&WeaponPart, &mut Transform, &mut Visible)>()
        .iter()
    {
        let held = f.alive && vm.weapon == f.weapon;
        let show = held
            && match vm.part {
                Part::Flash => flashing,
                Part::Mag => reload >= 1.0 || !(0.3..0.55).contains(&reload),
                Part::Body => true,
            };
        if visible.0 != show {
            visible.0 = show;
        }
        let mut at = vm.rest + bob + Vec3::new(0.0, 0.0, 0.05 * f.kick);
        let mut rot = Quat::from_rotation_x(0.12 * f.kick);
        if f.aiming {
            at.x -= 0.16;
            at.y += 0.05;
        }
        if reload < 1.0 {
            let s = math::sin(PI * reload);
            at.y -= 0.05 * s;
            rot *= Quat::from_rotation_z(0.45 * s);
            if vm.part == Part::Mag {
                at.y -= if reload < 0.5 {
                    0.4 * (reload / 0.5) * (reload / 0.5)
                } else {
                    0.25 * (1.0 - (reload - 0.5) / 0.5)
                };
            }
        }
        if inspect < 1.0 && vm.part == Part::Body {
            rot *= Quat::from_rotation_y(0.9 * math::sin(PI * inspect))
                * Quat::from_rotation_z(0.6 * math::sin(2.0 * PI * inspect));
        }
        if vm.weapon == Weapon::Knife && swing < 1.0 {
            let s = math::sin(swing * PI);
            at += Vec3::new(-0.15, 0.05, -0.15) * s;
            rot *= Quat::from_rotation_y(0.9 * s);
        }
        if vm.part == Part::Flash {
            rot = Quat::from_rotation_z(f.shots as f32 * 1.7);
        }
        if t.position != at || t.rotation != rot {
            t.position = at;
            t.rotation = rot;
        }
    }
    let light = if flashing { 1600.0 } else { 0.0 };
    if w.require::<PointLight>("vm-light").intensity != light {
        w.require_mut::<PointLight>("vm-light").intensity = light;
    }
}

pub fn publish(w: &World, args: &Options) {
    let now = w.seconds() as f32;
    let me = w.require::<Fighter>("player").clone();
    let rival = w
        .query::<&Fighter>()
        .iter()
        .filter(|(_, f)| f.bot)
        .max_by(|a, b| a.1.kills.cmp(&b.1.kills).then(b.0.cmp(&a.0)))
        .map(|(_, f)| f.clone())
        .unwrap_or_default();
    let r = w.resource::<Round>();
    let (ammo, mag) = match me.weapon {
        Weapon::Rifle => (me.rifle_ammo, weapons::RIFLE_MAG),
        Weapon::Rocket => (me.rocket_ammo, weapons::ROCKET_MAG),
        Weapon::Knife => (0, 0),
    };
    let hud = Hud {
        hp: me.hp.max(0.0).round() as u32,
        weapon: me.weapon.name().into(),
        slot: match me.weapon {
            Weapon::Rifle => 1,
            Weapon::Rocket => 2,
            Weapon::Knife => 3,
        },
        ammo,
        mag,
        reloading: me.reload_until > 0.0,
        kills: me.kills,
        deaths: me.deaths,
        rival_kills: rival.kills,
        rival: if args.bot_count() == 1 {
            rival.label.clone()
        } else {
            format!("{} (leader)", rival.label)
        },
        to_win: round::TO_WIN,
        round: r.number,
        you_rounds: me.rounds,
        rival_rounds: w
            .query::<&Fighter>()
            .iter()
            .filter(|(_, f)| f.bot)
            .map(|(_, f)| f.rounds)
            .max()
            .unwrap_or(0),
        alive: me.alive,
        respawn: if me.alive {
            0
        } else {
            exact_game::math::ceil((me.respawn_at - now).max(0.0)) as u32
        },
        killed_by: r.killed_by.clone(),
        marker: if now < r.marker_until {
            if r.marker_kill {
                "kill".into()
            } else if r.marker_head {
                "head".into()
            } else {
                "body".into()
            }
        } else {
            String::new()
        },
        hurt: now < r.hurt_until,
        over: r.over(now),
        winner: r.winner.clone(),
        you_won: r.winner == "player",
        sliding: me.sliding(now),
        sprinting: false,
        accuracy: if me.shots > 0 {
            me.hits * 100 / me.shots
        } else {
            0
        },
        feed: r.feed.clone(),
        marks: r.marks.clone(),
    };
    drop(r);
    w.publish_record(&hud);
}
