//! Round rules: first to the mode's kill target wins; the dead respawn after two
//! seconds at the spawn farthest from living enemies; a round-win screen holds
//! everyone for four seconds, then the next round starts clean.
use crate::arena::SPAWNS;
use crate::fighter::{self, Fighter};
use crate::weapons::{Damage, Rocket, Weapon};
use exact_game::*;

pub const RESPAWN: f32 = 2.0;
pub const OVER: f32 = 4.0;
const FEED_LIFE: f32 = 6.0;
const MARK_LIFE: f32 = 0.7;
const INCOMING_LIFE: f32 = 2.0;

/// The last hit's origin, not the attacker's current position.
#[derive(Clone, Debug, Default, Data)]
pub struct Incoming {
    pub origin: Vec3,
    pub until: f32,
}

impl Incoming {
    /// Clockwise degrees from the player's view and a readable eight-way name.
    pub fn bearing(&self, position: Vec3, yaw: f32) -> (f32, &'static str) {
        let to = self.origin - position;
        let forward = fighter::forward(yaw);
        let right = Vec3::new(-forward.z, 0.0, forward.x);
        let angle = exact_game::math::atan2(to.dot(right), to.dot(forward));
        let sector = (angle / std::f32::consts::FRAC_PI_4).round() as i32;
        let name = [
            "front",
            "front-right",
            "right",
            "back-right",
            "back",
            "back-left",
            "left",
            "front-left",
        ][sector.rem_euclid(8) as usize];
        (angle.to_degrees().round(), name)
    }
}

pub fn heading(yaw: f32) -> String {
    let degrees = ((-yaw.to_degrees() / 5.0).round() as i32 * 5).rem_euclid(360);
    let name = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][((degrees + 22) / 45 % 8) as usize];
    format!("{name} · {degrees}°")
}

#[derive(Clone, Debug, Default, Data)]
pub struct Feed {
    pub id: u32,
    pub killer: String,
    pub victim: String,
    pub weapon: String,
    pub head: bool,
    pub you: bool,
    pub until: f32,
}

