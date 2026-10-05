//! The world's half of presentation state: rows cleared before each present,
//! and the guards that keep ticks and presents on their own side.
//! @ref llp/1046.008-game-presentation-seams.plan.md#amendment-2026-10-04-derived-presentation-state-and-gamerender
use super::*;

impl World {
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
        // Fast path: a game that writes no presentation rows pays one scan of
        // the registry and nothing else.
        let written: Vec<&'static str> = self
            .registry
            .iter()
            .filter(|(name, registration)| {
                registration.presentation && self.components.get(*name).is_some_and(|s| s.len() > 0)
            })
            .map(|(name, _)| *name)
            .collect();
        if written.is_empty() {
            return;
        }
        self.leases.restructure();
        for name in written {
            if let Some(storage) = self.components.get_mut(name) {
                storage.clear();
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
