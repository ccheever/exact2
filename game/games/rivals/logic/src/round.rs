//! Round rules: first to five kills wins the round; the dead respawn after two
//! seconds at the spawn farthest from living enemies; a round-win screen holds
//! everyone for four seconds, then the next round starts clean.
use crate::arena::SPAWNS;
use crate::fighter::{self, Fighter};
use crate::weapons::{Damage, Rocket, Weapon};
use exact_game::*;

pub const TO_WIN: u32 = 5;
pub const RESPAWN: f32 = 2.0;
pub const OVER: f32 = 4.0;
const FEED_LIFE: f32 = 6.0;
const MARK_LIFE: f32 = 0.7;

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
}

/// Respawn the dead whose timer ran out, at the safest spawn.
pub fn respawn(w: &mut World) {
    let now = w.seconds() as f32;
    let all: Vec<(Entity, u32, bool, f32, Vec3)> = w
        .query::<(&Fighter, &Transform)>()
        .iter()
        .map(|(e, (f, t))| (e, f.slot, f.alive, f.respawn_at, t.position))
        .collect();
    for &(e, slot, alive, at, _) in &all {
        if alive || now < at {
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
        w.log(format!(
            "respawn {} at {:?}",
            w.require::<Fighter>(e).label,
            best
        ));
    }
}

/// Start the round-win screen when someone reaches five kills.
pub fn check_win(w: &mut World) {
    let now = w.seconds() as f32;
    if w.resource::<Round>().over(now) {
        return;
    }
    let winner = w
        .query::<&Fighter>()
        .iter()
        .find(|(_, f)| f.kills >= TO_WIN)
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
pub fn next_round(w: &mut World, duel: bool) {
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
        let spawn = if duel {
            SPAWNS[i % 2]
        } else {
            SPAWNS[i % SPAWNS.len()]
        };
        fighter::place(w, e, spawn);
    }
    let mut r = w.resource_mut::<Round>();
    r.number += 1;
    r.winner.clear();
    r.feed.clear();
    r.marks.clear();
    r.log.clear();
}
