//! The world's half of presentation state: the rows the last present wrote,
//! who wrote each, and the guards that keep ticks and presents on their own side.
//! @ref llp/1046.008-game-presentation-seams.plan.md#amendment-2026-10-04-derived-presentation-state-and-gamerender
use super::*;
use std::panic::Location;

/// No writer: the row is absent, or was written outside a present.
pub(crate) const NOBODY: u32 = 0;
/// The first derivation's id; below it, the frame's two parities.
const FIRST_DERIVATION: u32 = 3;

/// What the presents since the last full one wrote, so the next changes only
/// what differs (`Present`). Never saved or hashed; a new or loaded world has
/// none, and its first present is full.
#[derive(Default)]
pub(crate) struct Presented {
    /// Presents since the last full one. A row written outside `each` carries
    /// the parity of the present that wrote it ([`Presented::frame_writer`]).
    pub(crate) count: u64,
    /// Rows this present wrote outside `each`, and the rows the last one did.
    pub(crate) frame: Vec<(&'static str, Entity)>,
    pub(crate) previous: Vec<(&'static str, Entity)>,
    /// The writer of each presentation row, by component then entity index;
    /// meaningful only while the row exists.
    owners: Vec<(&'static str, Vec<u32>)>,
    pub(crate) derivations: Vec<Derivation>,
}

/// One `Present::each` call site's kept rows.
pub(crate) struct Derivation {
    pub(crate) at: &'static Location<'static>,
    pub(crate) id: u32,
    /// Each key's revision when it last derived.
    pub(crate) since: Vec<u64>,
    /// Every component it has written.
    pub(crate) components: Vec<&'static str>,
    /// Entities whose rows follow the time: derived again at every present.
    pub(crate) animated: Vec<Entity>,
    pub(crate) called: bool,
}

impl Presented {
    /// The writer id of rows written outside `each` in present number `count`.
    pub(crate) fn frame_writer(count: u64) -> u32 {
        1 + (count % 2) as u32
    }
    pub(crate) fn owner(&self, name: &'static str, index: usize) -> u32 {
        self.owners
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, o)| o.get(index).copied())
            .unwrap_or(NOBODY)
    }
    pub(crate) fn set_owner(&mut self, name: &'static str, index: usize, owner: u32) {
        let at = match self.owners.iter().position(|(n, _)| *n == name) {
            Some(at) => at,
            None => {
                self.owners.push((name, Vec::new()));
                self.owners.len() - 1
            }
        };
        let owners = &mut self.owners[at].1;
        if owners.len() <= index {
            owners.resize(index + 1, NOBODY);
        }
        owners[index] = owner;
    }
    /// A new derivation's id: one past every derivation since the last full present.
    pub(crate) fn next_id(&self) -> u32 {
        self.derivations
            .iter()
            .map(|d| d.id + 1)
            .max()
            .unwrap_or(FIRST_DERIVATION)
    }
    /// Where the derivation that wrote this row is called, if one did.
    pub(crate) fn derived_at(
        &self,
        name: &'static str,
        index: usize,
    ) -> Option<&'static Location<'static>> {
        let owner = self.owner(name, index);
        self.derivations
            .iter()
            .find(|d| d.id == owner)
            .map(|d| d.at)
    }
    /// Drop a derivation that this present did not call, and every row it kept.
    pub(crate) fn retire(&mut self, d: &Derivation, world: &mut World) {
        for &name in &d.components {
            let Some((_, owners)) = self.owners.iter_mut().find(|(n, _)| *n == name) else {
                continue;
            };
            for (index, owner) in owners.iter_mut().enumerate() {
                if *owner == d.id {
                    world.erase_presentation(name, index);
                    *owner = NOBODY;
                }
            }
        }
    }
}

impl World {
    /// A presentation component with rows, if any (setup must leave them to present).
    pub(crate) fn presentation_written(&self) -> Option<&'static str> {
        self.registry
            .iter()
            .filter(|(_, registration)| registration.presentation)
            .map(|(name, _)| *name)
            .find(|name| self.components.get(name).is_some_and(|s| s.len() > 0))
    }
    /// Erase every presentation row and what wrote them, so the next present
    /// rebuilds them all.
    pub(crate) fn clear_presentation(&mut self) {
        self.presented = Presented::default();
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
    /// Remove one presentation row by component name.
    pub(crate) fn erase_presentation(&mut self, name: &'static str, index: usize) {
        self.leases.restructure();
        if let Some(storage) = self.components.get_mut(name) {
            storage.remove(index);
        }
    }
    /// Whether this presentation row exists.
    pub(crate) fn has_presentation(&self, name: &'static str, index: usize) -> bool {
        self.components.get(name).is_some_and(|s| s.has(index))
    }
    /// Every presentation row's digest, by component then entity index, with
    /// the `each` call that kept it (a paranoid present compares these).
    pub(crate) fn presentation_rows(
        &self,
    ) -> Vec<(&'static str, usize, u64, Option<&'static Location<'static>>)> {
        let mut rows = Vec::new();
        for (&name, storage) in &self.components {
            if !self.registry.get(name).is_some_and(|r| r.presentation) {
                continue;
            }
            for page in 0..storage.page_count() {
                storage.digest_page(page, &mut |index, digest| {
                    rows.push((name, index, digest, self.presented.derived_at(name, index)));
                });
            }
        }
        rows
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
