//! The player's body and needs, the things they carry, and the lost children.
use crate::camp::{Cycle, Fire, MAX_FUEL};
use crate::forest::{self, height, Grove};
use exact_game::motion::{Gravity, Move};
use exact_game::*;
use exact_game_physics::{self as physics, CapsuleController};

pub const PACK: usize = 5;
const CHOP_REACH: f32 = 1.4;
const PICK_REACH: f32 = 1.8;
const FIRE_REACH: f32 = 3.6;
const FLASH_RANGE: f32 = 18.0;
const HUNGER_DRAIN: f32 = 0.45;
const FOOD_HUNGER: f32 = 35.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Data)]
pub enum Kind {
    #[default]
    Log,
    Scrap,
    Food,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Log => "log",
            Kind::Scrap => "scrap",
            Kind::Food => "food",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Item {
    pub kind: Kind,
    pub carried: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Component)]
pub struct Player {
    pub facing: Vec3,
    pub velocity: Vec3,
    pub health: f32,
    pub hunger: f32,
    pub battery: f32,
    pub flashlight: bool,
    pub pack: Vec<Entity>,
    pub cooldown: f32,
    pub dead: bool,
    pub chopped: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Fate {
    #[default]
    Lost,
    Following,
    Rescued,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Child {
    pub fate: Fate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum TrailKind {
    #[default]
    Rescue,
    Fuel,
    Food,
    Camp,
}
impl TrailKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rescue => "rescue",
            Self::Fuel => "fuel",
            Self::Food => "food",
            Self::Camp => "camp",
        }
    }
}

/// A chosen landmark stays chosen while walking and across saves. Finding a
/// new supply visits the forest on selection/collection, never every frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct Trail {
    pub kind: TrailKind,
    pub target: Option<Entity>,
}

pub fn track(w: &mut World, command: &str) {
    let kind = match command {
        "track rescue" => TrailKind::Rescue,
        "track fuel" => TrailKind::Fuel,
        "track food" => TrailKind::Food,
        "track camp" => TrailKind::Camp,
        _ => return,
    };
    *w.resource_mut::<Trail>() = Trail { kind, target: None };
    update_trail(w, true);
}

pub fn update_trail(w: &mut World, refresh: bool) {
    let trail = *w.resource::<Trail>();
    if !matches!(trail.kind, TrailKind::Fuel | TrailKind::Food) {
        return;
    }
    let valid = trail.target.is_some_and(|e| {
        w.get::<Item>(e).is_some_and(|i| !i.carried) || w.get::<forest::Tree>(e).is_some()
    });
    if !refresh && (valid || trail.target.is_none()) {
        return;
    }
    let radius = w.resource::<Grove>().half * 3.0;
    let mut target = w.nearest_xz_where::<Item>("player", radius, |i| {
        !i.carried && (i.kind == Kind::Food) == (trail.kind == TrailKind::Food)
    });
    if target.is_none() && trail.kind == TrailKind::Fuel {
        let at = w.require::<Transform>("player").position;
        let grove = w.resource::<Grove>();
        target = (0..grove.hp.len())
            .filter(|&c| grove.hp[c] > 0)
            .min_by(|&a, &b| {
                let distance = |c: usize| {
                    let (x, z) = (grove.x[c] - at.x, grove.z[c] - at.z);
                    x * x + z * z
                };
                distance(a).total_cmp(&distance(b)).then(a.cmp(&b))
            })
            .map(|c| grove.trunk[c]);
    }
    w.resource_mut::<Trail>().target = target;
}

fn item_look(kind: Kind) -> (Mesh, Material, f32) {
    match kind {
        Kind::Log => (
            Mesh::cylinder(0.18, 1.1),
            Material::rgb(0.3, 0.18, 0.09),
            0.18,
        ),
        Kind::Scrap => (
            Mesh::cube(0.45),
            Material::rgb(0.45, 0.47, 0.5).metallic(0.8).rough(0.4),
            0.23,
        ),
        Kind::Food => (Mesh::sphere(0.22), Material::rgb(0.75, 0.12, 0.08), 0.22),
    }
}

