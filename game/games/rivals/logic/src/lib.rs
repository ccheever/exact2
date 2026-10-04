//! Rivals: a fast first-person arena fight. Five kills wins; Mayhem needs 25.
//! Mouse (or arrow keys) looks, WASD moves, Shift sprints,
//! Space jumps, C slides; click or F fires; 1/2/3 pick rifle, rockets, knife.
pub mod arena;
pub mod bots;
pub mod fighter;
pub mod round;
pub mod training;
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
    /// The maximum roster needs room for reloads and respawns before a win.
    pub fn kills_to_win(&self) -> u32 {
        if self.bot_count() == 24 {
            25
        } else {
            5
        }
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
    pub heading: String,
    pub incoming: String,
    pub incoming_angle: f32,
    pub over: bool,
    pub winner: String,
    pub you_won: bool,
    pub sliding: bool,
    pub sprinting: bool,
    pub accuracy: u32,
    pub feed: Vec<round::Feed>,
    pub marks: Vec<round::Mark>,
    pub contacts: Vec<Contact>,
    pub training: bool,
    pub drill_done: bool,
    pub drill_left: u32,
    pub drill_score: u32,
    pub drill_combo: u32,
    pub drill_target: String,
    pub drill_cleared: u32,
}

/// A living opponent's visible head, in canvas points. Cover and the camera's
/// clip volume both hide its nameplate; no hidden positions enter this record.
#[derive(Clone, Debug, Default, Data)]
pub struct Contact {
    pub id: u32,
    pub label: String,
    pub hp: u32,
    pub x: f32,
    pub y: f32,
    pub target: bool,
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
    if args.range {
        w.insert_resource(training::Drill {
            target: if count >= 2 { 3 } else { 2 },
            ..training::Drill::default()
        });
    }
    let skill = if args.skill > 0.0 {
        args.skill.min(1.0)
    } else {
        0.55
    };
    for i in 0..count {
        let label = format!("bot-{}", i + 1);
        let e = fighter::spawn(w, i + 2, &label, true, COLORS[i as usize % COLORS.len()]);
        let spawn = if args.range {
            training::lane(i + 2)
        } else {
            arena::SPAWNS[i as usize + 1]
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
    publish(w, args, Vec2::ZERO);
}

/// First-person weapon models, children of the camera.
fn viewmodel(w: &mut World, camera: Entity) {
    let dark = Material::rgb(0.12, 0.13, 0.15).metallic(0.6).rough(0.4);
    let parts: [(&str, Weapon, Vec3, Mesh, Material, Quat); 5] = [
        (
            "vm-rifle",
            Weapon::Rifle,
            Vec3::new(0.2, -0.19, -0.58),
            Mesh::cuboid(Vec3::new(0.06, 0.08, 0.45)),
            dark,
            Quat::IDENTITY,
        ),
        (
            "vm-rifle-mag",
            Weapon::Rifle,
            Vec3::new(0.2, -0.27, -0.52),
            Mesh::cuboid(Vec3::new(0.04, 0.12, 0.07)),
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
                ViewModel,
                WeaponPart { weapon, rest: at },
            ),
        );
    }
}

/// One part of a first-person weapon model and where it rests.
#[derive(Clone, Debug, Default, Component)]
pub struct WeaponPart {
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
        switch,
        yaw,
        pitch,
    }
}

