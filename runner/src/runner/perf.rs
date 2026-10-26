//! The runner's half of `perf` (LLP 1079 D1): the switch a host turns once,
//! the transaction sequence, and the receipt tally.
use super::*;
use crate::perf::{Failures, Handler, SiteWork, ALERTS};

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
            self.ids.work.since = self.now_ms();
        }
        self.ids.work.moves = moves;
        self.begin_resource_inspection();
    }

    /// [`Runner::measure`] as a host holding its baked `compat.json` decides
    /// it: on unless the bake's trust is production (none: a development run).
    pub fn measure_unless_production(&mut self, compat: Option<&str>, moves: bool) {
        self.measure(!compat.is_some_and(crate::delivery::production), moves);
    }

    /// Measure from the host's first ask ([`Runner::measure_from_now`])
    /// rather than from boot, unless the bake's trust is production: until
    /// then a `perf` read says so, not that this build measures nothing.
    pub fn measure_later(&mut self, compat: Option<&str>) {
        self.ids.work.later = !compat.is_some_and(crate::delivery::production);
    }

    /// [`Runner::measure_unless_production`] begun after boot, at a host's
    /// first ask for `perf`: the transactions before it were not measured,
    /// so the next batch trailer's `seq` ([`Runner::seq_range`]) starts after
    /// them. Once measuring, a call changes nothing.
    pub fn measure_from_now(&mut self, compat: Option<&str>, moves: bool) {
        let began = self.ids.work.sites.is_empty();
        self.measure_unless_production(compat, moves);
        if began && !self.ids.work.sites.is_empty() {
            self.ids.work.reported.set(self.batch);
        }
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

    /// Declare that this host reports every kernel layout entry while measuring.
    pub fn measure_kernel_layouts(&mut self) {
        if !self.ids.work.sites.is_empty() {
            self.ids.work.kernel_layouts.get_or_insert(0);
        }
    }

    /// Record entry into a kernel layout operation, including failed attempts.
    /// Hosts call at the operation, never infer it from a receipt or moved nodes.
    pub fn count_kernel_layout(&mut self) {
        if !self.ids.work.sites.is_empty() {
            let count = self.ids.work.kernel_layouts.get_or_insert(0);
            *count = count.saturating_add(1);
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

    /// A failure line (LLP 1142 D4): in the journal, as [`Runner::log`]
    /// writes it, and while measuring also queued for the host, which writes
    /// it to its platform's log (the unified log, stderr, the console).
    pub fn alert(&mut self, line: impl Into<String>) {
        let line = line.into();
        if !self.ids.work.sites.is_empty() {
            let mut queue = self.ids.work.alerts.borrow_mut();
            queue.0.push_back(line.clone());
            if queue.0.len() > ALERTS {
                queue.0.pop_front();
                queue.1 += 1;
            }
        }
        self.log(line);
    }

    /// The alert lines queued since the last call, oldest first, each once:
    /// a batch carries them (`alerts`). Lines dropped past the bound are
    /// counted in a first line of their own.
    pub fn take_alerts(&self) -> Vec<String> {
        let mut queue = self.ids.work.alerts.borrow_mut();
        let dropped = std::mem::take(&mut queue.1);
        let mut lines: Vec<String> = queue.0.drain(..).collect();
        if dropped > 0 {
            lines.insert(
                0,
                format!("{dropped} earlier alert lines were dropped; `logs` has the journal"),
            );
        }
        lines
    }

    /// Resource `i`'s answer was refused (D4): counted while measuring.
    pub(super) fn count_failure(&mut self, i: usize) {
        if self.ids.work.sites.is_empty() {
            return;
        }
        let (n, now) = (self.plan.resources.len(), self.now_ms);
        let failures = &mut self.ids.work.failures;
        if failures.len() < n {
            failures.resize(n, Failures::default());
        }
        let f = &mut failures[i];
        if f.count == 0 {
            f.first = now;
        }
        f.count += 1;
        f.last = now;
    }

    /// Each resource refused since measuring began, by name: `state.failures`.
    pub fn failure_counts(&self) -> Vec<(&str, Failures)> {
        let work = &self.ids.work;
        work.failures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.count > 0)
            .map(|(i, f)| (self.plan.str(self.plan.resources[i].name), *f))
            .collect()
    }

    /// A dispatch to `view` (D6): counted by its plan site and `event`,
    /// and as committed when its receipt changed the tree.
    pub(super) fn count_dispatch(&mut self, view: ViewId, event: &'static str, committed: bool) {
        if self.ids.work.sites.is_empty() {
            return;
        }
        let Some(site) = self.ids.site(view) else {
            return;
        };
        let h: &mut Handler = self.ids.work.handlers.entry((site.0, event)).or_default();
        h.dispatched += 1;
        h.committed += u64::from(committed);
    }
}
