//! The art pass's motion (`Game::present`, `art="pass"`): drawn-only state the
//! engine never saves or hashes, rebuilt from the simulation at each shown tick.
//! The greybox art pass did all of this in the tick, moving its pins; here it
//! is `Offset` and `Opacity`, so the simulation is the greybox's exactly.
//!
//! - walk cycles: survivors' and creatures' limbs swing about their pivots as
//!   fast as their owners go, and survivors turn to face it;
//! - crowns between the camera and the player fade (dithered) instead of popping,
//!   and so does undergrowth the player walks through;
//! - carried supplies ride strapped on the survivor's backpack;
//! - the flashlight's beam shows only while it is on.
//!
//! Wind in the trees is the render hooks' (render/): a custom vertex material on
//! the GPU, for every tree in sight, at no cost here.
use crate::art::Part;
use crate::creatures::{Deer, Mind, Pack, Wolf};
use crate::forest::{Grove, CELL};
use crate::player::{Child, Fate, Item, Player};
use exact_game::*;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Crowns within this distance of the player may fade in front of them.
const REACH: f32 = 30.0;
/// A crown fading between the camera and the player keeps this much coverage.
const FADED: f32 = 0.3;

pub fn present(p: &mut Present<'_>) {
    let Some(player) = p.named("player") else {
        return;
    };
    let you = p.require::<Player>(player).clone();
    let at = p.require::<Transform>(player).position;
    let t = p.seconds() as f32;
    parts(p, t, player, &you, at);
    supplies(p, &you, at);
    crowns(p, at);
    undergrowth(p, at);
    if let Some(beam) = p.named("beam") {
        p.insert(beam, Opacity(if you.flashlight { 1.0 } else { 0.0 }));
    }
}

/// How fast an owner goes now (m/s), its full stride speed and its cadence
/// (rad/s). Cadence is constant within a gait, so a limb's phase never jumps
/// while its owner speeds up or slows down; only its swing grows and shrinks.
fn gait(p: &Present<'_>, owner: Entity, player: Entity, you: &Player, at: Vec3) -> (f32, f32, f32) {
    if owner == player {
        let v = Vec3::new(you.velocity.x, 0.0, you.velocity.z).length();
        return (if you.dead { 0.0 } else { v }, 6.0, TAU * 3.75);
    }
    if let Some(child) = p.get::<Child>(owner) {
        let pose = p.require::<Transform>(owner).position;
        let d = Vec3::new(at.x - pose.x, 0.0, at.z - pose.z).length();
        let v = if child.fate == Fate::Following && d > 2.0 {
            (d - 2.0).min(6.5) * 1.2
        } else {
            0.0
        };
        return (v, 6.0, TAU * 3.5);
    }
    if let Some(deer) = p.get::<Deer>(owner) {
        let v = match deer.mind {
            Mind::Stalk => 3.0,
            Mind::Chase => 6.6,
            Mind::Flee => 7.5,
            Mind::Hidden | Mind::Windup | Mind::Stunned => 0.0,
        };
        return (v, 5.0, TAU * v.max(3.0) / 2.6);
    }
    if let Some(wolf) = p.get::<Wolf>(owner) {
        let v = match wolf.mode {
            Pack::Wander => 1.6,
            Pack::Chase => 5.4,
            Pack::Flee => 6.0,
        };
        return (v, 4.0, TAU * v / 1.3);
    }
    (0.0, 1.0, 0.0)
}

/// The way a survivor faces: the player where they walk, a following child
/// toward the player, a rescued one toward the fire, a lost one away from camp.
fn facing(p: &Present<'_>, owner: Entity, player: Entity, you: &Player, at: Vec3) -> Vec3 {
    if owner == player {
        return you.facing;
    }
    let pose = p.require::<Transform>(owner).position;
    match p.get::<Child>(owner).map(|c| c.fate) {
        Some(Fate::Following) => at - pose,
        Some(Fate::Rescued) => -pose,
        _ => pose,
    }
}

fn parts(p: &mut Present<'_>, t: f32, player: Entity, you: &Player, at: Vec3) {
    let mut all = Vec::new();
    p.for_each::<Part>(|e, part| all.push((e, *part)));
    for (e, part) in all {
        let rotation = if part.swing == 0.0 {
            let f = facing(p, part.owner, player, you, at);
            if f.x * f.x + f.z * f.z < 1e-6 {
                continue;
            }
            Quat::from_rotation_y(math::atan2(f.x, f.z))
        } else {
            let (speed, full, cadence) = gait(p, part.owner, player, you, at);
            let pace = (speed / full).min(1.0);
            Quat::from_rotation_x(part.swing * pace * math::sin(t * cadence + part.phase))
        };
        p.insert(
            e,
            Offset(Transform {
                rotation,
                ..Transform::default()
            }),
        );
    }
}