pub fn drop_item(w: &mut World, kind: Kind, x: f32, z: f32) -> Entity {
    let (mesh, material, lift) = item_look(kind);
    let rotation = if kind == Kind::Log {
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    w.spawn((
        Transform {
            position: Vec3::new(x, height(x, z) + lift, z),
            rotation,
            scale: Vec3::ONE,
        },
        mesh,
        material,
        Item {
            kind,
            carried: false,
        },
    ))
}

/// Scatter scrap and food through the forest, avoiding trunks.
pub fn scatter(w: &mut World, kind: Kind, count: u32, near: f32, far: f32) {
    for _ in 0..count {
        let a = w.rand(0.0..std::f32::consts::TAU);
        let r = w.rand(near..far);
        let (s, c) = math::sin_cos(a);
        let (x, z) = w.resource::<Grove>().resolve(c * r, s * r, 0.8);
        drop_item(w, kind, x, z);
    }
}

pub fn spawn(w: &mut World, colliders: bool, children: u32) {
    let player = w.spawn_named(
        "player",
        (
            Transform::at(0.0, 0.91, 3.0),
            Mesh::capsule(0.35, 1.8),
            Material::rgb(0.85, 0.45, 0.12),
            Player {
                facing: Vec3::NEG_Z,
                health: 100.0,
                hunger: 100.0,
                battery: 100.0,
                ..Default::default()
            },
        ),
    );
    if colliders {
        w.insert(
            player,
            CapsuleController {
                radius: 0.35,
                step: 0.45,
                ..Default::default()
            },
        );
    }
    w.spawn_named(
        "flashlight",
        (
            Transform::at(0.0, 1.2, 0.0),
            SpotLight {
                color: [1.0, 0.95, 0.8],
                intensity: 0.0,
                range: FLASH_RANGE,
                inner: 0.25,
                outer: 0.5,
            },
            LightShadows,
        ),
    );
    w.spawn_named(
        "camera",
        (
            Transform::default(),
            Camera {
                far: 400.0,
                ..Camera::default()
            },
            Follow::new(player)
                .offset(0.0, 12.0, 11.0)
                .look_at_offset(0.0, 0.0, -1.5)
                .lag(0.12),
        ),
    );
    let half = w.resource::<Grove>().half;
    for k in 0..children {
        let a = w.rand(0.0..std::f32::consts::TAU);
        let r = (0.3 + 0.45 * (k as f32 / children.max(1) as f32)) * half;
        let (s, c) = math::sin_cos(a);
        let (x, z) = w.resource::<Grove>().resolve(c * r, s * r, 1.2);
        w.spawn_named(
            format!("child-{}", k + 1),
            (
                Transform::at(x, height(x, z) + 0.6, z),
                Mesh::capsule(0.25, 1.2),
                Material::rgb(0.25, 0.55, 0.95),
                Child::default(),
            ),
        );
    }
}

/// What pressing E would do now; also the HUD prompt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    Feed,
    Take(Entity, Kind),
    Rescue(Entity),
    Chop(u32),
}
impl Action {
    pub fn prompt(self, w: &World) -> String {
        let player = w.require::<Player>("player");
        if player.dead {
            return String::new();
        }
        let mut label = if player.cooldown > 0.0 {
            // Round up to tenths: the displayed wait must never end before E
            // is usable, and the label need not change on every fixed tick.
            let tenths = math::ceil(player.cooldown * 10.0) as u32;
            format!("Axe recovering · {}.{} s", tenths / 10, tenths % 10)
        } else {
            match self {
                Action::None => String::new(),
                Action::Feed => "E: feed the fire".into(),
                Action::Take(_, kind) => format!("E: pick up {}", kind.label()),
                Action::Rescue(_) => "E: take the child".into(),
                Action::Chop(_) => "Hold E: chop".into(),
            }
        };
        if let Action::Chop(cell) = self {
            let hits = w.resource::<Grove>().hp[cell as usize];
            label.push_str(&format!(
                " · {hits} hit{} left",
                if hits == 1 { "" } else { "s" }
            ));
        }
        label
    }
}

pub fn action(w: &World, at: Vec3) -> Action {
    let p = w.require::<Player>("player");
    if p.dead {
        return Action::None;
    }
    let carrying_fuel = p
        .pack
        .iter()
        .any(|&e| w.get::<Item>(e).is_some_and(|i| i.kind != Kind::Food));
    if carrying_fuel && Vec3::new(at.x, 0.0, at.z).length() < FIRE_REACH {
        return Action::Feed;
    }
    if p.pack.len() < PACK {
        if let Some(e) = w.nearest_xz_where::<Item>("player", PICK_REACH, |i| !i.carried) {
            return Action::Take(e, w.require::<Item>(e).kind);
        }
    }
    if let Some(e) = w.nearest_xz_where::<Child>("player", 2.0, |c| c.fate == Fate::Lost) {
        return Action::Rescue(e);
    }
    match w.resource::<Grove>().nearest(at.x, at.z, CHOP_REACH) {
        Some((cell, _)) => Action::Chop(cell),
        None => Action::None,
    }
}

