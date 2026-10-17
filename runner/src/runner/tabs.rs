//! A navigation tab built when it is first selected, as the runner shows it
//! (LLP 1075.003 §9.14; the instance tree's half is `instance/tabs.rs`).

use super::*;
use crate::instance::tabs::CarriedPanels;

impl<D: DataSource> Runner<D> {
    /// The tab panels whose contents wait for their first selection: empty
    /// in the runner until their root selects them (LLP 1075.003 §3.7). The
    /// agent's `tree` marks each `unopened`.
    pub fn unopened_tabs(&self) -> &[ViewId] {
        self.ids.tabs.waiting()
    }

    /// Build every tab panel now and from now on, none waiting for its first
    /// selection (LLP 1075.003 §9.14): a page rendered ahead of its runtime
    /// holds every panel, which the JS target adopts at boot.
    pub fn build_every_tab(&mut self) -> Result<CommitReceipt, RunnerError> {
        self.ids.tabs.build_every_tab();
        self.update_tree(true, |tree, u| tree.open_every_tab(u))
    }

    /// The tab panels' state a reload carries: each built panel's, and what
    /// the last reload carried for one not built since.
    pub(super) fn carry_panels(&self) -> CarriedPanels {
        let carried = &self.ids.tabs.carried;
        self.tree.as_ref().map_or_else(
            || carried.clone(),
            |tree| tree.carry_panels(&self.plan, carried),
        )
    }
}