/// Carried supplies are simulated stacked on the player's back; drawn, they ride
/// strapped across the survivor's backpack, one above another, turning with it.
/// Supplies on the ground near the player lie at varied angles.
fn supplies(p: &mut Present<'_>, you: &Player, at: Vec3) {
    let turn = Quat::from_rotation_y(math::atan2(you.facing.x, you.facing.z));
    let mut looks = Vec::new();
    for (k, &e) in you.pack.iter().enumerate() {
        let (Some(local), Some(item)) = (p.get::<Transform>(e), p.get::<Item>(e)) else {
            continue;
        };
        // Logs stand along Y: lay them across the pack.
        let lie = if item.kind == crate::player::Kind::Log {
            Quat::from_rotation_z(FRAC_PI_2)
        } else {
            Quat::IDENTITY
        };
        let slot = Vec3::new(0.0, -0.9, 0.0) + turn * Vec3::new(0.0, 1.5 + k as f32 * 0.3, -0.3);
        let back = local.rotation.inverse();
        looks.push((
            e,
            Transform {
                position: back * (slot - local.position),
                rotation: back * turn * lie,
                ..Transform::default()
            },
        ));
    }
    p.for_each::<Item>(|e, item| {
        if item.carried {
            return;
        }
        let Some(pose) = p.get::<Transform>(e) else {
            return;
        };
        let to = pose.position - at;
        if to.x * to.x + to.z * to.z > 40.0 * 40.0 {
            return;
        }
        // A log lies along X (Y turned down Z): its local X is the world's up.
        let yaw = math::sin(pose.position.x * 1.7 + pose.position.z) * 3.0;
        let spin = if item.kind == crate::player::Kind::Log {
            Quat::from_rotation_x(yaw)
        } else {
            Quat::from_rotation_y(yaw)
        };
        looks.push((
            e,
            Transform {
                rotation: spin,
                ..Transform::default()
            },
        ));
    });
    for (e, offset) in looks {
        p.insert(e, Offset(offset));
    }
}

/// Crowns between the camera and the player fade.
fn crowns(p: &mut Present<'_>, at: Vec3) {
    let camera = p
        .named("camera")
        .and_then(|c| p.global(c))
        .map(|g| Vec3::from(g.translation));
    let mut near = Vec::new();
    {
        let Some(g) = p.resource::<Grove>() else {
            return;
        };
        let span = (REACH / CELL).ceil() as i32;
        let Some((ci, cj)) = g.cell_of(at.x, at.z) else {
            return;
        };
        let n = g.side as i32;
        for j in (cj - span).max(0)..=(cj + span).min(n - 1) {
            for i in (ci - span).max(0)..=(ci + span).min(n - 1) {
                let c = (j * n + i) as usize;
                if g.hp[c] > 0 {
                    near.push((g.trunk[c], g.at(c as u32), g.scale[c]));
                }
            }
        }
    }
    for (e, base, scale) in near {
        if camera.is_some_and(|eye| hides(eye, at, base, scale)) {
            p.insert(e, Opacity(FADED));
        }
    }
}

/// Undergrowth has no collision: the player walks through it, so a bush or
/// fern standing on the player fades rather than hiding them.
fn undergrowth(p: &mut Present<'_>, at: Vec3) {
    let mut over = Vec::new();
    p.for_each::<Ambient>(|e, _| {
        if let Some(pose) = p.get::<Transform>(e) {
            let to = pose.position - at;
            let reach = 0.5 + 0.8 * pose.scale.x;
            if to.x * to.x + to.z * to.z < reach * reach {
                over.push(e);
            }
        }
    });
    for e in over {
        p.insert(e, Opacity(FADED));
    }
}

/// Whether a tree's crown stands across the sight line from the camera to the player.
fn hides(eye: Vec3, at: Vec3, base: Vec3, scale: f32) -> bool {
    let (e, a, b) = (eye.xz(), at.xz(), base.xz());
    let seg = a - e;
    let len = seg.length_squared();
    if len < 1e-4 {
        return false;
    }
    let f = (b - e).dot(seg) / len;
    if !(0.0..1.0).contains(&f) {
        return false;
    }
    let off = (b - (e + seg * f)).length();
    let sight = eye.y + (at.y + 0.6 - eye.y) * f;
    off < 2.6 * scale && sight < base.y + 8.5 * scale
}
