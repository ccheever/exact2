//! The runner's half of `perf` (LLP 1079 D1): the switch a host turns once,
//! the transaction sequence, and the receipt tally.
use super::*;
use crate::perf::SiteWork;

impl<D: DataSource> Runner<D> {
    /// Measure each plan site's work from here on (LLP 1079 D1): what a host
    /// built with development trust says once, after boot; a production host
    /// says `false`, and each counter is then one branch on an empty table.
    /// No plan reads it. Instances already live count as created now.
    /// `moves`: the host lays out, and reports what moved through
    /// [`Runner::moved`].
    pub fn measure(&mut self, on: bool, moves: bool) {
        if !on {
            self.ids.work = Default::default();
            return;
        }
        if self.ids.work.sites.is_empty() {
            // Views already gone were never measured: set aside, not retired.
            // (Forgetting them here would also forget views a content
            // region still names.)
            let mut sites = vec![SiteWork::default(); self.plan.nodes.len()];
            for (view, node) in self.ids.sites() {
                if let Some(w) = sites.get_mut(node.0 as usize) {
                    if self.kernel.node(view).is_some() {
                        w.created += 1;
                    } else {
                        w.before += 1;
                    }
                }
            }
            self.ids.work.sites = sites;
        }
        self.ids.work.moves = moves;
    }

    /// [`Runner::measure`] as a host holding its baked `compat.json` decides
    /// it: on unless the bake's trust is production (none: a development run).
    pub fn measure_unless_production(&mut self, compat: Option<&str>, moves: bool) {
        self.measure(!compat.is_some_and(crate::delivery::production), moves);
    }

    /// The nodes a layout pass moved (its `LayoutReceipt::changed`): each
    /// measured site's `moved` counts the pass once per instance.
    pub fn moved(&mut self, changed: &[exact_kernel::NodeKey]) {
        if self.ids.work.sites.is_empty() {
            return;
        }
        for key in changed {
            let Some(view) = self.kernel.node_by_key(*key).map(|n| n.id) else {
                continue;
            };
            if let Some(site) = self.ids.site(view) {
                self.ids.work.sites[site.0 as usize].moved += 1;
            }
        }
    }

    /// Kernel transactions this runner has applied: the `seq` a `perf` read
    /// and a frame record carry (LLP 1079 D2, D3).
    pub fn seq(&self) -> u64 {
        self.batch
    }

    /// The transactions applied since the last call, while measuring: a
    /// batch trailer's `seq` (LLP 1079 D3), for a host that applies batches
    /// on another turn than it produced them. `None` when there were none.
    pub fn seq_range(&self) -> Option<(u64, u64)> {
        if self.ids.work.sites.is_empty() {
            return None;
        }
        let from = self.ids.work.reported.replace(self.batch);
        (self.batch > from).then_some((from + 1, self.batch))
    }

    pub(crate) fn ids(&self) -> &Ids {
        &self.ids
    }

    /// One transaction's receipt into the measured sites: a touched node an
    /// op of `ops` named is `authored`, any other `inherited`.
    pub(super) fn tally(&mut self, ops: &[exact_kernel::Op], touched: &[exact_kernel::NodeKey]) {
        if self.ids.work.sites.is_empty() || touched.is_empty() {
            return;
        }
        use exact_kernel::Op;
        let mut named: Vec<ViewId> = ops
            .iter()
            .filter(|op| !matches!(op, Op::CreateView { .. } | Op::AttachRoot { .. }))
            .map(Op::target)
            .collect();
        named.sort_unstable();
        named.dedup();
        for key in touched {
            let Some(view) = self.kernel.node_by_key(*key).map(|n| n.id) else {
                continue;
            };
            if let Some(site) = self.ids.site(view) {
                let w = &mut self.ids.work.sites[site.0 as usize];
                if named.binary_search(&view).is_ok() {
                    w.authored += 1;
                } else {
                    w.inherited += 1;
                }
            }
        }
    }
}