/// Re-stack the carried items on the player's back.
fn restack(w: &mut World) {
    let pack = w.require::<Player>("player").pack.clone();
    for (k, e) in pack.into_iter().enumerate() {
        let mut t = w.require_mut::<Transform>(e);
        t.position = Vec3::new(0.0, -0.1 + k as f32 * 0.32, 0.42);
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    }
}

pub fn interact(w: &mut World, act: Action, eat: bool) {
    let player = w.resolve("player").unwrap();
    match act {
        Action::Feed => {
            let pack = std::mem::take(&mut w.require_mut::<Player>(player).pack);
            let mut keep = Vec::new();
            for e in pack {
                let kind = w.require::<Item>(e).kind;
                if kind == Kind::Food {
                    keep.push(e);
                    continue;
                }
                {
                    let mut f = w.resource_mut::<Fire>();
                    f.fuel = (f.fuel + if kind == Kind::Log { 12.0 } else { 20.0 }).min(MAX_FUEL);
                    f.fed += 1;
                }
                w.despawn(e);
            }
            w.require_mut::<Player>(player).pack = keep;
            restack(w);
            w.log("fed the fire");
        }
        Action::Take(e, _) => {
            w.require_mut::<Item>(e).carried = true;
            w.insert(e, Parent(player));
            w.require_mut::<Player>(player).pack.push(e);
            restack(w);
        }
        Action::Rescue(e) => {
            w.require_mut::<Child>(e).fate = Fate::Following;
            w.log("a child follows you");
        }
        Action::Chop(cell) => {
            let felled = {
                let mut g = w.resource_mut::<Grove>();
                let c = cell as usize;
                g.hp[c] = g.hp[c].saturating_sub(1);
                g.hp[c] == 0
            };
            w.require_mut::<Player>(player).cooldown = 0.35;
            if felled {
                // fell() expects a standing tree: restore the last blow's hp.
                w.resource_mut::<Grove>().hp[cell as usize] = 1;
                let stump = forest::fell(w, cell);
                for k in 0..2 {
                    let off = k as f32 * 1.4 - 0.7;
                    let (x, z) = w
                        .resource::<Grove>()
                        .resolve(stump.x + off, stump.z + 0.9, 0.3);
                    drop_item(w, Kind::Log, x, z);
                }
                w.require_mut::<Player>(player).chopped += 1;
            }
        }
        Action::None => {}
    }
    if eat {
        let food = {
            let p = w.require::<Player>(player);
            p.pack
                .iter()
                .copied()
                .find(|&e| w.require::<Item>(e).kind == Kind::Food)
        };
        if let Some(e) = food {
            {
                let mut p = w.require_mut::<Player>(player);
                p.pack.retain(|&x| x != e);
                p.hunger = (p.hunger + FOOD_HUNGER).min(100.0);
            }
            w.despawn(e);
            restack(w);
        }
    }
}

/// Walk, collide, and drain or restore the player's needs.
pub fn walk(w: &mut World, wish: Vec3, colliders: bool, safe: f32) {
    let dt = w.dt();
    let dead = w.require::<Player>("player").dead;
    let wish = if dead { Vec3::ZERO } else { wish };
    if colliders {
        let velocity = {
            let mut c = w.require_mut::<CapsuleController>("player");
            Move {
                speed: 6.0,
                accel: 30.0,
                brake: 40.0,
            }
            .step(&mut c.velocity, wish, dt);
            Gravity(9.81).step(&mut c.velocity, dt);
            c.velocity
        };
        physics::capsule(w, "player").step(velocity);
        let v = w.require::<CapsuleController>("player").velocity;
        w.require_mut::<Player>("player").velocity = v;
    } else {
        let mut v = w.require::<Player>("player").velocity;
        Move {
            speed: 6.0,
            accel: 30.0,
            brake: 40.0,
        }
        .step(&mut v, wish, dt);
        let at = w.require::<Transform>("player").position;
        let (x, z) = w
            .resource::<Grove>()
            .resolve(at.x + v.x * dt, at.z + v.z * dt, 0.35);
        w.require_mut::<Transform>("player").position = Vec3::new(x, height(x, z) + 0.9, z);
        w.require_mut::<Player>("player").velocity = v;
    }
    let at = w.require::<Transform>("player").position;
    let mut p = w.require_mut::<Player>("player");
    let planar = Vec3::new(p.velocity.x, 0.0, p.velocity.z);
    if planar.length() > 0.5 {
        p.facing = planar.normalize();
    }
    p.cooldown = (p.cooldown - dt).max(0.0);
    if p.dead {
        return;
    }
    p.hunger = (p.hunger - dt * HUNGER_DRAIN).max(0.0);
    let warm = Vec3::new(at.x, 0.0, at.z).length() < safe;
    if p.hunger <= 0.0 {
        p.health -= dt * 2.0;
    } else if warm && p.hunger > 40.0 {
        p.health = (p.health + dt * 1.5).min(100.0);
    }
    if p.flashlight {
        p.battery = (p.battery - dt * 2.5).max(0.0);
        if p.battery <= 0.0 {
            p.flashlight = false;
        }
    } else if warm {
        p.battery = (p.battery + dt * 4.0).min(100.0);
    }
    if p.health <= 0.0 {
        p.health = 0.0;
        p.dead = true;
    }
}

