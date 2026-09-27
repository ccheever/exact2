//! Paint motion (LLP 1055.000 D6, LLP 1062): colour and shadow transitions
//! and keyframes, sampled by the engine like every other property.
//!
//! Only a node whose `transition`, `animation` or `exit-animation` names a
//! paint property owns one in the engine ([`Kernel::paint_sync`]); every
//! other colour stays the style dictionary's. A box whose paint moves is
//! re-sent its style with the presented values over its rows
//! ([`Host::present_colors`]); once a value arrives, the row shows again, so
//! nothing is left to go stale when the row later changes without a
//! transition.
//!
//! Colours are resolved here, not in the presenter: the engine interpolates
//! concrete colours, so `light-dark()` is resolved by the appearance the
//! presenter reports ([`Host::set_scheme`]) and an appearance change re-targets
//! every owner, which transitions as a browser's computed value does. A view
//! whose own appearance differs from the session's (an override on a sheet,
//! say) reports it ([`Host::set_view_scheme`]), and its node resolves by it.
//!
//! [`Kernel::paint_sync`]: exact_kernel::Kernel::paint_sync

use super::Host;
use crate::batch::Batch;
use crate::style::Shown;
use exact_kernel::motion::{motion_node, MotionSync, PaintOwners};
use exact_kernel::{CommitReceipt, NodeKey, ViewId};
use exact_motion::{Property, Value};
use exact_runner::DataSource;
use std::collections::BTreeMap;

/// The host's paint-motion state.
#[derive(Debug, Default)]
pub(super) struct Paint {
    owners: PaintOwners,
    /// The appearance the presenter last reported; `None` before its first
    /// report, which snaps instead of transitioning.
    dark: Option<bool>,
    /// Each owner's `currentcolor` sides at its last sync: a side that stays
    /// one takes a new `color` at once, and shows the presented one.
    current: BTreeMap<u64, Vec<Property>>,
    /// Nodes whose view's appearance differs from the session's, and theirs.
    views: BTreeMap<u64, bool>,
    /// Inline runs painting an inherited `color` that moves, by run: their
    /// paragraph paints them (LLP 1062 D5).
    pub(super) runs: BTreeMap<ViewId, exact_motion::Value>,
}

/// The appearance `key`'s colours resolve by: its view's, else the session's.
fn resolve(views: &BTreeMap<u64, bool>, session: bool, key: NodeKey) -> bool {
    views.get(&motion_node(key)).copied().unwrap_or(session)
}

fn node_key(node: u64) -> NodeKey {
    NodeKey {
        index: node as u32,
        generation: (node >> 32) as u32,
    }
}

impl<D: DataSource> Host<D> {
    /// Adopt a commit's paint, after its `motion_sync` set the rows.
    pub(super) fn sync_paint(&mut self, receipt: &CommitReceipt, batch: &mut Batch) {
        for key in &receipt.destroyed {
            let node = motion_node(*key);
            self.paint.views.remove(&node);
            self.paint.current.remove(&node);
        }
        let (views, session) = (&self.paint.views, self.paint.dark.unwrap_or(false));
        let sync = self.runner.kernel().paint_sync(
            receipt,
            |key| resolve(views, session, key),
            &mut self.paint.owners,
        );
        self.apply_paint(sync, batch);
    }

    /// Adopt the whole tree's paint at boot.
    pub(super) fn boot_paint(&mut self, order: &[ViewId]) {
        let kernel = self.runner.kernel();
        let keys: Vec<NodeKey> = order
            .iter()
            .filter_map(|id| kernel.node(*id).map(|n| n.key))
            .collect();
        let (views, session) = (&self.paint.views, self.paint.dark.unwrap_or(false));
        let dark = |key| resolve(views, session, key);
        let sync = kernel.paint_adopt(keys, dark, &mut self.paint.owners);
        self.apply_paint(sync, &mut Batch::new());
    }

