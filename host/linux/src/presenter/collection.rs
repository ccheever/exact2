//! Bounded native post-layout collection feedback. The kernel owns row layout;
//! the runner owns membership, estimates and anchors. No recursive frame/layout.
use super::*;
use exact_kernel::{Dimension, Kernel, NodeKey};
use exact_runner::{CollectionFeedback, CollectionSnapshot, RowMeasurement};
use std::collections::{BTreeSet, VecDeque};

const PASSES: usize = 2;

/// One future model position, not the acknowledged picture's input position.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ModelScroll {
    key: NodeKey,
    sequence: u64,
    top: f32,
    left: Option<f32>,
}

#[derive(Default)]
pub(super) struct State {
    cursors: BTreeMap<ViewId, Cursor>,
    queue: VecDeque<ViewId>,
    interaction: Option<ViewId>,
    authored_scroll: Option<bool>,
    scroll_events: VecDeque<(ViewId, NodeKey)>,
}
#[derive(Default)]
struct Cursor {
    key: Option<NodeKey>,
    sequence: u64,
    corrected: Option<u64>,
    dimensions: Option<(f64, f64, f64, f64)>,
    sent: Option<CollectionFeedback>,
    queued: bool,
    requested_top: Option<f64>,
    model_scroll: Option<ModelScroll>,
    ordinary: bool,
    requested_left: Option<f64>,
    follow_end: bool,
    maximum: Option<(f32, f32)>,
}
impl Cursor {
    fn advance(&mut self) {
        self.sequence = self.sequence.saturating_add(1);
        self.model_scroll = None;
    }
    fn bind(&mut self, key: NodeKey, sequence: u64) {
        if self.key.is_some_and(|old| old != key) {
            *self = Self {
                sequence,
                ..Self::default()
            };
        }
        self.key = Some(key);
    }
    fn model_top(&mut self, key: NodeKey, top: f32, displayed: bool, offset: &mut (f32, f32)) {
        self.model_offset(key, (offset.0, top), displayed, offset);
        if let Some(pending) = &mut self.model_scroll {
            pending.left = None;
        }
    }
    fn model_offset(
        &mut self,
        key: NodeKey,
        target: (f32, f32),
        displayed: bool,
        offset: &mut (f32, f32),
    ) {
        if displayed {
            // An exhausted input sequence cannot qualify a future ACK, just
            // as it cannot qualify a Runner correction below.
            if self.sequence == u64::MAX {
                return;
            }
            self.model_scroll = Some(ModelScroll {
                key,
                sequence: self.sequence,
                top: target.1,
                left: Some(target.0),
            });
        } else {
            *offset = target;
        }
    }
    fn geometry(&mut self, dimensions: (f64, f64, f64, f64)) {
        if self.dimensions.is_some_and(|old| old != dimensions) {
            self.advance();
        }
        self.dimensions = Some(dimensions);
    }
    fn correction(&mut self, snapshot: &CollectionSnapshot) -> Option<f64> {
        let correction = snapshot.correction?;
        if self.sequence == u64::MAX
            || correction.scroll_sequence != self.sequence
            || self
                .corrected
                .is_some_and(|revision| revision >= snapshot.revision)
        {
            return None;
        }
        self.corrected = Some(snapshot.revision);
        Some(correction.offset)
    }
}
impl State {
    fn scroll_event(&mut self, view: ViewId, key: NodeKey) {
        if !self.scroll_events.contains(&(view, key)) {
            self.scroll_events.retain(|(id, _)| *id != view);
            self.scroll_events.push_back((view, key));
        }
    }
    pub(super) fn pending(&self) -> bool {
        !self.queue.is_empty()
    }
    pub(super) fn data_ready(&mut self) {
        // Activation is one stimulus for edges refused before the executor was
        // ready, including collections whose geometry and rows did not change.
        for cursor in self.cursors.values_mut() {
            cursor.sent = None;
        }
    }
    pub(super) fn advance_all(&mut self) {
        for cursor in self.cursors.values_mut() {
            if !cursor.ordinary {
                cursor.advance();
            }
        }
    }
    fn schedule(&mut self, snapshots: &[CollectionSnapshot]) {
        let live: BTreeSet<_> = snapshots.iter().map(|s| s.view).collect();
        self.cursors
            .retain(|id, cursor| cursor.ordinary || live.contains(id));
        self.queue.retain(|id| live.contains(id));
        for snapshot in snapshots {
            let cursor = self.cursors.entry(snapshot.view).or_insert_with(|| Cursor {
                sequence: snapshot.scroll_sequence,
                ..Cursor::default()
            });
            if cursor.ordinary {
                *cursor = Cursor {
                    sequence: snapshot.scroll_sequence,
                    ..Cursor::default()
                };
            }
            if !cursor.queued {
                cursor.queued = true;
                self.queue.push_back(snapshot.view);
            }
        }
    }
}

