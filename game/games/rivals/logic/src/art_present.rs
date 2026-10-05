//! The art pass's motion and flashes: rebuilt from nothing at every presented
//! tick by `Game::present` from what the simulation saved (a fighter's state,
//! when it was last hit or fired), never saved or hashed. The weapon's bob,
//! recoil, reload, inspect and aim are `Offset`s; which weapon shows and the
//! muzzle flashes are `Opacity`; a soldier's facing, flinch and fall are an
//! `Offset`; its team colour and hit flash are `MaterialOverrides`.
use crate::art::{BotFlash, Fx, Held, Joint, Limb, Part, Soldier, TEAMS};
use crate::fighter::{self, Fighter, BANDAGE_TIME};
use crate::presentation::Feedback;
use crate::round::RESPAWN;
use crate::weapons::{Effect, Weapon};
use exact_game::*;
use std::f32::consts::PI;

pub fn present(p: &mut Present<'_>) {
    weapons(p);
    soldiers(p);
    // The fight's swelling blast sphere thins out as it grows: the pooled
    // fireball, smoke and flash (art.rs) carry the explosion.
    let now = p.seconds() as f32;
    let mut blasts: Vec<(Entity, f32)> = Vec::new();
    p.for_each::<Effect>(|e, fx| {
        if fx.grow > 0.0 {
            blasts.push((e, fx.until - now));
        }
    });
    for (e, left) in blasts {
        p.insert(e, Opacity(0.8 * (left / 0.25).clamp(0.0, 1.0)));
    }
}

