//! The player's body and needs, the things they carry, and the lost children.
use crate::camp::{Fire, MAX_FUEL};
use crate::forest::{self, height, Grove};
use crate::rig::{self, Gait};
use exact_game::motion::{Gravity, Move};
use exact_game::*;
use exact_game_physics::{self as physics, CapsuleController};

pub const PACK: usize = 5;
const CHOP_REACH: f32 = 1.4;
const PICK_REACH: f32 = 1.8;
const FIRE_REACH: f32 = 3.6;
const FLASH_RANGE: f32 = 18.0;

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

fn item_look(kind: Kind) -> (Mesh, Material, f32) {
    match kind {
        Kind::Log => (Mesh::asset("log.model"), Material::default(), 0.16),
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
    let rotation = Quat::from_rotation_y(x * 1.7 + z);
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
            Gait::new(1.5),
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
    // The body turns on a child pivot at the feet: the capsule itself stays upright.
    let avatar = w.spawn_named(
        "avatar",
        (
            Parent(player),
            Transform::at(0.0, -0.9, 0.0).looking_at(Vec3::new(0.0, -0.9, -1.0), Vec3::Y),
            Mesh::asset("survivor_body.model"),
        ),
    );
    survivor_limbs(w, avatar, player);
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
    let flashlight = w.resolve("flashlight").unwrap();
    w.spawn_named(
        "beam",
        (
            Parent(flashlight),
            Transform::default(),
            Mesh::asset("beam.model"),
            Visible(false),
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
                .offset(0.0, 12.0, 10.5)
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
        let child = w.spawn_named(
            format!("child-{}", k + 1),
            (
                Transform::at(x, height(x, z), z).with_scale(0.62),
                Mesh::asset("survivor_body.model"),
                Material::rgb(0.45, 0.65, 1.0),
                Child::default(),
                Gait::new(1.0),
            ),
        );
        survivor_limbs(w, child, child);
    }
}

/// Arms swing against the legs; both hang from pivots on the survivor model.
fn survivor_limbs(w: &mut World, body: Entity, owner: Entity) {
    use std::f32::consts::PI;
    for side in [-1.0f32, 1.0] {
        let phase = if side < 0.0 { 0.0 } else { PI };
        rig::limb(
            w,
            body,
            owner,
            "survivor_leg.model",
            Vec3::new(side * 0.1, 0.84, 0.0),
            phase,
            0.55,
            5.0,
        );
        rig::limb(
            w,
            body,
            owner,
            "survivor_arm.model",
            Vec3::new(side * 0.25, 1.42, 0.0),
            phase + PI,
            0.45,
            5.0,
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
    pub fn prompt(self) -> String {
        match self {
            Action::None => String::new(),
            Action::Feed => "E: feed the fire".into(),
            Action::Take(_, kind) => format!("E: pick up {}", kind.label()),
            Action::Rescue(_) => "E: take the child".into(),
            Action::Chop(_) => "E: chop".into(),
        }
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
        // Strapped across the top of the backpack, one above another.
        t.position = Vec3::new(0.0, 1.5 + k as f32 * 0.3, -0.3);
        t.rotation = Quat::IDENTITY;
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
            let avatar = w.resolve("avatar").unwrap();
            w.insert(e, Parent(avatar));
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
                p.hunger = (p.hunger + 35.0).min(100.0);
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
    w.require_mut::<Gait>("player")
        .walk(planar.length() * dt, dt);
    let turn = Quat::from_rotation_y(math::atan2(p.facing.x, p.facing.z));
    {
        let mut avatar = w.require_mut::<Transform>("avatar");
        if avatar.rotation != turn {
            avatar.rotation = turn;
        }
    }
    p.cooldown = (p.cooldown - dt).max(0.0);
    if p.dead {
        return;
    }
    p.hunger = (p.hunger - dt * 0.45).max(0.0);
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
    w.require_mut::<SpotLight>("flashlight").intensity = if on { 500_000.0 } else { 0.0 };
    if w.require::<Visible>("beam").0 != on {
        w.require_mut::<Visible>("beam").0 = on;
    }
}

/// Lost children wait; followers trail the player and are rescued in the light.
pub fn children(w: &World, player: Vec3, safe: f32) -> u32 {
    let dt = w.dt();
    let g = w.resource::<Grove>();
    let mut rescued = 0;
    for (_, (pose, child, gait)) in w.query::<(&mut Transform, &mut Child, &mut Gait)>().iter() {
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
                    let next = Vec3::new(x, height(x, z), z);
                    gait.walk(
                        Vec3::new(next.x - pose.position.x, 0.0, next.z - pose.position.z).length(),
                        dt,
                    );
                    if vx * vx + vz * vz > 1e-4 {
                        pose.rotation = Quat::from_rotation_y(math::atan2(vx, vz));
                    }
                    pose.position = next;
                } else {
                    gait.walk(0.0, dt);
                }
                if Vec3::new(pose.position.x, 0.0, pose.position.z).length() < safe.max(3.0) {
                    child.fate = Fate::Rescued;
                }
            }
            Fate::Rescued => {}
        }
        if child.fate == Fate::Rescued {
            rescued += 1;
        }
    }
    rescued
}