struct Geometry {
    width: f64,
    height: f64,
    row_width: f64,
    padding_top: f64,
    max_top: f32,
}
fn length(kernel: &Kernel, value: Dimension, basis: f64) -> f64 {
    match value.resolve(&kernel.env()) {
        Dimension::Points(n) => n as f64,
        Dimension::Percent(n) => basis * n as f64 / 100.,
        _ => 0.,
    }
}

fn containing_width(kernel: &Kernel, view: ViewId, viewport: f64) -> f64 {
    let mut ancestors = Vec::new();
    let mut at = kernel.node(view).and_then(|n| n.parent);
    while let Some(parent) = at.and_then(|id| kernel.node(id)) {
        ancestors.push(parent);
        at = parent.parent;
    }
    ancestors.into_iter().rev().fold(viewport, |basis, node| {
        let [_, right, _, left] = node.style.border_widths();
        (node.frame.width as f64
            - left as f64
            - right as f64
            - length(kernel, node.style.padding_left, basis)
            - length(kernel, node.style.padding_right, basis))
        .max(0.)
    })
}

fn geometry(kernel: &Kernel, snapshot: &CollectionSnapshot, viewport: f64) -> Option<Geometry> {
    let node = kernel.node(snapshot.view)?;
    let [top, right, bottom, left] = node.style.border_widths();
    let width = (node.frame.width - left - right).max(0.) as f64;
    let height = (node.frame.height - top - bottom).max(0.) as f64;
    // Read the origin from authored style, independently of the mounted row.
    // Subtracting an f64 logical row top from its f32 layout position invents
    // padding at large offsets, invalidating valid anchor corrections.
    let basis = containing_width(kernel, snapshot.view, viewport);
    let pad = |d| length(kernel, d, basis);
    let padding_top = pad(node.style.padding_top);
    // A 100%-width wrapper reports the actual width Taffy offered the row,
    // including its real containing block's percentage-padding resolution.
    let row_width = snapshot
        .rows
        .first()
        .and_then(|r| kernel.node(r.view))
        .map_or_else(
            || (width - pad(node.style.padding_left) - pad(node.style.padding_right)).max(0.),
            |wrapper| wrapper.frame.width as f64,
        );
    (width > 0. && height > 0. && row_width > 0.).then_some(Geometry {
        width,
        height,
        row_width,
        padding_top,
        // Native layout may omit the trailing border from content_size. The
        // scroll range must still reach the index's end through the inner port.
        max_top: (content_size(&node, kernel).1 - node.frame.height)
            .max(
                (snapshot.total_extent + padding_top + pad(node.style.padding_bottom) - height)
                    as f32,
            )
            .max(0.),
    })
}

// Each global pin belongs to at most one collection, even with nested lists.
fn pin_owner(
    kernel: &Kernel,
    snapshots: &[CollectionSnapshot],
    view: Option<ViewId>,
) -> Option<ViewId> {
    let mut at = view;
    while let Some(id) = at {
        if let Some(owner) = snapshots
            .iter()
            .find(|s| s.rows.iter().any(|row| row.view == id))
        {
            return Some(owner.view);
        }
        at = kernel.node(id)?.parent;
    }
    None
}