    fn apply_paint(&mut self, sync: MotionSync, batch: &mut Batch) {
        // A side that was and stays `currentcolor` has no transition of its
        // own (CSS: its computed value never changed); one that changes to or
        // from an explicit colour moves under its row.
        let kernel = self.runner.kernel();
        let mut nodes: Vec<u64> = sync.changes.iter().map(|c| c.node).collect();
        nodes.dedup();
        for node in nodes {
            let now = kernel.current_color_sides(node_key(node));
            let was = self.paint.current.remove(&node).unwrap_or_default();
            for side in now.iter().filter(|s| was.contains(s)) {
                if !self.engine.is_active(node, *side) {
                    self.engine.remove_property(node, *side);
                }
            }
            if !now.is_empty() {
                self.paint.current.insert(node, now);
            }
        }
        let mut retired: BTreeMap<ViewId, bool> = BTreeMap::new();
        for (node, property) in &sync.retired {
            self.paint.current.remove(node);
            if let Some(view) = self.keys.get(&node_key(*node)).copied() {
                *retired.entry(view).or_default() |= *property == Property::Color;
            }
        }
        let applied = sync.apply(&mut self.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        // What no longer moves shows its row again.
        for (view, inherits) in retired {
            self.present_colors(view, batch, inherits);
        }
    }

    /// What a node's paint shows now, over its rows: each owned property's
    /// value while it differs from its target. A `currentcolor` side that
    /// stays one follows the presented `color` through its row.
    pub(super) fn shown_paint(&self, node: u64) -> Shown {
        let mut shown = Shown::default();
        for property in Property::PAINT {
            // A leaving node's paint is no longer owned; its exit still plays.
            let playing = self.engine.animated(node, property, Value::ZERO).is_some();
            if !self.paint.owners.owns(node, property)
                && !self.engine.is_active(node, property)
                && !playing
            {
                continue;
            }
            let Some(value) = self.engine.sampled_value(node, property) else {
                continue;
            };
            if self.engine.target(node, property) != Some(value) {
                shown.set(property, Some(value));
            }
        }
        if let Some(sides) = self.paint.current.get(&node) {
            for side in sides {
                if !self.engine.is_active(node, *side) {
                    shown.set(*side, None);
                }
            }
        }
        shown
    }

    /// The presenter's appearance: a `light-dark()` colour an owner shows
    /// resolves by it. A change re-targets every owner, transitioning under
    /// its row; the first report only corrects boot's guess, without motion.
    pub fn set_scheme(&mut self, dark: bool) -> String {
        let mut batch = Batch::new();
        if self.paint.dark != Some(dark) {
            let first = self.paint.dark.is_none();
            self.paint.dark = Some(dark);
            self.engine.set_dark(dark, first);
            let seek = self.engine.advance(self.now_ms / 1000.0);
            debug_assert!(seek.is_ok(), "the clock never runs backwards here");
            let views = &self.paint.views;
            let sync = self
                .runner
                .kernel()
                .paint_resync(|key| resolve(views, dark, key), &mut self.paint.owners);
            if first {
                // A property the engine has not seen takes its value.
                for c in &sync.changes {
                    self.engine.remove_property(c.node, c.property);
                }
            }
            self.apply_paint(sync, &mut batch);
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
            let node = motion_node(key);
            let own = (Some(dark) != self.paint.dark).then_some(dark);
            let before = self.paint.views.get(&node).copied();
            if own != before {
                let first = before.is_none();
                match own {
                    Some(dark) => self.paint.views.insert(node, dark),
                    None => self.paint.views.remove(&node),
                };
                let sync = self
                    .runner
                    .kernel()
                    .paint_adopt([key], dark, &mut self.paint.owners);
                if first {
                    for c in &sync.changes {
                        self.engine.remove_property(c.node, c.property);
                    }
                }
                self.apply_paint(sync, &mut batch);
                // After the rows: dropping a slot drops its dirt, and the
                // playing keyframes' colours must still be shown again.
                self.engine.set_node_dark(node, own, first);
                let seek = self.engine.advance(self.now_ms / 1000.0);
                debug_assert!(seek.is_ok(), "the clock never runs backwards here");
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
