//! Numeric host seam for an opt-in viewport collection. Coordinates are logical
//! CSS pixels/points, relative to the list's content origin (excluding padding).
use exact_kernel::ViewId;
use exact_plan::bytes::{Reader, Writer};

/// A mounted wrapper's measured border-box height, using its published epoch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowMeasurement {
    /// Wrapper view, not the authored child root.
    pub view: ViewId,
    /// Epoch copied from the snapshot used for this layout.
    pub epoch: u64,
    /// Finite nonnegative border-box height.
    pub height: f64,
}
/// Facts from the actual nested scrollport. Never use the window's size instead.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionFeedback {
    /// List view; view IDs are never reused during a runner lifetime.
    pub view: ViewId,
    /// Snapshot revision these facts describe.
    pub revision: u64,
    /// Monotonically increasing host scroll/layout sequence.
    pub scroll_sequence: u64,
    /// Actual content-relative vertical offset, clamped past rubber-banding.
    pub scroll_top: f64,
    /// Actual inner scrollport width.
    pub port_width: f64,
    /// Actual inner scrollport height.
    pub port_height: f64,
    /// Available row width after content padding and any scrollbar reservation.
    pub row_width: f64,
    /// Only currently mounted row wrappers may be measured.
    pub measurements: Vec<RowMeasurement>,
    /// Focused authored descendant; pins at most its containing row.
    pub focus_view: Option<ViewId>,
    /// Active interaction descendant; pins at most one additional row.
    pub interaction_view: Option<ViewId>,
}
/// What one report may build beyond the rows it owes (LLP 1050.000 §6).
/// The host owns time: it turns its slice into `limit` from measured cost.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CollectionFill {
    /// The scrollport's velocity, logical px/s, positive toward the end.
    /// Leads the window in the direction of travel.
    pub velocity: f64,
    /// Rows this report may create beyond the owed set (visible and pinned
    /// rows), and a bound on the rows it retires. `None` is unlimited.
    pub limit: Option<u32>,
}
/// A mounted row; unmounted keys and records never cross the host seam.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionRow {
    /// Noninteractive wrapper, measured as one flow root.
    pub view: ViewId,
    /// Authored root within the wrapper.
    pub root: ViewId,
    /// Zero-based logical item position.
    pub index: usize,
    /// Logical top, including estimated heights before this item.
    pub top: f64,
    /// Current measured height or provisional estimate.
    pub height: f64,
    /// Copy verbatim into feedback; changes on remount, width/content changes.
    pub epoch: u64,
    /// Whether the estimate has been confirmed in this epoch.
    pub measured: bool,
}
/// Apply only if the host has not advanced beyond `scroll_sequence`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorCorrection {
    /// Scroll sequence whose anchor was preserved.
    pub scroll_sequence: u64,
    /// Corrected content-relative scroll position.
    pub scroll_top: f64,
}
/// Current O(mounted rows) host metadata, separate from the ordinary kernel ops.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionSnapshot {
    /// List view and collection identity for this runner lifetime.
    pub view: ViewId,
    /// Monotonic revision; old feedback cannot mutate a newer snapshot.
    pub revision: u64,
    /// Last accepted host sequence.
    pub scroll_sequence: u64,
    /// Logical item count, including unmounted rows.
    pub count: usize,
    /// Measured plus estimated full content height.
    pub total_extent: f64,
    /// Only live wrappers, in logical order, including at most two pinned rows.
    pub rows: Vec<CollectionRow>,
    /// Optional anchor correction; consume at most once per revision.
    pub correction: Option<AnchorCorrection>,
    /// A limited report left window rows unbuilt or rows past the window
    /// mounted: the host owes another report, even with unchanged facts.
    pub pending: bool,
}
/// Malformed or nonfinite host feedback. Stale valid feedback is ignored instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackError;