impl<D: DataSource> Presenter<D> {
    /// Ordinary scroll containers consume changed requests after layout, just
    /// like the browser's prop writes. The existing picture receipt also owns
    /// these offsets: an in-flight frame cannot move the input base early.
    pub(super) fn sync_authored_scroll(&mut self) {
        let enabled = *self.collection.authored_scroll.get_or_insert_with(|| {
            self.host.runner().plan().bindings.iter().any(|binding| {
                binding.kind == exact_plan::BindingKind::Prop
                    && [
                        PropId::ScrollTop,
                        PropId::ScrollLeft,
                        PropId::ScrollFollowEnd,
                    ]
                    .iter()
                    .any(|id| binding.id == *id as u16)
            })
        });
        if !enabled {
            return;
        }
        let collections: BTreeSet<_> = self.host.collections().iter().map(|s| s.view).collect();
        let mut live = BTreeSet::new();
        for view in self.host.preorder() {
            if collections.contains(&view) {
                continue;
            }
            let Some(node) = self.host.kernel().node(view) else {
                continue;
            };
            let number = |id| {
                node.props
                    .get(id)
                    .and_then(exact_kernel::PropValue::as_float)
                    .filter(|n| n.is_finite())
            };
            let top = number(PropId::ScrollTop);
            let left = number(PropId::ScrollLeft);
            let follow = node.props.bool(PropId::ScrollFollowEnd) == Some(true);
            if top.is_none() && left.is_none() && !follow {
                continue;
            }
            let axes = effective_overflow(&node);
            if axes == (Overflow::Visible, Overflow::Visible) {
                continue;
            }
            live.insert(view);
            let bounds = self.brush.scroll_bounds(
                self.host.kernel(),
                self.host.content_region(),
                &node,
                None,
            );
            let cursor = self.collection.cursors.entry(view).or_default();
            cursor.bind(node.key, 0);
            cursor.ordinary = true;
            let offset = self.scroll.entry(view).or_default();
            let current = cursor
                .model_scroll
                .map_or(*offset, |p| (p.left.unwrap_or(offset.0), p.top));
            let changed_top = top != cursor.requested_top;
            let changed_left = left != cursor.requested_left;
            if changed_top || changed_left {
                cursor.advance();
            }
            cursor.requested_top = top;
            cursor.requested_left = left;
            if self.host.route_visibility(view).0 {
                cursor.model_scroll = None;
                cursor.follow_end = follow;
                continue;
            }
            let mut target = current;
            if follow
                && (!cursor.follow_end || current.1 >= cursor.maximum.map_or(0., |m| m.1) - 1.)
            {
                target.1 = bounds.max.1;
            }
            cursor.follow_end = follow;
            cursor.maximum = Some(bounds.max);
            // Explicit requests win over end-following in the same commit.
            if changed_top {
                if let Some(top) = top {
                    target.1 = top.clamp(0., bounds.max.1 as f64) as f32;
                }
            }
            if changed_left {
                if let Some(left) = left {
                    target.0 = left.clamp(0., bounds.max.0 as f64) as f32;
                }
            }
            target = bounds.clamp(target);
            if target != current || ((changed_top || changed_left) && target != *offset) {
                let before = *offset;
                cursor.model_offset(node.key, target, self.display.attached(), offset);
                if *offset != before {
                    self.collection.scroll_event(view, node.key);
                    self.executor.notify();
                }
                self.dirty = true;
            }
        }
        self.collection
            .cursors
            .retain(|view, cursor| !cursor.ordinary || live.contains(view));
    }

