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
const COLORS: [[f32; 3]; 8] = [
    [0.86, 0.22, 0.24],
    [0.94, 0.62, 0.12],
    [0.55, 0.30, 0.85],
    [0.15, 0.70, 0.45],
    [0.90, 0.35, 0.65],
    [0.20, 0.55, 0.90],
    [0.65, 0.65, 0.20],
    [0.40, 0.80, 0.85],
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
        .stick("look", Stick::keys("ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"))
        .button("jump", &["Space"])
        .button("sprint", &["ShiftLeft", "ShiftRight"])
        .button("slide", &["KeyC", "ControlLeft"])
        .button("fire", &["KeyF"])
        .button("reload", &["KeyR"])
        .button("rifle", &["Digit1"])
        .button("rocket", &["Digit2"])
        .button("knife", &["Digit3"])
}

pub fn register(w: &mut World) {
    exact_game_physics::register(w);
    w.register::<Fighter>()
        .register::<Brain>()
        .register::<Rocket>()
        .register::<Effect>()
        .register::<Ambient>();
    w.register_resource::<Round>();
}

pub fn setup(w: &mut World, args: &Options) {
    w.reseed(args.seed);
    register(w);
    arena::build(w);
    let player = fighter::spawn(w, 1, "player", false, [0.2, 0.5, 0.9]);
    fighter::place(w, player, arena::SPAWNS[0]);
    let count = args.bot_count();
    let skill = if args.skill > 0.0 { args.skill.min(1.0) } else { 0.55 };
    for i in 0..count {
        let label = format!("bot-{}", i + 1);
        let e = fighter::spawn(w, i + 2, &label, true, COLORS[i as usize % COLORS.len()]);
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
    camera_follow(w);
    publish(w, args);
}

/// First-person weapon models, children of the camera.
fn viewmodel(w: &mut World, camera: Entity) {
    let dark = Material::rgb(0.12, 0.13, 0.15).metallic(0.6).rough(0.4);
    let parts: [(&str, Weapon, Vec3, Mesh, Material, Quat); 5] = [
        (
            "vm-rifle",
            Weapon::Rifle,
            Vec3::new(0.2, -0.2, -0.45),
            Mesh::cuboid(Vec3::new(0.07, 0.1, 0.6)),
            dark,
            Quat::IDENTITY,
        ),
        (
            "vm-rifle-mag",
            Weapon::Rifle,
            Vec3::new(0.2, -0.29, -0.4),
            Mesh::cuboid(Vec3::new(0.05, 0.14, 0.08)),
            Material::rgb(0.75, 0.55, 0.2),
            Quat::IDENTITY,
        ),
        (
            "vm-rocket",
            Weapon::Rocket,
            Vec3::new(0.22, -0.2, -0.4),
            Mesh::cylinder(0.08, 0.75),
            Material::rgb(0.25, 0.42, 0.25).rough(0.7),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        (
            "vm-knife",
            Weapon::Knife,
            Vec3::new(0.22, -0.2, -0.38),
            Mesh::cuboid(Vec3::new(0.02, 0.05, 0.3)),
            Material::rgb(0.85, 0.87, 0.9).metallic(0.9).rough(0.2),
            Quat::IDENTITY,
        ),
        (
            "vm-knife-grip",
            Weapon::Knife,
            Vec3::new(0.22, -0.21, -0.2),
            Mesh::cuboid(Vec3::new(0.035, 0.06, 0.12)),
            Material::rgb(0.1, 0.1, 0.1),
            Quat::IDENTITY,
        ),
    ];
    for (name, weapon, at, mesh, material, rotation) in parts {
        let mut t = Transform::at(at.x, at.y, at.z);
        t.rotation = rotation;
        w.spawn_named(
            name,
            (
                Parent(camera),
                t,
                mesh,
                material,
                Visible(weapon == Weapon::Rifle),
                Viewmodel { weapon, rest: at },
            ),
        );
    }
}

#[derive(Clone, Debug, Default, Component)]
pub struct Viewmodel {
    pub weapon: Weapon,
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
    let mut clicked = false;
    if let Some(p) = input.pointer() {
        yaw -= p.delta.x * sens;
        pitch -= p.delta.y * sens;
        clicked = p.down;
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
        fire: input.held("fire") || input.pressed("fire") || clicked,
        reload: input.pressed("reload"),
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
    round::score(w, hits);
    round::respawn(w);
    round::check_win(w);
    for s in bots::snapshot(w) {
        let f = w.require::<Fighter>(s.entity).clone();
        if f.bot {
            fighter::face(w, &f.label, f.yaw);
        }
    }
    camera_follow(w);
    weapons::effects(w);
    publish(w, args);
}

/// The camera sits at the player's eye; the viewmodel kicks with recoil.
pub fn camera_follow(w: &mut World) {
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
    let swing = (now - f.swing_at).clamp(0.0, 0.3) / 0.3;
    for (_, (vm, t, visible)) in w
        .query::<(&Viewmodel, &mut Transform, &mut Visible)>()
        .iter()
    {
        let show = f.alive && vm.weapon == f.weapon;
        if visible.0 != show {
            visible.0 = show;
        }
        let mut at = vm.rest + Vec3::new(0.0, 0.0, 0.06 * f.kick);
        if f.reload_until > 0.0 {
            at.y -= 0.12;
        }
        if vm.weapon == Weapon::Knife && swing < 1.0 {
            at += Vec3::new(-0.15, 0.05, -0.15) * exact_game::math::sin(swing * std::f32::consts::PI);
        }
        if t.position != at {
            t.position = at;
        }
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