/// The flashlight: a shadowed spot at chest height along the facing, tipped down
/// to meet the ground ahead.
pub fn flashlight(w: &mut World, toggle: bool) {
    let (on, facing) = {
        let mut p = w.require_mut::<Player>("player");
        if toggle && !p.dead && (p.flashlight || p.battery > 5.0) {
            p.flashlight = !p.flashlight;
        }
        (p.flashlight, p.facing)
    };
    let at = w.require::<Transform>("player").position;
    let from = at + facing * 0.4 + Vec3::Y * 0.5;
    *w.require_mut::<Transform>("flashlight") = Transform::at(from.x, from.y, from.z)
        .looking_at(at + facing * 12.0 - Vec3::Y * 0.6, Vec3::Y);
    w.require_mut::<SpotLight>("flashlight").intensity = if on { 250_000.0 } else { 0.0 };
}

/// Lost children wait; followers trail the player and are rescued in the light.
pub fn children(w: &mut World, player: Vec3, safe: f32) -> u32 {
    let dt = w.dt();
    if w.require::<Player>("player").dead {
        return w.count::<Child>(|child| child.fate == Fate::Rescued);
    }
    let g = w.resource::<Grove>();
    let mut rescued = 0;
    let mut arrivals = 0;
    for (_, (pose, child)) in w.query::<(&mut Transform, &mut Child)>().iter() {
        match child.fate {
            Fate::Lost => {}
            Fate::Following => {
                let to = Vec3::new(player.x - pose.position.x, 0.0, player.z - pose.position.z);
                let d = to.length();
                if d > 2.0 {
                    let v = to / d * (d - 2.0).min(6.5) * 1.2;
                    let (vx, vz) = g.steer(pose.position.x, pose.position.z, v.x, v.z, 0.25);
                    let (x, z) =
                        g.resolve(pose.position.x + vx * dt, pose.position.z + vz * dt, 0.25);
                    pose.position = Vec3::new(x, height(x, z) + 0.6, z);
                }
                if safe > 0.0 && Vec3::new(pose.position.x, 0.0, pose.position.z).length() < safe {
                    child.fate = Fate::Rescued;
                    arrivals += 1;
                }
            }
            Fate::Rescued => {}
        }
        if child.fate == Fate::Rescued {
            rescued += 1;
        }
    }
    drop(g);
    if arrivals > 0 {
        let mut fire = w.resource_mut::<Fire>();
        fire.fuel = (fire.fuel + 20.0 * arrivals as f32).min(MAX_FUEL);
        drop(fire);
        for k in 0..arrivals * 2 {
            drop_item(w, Kind::Food, -2.0 - k as f32 * 0.6, 2.0);
        }
        // A previously empty food search must notice the new rescue supplies.
        update_trail(w, true);
        w.emit("rescued");
        w.log("Rescue supplies: +20 fire fuel and 2 food per child");
    }
    rescued
}