impl CollectionFeedback {
    /// [`CollectionFeedback::encode_with`] an unlimited, motionless fill.
    pub fn encode(&self) -> Result<Vec<u8>, FeedbackError> {
        self.encode_with(CollectionFill::default())
    }
    /// Portable LE wire version 2: u32 version, u32 view, u64 revision,
    /// u64 sequence, f64 top/port_width/port_height/row_width, u32 focus and
    /// interaction (zero means none), f64 velocity, u32 limit (`u32::MAX`
    /// means none), u32 count, then count × (u32 wrapper, u64 epoch, f64
    /// height). No keys, strings, or JSON parsing.
    pub fn encode_with(&self, fill: CollectionFill) -> Result<Vec<u8>, FeedbackError> {
        self.validate()?;
        if !fill.velocity.is_finite() || fill.limit == Some(u32::MAX) {
            return Err(FeedbackError);
        }
        let mut w = Writer::default();
        w.u32(2);
        w.u32(self.view);
        w.u64(self.revision);
        w.u64(self.scroll_sequence);
        for n in [
            self.scroll_top,
            self.port_width,
            self.port_height,
            self.row_width,
        ] {
            w.f64(n);
        }
        w.u32(self.focus_view.unwrap_or(0));
        w.u32(self.interaction_view.unwrap_or(0));
        w.f64(fill.velocity);
        w.u32(fill.limit.unwrap_or(u32::MAX));
        w.u32(
            self.measurements
                .len()
                .try_into()
                .map_err(|_| FeedbackError)?,
        );
        for row in &self.measurements {
            w.u32(row.view);
            w.u64(row.epoch);
            w.f64(row.height);
        }
        Ok(w.into_vec())
    }
    /// The facts of [`CollectionFeedback::decode_with_fill`].
    pub fn decode(bytes: &[u8]) -> Result<Self, FeedbackError> {
        Self::decode_with_fill(bytes).map(|(facts, _)| facts)
    }
    /// Decode once in the common runner layer. Reject trailing/truncated bytes,
    /// duplicate wrappers, zero identities, and invalid numbers before mutation.
    pub fn decode_with_fill(bytes: &[u8]) -> Result<(Self, CollectionFill), FeedbackError> {
        let mut r = Reader::new(bytes);
        let out = (|| -> Result<(Self, CollectionFill), exact_plan::PlanError> {
            if r.u32()? != 2 {
                return Err(exact_plan::PlanError::BadCount(0));
            }
            let view = r.u32()?;
            let revision = r.u64()?;
            let scroll_sequence = r.u64()?;
            let scroll_top = r.f64()?;
            let port_width = r.f64()?;
            let port_height = r.f64()?;
            let row_width = r.f64()?;
            let focus = r.u32()?;
            let interaction = r.u32()?;
            let velocity = r.f64()?;
            let limit = r.u32()?;
            let fill = CollectionFill {
                velocity,
                limit: (limit != u32::MAX).then_some(limit),
            };
            let count = r.u32()? as usize;
            if count.checked_mul(20) != Some(r.remaining()) {
                return Err(exact_plan::PlanError::BadCount(count as u32));
            }
            let mut measurements = Vec::with_capacity(count);
            for _ in 0..count {
                measurements.push(RowMeasurement {
                    view: r.u32()?,
                    epoch: r.u64()?,
                    height: r.f64()?,
                });
            }
            let facts = Self {
                view,
                revision,
                scroll_sequence,
                scroll_top,
                port_width,
                port_height,
                row_width,
                measurements,
                focus_view: (focus != 0).then_some(focus),
                interaction_view: (interaction != 0).then_some(interaction),
            };
            Ok((facts, fill))
        })()
        .map_err(|_| FeedbackError)?;
        out.0.validate()?;
        if !out.1.velocity.is_finite() {
            return Err(FeedbackError);
        }
        Ok(out)
    }
    pub(crate) fn validate(&self) -> Result<(), FeedbackError> {
        let valid = |n: f64| n.is_finite() && n >= 0.0 && n <= f32::MAX as f64;
        if self.view == 0
            || self.focus_view == Some(0)
            || self.interaction_view == Some(0)
            || ![
                self.scroll_top,
                self.port_width,
                self.port_height,
                self.row_width,
            ]
            .into_iter()
            .all(valid)
        {
            return Err(FeedbackError);
        }
        let mut seen = std::collections::BTreeSet::new();
        for row in &self.measurements {
            if row.view == 0 || !valid(row.height) || !seen.insert(row.view) {
                return Err(FeedbackError);
            }
        }
        Ok(())
    }
}

/// Array for existing host batch envelopes. Contains no keys or application text.
/// Revisions, scroll sequences, and row epochs are decimal strings to preserve
/// all u64 values in JavaScript; view IDs, counts, and indices remain numbers.
pub fn snapshots_json(snapshots: &[CollectionSnapshot]) -> String {
    use std::fmt::Write;
    let mut out = String::from("[");
    for (i, c) in snapshots.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{{\"view\":{},\"revision\":\"{}\",\"scrollSequence\":\"{}\",\"count\":{},\"totalExtent\":{},\"rows\":[", c.view, c.revision, c.scroll_sequence, c.count, exact_num::Shortest(c.total_extent)).unwrap();
        for (i, row) in c.rows.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write!(out, "{{\"view\":{},\"root\":{},\"index\":{},\"top\":{},\"height\":{},\"epoch\":\"{}\",\"measured\":{}}}", row.view, row.root, row.index, exact_num::Shortest(row.top), exact_num::Shortest(row.height), row.epoch, row.measured).unwrap();
        }
        write!(out, "],\"pending\":{},\"correction\":", c.pending).unwrap();
        if let Some(correction) = c.correction {
            write!(
                out,
                "{{\"scrollSequence\":\"{}\",\"scrollTop\":{}}}",
                correction.scroll_sequence,
                exact_num::Shortest(correction.scroll_top)
            )
            .unwrap();
        } else {
            out.push_str("null");
        }
        out.push('}');
    }
    out.push(']');
    out
}

impl CollectionSnapshot {
    /// This snapshot as the common camelCase host JSON object.
    pub fn json(&self) -> String {
        let array = snapshots_json(std::slice::from_ref(self));
        array[1..array.len() - 1].to_owned()
    }
    /// Append this snapshot to an existing host batch envelope.
    pub fn write_json(&self, out: &mut String) {
        out.push_str(&self.json());
    }
}