    // Browser scroll events are coalesced per event-loop turn. Report the
    // current acknowledged position, never an unpresented model target.
    pub(super) fn dispatch_authored_scroll(&mut self) -> Option<String> {
        let events = std::mem::take(&mut self.collection.scroll_events);
        let mut error = None;
        for (view, key) in events {
            if self.host.kernel().node(view).is_none_or(|n| n.key != key)
                || self.host.route_visibility(view).1
                || !self
                    .host
                    .runner()
                    .handlers_of(view)
                    .contains(&EventKind::Scroll)
            {
                continue;
            }
            // An earlier handler may have changed this pending target. Fold
            // that change into this event, at its original queue position.
            self.collection.scroll_events.retain(|(id, _)| *id != view);
            let (x, y) = self.scroll_of(view);
            let result =
                self.host
                    .dispatch_at(view, Event::Scroll(x as f64, y as f64), self.host.now());
            let after = self.after_commit();
            error = error.or(result).or(after);
        }
        error
    }

    fn pending_model_scroll(&self, view: ViewId) -> Option<ModelScroll> {
        let cursor = self.collection.cursors.get(&view)?;
        let pending = cursor.model_scroll?;
        (pending.sequence == cursor.sequence
            && self
                .host
                .kernel()
                .node(view)
                .is_some_and(|n| n.key == pending.key)
            && !self.host.route_visibility(view).0)
            .then_some(pending)
    }

