//! Native paint ownership and appearance, shared by the hosts (LLP 1062).

use super::{motion_node, node_key, MotionSync, PaintOwners};
use crate::{CommitReceipt, Kernel, NodeKey};
use exact_motion::{Engine, Property, Value};
use std::collections::BTreeMap;

/// Paint targets owned by one native session. Hosts report appearance and
/// present the resulting values; the browser owns this policy on the web.
#[derive(Debug, Default)]
pub struct PaintMotion {
    owners: PaintOwners,
    /// Before-change targets until a transition actually needs a slot.
    pending: BTreeMap<u64, Vec<(Property, Value)>>,
    /// `None` until the first report, which corrects boot without motion.
    dark: Option<bool>,
    /// Each owner's `currentcolor` sides at its last sync.
    current: BTreeMap<u64, Vec<Property>>,
    /// Views whose appearance differs from the session's.
    views: BTreeMap<u64, bool>,
}

/// The appearance a node's colours resolve by: its subtree's `color-scheme`
/// (LLP 1034 §8), which is what a host's view of it is set to, else a host's
/// report for its view (a sheet's own override, LLP 1062 D4), else the
/// session's.
fn appearance(kernel: &Kernel, views: &BTreeMap<u64, bool>, key: NodeKey, session: bool) -> bool {
    kernel
        .node_by_key(key)
        .and_then(|n| n.color_scheme_dark())
        .or_else(|| views.get(&motion_node(key)).copied())
        .unwrap_or(session)
}

impl PaintMotion {
    /// Whether the node owns this paint property.
    pub fn owns(&self, node: u64, property: Property) -> bool {
        self.owners.owns(node, property)
            && !self
                .pending
                .get(&node)
                .is_some_and(|targets| targets.iter().any(|(p, _)| *p == property))
    }

    /// The appearance a view's colours resolve by, else the session's.
    pub fn dark(&self, key: NodeKey) -> bool {
        self.views
            .get(&motion_node(key))
            .copied()
            .unwrap_or(self.dark.unwrap_or(false))
    }

    /// A host's own report for a view whose appearance differs from the
    /// session's (LLP 1062 D4), if any.
    pub fn view_dark(&self, key: NodeKey) -> Option<bool> {
        self.views.get(&motion_node(key)).copied()
    }

    /// Adopt a commit after its ordinary motion rows. Returned properties
    /// have retired: the host must show their style rows again.
    pub fn sync(
        &mut self,
        kernel: &Kernel,
        receipt: &CommitReceipt,
        engine: &mut Engine,
    ) -> Vec<(u64, Property)> {
        for key in receipt.destroyed.iter().chain(&receipt.renewed) {
            let node = motion_node(*key);
            self.views.remove(&node);
            self.current.remove(&node);
            self.pending.remove(&node);
        }
        let (views, session) = (&self.views, self.dark.unwrap_or(false));
        let sync = kernel.paint_sync(
            receipt,
            |key| appearance(kernel, views, key, session),
            &mut self.owners,
        );
        self.apply(kernel, sync, engine, false)
    }

    /// Adopt the tree at boot, after its ordinary motion rows.
    pub fn adopt(
        &mut self,
        kernel: &Kernel,
        keys: impl IntoIterator<Item = NodeKey>,
        engine: &mut Engine,
    ) {
        let (views, session) = (&self.views, self.dark.unwrap_or(false));
        let sync = kernel.paint_adopt(
            keys,
            |key| appearance(kernel, views, key, session),
            &mut self.owners,
        );
        self.apply(kernel, sync, engine, false);
    }

