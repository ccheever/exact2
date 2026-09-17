//! Bounded native post-layout collection feedback. The kernel owns row layout;
//! the runner owns membership, estimates and anchors. No recursive frame/layout.
use super::*;
use exact_kernel::{Dimension, Kernel};
use exact_runner::{CollectionFeedback, CollectionSnapshot, RowMeasurement};
use std::collections::{BTreeSet, VecDeque};

const PASSES: usize = 2;

#[derive(Default)]
pub(super) struct State {
    cursors: BTreeMap<ViewId, Cursor>,
    queue: VecDeque<ViewId>,
    interaction: Option<ViewId>,
}
#[derive(Default)]
struct Cursor {
    sequence: u64,
    corrected: Option<u64>,
    dimensions: Option<(f64, f64, f64, f64)>,
    sent: Option<CollectionFeedback>,
    queued: bool,
}
impl Cursor {
    fn advance(&mut self) {
        self.sequence = self.sequence.saturating_add(1);
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
        Some(correction.scroll_top)
    }
}
impl State {
    pub(super) fn pending(&self) -> bool {
        !self.queue.is_empty()
    }
    pub(super) fn advance_all(&mut self) {
        for cursor in self.cursors.values_mut() {
            cursor.advance();
        }
    }
    fn schedule(&mut self, snapshots: &[CollectionSnapshot]) {
        let live: BTreeSet<_> = snapshots.iter().map(|s| s.view).collect();
        self.cursors.retain(|id, _| live.contains(id));
        self.queue.retain(|id| live.contains(id));
        for snapshot in snapshots {
            let cursor = self.cursors.entry(snapshot.view).or_insert_with(|| Cursor {
                sequence: snapshot.scroll_sequence,
                ..Cursor::default()
            });
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

    pub(super) fn queue_collections(&mut self) {
        if self.collection.interaction.is_some_and(|id| {
            self.host.kernel().node(id).is_none() || self.host.route_visibility(id).1
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
            self.collection.interaction = view;
            self.queue_collections();
            self.dirty = true;
        }
    }

    /// Current live interaction target. Carriers discard held pointer state when
    /// navigation or runner replacement invalidates its pin.
    pub fn collection_interaction(&self) -> Option<ViewId> {
        self.collection.interaction.filter(|id| {
            self.host.kernel().node(*id).is_some() && !self.host.route_visibility(*id).1
        })
    }

    pub(super) fn collection_scrolled(&mut self, view: ViewId) {
        if let Some(cursor) = self.collection.cursors.get_mut(&view) {
            cursor.advance();
        }
        // Collection observation supplements the ordinary authored handler.
        let error = if self
            .host
            .runner()
            .handlers_of(view)
            .contains(&EventKind::Scroll)
        {
            let (x, y) = self.scroll_of(view);
            let error =
                self.host
                    .dispatch_at(view, Event::Scroll(x as f64, y as f64), self.host.now());
            let after = self.after_commit();
            error.or(after)
        } else {
            self.queue_collections();
            self.refine_collections()
        };
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
            let cursor = self.collection.cursors.get_mut(&view).unwrap();
            cursor.queued = false;
            if self.host.route_visibility(view).0 {
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
            if let Some(top) = cursor.correction(snapshot) {
                let off = self.scroll.entry(view).or_default();
                off.1 = ((top + g.padding_top) as f32).clamp(0., g.max_top);
                self.dirty = true;
            }
            let feedback = CollectionFeedback {
                view,
                revision: snapshot.revision,
                scroll_sequence: cursor.sequence,
                scroll_top: (self.scroll.get(&view).map_or(0., |off| off.1) as f64 - g.padding_top)
                    .max(0.),
                port_width: g.width,
                port_height: g.height,
                row_width: g.row_width,
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
                                height: node.frame.height as f64,
                            })
                    })
                    .collect(),
                focus_view: self.focus.filter(|_| {
                    pin_owner(self.host.kernel(), &snapshots, self.focus) == Some(view)
                }),
                interaction_view: self.collection.interaction.filter(|_| {
                    pin_owner(self.host.kernel(), &snapshots, self.collection.interaction)
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
                    error = error.or(Some(why));
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
                scroll_top: 200.,
            }),
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
