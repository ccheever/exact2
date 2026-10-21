//! The world's half of presentation state: rows cleared before each present,
//! and the guards that keep ticks and presents on their own side.
//! @ref llp/1046.008-game-presentation-seams.plan.md#amendment-2026-10-04-derived-presentation-state-and-gamerender
use super::*;

impl World {
    /// Retained presentation payload pages and their index metadata, in bytes.
    /// Excludes registry entries and borrow-lease bookkeeping. Empty payload pages
    /// are reclaimed after 60 consecutive completed presents leave them unused.
    pub fn presentation_bytes(&self) -> usize {
        self.registry
            .iter()
            .filter(|(_, r)| r.presentation)
            .filter_map(|(name, _)| self.components.get(name))
            .map(|s| s.retained_bytes())
            .sum()
    }

    /// A presentation component with rows, if any (setup must leave them to present).
    pub(crate) fn presentation_written(&self) -> Option<&'static str> {
        self.registry
            .iter()
            .filter(|(_, registration)| registration.presentation)
            .map(|(name, _)| *name)
            .find(|name| self.components.get(name).is_some_and(|s| s.len() > 0))
    }
    /// Erase every presentation row, so `Game::present` rebuilds them all.
    pub(crate) fn clear_presentation(&mut self) {
        self.leases.restructure();
        for (name, registration) in &self.registry {
            if registration.presentation {
                if let Some(storage) = self.components.get_mut(name) {
                    storage.clear_presentation();
                }
            }
        }
    }
    pub(crate) fn present_each<C: Component, P: crate::PresentationComponent>(
        &mut self,
        derive: &mut impl FnMut(&crate::PresentRead<'_>, Entity, &C) -> Option<P>,
    ) {
        self.leases.restructure();
        self.register::<P>();
        self.components
            .entry(P::NAME)
            .or_insert_with(|| storage::make::<P>(P::NAME, self.presentation_epoch.clone()));
        let mut next = 0;
        while let Some(index) = self.storage::<C>().and_then(|s| s.next_index(next)) {
            next = index + 1;
            let slot = &self.state.slots[index];
            let e = Entity {
                index: index as u32,
                generation: slot.generation,
            };
            let value = {
                let value = self.get::<C>(e).expect("present source row");
                derive(&crate::PresentRead { world: self }, e, &value)
            };
            if let Some(value) = value {
                if P::accepts(self, e) {
                    self.components
                        .get_mut(P::NAME)
                        .unwrap()
                        .any_mut()
                        .downcast_mut::<Storage<P>>()
                        .unwrap()
                        .insert(index, value);
                }
            }
        }
    }
    /// Game::present writes presentation components only; anything else it
    /// changed would be simulation state a restore runs it over again.
    #[inline]
    pub(crate) fn sim_writes(&self, what: std::fmt::Arguments<'_>) {
        // Never cleared here: a caught panic leaves the guard armed, so a
        // present that failed cannot write simulation state on a retry.
        if self.presenting.get() {
            panic!("Game::present changed simulation state ({what}); present may only insert, change or remove presentation components");
        }
    }
    /// A tick never sees presentation state: `Game::present` rebuilds it after
    /// the tick, so a value read there would differ after a restore.
    #[inline]
    pub(crate) fn sim_reads<C: Component>(&self) {
        if C::PRESENTATION && self.in_tick {
            panic!(
                "a tick read or wrote presentation component `{}`; presentation state is written by Game::present and read by renderers, hooks and agents, never by the simulation",
                C::NAME
            );
        }
    }
}
