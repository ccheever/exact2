//! Paint motion (LLP 1062): colour and shadow transitions and keyframes,
//! sampled by the engine like every other property and repainted by the
//! presenter.
//!
//! Only a node whose `transition` or `animation` names a paint property owns
//! one in the engine ([`Kernel::paint_sync`]); every other colour stays the
//! style dictionary's. While an owned value differs from its target the
//! presenter paints a `present` op's value in place of the style's; once it
//! arrives the op is `unpresent` and the style shows again, so nothing is left
//! to go stale when the row later changes without a transition.
//!
//! Colours are resolved here, not in the presenter: the engine interpolates
//! concrete colours, so `light-dark()` is resolved by the appearance the
//! presenter reports ([`Host::set_scheme`]) and an appearance change re-targets
//! every owner, which transitions as a browser's computed value does. A view
//! whose own appearance differs from the session's (an override on a sheet,
//! say) reports it ([`Host::set_view_scheme`]), and its node resolves by it.
//!
//! `color` is inherited: a view that inherits an animating node's colour
//! paints the same presented value, as a browser's inheriting element does,
//! and so does an inline run, whose colour its paragraph paints (a `present`
//! op on the paragraph that names the run). A `currentcolor` border side
//! paints the view's presented `color` frame by frame, as CSS's used value
//! follows the animating `color` ([`Kernel::current_color_sides`]).
//!
//! [`Kernel::current_color_sides`]: exact_kernel::Kernel::current_color_sides

use super::Host;
use crate::batch::Batch;
use exact_kernel::motion::{MotionSync, PaintOwners};
use exact_kernel::{CommitReceipt, NodeKey, StyleId, ViewId};
use exact_motion::{Presentation, Property};
use exact_runner::DataSource;
use std::collections::BTreeMap;

/// The host's paint-motion state.
#[derive(Debug, Default)]
pub(super) struct Paint {
    owners: PaintOwners,
    /// The appearance the presenter last reported; `None` before its first
    /// report, which snaps instead of transitioning.
    dark: Option<bool>,
    /// Views painting an animating node's `color` they inherit, by that node.
    inheritors: BTreeMap<u64, Vec<ViewId>>,
    /// Each view's `currentcolor` border sides painting its presented `color`.
    sides: BTreeMap<ViewId, Vec<Property>>,
    /// Each owner's `currentcolor` sides at its last sync: a side that stays
    /// one takes a new `color` at once, and shows the presented one.
    current: BTreeMap<u64, Vec<Property>>,
    /// Nodes whose view's appearance differs from the session's, and theirs.
    views: BTreeMap<u64, bool>,
}

/// The appearance `key`'s colours resolve by: its view's, else the session's.
fn resolve(views: &BTreeMap<u64, bool>, session: bool, key: NodeKey) -> bool {
    views
        .get(&exact_kernel::motion::motion_node(key))
        .copied()
        .unwrap_or(session)
}

/// The presenter's style key for a paint property's presented value.
fn style_key(property: Property) -> &'static str {
    match property {
        Property::BackgroundColor => "background_color",
        Property::Color => "text_color",
        Property::BorderTopColor => "border_color_top",
        Property::BorderRightColor => "border_color_right",
        Property::BorderBottomColor => "border_color_bottom",
        Property::BorderLeftColor => "border_color_left",
        Property::TintColor => "tint_color",
        Property::BoxShadow => "shadow_geometry",
        _ => "shadow_color",
    }
}

