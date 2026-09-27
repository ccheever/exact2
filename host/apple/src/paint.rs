//! Paint motion (LLP 1055.000 D6, LLP 1062): colour and shadow transitions
//! and keyframes, sampled by the engine like every other property.
//!
//! Ownership, `currentcolor` and per-view appearance are shared in
//! `PaintMotion`; this host presents the values as styles over the rows.

use super::Host;
use crate::batch::Batch;
use crate::style::Shown;
use exact_kernel::motion::{node_key, PaintMotion};
use exact_kernel::{CommitReceipt, NodeKey, ViewId};
use exact_motion::Property;
use exact_runner::DataSource;
use std::collections::BTreeMap;

/// The host's paint-motion state.
#[derive(Debug, Default)]
pub(super) struct Paint {
    motion: PaintMotion,
    /// Inline runs painting an inherited `color` that moves, by run: their
    /// paragraph paints them (LLP 1062 D5).
    pub(super) runs: BTreeMap<ViewId, exact_motion::Value>,
}

impl<D: DataSource> Host<D> {
    /// Adopt a commit's paint, after its `motion_sync` set the rows.
    pub(super) fn sync_paint(&mut self, receipt: &CommitReceipt, batch: &mut Batch) {
        let retired = self
            .paint
            .motion
            .sync(self.runner.kernel(), receipt, &mut self.engine);
        self.retire_paint(retired, batch);
    }

    /// Adopt the whole tree's paint at boot.
    pub(super) fn boot_paint(&mut self, order: &[ViewId]) {
        let kernel = self.runner.kernel();
        let keys: Vec<NodeKey> = order
            .iter()
            .filter_map(|id| kernel.node(*id).map(|n| n.key))
            .collect();
        self.paint.motion.adopt(kernel, keys, &mut self.engine);
    }

    fn retire_paint(&mut self, retired: Vec<(u64, Property)>, batch: &mut Batch) {
        let mut views: BTreeMap<ViewId, bool> = BTreeMap::new();
        for (node, property) in retired {
            if let Some(view) = self.keys.get(&node_key(node)).copied() {
                *views.entry(view).or_default() |= property == Property::Color;
            }
        }
        for (view, inherits) in views {
            self.present_colors(view, batch, inherits);
        }
    }

    /// What a node's paint shows now, over its rows: each owned property's
    /// value while it differs from its target. A `currentcolor` side that
    /// stays one follows the presented `color` through its row.
    pub(super) fn shown_paint(&self, node: u64) -> Shown {
        let mut shown = Shown::default();
        for property in Property::PAINT {
            shown.set(
                property,
                self.paint.motion.shown(&self.engine, node, property),
            );
        }
        shown
    }

    /// The presenter's appearance: a `light-dark()` colour an owner shows
    /// resolves by it. A change re-targets every owner, transitioning under
    /// its row; the first report only corrects boot's guess, without motion.
    pub fn set_scheme(&mut self, dark: bool) -> String {
        let mut batch = Batch::new();
        if let Some(retired) = self.paint.motion.set_scheme(
            self.runner.kernel(),
            &mut self.engine,
            dark,
            self.now_ms / 1000.0,
        ) {
            self.retire_paint(retired, &mut batch);
            self.present(&mut batch, false);
        }
        self.finish(batch, None)
    }

    /// A view's own appearance, when the presenter finds it differs from the
    /// session's (LLP 1062 D4): its node's colours resolve by it. The first
    /// report for a view corrects what was presented without motion, as the
    /// session's first does; a later change transitions, and so does a view
    /// that agrees with the session again.
    pub fn set_view_scheme(&mut self, view: ViewId, dark: bool) -> String {
        let mut batch = Batch::new();
        let key = self.runner.kernel().node(view).map(|n| n.key);
        if let Some(key) = key {
            if let Some(retired) = self.paint.motion.set_view_scheme(
                self.runner.kernel(),
                &mut self.engine,
                key,
                dark,
                self.now_ms / 1000.0,
            ) {
                self.retire_paint(retired, &mut batch);
                self.present(&mut batch, false);
            }
        }
        self.finish(batch, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::{DataError, Event};

    struct NoData;
    impl DataSource for NoData {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn destroying_an_inline_run_releases_its_inherited_paint() {
        let plan = contract::compile(
            "component App\n  state on = false\n  state shown = true\n  action go writes on\n    on = true\n  action hide writes shown\n    shown = false\n  view\n    column color=(on ? \"#ffffff\" : \"#000000\") transition=\"color 1s linear\"\n      button \"Go\" testId=\"go\" press=go\n      button \"Hide\" testId=\"hide\" press=hide\n      text\n        when shown\n          text \"Run\" testId=\"run\"\n",
        ).unwrap();
        let (mut host, _) = Host::boot(
            &plan.encode(),
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            390.0,
            844.0,
        )
        .unwrap();
        let view = |host: &Host<NoData>, name| {
            let kernel = host.runner().kernel();
            kernel
                .node_by_key(kernel.find_by_test_id(name)[0])
                .unwrap()
                .id
        };
        let run = view(&host, "run");
        host.dispatch_at(view(&host, "go"), Event::Press, 0.0);
        host.tick(500.0);
        assert!(host.paint.runs.contains_key(&run));
        host.dispatch_at(view(&host, "hide"), Event::Press, 500.0);
        assert!(!host.paint.runs.contains_key(&run));
    }
}
