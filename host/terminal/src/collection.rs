//! Vertical viewport collections on the terminal's row-based scroller.
//! @ref LLP 1010 §6.5/§6.9 — the runner owns windows and anchors; the host
//! reports the actual port and wrapper measurements after layout.

use crate::host::Host;
use exact_kernel::{Display, PropId, ViewId};
use exact_runner::{CollectionFeedback, DataSource, ListAxis, RowMeasurement};
use std::collections::HashMap;

mod focus;

#[derive(Default)]
pub(crate) struct Collections {
    pub(crate) settling: bool,
    pub(crate) focus_layer: Option<ViewId>,
    cursors: HashMap<ViewId, Cursor>,
}

#[derive(Default)]
struct Cursor {
    sequence: u64,
    corrected: Option<u64>,
    requested: Option<f64>,
    sent: Option<CollectionFeedback>,
}

impl<D: DataSource> Host<D> {
    /// A person's scroll, or focus reveal, supersedes an older correction.
    pub(crate) fn collection_scrolled(&mut self, view: ViewId) {
        if let Some(cursor) = self.collections.cursors.get_mut(&view) {
            cursor.sequence = cursor.sequence.saturating_add(1);
        }
    }

    fn collection_visible(&self, view: ViewId) -> bool {
        let kernel = self.kernel();
        let mut at = Some(view);
        while let Some(id) = at {
            let Some(n) = kernel.node(id) else {
                return false;
            };
            if n.style.display == Display::None
                || (Self::is_layer(&n) && !self.layers.iter().any(|(open, _)| *open == id))
            {
                return false;
            }
            at = n.parent;
        }
        true
    }

    pub(crate) fn inside(&self, view: ViewId, ancestor: ViewId) -> bool {
        let mut at = Some(view);
        while let Some(id) = at {
            if id == ancestor {
                return true;
            }
            at = self.kernel().node(id).and_then(|n| n.parent);
        }
        false
    }

    /// Settle measured windows before painting. A feedback commit can run an
    /// edge action and change the tree, so use fresh snapshots on each pass.
    /// A finite pass budget also bounds an app that keeps growing at an edge.
    pub(crate) fn settle_collections(&mut self) {
        if self.collections.settling {
            return;
        }
        self.collections.settling = true;
        for _ in 0..16 {
            // The runner returns immediately when the plan has no collections:
            // ordinary terminal apps do not acquire another tree traversal.
            let snapshots = self.runner.collections();
            if snapshots.is_empty() {
                self.collections.cursors.clear();
                break;
            }
            let mut at = self.focus;
            let mut focus_owner = None;
            while let Some(view) = at {
                if let Some(owner) = snapshots
                    .iter()
                    .find(|s| s.rows.iter().any(|r| r.view == view))
                {
                    focus_owner = Some(owner.view);
                    break;
                }
                at = self.kernel().node(view).and_then(|n| n.parent);
            }
            self.collections
                .cursors
                .retain(|view, _| snapshots.iter().any(|s| s.view == *view));
            let mut changed = false;
            for snapshot in snapshots {
                // Horizontal scrolling is not part of the terminal host yet.
                if snapshot.axis != ListAxis::Vertical {
                    continue;
                }
                self.collections.cursors.entry(snapshot.view).or_default();
                let visible = self.collection_visible(snapshot.view);
                let kernel = self.kernel();
                let Some(node) = kernel.node(snapshot.view) else {
                    continue;
                };
                let [bt, br, bb, bl] = node.style.border_widths_in(&kernel.env());
                let (pl, pt, pr, pb) = kernel.resolved_padding(node.key).unwrap_or_default();
                let port_main = if visible {
                    (node.frame.height - bt - bb).max(0.0) as f64
                } else {
                    0.0
                };
                let port_cross = (node.frame.width - bl - br).max(0.0) as f64;
                let cross = snapshot
                    .rows
                    .first()
                    .and_then(|row| kernel.node(row.view))
                    .map_or((port_cross - (pl + pr) as f64).max(0.0), |row| {
                        row.frame.width as f64
                    });
                let cross = if visible {
                    cross
                } else {
                    self.collections.cursors[&snapshot.view]
                        .sent
                        .as_ref()
                        .map_or(cross, |f| f.cross)
                };
                if visible && (port_main == 0.0 || port_cross == 0.0 || cross == 0.0) {
                    continue;
                }
                let origin = pt as f64;
                let reach = (snapshot.total_extent + (pt + pb) as f64 - port_main)
                    .max(node.content.1 as f64 - port_main)
                    .max(0.0);
                let requested = node
                    .props
                    .get(PropId::ScrollTop)
                    .and_then(exact_kernel::PropValue::as_float)
                    .filter(|v| v.is_finite());
                let measurements = snapshot
                    .rows
                    .iter()
                    .filter(|_| visible)
                    .filter_map(|row| {
                        kernel.node(row.view).map(|n| RowMeasurement {
                            view: row.view,
                            epoch: row.epoch,
                            size: n.frame.height as f64,
                        })
                    })
                    .collect();
                let focus_view = self
                    .focus
                    .filter(|_| visible && focus_owner == Some(snapshot.view));
                let cursor = self.collections.cursors.entry(snapshot.view).or_default();
                cursor.sequence = cursor.sequence.max(snapshot.scroll_sequence);
                let offset = self.scroll.entry(snapshot.view).or_insert(0.0);
                let before = *offset;
                if requested != cursor.requested {
                    cursor.requested = requested;
                    if let Some(requested) = requested {
                        cursor.sequence = cursor.sequence.saturating_add(1);
                        *offset = requested.clamp(0.0, reach) as f32;
                    }
                }
                if let Some(correction) = snapshot.correction.filter(|c| {
                    c.scroll_sequence == cursor.sequence
                        && cursor.corrected.is_none_or(|r| r < snapshot.revision)
                }) {
                    cursor.corrected = Some(snapshot.revision);
                    let next = match correction.from {
                        Some(from) => *offset as f64 + correction.offset - from,
                        None => correction.offset + origin,
                    };
                    *offset = next.clamp(0.0, reach) as f32;
                }
                // A layout clamp is not another user scroll and must not
                // invalidate the runner's pending anchor or end correction.
                *offset = (*offset as f64).clamp(0.0, reach) as f32;
                let feedback = CollectionFeedback {
                    view: snapshot.view,
                    revision: snapshot.revision,
                    scroll_sequence: cursor.sequence,
                    offset: *offset as f64 - origin,
                    port_main,
                    port_cross,
                    cross,
                    measurements,
                    focus_view,
                    interaction_view: None,
                };
                let moved = *offset != before;
                if cursor.sent.as_ref() == Some(&feedback) && !snapshot.pending {
                    if moved {
                        self.changed();
                    }
                    continue;
                }
                cursor.sent = Some(feedback.clone());
                let result = self.runner.collection_feedback(feedback);
                match result {
                    Ok(advanced) => {
                        let committed = !advanced.receipts.is_empty();
                        if committed || advanced.error.is_some() {
                            self.after_commit();
                        }
                        if let Some(error) = advanced.error {
                            eprintln!("terminal list #{}: {error:?}", snapshot.view);
                        }
                        changed |= committed || snapshot.pending;
                    }
                    Err(error) => {
                        eprintln!("terminal list #{}: {error:?}", snapshot.view);
                    }
                }
                if moved {
                    self.changed();
                }
            }
            if !changed {
                break;
            }
        }
        self.collections.settling = false;
        if let Some(layer) = self.collections.focus_layer.take() {
            if self.layers.last().is_some_and(|(top, _)| *top == layer) {
                self.focus_open_layer();
            }
        }
    }
}