    fn apply(
        &mut self,
        kernel: &Kernel,
        mut sync: MotionSync,
        engine: &mut Engine,
        snap: bool,
    ) -> Vec<(u64, Property)> {
        // A side that stays `currentcolor` has no transition of its own:
        // its computed value is still the keyword. A change to or from an
        // explicit colour moves under its row.
        let changes = std::mem::take(&mut sync.changes);
        for changes in changes.chunk_by(|a, b| a.node == b.node) {
            let node = changes[0].node;
            let now = kernel.current_color_sides(node_key(node));
            let was = self.current.remove(&node).unwrap_or_default();
            let style = kernel.node_by_key(node_key(node)).unwrap().style;
            let animated: Vec<_> = style
                .animation
                .properties()
                .into_iter()
                .chain(style.exit_animation.properties())
                .collect();
            let pending = self
                .pending
                .entry(node)
                .or_insert_with(|| Vec::with_capacity(changes.len()));
            for change in changes {
                let property = change.property;
                let before = pending.iter().position(|(p, _)| *p == property);
                let owned = engine.target(node, property).is_some();
                let follows = now.contains(&property)
                    && was.contains(&property)
                    && !engine.is_active(node, property);
                let changed = before.is_some_and(|i| pending[i].1 != change.value);
                if owned || animated.contains(&property) || (changed && !snap && !follows) {
                    if snap || follows {
                        engine.remove_property(node, property);
                    } else if !owned {
                        if let Some(i) = before {
                            // Seed the before-change value, then let the ordinary
                            // observe below start the first curve from it.
                            let seeded = engine.observe(exact_motion::Change {
                                value: pending[i].1,
                                ..*change
                            });
                            debug_assert!(seeded.is_ok(), "a computed paint target is valid");
                        }
                    }
                    if let Some(i) = before {
                        pending.swap_remove(i);
                    }
                    sync.changes.push(*change);
                } else if let Some(i) = before {
                    pending[i].1 = change.value;
                } else {
                    pending.push((property, change.value));
                }
            }
            if pending.is_empty() {
                self.pending.remove(&node);
            }
            if !now.is_empty() {
                self.current.insert(node, now);
            }
        }
        for (node, property) in &sync.retired {
            self.current.remove(node);
            if let Some(pending) = self.pending.get_mut(node) {
                pending.retain(|(p, _)| p != property);
                if pending.is_empty() {
                    self.pending.remove(node);
                }
            }
        }
        let applied = sync.apply(engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        sync.retired
    }

    /// The value painted over a row while it moves. A settled
    /// `currentcolor` side follows the presented `color` through its row.
    pub fn shown(&self, engine: &Engine, node: u64, property: Property) -> Option<Value> {
        // A leaving node no longer owns paint; its exit still plays.
        let playing = engine.animated(node, property, Value::ZERO).is_some();
        if !self.owns(node, property) && !engine.is_active(node, property) && !playing {
            return None;
        }
        if self
            .current
            .get(&node)
            .is_some_and(|s| s.contains(&property))
            && !engine.is_active(node, property)
        {
            return None;
        }
        let value = engine.sampled_value(node, property)?;
        (engine.target(node, property) != Some(value)).then_some(value)
    }

    /// Report the session's appearance. The first report corrects boot's
    /// guess, including playing keyframes; later changes transition targets
    /// and leave playing keyframes in their starting appearance.
    pub fn set_scheme(
        &mut self,
        kernel: &Kernel,
        engine: &mut Engine,
        dark: bool,
        now: f64,
    ) -> Option<Vec<(u64, Property)>> {
        if self.dark == Some(dark) {
            return None;
        }
        let first = self.dark.is_none();
        self.dark = Some(dark);
        engine.set_dark(dark, first);
        let seek = engine.advance(now);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        let views = &self.views;
        let sync =
            kernel.paint_resync(|key| appearance(kernel, views, key, dark), &mut self.owners);
        Some(self.apply(kernel, sync, engine, first))
    }

    /// The platform's colours resolve differently (a host reported new
    /// resolutions, LLP 1095 D1): every owner re-targets under its
    /// appearance, transitioning under its row as an appearance change does
    /// (LLP 1095 D6). Nothing before the first appearance report. `first`:
    /// the session's first report, which corrects what boot resolved from
    /// the fallback (or another session's report) in place, without motion.
    pub fn colors_changed(
        &mut self,
        kernel: &Kernel,
        engine: &mut Engine,
        now: f64,
        first: bool,
    ) -> Option<Vec<(u64, Property)>> {
        let dark = self.dark?;
        let seek = engine.advance(now);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        let views = &self.views;
        let sync =
            kernel.paint_resync(|key| appearance(kernel, views, key, dark), &mut self.owners);
        Some(self.apply(kernel, sync, engine, first))
    }

    /// Report one view's appearance. Its first differing report corrects
    /// in place; subsequent changes, including rejoining the session, move.
    pub fn set_view_scheme(
        &mut self,
        kernel: &Kernel,
        engine: &mut Engine,
        key: NodeKey,
        dark: bool,
        now: f64,
    ) -> Option<Vec<(u64, Property)>> {
        let authored = kernel.node_by_key(key)?.color_scheme_dark().is_some();
        let node = motion_node(key);
        let own = (Some(dark) != self.dark).then_some(dark);
        let before = self.views.get(&node).copied();
        if own == before {
            return None;
        }
        let first = before.is_none();
        match own {
            Some(dark) => self.views.insert(node, dark),
            None => self.views.remove(&node),
        };
        // A view whose node sets or inherits a `color-scheme` reports what
        // the commit already resolved by (`appearance`, LLP 1034 §8): its
        // transitions started there, and are not snapped as a first report's
        // correction would.
        let session = self.dark.unwrap_or(false);
        let views = &self.views;
        let sync = kernel.paint_adopt(
            [key],
            |k| appearance(kernel, views, k, session),
            &mut self.owners,
        );
        let retired = self.apply(kernel, sync, engine, first && !authored);
        // Dropping a slot drops its dirt; playing keyframes must be marked
        // after the rows so the host presents their corrected colours.
        engine.set_node_dark(node, own, first);
        let seek = engine.advance(now);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        Some(retired)
    }
}