/// Bearings use the walking axes: W is north, D is east. No hidden target
/// positions are needed by a player or agent following the visible objective.
pub fn guidance(w: &World) -> String {
    let trail = *w.resource::<Trail>();
    let at = w.require::<Transform>("player").position;
    if trail.kind == TrailKind::Camp {
        return format!("Return to camp · {}", bearing(-at));
    }
    if matches!(trail.kind, TrailKind::Food | TrailKind::Fuel) {
        let p = w.require::<Player>("player");
        if p.pack.len() >= PACK {
            let fuel = p
                .pack
                .iter()
                .any(|&e| w.require::<Item>(e).kind != Kind::Food);
            return if fuel {
                format!("Pack full · Feed the fire · {}", bearing(-at))
            } else {
                "Pack full of food · Q eats when hungry".into()
            };
        }
        if let Some(target) = trail.target {
            if let Some(t) = w.get::<Transform>(target) {
                let label = w
                    .get::<Item>(target)
                    .map(|i| i.kind.label())
                    .unwrap_or("tree");
                return format!(
                    "Gather {label} · {}",
                    bearing_with_reach(t.position - at, 1.5)
                );
            }
        }
        return if trail.kind == TrailKind::Food {
            "No food found · More arrives at dawn".into()
        } else {
            "No fuel found · Return to camp".into()
        };
    }
    rescue_guidance(w)
}

pub fn camp_bearing(w: &World) -> String {
    format!(
        "Campfire · {}",
        bearing(-w.require::<Transform>("player").position)
    )
}

/// A public supply budget for the next dawn. Carried food counts as a reserve;
/// carried fuel still needs feeding. This never changes the selected compass.
pub fn preparation(w: &World, children_safe: bool) -> (String, String) {
    let c = *w.resource::<Cycle>();
    let fire = *w.resource::<Fire>();
    let p = w.require::<Player>("player");
    let food = p
        .pack
        .iter()
        .filter(|&&e| w.require::<Item>(e).kind == Kind::Food)
        .count();
    let fuel_short = (c.dawn_fuel() - fire.fuel).max(0.0);
    let food_short = math::ceil(
        ((c.until_dawn() * HUNGER_DRAIN + 10.0 - p.hunger) / FOOD_HUNGER - food as f32).max(0.0),
    ) as u32;
    let fuel_status = if fuel_short > 0.0 {
        format!("+{} fire fuel", math::ceil(fuel_short) as u32)
    } else {
        "fire ready".into()
    };
    let food_status = if food_short > 0 {
        format!("{food_short} food needed")
    } else {
        "food ready".into()
    };
    let supplies = format!("To dawn: {fuel_status} · {food_status}");
    let at = w.require::<Transform>("player").position.with_y(0.0);
    let plan = if p.dead {
        "The next dawn will wait for another attempt".into()
    } else if p.hunger <= FOOD_HUNGER && food > 0 {
        "Q: eat a carried meal before sheltering".into()
    } else if !children_safe {
        "Bring the children home for rescue supplies".into()
    } else if fuel_short > 0.0 {
        if food < p.pack.len() {
            "Feed your carried fuel into the campfire".into()
        } else {
            "Gather fuel for the next dawn".into()
        }
    } else if food_short > 0 {
        "Gather food for the next dawn".into()
    } else if at.length() >= FIRE_REACH {
        "Supplies ready · Return to camp to shelter".into()
    } else {
        format!(
            "Shelter by the fire until dawn · {} s",
            math::ceil(c.until_dawn()) as u32
        )
    };
    (plan, supplies)
}

fn rescue_guidance(w: &World) -> String {
    let at = w.require::<Transform>("player").position;
    let following = w.count::<Child>(|child| child.fate == Fate::Following);
    if following > 0 {
        return format!("Escort {following} to the fire · {}", bearing(-at));
    }
    let nearest = w
        .query::<(&Transform, &Child)>()
        .iter()
        .filter(|(_, (_, child))| child.fate == Fate::Lost)
        .map(|(_, (pose, _))| (pose.position - at).with_y(0.0))
        .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
    match nearest {
        Some(to) => format!("Find a lost child · {}", bearing(to)),
        None => "All children safe · Keep the fire burning".into(),
    }
}

fn bearing(to: Vec3) -> String {
    bearing_with_reach(to, 2.0)
}

fn bearing_with_reach(to: Vec3, reach: f32) -> String {
    let length = to.with_y(0.0).length();
    let distance = math::ceil(length) as u32;
    if length <= reach {
        return "within reach".into();
    }
    let angle = math::atan2(to.x, -to.z);
    let sector = (math::round(angle / std::f32::consts::FRAC_PI_4) as i32).rem_euclid(8);
    format!(
        "{} · {distance} m",
        ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][sector as usize]
    )
}