/// A damage number the player dealt.
#[derive(Clone, Debug, Default, Data)]
pub struct Mark {
    pub id: u32,
    pub amount: u32,
    pub head: bool,
    pub kill: bool,
    pub until: f32,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct Round {
    pub number: u32,
    pub over_until: f32,
    pub winner: String,
    pub feed: Vec<Feed>,
    pub marks: Vec<Mark>,
    pub next_id: u32,
    pub marker_until: f32,
    pub marker_head: bool,
    pub marker_kill: bool,
    pub hurt_until: f32,
    pub incoming: Option<Incoming>,
    pub killed_by: String,
    /// Every landed hit this round, for tests and the agent.
    pub log: Vec<Damage>,
}
impl Round {
    pub fn over(&self, now: f32) -> bool {
        now < self.over_until
    }
}

fn label(w: &World, slot: u32) -> String {
    crate::weapons::fighter_root(w, slot)
        .and_then(|e| w.get::<Fighter>(e).map(|f| f.label.clone()))
        .unwrap_or_default()
}

/// Score this tick's hits: kill feed, damage numbers, the player's hit marker.
pub fn score(w: &mut World, hits: Vec<Damage>) {
    let now = w.seconds() as f32;
    for hit in hits {
        let killer = label(w, hit.attacker);
        let victim = label(w, hit.victim);
        if hit.killed {
            if let Some(e) = crate::weapons::fighter_root(w, hit.victim) {
                let mut f = w.require_mut::<Fighter>(e);
                f.deaths += 1;
                f.respawn_at = now + RESPAWN;
            }
            if hit.attacker != hit.victim {
                if let Some(e) = crate::weapons::fighter_root(w, hit.attacker) {
                    w.require_mut::<Fighter>(e).kills += 1;
                }
            }
            w.log(format!(
                "kill {killer} -> {victim} ({}{})",
                hit.weapon.name(),
                if hit.head { ", headshot" } else { "" }
            ));
        }
        if hit.attacker == 1 && hit.victim != 1 {
            w.require_mut::<Fighter>("player").hits += 1;
            if hit.head {
                w.require_mut::<Fighter>("player").headshots += 1;
            }
        }
        let mut r = w.resource_mut::<Round>();
        r.next_id += 1;
        let id = r.next_id;
        if hit.killed {
            r.feed.push(Feed {
                id,
                killer: if hit.attacker == hit.victim {
                    String::new()
                } else {
                    killer.clone()
                },
                victim: victim.clone(),
                weapon: match hit.weapon {
                    Weapon::Rifle => "rifle",
                    Weapon::Rocket => "rocket",
                    Weapon::Knife => "knife",
                }
                .into(),
                head: hit.head,
                you: hit.attacker == 1 || hit.victim == 1,
                until: now + FEED_LIFE,
            });
            let excess = r.feed.len().saturating_sub(5);
            r.feed.drain(..excess);
        }
        if hit.attacker == 1 && hit.victim != 1 {
            r.marks.push(Mark {
                id,
                amount: hit.amount.round() as u32,
                head: hit.head,
                kill: hit.killed,
                until: now + MARK_LIFE,
            });
            let excess = r.marks.len().saturating_sub(6);
            r.marks.drain(..excess);
            r.marker_until = now + 0.15;
            r.marker_head = hit.head;
            r.marker_kill = hit.killed;
        }
        if hit.victim == 1 {
            r.hurt_until = now + 0.3;
            r.incoming = Some(Incoming {
                origin: hit.origin,
                until: now + INCOMING_LIFE,
            });
            if hit.killed {
                r.killed_by = if hit.attacker == 1 {
                    "yourself".into()
                } else {
                    killer.clone()
                };
            }
        }
        r.log.push(hit);
    }
    let mut r = w.resource_mut::<Round>();
    r.feed.retain(|f| f.until > now);
    r.marks.retain(|m| m.until > now);
    if r.incoming.as_ref().is_some_and(|hit| hit.until <= now) {
        r.incoming = None;
    }
}

/// Respawn the dead whose timer ran out, at the safest spawn.
pub fn respawn(w: &mut World) {
    let now = w.seconds() as f32;
    let mut all: Vec<(Entity, u32, bool, f32, Vec3)> = w
        .query::<(&Fighter, &Transform)>()
        .iter()
        .map(|(e, (f, t))| (e, f.slot, f.alive, f.respawn_at, t.position))
        .collect();
    for index in 0..all.len() {
        let (e, slot, alive, at, _) = all[index];
        if alive || now < at {
            continue;
        }
        if w.get::<crate::bots::Brain>(e).is_some_and(|b| b.dummy) {
            crate::training::place(w, e, slot);
            continue;
        }
        let best = SPAWNS
            .iter()
            .max_by(|a, b| {
                let far = |s: &[f32; 2]| {
                    all.iter()
                        .filter(|o| o.1 != slot && o.2)
                        .map(|o| Vec3::new(s[0] - o.4.x, 0.0, s[1] - o.4.z).length())
                        .fold(f32::MAX, f32::min)
                };
                far(a).total_cmp(&far(b))
            })
            .copied()
            .unwrap_or([0.0, 0.0]);
        fighter::place(w, e, best);
        // A second fighter respawning this tick must see this placement too.
        all[index].2 = true;
        all[index].4 = Vec3::new(best[0], 0.92, best[1]);
        w.log(format!(
            "respawn {} at {:?}",
            w.require::<Fighter>(e).label,
            best
        ));
    }
}

/// Start the round-win screen when someone reaches the mode's kill target.
pub fn check_win(w: &mut World, to_win: u32) {
    let now = w.seconds() as f32;
    if w.resource::<Round>().over(now) {
        return;
    }
    let winner = w
        .query::<&Fighter>()
        .iter()
        .find(|(_, f)| f.kills >= to_win)
        .map(|(e, f)| (e, f.label.clone()));
    if let Some((e, label)) = winner {
        w.require_mut::<Fighter>(e).rounds += 1;
        let mut r = w.resource_mut::<Round>();
        r.over_until = now + OVER;
        r.winner = label.clone();
        drop(r);
        w.log(format!("round won by {label}"));
        w.emit("round");
    }
}

/// Begin the next round: scores cleared, rockets gone, everyone at a spawn.
pub fn next_round(w: &mut World) {
    let rockets: Vec<Entity> = w.query::<&Rocket>().iter().map(|(e, _)| e).collect();
    for e in rockets {
        w.despawn(e);
    }
    let fighters: Vec<Entity> = w.query::<&Fighter>().iter().map(|(e, _)| e).collect();
    for (i, e) in fighters.into_iter().enumerate() {
        {
            let mut f = w.require_mut::<Fighter>(e);
            f.kills = 0;
            f.deaths = 0;
        }
        fighter::place(w, e, SPAWNS[i]);
    }
    let mut r = w.resource_mut::<Round>();
    r.number += 1;
    r.winner.clear();
    r.feed.clear();
    r.marks.clear();
    r.log.clear();
}