fn pose(a: Affine3A) -> Transform {
    let (scale, rotation, position) = a.to_scale_rotation_translation();
    Transform {
        position,
        rotation,
        scale,
    }
}
fn affine(t: &Transform) -> Affine3A {
    Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The first-person weapon: every part of the held one is drawn through one
/// camera-space motion `gun`, about the weapon's grip, so the magazine and
/// the flash stay on the gun while it bobs, kicks, tilts or turns over.
fn weapons(p: &mut Present<'_>) {
    let Some(player) = p.named("player") else {
        return;
    };
    let (tick, hz, now) = (p.tick(), p.hz() as f32, p.seconds() as f32);
    let age = |t: Option<u64>| t.map_or(f32::MAX, |t| tick.saturating_sub(t) as f32 / hz);
    let f = p.require::<Fighter>(player).clone();
    let feedback = p
        .resource::<Feedback>()
        .map(|f| f.clone())
        .unwrap_or_default();
    let inspect_tick = p.resource::<Fx>().and_then(|fx| fx.inspect_tick);
    let shot = if feedback.weapon == f.weapon {
        age(feedback.shot_tick)
    } else {
        f32::MAX
    };
    let mut parts: Vec<(Entity, Held, Transform)> = Vec::new();
    p.for_each::<Held>(|e, h| parts.push((e, h.clone(), Transform::default())));
    for (e, _, rest) in &mut parts {
        *rest = *p.require::<Transform>(*e);
    }
    let grip = parts
        .iter()
        .find(|(_, h, _)| h.weapon == f.weapon && h.part == Part::Body)
        .map_or(Vec3::ZERO, |(_, _, t)| t.position);

    let speed = if f.alive {
        (f.planar.length() / fighter::SPRINT).min(1.0)
    } else {
        0.0
    };
    let mut shift = Vec3::new(
        0.006 * math::cos(now * 7.0),
        0.009 * math::sin(now * 14.0),
        0.0,
    ) * speed
        + Vec3::new(0.0, 0.0, 0.05 * f.kick);
    let mut turn = Quat::from_rotation_x(0.12 * f.kick);
    if f.aiming {
        shift += match f.weapon {
            Weapon::Rocket => Vec3::new(-0.2, 0.045, 0.02),
            _ => Vec3::new(-0.12, 0.068, 0.06),
        };
    }
    if f.bandage_until > 0.0 {
        let begun = now - (f.bandage_until - BANDAGE_TIME);
        shift.y -= 0.25 * ease(begun / 0.15);
    }
    let reload = if f.reload_until > 0.0 {
        f.reload_progress(now)
    } else {
        1.0
    };
    if reload < 1.0 {
        let s = math::sin(PI * reload);
        shift.y -= 0.05 * s;
        turn *= Quat::from_rotation_z(0.45 * s);
    }
    let inspecting = inspect_tick
        .filter(|t| f.alive && f.reload_until == 0.0 && feedback.shot_tick.is_none_or(|s| s < *t));
    let inspect = (age(inspecting) / 1.6).min(1.0);
    if inspect < 1.0 {
        turn *= Quat::from_rotation_y(0.9 * math::sin(PI * inspect))
            * Quat::from_rotation_z(0.6 * math::sin(2.0 * PI * inspect));
    }
    let swing = (now - f.swing_at).clamp(0.0, 0.3) / 0.3;
    if f.weapon == Weapon::Knife && swing < 1.0 {
        let s = math::sin(swing * PI);
        shift += Vec3::new(-0.15, 0.05, -0.15) * s;
        turn *= Quat::from_rotation_y(0.9 * s);
    }
    let gun = Affine3A::from_translation(grip + shift)
        * Affine3A::from_quat(turn)
        * Affine3A::from_translation(-grip);

    for (e, h, rest) in parts {
        let held = f.alive && h.weapon == f.weapon;
        let mut shown = held;
        let mut local = affine(&rest);
        match h.part {
            Part::Body => {}
            // The magazine drops out of sight and a fresh one rises in.
            Part::Magazine if reload < 1.0 => {
                let drop = if reload < 0.5 {
                    0.4 * (reload / 0.5) * (reload / 0.5)
                } else {
                    0.25 * (1.0 - (reload - 0.5) / 0.5)
                };
                shown &= !(0.3..0.55).contains(&reload);
                local = Affine3A::from_translation(Vec3::new(0.0, -drop, 0.0)) * local;
            }
            Part::Magazine => {}
            Part::Flash => {
                shown &= shot < 0.045;
                let size = 1.0 - 0.5 * (shot / 0.045).clamp(0.0, 1.0);
                local = local
                    * Affine3A::from_scale_rotation_translation(
                        Vec3::splat(size),
                        Quat::from_rotation_z(f.shots as f32 * 1.7),
                        Vec3::ZERO,
                    );
            }
        }
        let offset = affine(&rest).inverse() * gun * local;
        p.insert(e, Offset(pose(offset)));
        p.insert(e, Opacity(if shown { 1.0 } else { 0.0 }));
    }
}

/// `rot(x, y, z)`: yaw, then pitch, then roll, as the soldier's poses are authored.
/// A hanging limb swings forward with positive x; a knee bends with negative x.
fn rot(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_rotation_y(y) * Quat::from_rotation_x(x) * Quat::from_rotation_z(z)
}
/// A soldier's pose at gait phase `phase` (radians) and running weight `run`
/// (0 stands, 1 runs at full speed), with `breath` the idle sway: arms hold the
/// rifle forward, legs stride, the hips bob twice a stride.
fn limb(joint: Joint, phase: f32, run: f32, breath: f32) -> Transform {
    let (s, c) = (math::sin(phase), math::cos(phase));
    let rotation = match joint {
        Joint::Hips => Quat::IDENTITY,
        Joint::Spine => rot(
            -0.12 * run - 0.03 * breath * (1.0 - run),
            0.08 * run * c,
            0.0,
        ),
        Joint::Head => rot(0.12 * run + 0.03 * breath * (1.0 - run), 0.0, 0.0),
        Joint::ArmR => rot(1.25, 0.15, 0.05),
        Joint::ArmL => rot(1.3 + 0.15 * run * c + 0.02 * breath, -0.55, -0.25),
        Joint::ForeR => rot(0.35, 0.0, 0.0),
        Joint::ForeL => rot(0.55, 0.0, 0.0),
        Joint::Gun => Quat::IDENTITY,
        Joint::LegL => rot(0.7 * run * c, 0.0, 0.0),
        Joint::LegR => rot(-0.7 * run * c, 0.0, 0.0),
        Joint::ShinL => rot(-(0.2 + 0.7 * s.max(0.0)) * run, 0.0, 0.0),
        Joint::ShinR => rot(-(0.2 + 0.7 * (-s).max(0.0)) * run, 0.0, 0.0),
    };
    let lift = if joint == Joint::Hips {
        0.03 * run * math::cos(2.0 * phase) - 0.01 * breath * (1.0 - run)
    } else {
        0.0
    };
    Transform {
        position: Vec3::new(0.0, lift, 0.0),
        rotation,
        ..Transform::default()
    }
}

/// Soldiers face where their bot aims, run at its speed, flinch when hit, fall
/// when killed and fade out before they respawn; armour takes the team colour
/// and flashes on a hit.
fn soldiers(p: &mut Present<'_>) {
    let (tick, hz, now) = (p.tick(), p.hz() as f32, p.seconds() as f32);
    let age = |t: Option<u64>| t.map_or(f32::MAX, |t| tick.saturating_sub(t) as f32 / hz);
    let mut fighters: Vec<Fighter> = Vec::new();
    p.for_each::<Fighter>(|_, f| fighters.push(f.clone()));
    let mut soldiers: Vec<(Entity, Soldier)> = Vec::new();
    p.for_each::<Soldier>(|e, s| soldiers.push((e, s.clone())));
    let mut limbs: Vec<(Entity, Limb)> = Vec::new();
    p.for_each::<Limb>(|e, l| limbs.push((e, l.clone())));
    let mut flashes: Vec<(Entity, u32)> = Vec::new();
    p.for_each::<BotFlash>(|e, b| flashes.push((e, b.slot)));
    let feet = Vec3::new(0.0, fighter::HEIGHT / 2.0, 0.0);
    for (e, s) in &soldiers {
        let Some(f) = fighters.iter().find(|f| f.slot == s.slot) else {
            continue;
        };
        let mut tilt = 0.0;
        let mut opacity = 1.0;
        if f.alive {
            let hit = age(s.hit_tick);
            if hit < 0.3 {
                tilt = 0.22 * math::sin(PI * hit / 0.3);
            }
        } else {
            let fallen = ((now - (f.respawn_at - RESPAWN)) / 0.4).clamp(0.0, 1.0);
            tilt = 1.45 * fallen * fallen;
            opacity = ((f.respawn_at - now) / 0.6).clamp(0.0, 1.0);
        }
        let turn = Quat::from_rotation_y(f.yaw) * Quat::from_rotation_x(tilt);
        let offset = Affine3A::from_translation(-feet)
            * Affine3A::from_quat(turn)
            * Affine3A::from_translation(feet);
        p.insert(*e, Offset(pose(offset)));
        if opacity < 1.0 {
            p.insert(*e, Opacity(opacity));
        }
    }
    for (e, l) in limbs {
        let Some(f) = fighters.iter().find(|f| f.slot == l.slot) else {
            continue;
        };
        let hit = soldiers
            .iter()
            .find(|(_, s)| s.slot == l.slot)
            .and_then(|(_, s)| s.hit_tick);
        // A stride every 0.6 s, offset per bot so a crowd does not march in step.
        let run = if f.alive {
            (f.planar.length() / fighter::WALK).min(1.0)
        } else {
            0.0
        };
        let phase = 2.0 * PI * (now / 0.6 + 0.37 * l.slot as f32);
        let breath = math::sin(2.0 * PI * (now / 2.4 + 0.21 * l.slot as f32));
        p.insert(e, Offset(limb(l.joint, phase, run, breath)));
        let [r, g, b] = TEAMS[(l.slot as usize).saturating_sub(2) % TEAMS.len()];
        let flash = (1.0 - age(hit) / 0.1).clamp(0.0, 1.0);
        p.insert(
            e,
            MaterialOverrides(vec![
                MaterialOverride {
                    material: 0,
                    color: Some([r, g, b, 1.0]),
                    emissive: [2.4 * flash, 0.6 * flash, 0.4 * flash],
                    ..MaterialOverride::default()
                },
                MaterialOverride {
                    material: 2,
                    color: None,
                    emissive: [r * 6.0, g * 6.0, b * 6.0],
                    ..MaterialOverride::default()
                },
            ]),
        );
    }
    for (e, slot) in flashes {
        let fighter = fighters.iter().find(|f| f.slot == slot);
        let soldier = soldiers.iter().find(|(_, s)| s.slot == slot);
        let (Some(f), Some((_, s))) = (fighter, soldier) else {
            continue;
        };
        let shown = f.alive && f.weapon != Weapon::Knife && age(s.shot_tick) < 0.05;
        p.insert(e, Opacity(if shown { 1.0 } else { 0.0 }));
        p.insert(
            e,
            Offset(Transform {
                rotation: Quat::from_rotation_z(s.shots as f32 * 1.7),
                ..Transform::default()
            }),
        );
    }
}
