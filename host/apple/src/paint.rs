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
//! every owner, which transitions as a browser's computed value does.
//!
//! `color` is inherited: a view that inherits an animating node's colour
//! paints the same presented value, as a browser's inheriting element does;
//! so are a path's `fill` and `stroke` (LLP 1065).
//!
//! [`Kernel::paint_sync`]: exact_kernel::Kernel::paint_sync

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
    /// Views painting an animating node's `color`, `fill` or `stroke` they
    /// inherit, by that node and property.
    inheritors: BTreeMap<(u64, Property), Vec<ViewId>>,
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
        Property::Fill => "fill",
        Property::Stroke => "stroke",
        _ => "shadow_color",
    }
}

/// The row an inherited paint property is set by: `color`, and a path's
/// `fill` and `stroke` (LLP 1065). Others do not inherit.
fn inherited_row(property: Property) -> Option<StyleId> {
    match property {
        Property::Color => Some(StyleId::TextColor),
        Property::Fill => Some(StyleId::Fill),
        Property::Stroke => Some(StyleId::Stroke),
        _ => None,
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
        let dark = self.paint.dark.unwrap_or(false);
        let sync = self
            .runner
            .kernel()
            .paint_sync(receipt, dark, &mut self.paint.owners);
        for key in &receipt.destroyed {
            let node = exact_kernel::motion::motion_node(*key);
            self.paint.inheritors.retain(|(n, _), _| *n != node);
        }
        self.apply_paint(sync, batch);
    }

    /// Adopt the whole tree's paint at boot.
    pub(super) fn boot_paint(&mut self, order: &[ViewId]) {
        let kernel = self.runner.kernel();
        let keys: Vec<NodeKey> = order
            .iter()
            .filter_map(|id| kernel.node(*id).map(|n| n.key))
            .collect();
        let dark = self.paint.dark.unwrap_or(false);
        let sync = kernel.paint_adopt(keys, dark, &mut self.paint.owners);
        self.apply_paint(sync, &mut Batch::new());
    }

    fn apply_paint(&mut self, sync: MotionSync, batch: &mut Batch) {
        for (node, property) in &sync.retired {
            if let Some(view) = self.keys.get(&node_key(*node)).copied() {
                batch.unpresent(view, style_key(*property));
            }
            for view in self
                .paint
                .inheritors
                .remove(&(*node, *property))
                .unwrap_or_default()
            {
                batch.unpresent(view, style_key(*property));
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
            let sync = self
                .runner
                .kernel()
                .paint_resync(dark, &mut self.paint.owners);
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

    /// One paint presentation on `view` (a leaving one included): the value
    /// while it differs from the target, else the style again. An animating `color` also reaches the views that
    /// inherit it.
    pub(super) fn present_paint(&mut self, p: Presentation, view: ViewId, batch: &mut Batch) {
        let settled = self.engine.target(p.node, p.property) == Some(p.value);
        if !self.inline_runs.contains_key(&view) {
            match settled {
                // A view this batch creates has nothing to take back.
                true if batch.creates(view) => {}
                true => batch.unpresent(view, style_key(p.property)),
                false => batch.present4(view, style_key(p.property), channels(&p)),
            }
        }
        let Some(row) = inherited_row(p.property) else {
            return;
        };
        let key = style_key(p.property);
        let now = if settled {
            Vec::new()
        } else {
            self.inheritors_of(p.node, p.property, row)
        };
        let before = self
            .paint
            .inheritors
            .remove(&(p.node, p.property))
            .unwrap_or_default();
        for view in before.iter().filter(|v| !now.contains(v)) {
            batch.unpresent(*view, key);
        }
        for view in &now {
            batch.present4(*view, key, channels(&p));
        }
        if !now.is_empty() {
            self.paint.inheritors.insert((p.node, p.property), now);
        }
    }

    /// The views below `node` whose `property` is `node`'s: no own `row` on
    /// the way, and no paint motion of their own.
    fn inheritors_of(&self, node: u64, property: Property, row: StyleId) -> Vec<ViewId> {
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
            if child.style.mask.has(row) || self.paint.owners.owns(n, property) {
                continue;
            }
            if !self.inline_runs.contains_key(&id) {
                out.push(id);
            }
            stack.extend(child.children());
        }
        out.sort_unstable();
        out
    }
}