/// A presented value in the style dictionary's units: a colour's straight
/// channels 0–255, alpha too; a shadow's offset and blur in points.
fn channels(p: &Presentation) -> [f64; 4] {
    if p.property.is_color() {
        p.value.straight().map(|c| c * 255.0)
    } else {
        p.value.components()
    }
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
            let node = exact_kernel::motion::motion_node(*key);
            self.paint.inheritors.remove(&node);
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
        for (node, property) in &sync.retired {
            self.paint.current.remove(node);
            if let Some(view) = self.keys.get(&node_key(*node)).copied() {
                match property {
                    Property::Color => self.paint_color(view, None, batch),
                    p => self.paint_view(view, style_key(*p), None, batch),
                }
            }
            if *property == Property::Color {
                for view in self.paint.inheritors.remove(node).unwrap_or_default() {
                    self.paint_color(view, None, batch);
                }
            }
        }
        let applied = sync.apply(&mut self.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
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
            let node = exact_kernel::motion::motion_node(key);
            let own = (Some(dark) != self.paint.dark).then_some(dark);
            let before = self.paint.views.get(&node).copied();
            if own != before {
                let first = before.is_none();
                match own {
                    Some(dark) => self.paint.views.insert(node, dark),
                    None => self.paint.views.remove(&node),
                };
                self.engine.set_node_dark(node, own, first);
                let seek = self.engine.advance(self.now_ms / 1000.0);
                debug_assert!(seek.is_ok(), "the clock never runs backwards here");
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
                self.present(&mut batch, false);
            }
        }
        self.finish(batch, None)
    }

    /// One paint presentation on `view` (a leaving one included): the value
    /// while it differs from the target, else the style again. An animating
    /// `color` also reaches the views and runs that inherit it, and every
    /// `currentcolor` side among them.
    pub(super) fn present_paint(&mut self, p: Presentation, view: ViewId, batch: &mut Batch) {
        let settled = self.engine.target(p.node, p.property) == Some(p.value);
        let value = (!settled).then(|| channels(&p));
        let following = |paint: &Paint| {
            paint
                .sides
                .get(&view)
                .is_some_and(|s| s.contains(&p.property))
        };
        if p.property != Property::Color {
            // A settled `currentcolor` side shows the presented `color`.
            if !(settled && following(&self.paint)) {
                self.paint_view(view, style_key(p.property), value, batch);
            }
            return;
        }
        self.paint_color(view, value, batch);
        let now = if settled {
            Vec::new()
        } else {
            self.inheritors_of(p.node)
        };
        let before = self.paint.inheritors.remove(&p.node).unwrap_or_default();
        for view in before.into_iter().filter(|v| !now.contains(v)) {
            self.paint_color(view, None, batch);
        }
        for view in &now {
            self.paint_color(*view, value, batch);
        }
        if !now.is_empty() {
            self.paint.inheritors.insert(p.node, now);
        }
    }

    /// `color` on `view`, and on each of its `currentcolor` border sides.
    fn paint_color(&mut self, view: ViewId, value: Option<[f64; 4]>, batch: &mut Batch) {
        self.paint_view(view, "text_color", value, batch);
        let kernel = self.runner.kernel();
        // A side moving under its own row (to or from an explicit colour)
        // shows its own value.
        let now = match (value, kernel.node(view)) {
            (Some(_), Some(node)) => kernel
                .current_color_sides(node.key)
                .into_iter()
                .filter(|s| {
                    !self
                        .engine
                        .is_active(exact_kernel::motion::motion_node(node.key), *s)
                })
                .collect(),
            _ => Vec::new(),
        };
        let before = self.paint.sides.remove(&view).unwrap_or_default();
        for side in before.iter().filter(|s| !now.contains(s)) {
            self.paint_view(view, style_key(*side), None, batch);
        }
        for side in &now {
            self.paint_view(view, style_key(*side), value, batch);
        }
        if !now.is_empty() {
            self.paint.sides.insert(view, now);
        }
    }

    /// Present `value` under `key` on `view`, or hand the row back. An
    /// inline run is no view: its paragraph paints its colour.
    fn paint_view(&self, view: ViewId, key: &str, value: Option<[f64; 4]>, batch: &mut Batch) {
        if let Some((owner, _)) = self.inline_runs.get(&view) {
            if key == "text_color" && !(value.is_none() && batch.creates(*owner)) {
                batch.present_run(*owner, view, key, value);
            }
            return;
        }
        match value {
            Some(v) => batch.present4(view, key, v),
            // A view this batch creates has nothing to take back.
            None if batch.creates(view) => {}
            None => batch.unpresent(view, key),
        }
    }

    /// The views below `node` whose `color` is `node`'s: no own row on the
    /// way, and no paint motion of their own.
    fn inheritors_of(&self, node: u64) -> Vec<ViewId> {
        let kernel = self.runner.kernel();
        let Some(source) = kernel.node_by_key(node_key(node)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = source.children();
        while let Some(id) = stack.pop() {
            let Some(child) = kernel.node(id) else {
                continue;
            };
            let n = exact_kernel::motion::motion_node(child.key);
            if child.style.mask.has(StyleId::TextColor)
                || self.paint.owners.owns(n, Property::Color)
            {
                continue;
            }
            out.push(id);
            stack.extend(child.children());
        }
        out.sort_unstable();
        out
    }
}