pub fn tick(w: &mut World, input: &Input, args: &Options) {
    let now = w.seconds() as f32;
    if args.range && now >= training::DURATION {
        mouse_look(w, args, false);
        weapons::effects(w);
        publish(w, args, input.viewport());
        return;
    }
    let over_until = w.resource::<Round>().over_until;
    if over_until > 0.0 {
        if now >= over_until {
            w.resource_mut::<Round>().over_until = 0.0;
            round::next_round(w);
        } else {
            mouse_look(w, args, false);
            weapons::effects(w);
            publish(w, args, input.viewport());
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
    if args.range {
        training::score(w, &hits, args.bot_count());
    }
    round::score(w, hits);
    round::respawn(w);
    if !args.range {
        round::check_win(w, args.kills_to_win());
    }
    for s in bots::snapshot(w) {
        let f = w.require::<Fighter>(s.entity).clone();
        if f.bot {
            fighter::face(w, &f.label, f.yaw);
        }
    }
    camera_follow(w, args);
    weapons::effects(w);
    publish(w, args, input.viewport());
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

/// The camera sits at the player's eye; the viewmodel kicks with recoil and
/// comes to the centre when aiming down sights.
pub fn camera_follow(w: &mut World, args: &Options) {
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
    let swing = (now - f.swing_at).clamp(0.0, 0.3) / 0.3;
    for (_, (vm, t, visible)) in w
        .query::<(&WeaponPart, &mut Transform, &mut Visible)>()
        .iter()
    {
        let show = f.alive && vm.weapon == f.weapon;
        if visible.0 != show {
            visible.0 = show;
        }
        let mut at = vm.rest + Vec3::new(0.0, 0.0, 0.06 * f.kick);
        if f.aiming {
            at.x -= 0.2;
            at.y += 0.06;
        }
        if f.reload_until > 0.0 {
            at.y -= 0.12;
        }
        if vm.weapon == Weapon::Knife && swing < 1.0 {
            at +=
                Vec3::new(-0.15, 0.05, -0.15) * exact_game::math::sin(swing * std::f32::consts::PI);
        }
        if t.position != at {
            t.position = at;
        }
    }
}

pub fn contacts(w: &World, viewport: Vec2, target: Option<u32>) -> Vec<Contact> {
    let me = w.require::<Fighter>("player");
    if !me.alive {
        return Vec::new();
    }
    let eye = w.require::<Transform>("player").position + Vec3::Y * me.eye;
    let mut candidates: Vec<Contact> = w
        .query::<(&Fighter, &Transform)>()
        .iter()
        .filter(|(_, (f, _))| f.bot && f.alive)
        .filter_map(|(e, (f, t))| {
            let head = t.position + Vec3::Y * 0.66;
            let screen = w.project(head, viewport)?;
            let ray = head - eye;
            let visible = weapons::hitscan(w, eye, ray, ray.length() + 0.3, me.bit())
                .is_some_and(|hit| hit.fighter == Some(e));
            visible.then(|| Contact {
                id: f.slot,
                label: f.label.clone(),
                hp: f.hp.max(0.0).round() as u32,
                x: screen.x.round(),
                y: screen.y.round(),
                target: target == Some(f.slot),
            })
        })
        .collect();
    // The Contract's plates are 128 × 48, centred above the head with an
    // 18 px gap. Keep the anchor on that head: moving labels would also move
    // the visible point an agent or player uses to aim. Prefer the drill's
    // target, then the head nearest the crosshair, with slot as a stable tie.
    let distance = |c: &Contact| (Vec2::new(c.x, c.y) - viewport * 0.5).length_squared();
    candidates.sort_by(|a, b| {
        b.target
            .cmp(&a.target)
            .then_with(|| distance(a).total_cmp(&distance(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut shown: Vec<Contact> = Vec::new();
    for c in candidates {
        if c.x < 68.0 || c.x > viewport.x - 68.0 || c.y < 70.0 {
            continue;
        }
        if shown
            .iter()
            .all(|other| (c.x - other.x).abs() >= 134.0 || (c.y - other.y).abs() >= 54.0)
        {
            shown.push(c);
        }
    }
    shown.sort_by_key(|c| c.id);
    shown
}

pub fn publish(w: &World, args: &Options, viewport: Vec2) {
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
    let drill = args.range.then(|| w.resource::<training::Drill>().clone());
    let drill_done = args.range && now >= training::DURATION;
    let incoming = r
        .incoming
        .as_ref()
        .filter(|hit| hit.until > now && me.alive && !r.over(now) && !drill_done)
        .map(|hit| hit.bearing(w.require::<Transform>("player").position, me.yaw));
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
        to_win: args.kills_to_win(),
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
        heading: round::heading(me.yaw),
        incoming: incoming.map_or_else(String::new, |(_, name)| format!("Hit from {name}")),
        incoming_angle: incoming.map_or(0.0, |(angle, _)| angle),
        over: r.over(now),
        winner: r.winner.clone(),
        you_won: r.winner == "player",
        sliding: me.sliding(now),
        sprinting: false,
        accuracy: (me.hits * 100).checked_div(me.shots).unwrap_or(0),
        feed: r.feed.clone(),
        marks: r.marks.clone(),
        contacts: if drill_done || r.over(now) {
            Vec::new()
        } else {
            contacts(w, viewport, drill.as_ref().map(|d| d.target))
        },
        training: args.range,
        drill_done,
        drill_left: exact_game::math::ceil((training::DURATION - now).max(0.0)) as u32,
        drill_score: drill.as_ref().map_or(0, |d| d.score),
        drill_combo: drill.as_ref().map_or(0, |d| d.combo),
        drill_target: drill
            .as_ref()
            .map_or_else(String::new, |d| format!("bot-{}", d.target - 1)),
        drill_cleared: drill.as_ref().map_or(0, |d| d.cleared),
    };
    drop(r);
    w.publish_record(&hud);
}