    /// Only a future paint/feedback uses this offset. A's wheel, hits and
    /// clamping continue using self.scroll until the matching B is ACKed.
    pub(super) fn collection_paint_scroll(&self) -> Option<BTreeMap<ViewId, (f32, f32)>> {
        let mut next = None;
        for &view in self.collection.cursors.keys() {
            if let Some(pending) = self.pending_model_scroll(view) {
                let offset = next
                    .get_or_insert_with(|| self.scroll.clone())
                    .entry(view)
                    .or_default();
                offset.1 = pending.top;
                if let Some(left) = pending.left {
                    offset.0 = left;
                }
            }
        }
        next
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn painted_collection_scroll(
        &self,
        boxes: &[PaintedBox],
    ) -> BTreeMap<ViewId, ModelScroll> {
        boxes
            .iter()
            .filter_map(|b| {
                let pending = self.pending_model_scroll(b.id)?;
                // A retained-region replay may clamp to older pixels. It cannot
                // acknowledge a future target that this picture did not paint.
                let offset = b.scroll?;
                (offset.1 == pending.top && pending.left.is_none_or(|left| offset.0 == left))
                    .then_some((b.id, pending))
            })
            .collect()
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn acknowledge_collection_scroll(&mut self, painted: BTreeMap<ViewId, ModelScroll>) {
        for (view, accepted) in painted {
            let Some(current) = self.pending_model_scroll(view) else {
                continue;
            };
            if current.key != accepted.key || current.sequence != accepted.sequence {
                continue;
            }
            // A newer model correction on the same input sequence survives B,
            // but the interaction base becomes precisely B's painted position.
            let offset = self.scroll.entry(view).or_default();
            let before = *offset;
            offset.1 = accepted.top;
            if let Some(left) = accepted.left {
                offset.0 = left;
            }
            if *offset != before && self.collection.cursors[&view].ordinary {
                self.collection.scroll_event(view, accepted.key);
                self.executor.notify();
            }
            if current == accepted {
                self.collection.cursors.get_mut(&view).unwrap().model_scroll = None;
            } else {
                self.dirty = true;
            }
        }
    }

    /// A Runner witness is usable only after feedback for the actual native
    /// scroll/layout has landed. Pending feedback never certifies the old port.
    pub(super) fn reorder_facts_current(&self, facts: &exact_runner::ReorderGeometry) -> bool {
        let Some(node) = self.host.kernel().node_by_key(facts.list) else {
            return false;
        };
        let snapshots = self.host.collections();
        let Some(snapshot) = snapshots.iter().find(|s| s.view == node.id) else {
            return false;
        };
        let Some(g) = geometry(self.host.kernel(), snapshot, self.viewport.0 as f64) else {
            return false;
        };
        // Nonzero vertical inset authoring is outside the measured-row policy.
        // Do not admit a handbuilt plan that bypassed the compiler's rejection.
        let basis = containing_width(self.host.kernel(), node.id, self.viewport.0 as f64);
        g.padding_top == 0.
            && length(self.host.kernel(), node.style.padding_bottom, basis) == 0.
            && self
                .collection
                .cursors
                .get(&node.id)
                .is_some_and(|c| c.sequence == facts.scroll_sequence)
            && facts.scroll_top == self.scroll_of(node.id).1 as f64
            && facts.port_width == g.width
            && facts.port_height == g.height
            && facts.row_width == g.row_width
    }

    pub(super) fn collection_scroll_limits(&self) -> BTreeMap<ViewId, f32> {
        self.host
            .collections()
            .iter()
            .filter_map(|snapshot| {
                geometry(self.host.kernel(), snapshot, self.viewport.0 as f64)
                    .map(|g| (snapshot.view, g.max_top))
            })
            .collect()
    }

    pub(crate) fn queue_collections(&mut self) {
        let retained = self.arrange_pin().map(|p| p.0);
        if self.collection.interaction.is_some_and(|id| {
            Some(id) != retained
                && (self.host.kernel().node(id).is_none() || self.host.route_visibility(id).1)
        }) {
            self.collection.interaction = None;
        }
        self.collection.schedule(&self.host.collections());
        if self.collection.pending() {
            // GUI poll observes this FD. Headless advances on existing pump/frame
            // calls; it does not run while the carrier blocks waiting for stdin.
            self.executor.notify();
        }
    }

    /// Pin at most one active native interaction in addition to focused input.
    /// The event-loop owner must clear this on release/cancellation; hover is not a pin.
    pub fn set_collection_interaction(&mut self, view: Option<ViewId>) {
        let view = view.filter(|id| {
            self.host.kernel().node(*id).is_some() && !self.host.route_visibility(*id).1
        });
        if self.collection.interaction != view {
            self.arrange_pin_transfer(view);
            self.collection.interaction = view;
            self.queue_collections();
            self.dirty = true;
        }
    }

    /// Current live interaction target. Carriers discard held pointer state when
    /// navigation or runner replacement invalidates its pin.
    pub fn collection_interaction(&self) -> Option<ViewId> {
        let retained = self.arrange_pin().map(|p| p.0);
        self.collection.interaction.filter(|id| {
            Some(*id) == retained
                || (self.host.kernel().node(*id).is_some() && !self.host.route_visibility(*id).1)
        })
    }

    pub(super) fn collection_scrolled(&mut self, view: ViewId) {
        self.collection_scroll_turn(view, false);
    }

    // Only Arrange's prevalidated, adapter-owned edge step uses this order.
    // External scroll still retires stale contact before accepting new facts.
    pub(super) fn collection_scrolled_by_arrange(&mut self, view: ViewId) {
        self.collection_scroll_turn(view, true);
    }

    fn collection_scroll_turn(&mut self, view: ViewId, owned_edge: bool) {
        self.collection.scroll_events.retain(|(id, _)| *id != view);
        if let Some(cursor) = self.collection.cursors.get_mut(&view) {
            cursor.advance();
        }
        // Collection observation supplements the ordinary authored handler.
        let authored = self
            .host
            .runner()
            .handlers_of(view)
            .contains(&EventKind::Scroll);
        let mut error = if authored {
            let (x, y) = self.scroll_of(view);
            self.host
                .dispatch_at(view, Event::Scroll(x as f64, y as f64), self.host.now())
        } else {
            None
        };
        if owned_edge {
            // The accepted receipt has already synchronized current targets and
            // layout. Feed THIS port's new offset/sequence before sync_commit's
            // contact check, not an unrelated queued List first. No guard is
            // bypassed: deletion/reflow still fails during that feedback turn.
            self.queue_collections();
            if let Some(index) = self.collection.queue.iter().position(|id| *id == view) {
                self.collection.queue.remove(index);
                self.collection.queue.push_front(view);
            }
            error = error.or(self.refine_collections());
        }
        error = error.or(if owned_edge {
            // Finish ordinary receipt effects/retirement without scheduling a
            // second collection pass in this same edge step.
            let after = self.sync_commit();
            after.or(self.refresh_transform_geometry())
        } else if authored {
            self.after_commit()
        } else {
            self.queue_collections();
            self.refine_collections()
        });
        if let Some(error) = error {
            self.host.log(error);
        }
    }

    pub(super) fn refine_collections(&mut self) -> Option<String> {
        let mut error = None;
        for _ in 0..PASSES {
            let Some(view) = self.collection.queue.pop_front() else {
                break;
            };
            let snapshots = self.host.collections();
            let Some(snapshot) = snapshots.iter().find(|s| s.view == view) else {
                self.collection.cursors.remove(&view);
                continue;
            };
            let retained_pin = self.arrange_pin();
            let cursor = self.collection.cursors.get_mut(&view).unwrap();
            cursor.queued = false;
            let Some(key) = self.host.kernel().node(view).map(|n| n.key) else {
                continue;
            };
            cursor.bind(key, snapshot.scroll_sequence);
            if self.host.route_visibility(view).0 {
                cursor.model_scroll = None;
                continue;
            }
            if self.host.content_region().is_some_and(|region| {
                !region.collection_feedback_allowed(self.host.kernel(), snapshot)
            }) {
                // A completion queues this view again. Do not spin frames, or
                // relabel old accepted height with current row epochs.
                continue;
            }
            let Some(g) = geometry(self.host.kernel(), snapshot, self.viewport.0 as f64) else {
                continue;
            };
            cursor.geometry((g.width, g.height, g.row_width, g.padding_top));
            // Match the browser's post-layout scrollTop prop write. Consume
            // each changed request once; an unchanged binding never owns the
            // reader's offset. Advance the sequence so old anchor corrections
            // cannot override an explicit Latest/jump request.
            let requested = self
                .host
                .kernel()
                .node(view)
                .and_then(|node| {
                    node.props
                        .get(exact_kernel::PropId::ScrollTop)
                        .and_then(exact_kernel::PropValue::as_float)
                })
                .filter(|top| top.is_finite());
            if requested != cursor.requested_top {
                cursor.requested_top = requested;
                if let Some(top) = requested {
                    cursor.advance();
                    cursor.model_top(
                        key,
                        top.clamp(0., g.max_top as f64) as f32,
                        self.display.attached(),
                        self.scroll.entry(view).or_default(),
                    );
                    self.dirty = true;
                }
            }
            if let Some(top) = cursor.correction(snapshot) {
                cursor.model_top(
                    key,
                    ((top + g.padding_top) as f32).clamp(0., g.max_top),
                    self.display.attached(),
                    self.scroll.entry(view).or_default(),
                );
                self.dirty = true;
            }
            let feedback_top = if let Some(pending) = cursor.model_scroll.as_mut() {
                let top = pending.top.clamp(0., g.max_top);
                self.dirty |= top != pending.top;
                pending.top = top;
                top
            } else {
                self.scroll.get(&view).map_or(0., |off| off.1)
            };
            let feedback = CollectionFeedback {
                view,
                revision: snapshot.revision,
                scroll_sequence: cursor.sequence,
                offset: (feedback_top as f64 - g.padding_top).max(0.),
                port_cross: g.width,
                port_main: g.height,
                cross: g.row_width,
                measurements: snapshot
                    .rows
                    .iter()
                    .filter_map(|row| {
                        self.host
                            .kernel()
                            .node(row.view)
                            .map(|node| RowMeasurement {
                                view: row.view,
                                epoch: row.epoch,
                                size: node.frame.height as f64,
                            })
                    })
                    .collect(),
                focus_view: self.focus.filter(|_| {
                    pin_owner(self.host.kernel(), &snapshots, self.focus) == Some(view)
                }),
                interaction_view: self.collection.interaction.filter(|_| {
                    retained_pin
                        .filter(|(pin, _)| Some(*pin) == self.collection.interaction)
                        .map(|p| p.1)
                        .or_else(|| {
                            pin_owner(self.host.kernel(), &snapshots, self.collection.interaction)
                        })
                        == Some(view)
                }),
            };
            if cursor.sent.as_ref() == Some(&feedback) {
                continue;
            }
            cursor.sent = Some(feedback.clone());
            match self.host.collection_feedback(feedback) {
                Ok(true) => {
                    let after = self.sync_commit();
                    error = error.or(after);
                    self.collection.schedule(&self.host.collections());
                }
                Ok(false) => {}
                Err(why) => {
                    // An edge action can refuse after geometry committed. Run
                    // the same post-commit synchronization as timer refusals.
                    let after = self.sync_commit();
                    error = error.or(Some(why));
                    error = error.or(after);
                    self.collection.schedule(&self.host.collections());
                }
            }
        }
        if self.collection.pending() {
            self.executor.notify();
            self.dirty = true;
        }
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::AnchorCorrection;

    fn snapshot() -> CollectionSnapshot {
        CollectionSnapshot {
            view: 1,
            revision: 4,
            scroll_sequence: 2,
            count: 0,
            total_extent: 0.,
            rows: vec![],
            correction: Some(AnchorCorrection {
                scroll_sequence: 2,
                offset: 200.,
            }),
            pending: false,
        }
    }
    #[test]
    fn corrections_are_consumed_once_and_user_scroll_or_resize_wins() {
        let s = snapshot();
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        assert_eq!(cursor.correction(&s), Some(200.));
        assert_eq!(cursor.correction(&s), None);
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.advance();
        assert_eq!(cursor.correction(&s), None);
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.geometry((200., 300., 180., 10.));
        cursor.geometry((190., 300., 170., 10.));
        assert_eq!(cursor.correction(&s), None);
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.geometry((200., 300., 180., 10.));
        cursor.geometry((200., 300., 180., 20.));
        assert_eq!(
            cursor.correction(&s),
            None,
            "origin changes also invalidate old corrections"
        );
        let mut cursor = Cursor {
            sequence: u64::MAX,
            ..Cursor::default()
        };
        let mut overflow = s;
        overflow.correction.as_mut().unwrap().scroll_sequence = u64::MAX;
        assert_eq!(cursor.correction(&overflow), None);
    }

    #[test]
    fn refinement_queue_deduplicates_and_forgets_unmounted_collections() {
        let mut state = State::default();
        for _ in 0..1000 {
            state.schedule(&[snapshot()]);
        }
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.cursors.len(), 1);
        state.schedule(&[]);
        assert!(!state.pending());
        assert!(state.cursors.is_empty());
    }

    #[test]
    fn empty_collection_percentage_padding_uses_its_containing_block() {
        struct Empty;
        impl DataSource for Empty {
            fn query(
                &mut self,
                _: &str,
                _: &[exact_runner::Value],
            ) -> Result<exact_runner::Value, exact_runner::DataError> {
                Ok(exact_runner::Value::list(vec![]))
            }
        }
        let plan = contract::compile("component App\n  resource rows = rows() as shape list<number>\n  view\n    column width=200\n      list virtualized=true width=\"50%\" height=100 padding-left=\"10%\" padding-right=\"10%\" box-sizing=\"border-box\"\n        each x in rows key=x\n          text `${x}`\n").unwrap();
        let (host, error) = Host::boot(
            &plan.encode(),
            Empty,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            500.,
        )
        .unwrap();
        assert!(error.is_none());
        let facts = geometry(host.kernel(), &host.collections()[0], 400.).unwrap();
        assert_eq!(facts.width, 100.);
        assert_eq!(facts.row_width, 60.);
        assert_eq!(facts.padding_top, 0.);
    }
}
