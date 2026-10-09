//! Bounded native post-layout collection feedback. The kernel owns row layout;
//! the runner owns membership, estimates and anchors. No recursive frame/layout.
use super::*;
use exact_kernel::{Dimension, Kernel, NodeKey};
use exact_runner::{
    CollectionFeedback, CollectionFill, CollectionSnapshot, ListAxis, RowMeasurement, ScrollEvent,
};
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
    /// The host runs a scroll's collection pass itself, after the frame it
    /// draws (as RecyclerView prefetches between frames): a scroll only
    /// queues it.
    pub(super) defer: bool,
    /// While set (by such a host, for the frame a scroll draws), passes wait
    /// for [`Presenter::refine_deferred`].
    pub(super) hold: bool,
    /// Slices (LLP 1050.000 §6): a host that fills between frames builds at
    /// most this many rows past what shows per report, and sends one report
    /// per list per pass; the rest is the list's `pending`. `None` builds the
    /// whole window at once.
    pub(super) limit: Option<u32>,
    /// The scrolled list and its velocity (logical px/s along its axis): its
    /// window leads that way and builds that side first.
    pub(super) velocity: Option<(ViewId, f64)>,
}
#[derive(Default)]
struct Cursor {
    key: Option<NodeKey>,
    sequence: u64,
    corrected: Option<u64>,
    dimensions: Option<(f64, f64, f64, f64)>,
    /// The last report, and the list's main-axis padding it went with: a
    /// change of padding alone is news to the runner (LLP 1010 §6.9).
    sent: Option<(CollectionFeedback, [f64; 2])>,
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
    /// A model position on the list's main axis. A vertical list leaves x to
    /// the reader; a horizontal one keeps its current (or pending) y.
    fn model_main(
        &mut self,
        key: NodeKey,
        axis: ListAxis,
        main: f32,
        displayed: bool,
        offset: &mut (f32, f32),
    ) {
        match axis {
            ListAxis::Vertical => {
                self.model_offset(key, (offset.0, main), displayed, offset);
                if let Some(pending) = &mut self.model_scroll {
                    pending.left = None;
                }
            }
            ListAxis::Horizontal => {
                let top = self.model_scroll.map_or(offset.1, |p| p.top);
                self.model_offset(key, (main, top), displayed, offset);
            }
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
    /// `resized_from`: the sequence a port resize in this same pass moved on
    /// from. The resize is not the reader moving, so a correction planned at
    /// it still lands (feed F14: posts put above the reader as a
    /// pull-to-refresh zone closes; a sent message's end-follow as the
    /// composer shrinks back), as on Apple.
    fn correction(
        &mut self,
        snapshot: &CollectionSnapshot,
        resized_from: Option<u64>,
    ) -> Option<f64> {
        let correction = snapshot.correction?;
        if self.sequence == u64::MAX
            || (correction.scroll_sequence != self.sequence
                && Some(correction.scroll_sequence) != resized_from)
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

/// A collection's port facts along its axis (LLP 1070 H1): `main` is the
/// inner height of a vertical list and the inner width of a horizontal one;
/// `origin` is the main-axis padding before the content; `max` the offset's
/// range on the main axis; `padding` the main-axis padding at each end.
struct Geometry {
    axis: ListAxis,
    width: f64,
    height: f64,
    cross: f64,
    origin: f64,
    max: f32,
    padding: [f64; 2],
}
impl Geometry {
    fn main(&self) -> f64 {
        match self.axis {
            ListAxis::Vertical => self.height,
            ListAxis::Horizontal => self.width,
        }
    }
    fn port_cross(&self) -> f64 {
        match self.axis {
            ListAxis::Vertical => self.width,
            ListAxis::Horizontal => self.height,
        }
    }
}
/// The main-axis component of a scroll offset.
fn main_of(axis: ListAxis, offset: (f32, f32)) -> f32 {
    match axis {
        ListAxis::Vertical => offset.1,
        ListAxis::Horizontal => offset.0,
    }
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
    let s = node.style;
    let axis = snapshot.axis;
    // Main-axis padding before and after the content, the cross-axis padding
    // pair, and the inner port along each axis.
    let (origin, end, cross_pad, main, port_cross) = match axis {
        ListAxis::Vertical => (
            pad(s.padding_top),
            pad(s.padding_bottom),
            pad(s.padding_left) + pad(s.padding_right),
            height,
            width,
        ),
        ListAxis::Horizontal => (
            pad(s.padding_left),
            pad(s.padding_right),
            pad(s.padding_top) + pad(s.padding_bottom),
            width,
            height,
        ),
    };
    // A wrapper stretched across reports the actual cross size Taffy offered
    // the row, including its real containing block's percentage padding.
    let cross = snapshot
        .rows
        .first()
        .and_then(|r| kernel.node(r.view))
        .map_or_else(
            || (port_cross - cross_pad).max(0.),
            |wrapper| match axis {
                ListAxis::Vertical => wrapper.frame.width as f64,
                ListAxis::Horizontal => wrapper.frame.height as f64,
            },
        );
    let content = content_size(&node, kernel);
    let (content, frame) = match axis {
        ListAxis::Vertical => (content.1, node.frame.height),
        ListAxis::Horizontal => (content.0, node.frame.width),
    };
    (width > 0. && height > 0. && cross > 0.).then_some(Geometry {
        axis,
        width,
        height,
        cross,
        origin,
        // Native layout may omit the trailing border from content_size. The
        // scroll range must still reach the index's end through the inner port.
        max: (content - frame)
            .max((snapshot.total_extent + origin + end - main) as f32)
            .max(0.),
        padding: [origin, end],
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
        let collections: BTreeSet<_> = self
            .host
            .collections_shallow()
            .iter()
            .map(|s| s.view)
            .collect();
        let mut live = BTreeSet::new();
        // Only nodes with a scroll binding prop can be bound (most of a list's
        // nodes have none): the walk keeps those.
        let kernel = self.host.kernel();
        let bound = if [
            PropId::ScrollTop,
            PropId::ScrollLeft,
            PropId::ScrollFollowEnd,
        ]
        .into_iter()
        .any(|id| kernel.has_prop(id))
        {
            kernel.preorder_where(&self.host.roots(), |_, props| {
                props.get(PropId::ScrollTop).is_some()
                    || props.get(PropId::ScrollLeft).is_some()
                    || props.get(PropId::ScrollFollowEnd).is_some()
            })
        } else {
            Vec::new()
        };
        for view in bound {
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
                // The boot's own offsets are no reader's scroll: a browser
                // page hears none, its input opening after them (rt.js
                // `Booting`; Messages' rows opened at scrollLeft 70).
                if *offset != before && !self.booting {
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
            let event = self.scroll_event(view);
            let result = self.host.dispatch_at(view, event, self.host.now());
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

    #[cfg(any(target_os = "linux", target_os = "android", test))]
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

    #[cfg(any(target_os = "linux", target_os = "android", test))]
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
            if *offset != before {
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
        g.axis == ListAxis::Vertical
            && g.origin == 0.
            && length(self.host.kernel(), node.style.padding_bottom, basis) == 0.
            && self
                .collection
                .cursors
                .get(&node.id)
                .is_some_and(|c| c.sequence == facts.scroll_sequence)
            && facts.scroll_top == self.scroll_of(node.id).1 as f64
            && facts.port_width == g.width
            && facts.port_height == g.height
            && facts.row_width == g.cross
    }

    /// The authored `scroll` event at the port's offset, with its extents
    /// (chat F4): the box is the port, as the scroll range is measured, and
    /// the content is the port plus the range the offset clamps to.
    fn scroll_event(&self, view: ViewId) -> Event {
        let (x, y) = self.scroll_of(view);
        let kernel = self.host.kernel();
        let (port, max) = kernel.node(view).map_or(((0., 0.), (0., 0.)), |node| {
            let bounds = self.display.bounds(kernel, view).unwrap_or_else(|| {
                let limit = self.collection_scroll_limits().get(&view).copied();
                self.brush
                    .scroll_bounds(kernel, self.host.content_region(), &node, limit)
            });
            ((node.frame.width, node.frame.height), bounds.max)
        });
        Event::Scroll(ScrollEvent {
            left: x as f64,
            top: y as f64,
            width: (port.0 + max.0) as f64,
            height: (port.1 + max.1) as f64,
            client_width: port.0 as f64,
            client_height: port.1 as f64,
        })
    }

    pub(super) fn collection_scroll_limits(&self) -> BTreeMap<ViewId, f32> {
        self.host
            .collections_shallow()
            .iter()
            .filter_map(|snapshot| {
                geometry(self.host.kernel(), snapshot, self.viewport.0 as f64)
                    .map(|g| (snapshot.view, g.max))
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
        self.collection.schedule(&self.host.collections_shallow());
        // A host that runs a scroll's pass itself (`defer`) is not woken for it.
        if self.collection.pending() && !self.collection.defer {
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

    /// A wheel moved `view`: its collection turn now, or after the frame
    /// when turns are deferred.
    pub(super) fn collection_scrolled_or_deferred(&mut self, view: ViewId) {
        match &mut self.deferred_collections {
            Some(views) => {
                if !views.contains(&view) {
                    views.push(view);
                }
            }
            None => self.collection_scrolled(view),
        }
    }

    /// Run a scrolled list's collection turn (feedback, rows mounted and
    /// unmounted, the commit's layout) after the frame that shows the new
    /// offset rather than before it: the frame paints the rows already
    /// mounted — the list's overscan covers the travel — and the turn runs
    /// in the time left before the next one (RecyclerView's prefetch after
    /// the frame). The host calls [`Presenter::run_deferred_collections`]
    /// once its frame is submitted.
    pub fn set_deferred_collections(&mut self, on: bool) {
        if !on {
            self.run_deferred_collections();
        }
        self.deferred_collections = on.then(Vec::new);
    }

    /// The collection turns deferred since the last call; whether any ran.
    pub fn run_deferred_collections(&mut self) -> bool {
        let views = match &mut self.deferred_collections {
            Some(views) if !views.is_empty() => std::mem::take(views),
            _ => return false,
        };
        for id in views {
            self.collection_scrolled(id);
        }
        true
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
            let event = self.scroll_event(view);
            self.host.dispatch_at(view, event, self.host.now())
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
        } else if self.collection.defer {
            self.queue_collections();
            None
        } else {
            self.queue_collections();
            self.refine_collections()
        });
        if let Some(error) = error {
            self.host.log(error);
        }
    }

    /// A host that defers scroll's collection passes: they run now (rows
    /// mount and retire). Whether the presenter wants a frame after them.
    #[cfg(target_os = "android")]
    pub(crate) fn refine_deferred(&mut self, defer: bool) -> bool {
        self.collection.defer = defer;
        self.collection.hold = false;
        if self.collection.pending() {
            if let Some(error) = self.refine_collections() {
                self.host.log(error);
            }
        }
        self.dirty
    }

    /// Slice the next passes ([`State::limit`]) with the scrolled list's
    /// velocity, or build whole windows (`None`).
    #[cfg(target_os = "android")]
    pub(crate) fn slice_collections(&mut self, limit: Option<u32>, velocity: f64) {
        self.collection.limit = limit;
        self.collection.velocity = self.last_wheel.map(|v| (v, velocity));
    }

    /// Whether any list owes another report (a slice left rows unbuilt).
    #[cfg(target_os = "android")]
    pub(crate) fn collections_pending(&self) -> bool {
        self.collection.pending()
    }

    /// Hold collection passes (a frame a scroll draws) or let them run.
    #[cfg(target_os = "android")]
    pub(crate) fn hold_collections(&mut self, hold: bool) {
        self.collection.hold = hold;
    }

    /// The agent's `clock settle`: every queued report, nested lists'
    /// included, before the fixed point is read (LLP 1070 G3). Bounded.
    pub(crate) fn settle_collections(&mut self) -> Option<String> {
        let mut error = None;
        for _ in 0..16 {
            if !self.collection.pending() {
                break;
            }
            error = error.or(self.refine_collections());
        }
        error
    }
    pub(super) fn refine_collections(&mut self) -> Option<String> {
        if self.collection.hold {
            return None;
        }
        let mut error = None;
        // Lists whose port this pass moved: a browser's `scroll` follows a
        // write to scrollTop (list.js's), a correction's or a request's.
        let mut moved = Vec::new();
        // A sliced pass reports each list once: its pending rest waits for
        // the host's next pass, in the next frame's idle time.
        let mut sliced = BTreeSet::new();
        for _ in 0..PASSES {
            let Some(view) = self.collection.queue.pop_front() else {
                break;
            };
            if self.collection.limit.is_some() && !sliced.insert(view) {
                self.collection.queue.push_front(view);
                break;
            }
            // This list's snapshot; every list's only to find a pin's owner.
            let Some(snapshot) = self.host.collection(view) else {
                self.collection.cursors.remove(&view);
                continue;
            };
            let snapshot = &snapshot;
            let all = std::cell::OnceCell::new();
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
            let planned = cursor.sequence;
            cursor.geometry((g.width, g.height, g.cross, g.origin));
            let resized_from = (cursor.sequence != planned).then_some(planned);
            // Match the browser's post-layout scrollTop (scrollLeft on a
            // horizontal list) prop write. Consume each changed request once;
            // an unchanged binding never owns the reader's offset. Advance the
            // sequence so old anchor corrections cannot override an explicit
            // Latest/jump request. The target is feedback before it paints,
            // so its rows are built before the port moves.
            let axis = g.axis;
            let (prop, requested_was) = match axis {
                ListAxis::Vertical => (PropId::ScrollTop, &mut cursor.requested_top),
                ListAxis::Horizontal => (PropId::ScrollLeft, &mut cursor.requested_left),
            };
            let requested = self
                .host
                .kernel()
                .node(view)
                .and_then(|node| {
                    node.props
                        .get(prop)
                        .and_then(exact_kernel::PropValue::as_float)
                })
                .filter(|main| main.is_finite());
            let before = self.scroll.get(&view).copied();
            if requested != *requested_was {
                *requested_was = requested;
                if let Some(main) = requested {
                    cursor.advance();
                    cursor.model_main(
                        key,
                        axis,
                        main.clamp(0., g.max as f64) as f32,
                        self.display.attached(),
                        self.scroll.entry(view).or_default(),
                    );
                    self.dirty = true;
                }
            }
            // An authored request this pass is a jump: nothing planned before it lands.
            let resized_from = resized_from.filter(|_| cursor.sequence == planned + 1);
            if let Some(main) = cursor.correction(snapshot, resized_from) {
                cursor.model_main(
                    key,
                    axis,
                    ((main + g.origin) as f32).clamp(0., g.max),
                    self.display.attached(),
                    self.scroll.entry(view).or_default(),
                );
                self.dirty = true;
            }
            if self.scroll.get(&view).copied() != before {
                moved.push((view, key));
            }
            let feedback_main = if let Some(pending) = cursor.model_scroll.as_mut() {
                let slot = match axis {
                    ListAxis::Vertical => &mut pending.top,
                    ListAxis::Horizontal => pending.left.get_or_insert(0.),
                };
                let main = slot.clamp(0., g.max);
                self.dirty |= main != *slot;
                *slot = main;
                main
            } else {
                self.scroll.get(&view).map_or(0., |off| main_of(axis, *off))
            };
            let feedback = CollectionFeedback {
                view,
                revision: snapshot.revision,
                scroll_sequence: cursor.sequence,
                // From the first row: negative in the padding before it, down
                // to that padding (LLP 1010 §6.9).
                offset: (feedback_main as f64 - g.origin).max(-g.origin),
                port_main: g.main(),
                port_cross: g.port_cross(),
                cross: g.cross,
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
                                size: match axis {
                                    ListAxis::Vertical => node.frame.height,
                                    ListAxis::Horizontal => node.frame.width,
                                } as f64,
                            })
                    })
                    .collect(),
                focus_view: self.focus.filter(|_| {
                    let snapshots = all.get_or_init(|| self.host.collections());
                    pin_owner(self.host.kernel(), snapshots, self.focus) == Some(view)
                }),
                interaction_view: self.collection.interaction.filter(|_| {
                    retained_pin
                        .filter(|(pin, _)| Some(*pin) == self.collection.interaction)
                        .map(|p| p.1)
                        .or_else(|| {
                            let snapshots = all.get_or_init(|| self.host.collections());
                            pin_owner(self.host.kernel(), snapshots, self.collection.interaction)
                        })
                        == Some(view)
                }),
            };
            // Unchanged facts and padding are news only to a list a slice left
            // pending.
            let sent = (feedback.clone(), g.padding);
            if cursor.sent.as_ref() == Some(&sent) && !snapshot.pending {
                continue;
            }
            cursor.sent = Some(sent);
            let fill = CollectionFill {
                velocity: self
                    .collection
                    .velocity
                    .filter(|(v, _)| *v == view)
                    .map_or(0.0, |(_, v)| v),
                limit: self.collection.limit,
                ..CollectionFill::default()
            };
            match self.host.collection_feedback_filled(feedback, fill) {
                Ok(true) => {
                    let after = self.sync_commit();
                    error = error.or(after);
                    self.collection.schedule(&self.host.collections_shallow());
                }
                Ok(false) => {}
                Err(why) => {
                    // An edge action can refuse after geometry committed. Run
                    // the same post-commit synchronization as timer refusals.
                    let after = self.sync_commit();
                    error = error.or(Some(why));
                    error = error.or(after);
                    self.collection.schedule(&self.host.collections_shallow());
                }
            }
        }
        for &(view, key) in &moved {
            self.collection.scroll_event(view, key);
        }
        if self.collection.pending() || !moved.is_empty() {
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
            axis: exact_runner::ListAxis::Vertical,
            parent: None,
            restored: false,
            seeking: false,
            revision: 4,
            scroll_sequence: 2,
            count: 0,
            total_extent: 0.,
            rows: vec![],
            correction: Some(AnchorCorrection {
                scroll_sequence: 2,
                offset: 200.,
                from: None,
                smooth: false,
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
        assert_eq!(cursor.correction(&s, None), Some(200.));
        assert_eq!(cursor.correction(&s, None), None);
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.advance();
        assert_eq!(cursor.correction(&s, None), None);
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.geometry((200., 300., 180., 10.));
        cursor.geometry((190., 300., 170., 10.));
        assert_eq!(cursor.correction(&s, None), None);
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.geometry((200., 300., 180., 10.));
        cursor.geometry((200., 300., 180., 20.));
        assert_eq!(
            cursor.correction(&s, None),
            None,
            "origin changes also invalidate old corrections"
        );
        // A resize in the pass that brings the correction (feed F14) is
        // not the reader: the correction planned before it lands.
        let mut cursor = Cursor {
            sequence: 2,
            ..Cursor::default()
        };
        cursor.geometry((200., 300., 180., 10.));
        cursor.geometry((200., 348., 180., 10.));
        assert_eq!(cursor.correction(&s, Some(2)), Some(200.));
        let mut cursor = Cursor {
            sequence: u64::MAX,
            ..Cursor::default()
        };
        let mut overflow = s;
        overflow.correction.as_mut().unwrap().scroll_sequence = u64::MAX;
        assert_eq!(cursor.correction(&overflow, None), None);
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
        assert_eq!(facts.cross, 60.);
        assert_eq!(facts.origin, 0.);
    }
}
