//! A thirty-second target drill. Correct eliminations build a multiplier;
//! hitting another dummy breaks it. All progress lives in the saved world.
use crate::{fighter, weapons::Damage};
use exact_game::*;

pub const DURATION: f32 = 30.0;

#[derive(Clone, Debug, Default, Resource)]
pub struct Drill {
    pub target: u32,
    pub score: u32,
    pub combo: u32,
    pub cleared: u32,
}

pub fn lane(slot: u32) -> [f32; 2] {
    let i = slot.saturating_sub(2);
    [-2.0 + 2.0 * (i % 3) as f32, 12.0 - 3.0 * (i / 3) as f32]
}

pub fn place(w: &mut World, entity: Entity, slot: u32) {
    fighter::place(w, entity, lane(slot));
    w.require_mut::<fighter::Fighter>(entity).yaw = std::f32::consts::PI;
}

pub fn score(w: &World, hits: &[Damage], count: u32) {
    let mut drill = w.resource_mut::<Drill>();
    for hit in hits.iter().filter(|h| h.attacker == 1 && h.victim != 1) {
        if hit.victim != drill.target {
            drill.combo = 0;
        } else if hit.killed {
            drill.combo = (drill.combo + 1).min(5);
            drill.score += 100 * drill.combo + if hit.head { 50 } else { 0 };
            drill.cleared += 1;
            drill.target = 2 + (drill.target - 1) % count;
        }
    }
}
