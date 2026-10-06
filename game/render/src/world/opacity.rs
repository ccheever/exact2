//! Faded entities: each one's effective opacity, written when it changes.
use super::{Feed, Writes};
use exact_game::{Parent, World};

/// Each faded entity's effective opacity: the product of its own and every
/// ancestor's `Opacity`, so a multipart unit fades as one. By slot.
fn effective_opacity(w: &World, out: &mut Vec<(u32, f32)>) {
    use exact_game::Opacity;
    out.clear();
    for (e, o) in w.query::<&Opacity>().iter() {
        // Start at the topmost faded entity of each chain; its walk covers the rest.
        let mut product = exact_game::opacity(o.0);
        let mut at = e;
        let mut top = true;
        for _ in 0..=w.len() {
            let Some(p) = w.get::<Parent>(at) else { break };
            at = p.0;
            if w.has::<Opacity>(at) {
                top = false;
                break;
            }
        }
        if !top {
            continue;
        }
        let mut stack = vec![(e, std::mem::take(&mut product))];
        let mut budget = w.len() + 1;
        while let Some((x, f)) = stack.pop() {
            out.push((x.index(), f));
            budget = budget.saturating_sub(1);
            if budget == 0 {
                break;
            }
            for c in w.children(x) {
                let own = w.get::<Opacity>(c).map_or(1., |o| exact_game::opacity(o.0));
                stack.push((c, f * own));
            }
        }
    }
    out.sort_by_key(|&(slot, _)| slot);
    out.dedup_by_key(|&mut (slot, _)| slot);
}
impl Feed {
    pub(super) fn fades(&mut self, w: &World, r: &mut impl Writes, initial: bool) {
        effective_opacity(w, &mut self.fades_next);
        if initial || self.fades_next != self.fades {
            std::mem::swap(&mut self.fades, &mut self.fades_next);
            r.opacity(&self.fades);
        }
    }
}
