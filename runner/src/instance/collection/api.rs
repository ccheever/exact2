//! Numeric host seam for an opt-in viewport collection. Coordinates are logical
//! CSS pixels/points, relative to the list's content origin (excluding padding).
use exact_kernel::ViewId;
use exact_plan::bytes::{Reader, Writer};

/// A mounted wrapper's measured border-box size on the list's main axis,
/// using its published epoch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowMeasurement {
    /// Wrapper view, not the authored child root.
    pub view: ViewId,
    /// Epoch copied from the snapshot used for this layout.
    pub epoch: u64,
    /// Finite nonnegative border-box size along the main axis.
    pub size: f64,
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
    /// Actual content-relative offset on the main axis, clamped past
    /// rubber-banding: `scrollTop` for a vertical list.
    pub offset: f64,
    /// Actual inner scrollport size on the main axis.
    pub port_main: f64,
    /// Actual inner scrollport size on the cross axis.
    pub port_cross: f64,
    /// The rows' available cross size, after content padding and any
    /// scrollbar reservation: a vertical list's row width.
    pub cross: f64,
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
    /// An ancestor list is being dragged, flung or wheeled (LLP 1070 F2).
    /// Carried on the wire from version 3; nesting reads it.
    pub ancestor_moving: bool,
    /// Build only (LLP 1072 §5): a report the host commits off its main
    /// thread creates rows and destroys none. Rows the window no longer
    /// wants stay mounted and no edge action runs; the report is `pending`,
    /// and the host's next report, made where it applies, settles both.
    /// Refused with a new port, width or pin: those reports are immediate.
    pub create_only: bool,
    /// Retire only (LLP 1072 §5): the report builds what it owes and no
    /// optional row, retires within `limit` as a slice does, and runs edge
    /// actions. The immediate report after a build-only one.
    pub no_build: bool,
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
    /// Logical start on the main axis, including estimated sizes before
    /// this item.
    pub start: f64,
    /// Current measured size or provisional estimate.
    pub size: f64,
    /// Copy verbatim into feedback; changes on remount, width/content changes.
    pub epoch: u64,
    /// Whether the estimate has been confirmed in this epoch.
    pub measured: bool,
}
/// Apply only if the host has not advanced beyond `scroll_sequence`, unless
/// it is relative (`from`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorCorrection {
    /// Scroll sequence whose anchor was preserved.
    pub scroll_sequence: u64,
    /// Corrected content-relative offset on the main axis. Negative only
    /// for an authored `scrollIntoView` into the padding before the first
    /// row, down to that padding: `scrollTop` 0 (LLP 1010 §6.9).
    pub offset: f64,
    /// An anchor's correction: the offset the anchor was taken at, so
    /// `offset - from` is how far the content before it moved. A host may
    /// add that to the offset it has now, whatever it sampled since, in the
    /// frame that lays the moved rows out (a fling keeps its velocity).
    /// None: an authored position (a restore, `scrollIntoView`), absolute.
    pub from: Option<f64>,
    /// Animate to `offset` with the platform's scroll animation (a smooth
    /// `scrollIntoView`, or a `scroll-behavior: smooth` list following its
    /// end), rather than set it (LLP 1070.000 §6.2). Absolute only.
    pub smooth: bool,
}
/// The axis a collection scrolls on, fixed when it is created from its
/// list's style (LLP 1070 H1): `display: block` is vertical, `display:
/// flex` (a `row`) horizontal. JSON names it as CSS's `scroll-snap-type`
/// and `overscroll-behavior-x` do: `y` or `x`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListAxis {
    /// Rows stack top to bottom; the main axis is `y`.
    #[default]
    Vertical,
    /// Items run left to right; the main axis is `x`.
    Horizontal,
}
impl ListAxis {
    /// `y` or `x`, as CSS names an axis.
    pub fn name(self) -> &'static str {
        match self {
            ListAxis::Vertical => "y",
            ListAxis::Horizontal => "x",
        }
    }
}
/// Current O(mounted rows) host metadata, separate from the ordinary kernel ops.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionSnapshot {
    /// List view and collection identity for this runner lifetime.
    pub view: ViewId,
    /// The main axis: what `offset`, `start`, `size` and the extent measure.
    pub axis: ListAxis,
    /// The outer list whose mounted row holds this one (LLP 1070 G3).
    pub parent: Option<ViewId>,
    /// It started where a kept position said (LLP 1070 §4.2).
    pub restored: bool,
    /// A `scrollIntoView` is under way (LLP 1070.000): its corrections are
    /// authored moves, which a host applies whatever it has sampled since,
    /// until the reader's own input takes the port over.
    pub seeking: bool,
    /// Monotonic revision; old feedback cannot mutate a newer snapshot.
    pub revision: u64,
    /// Last accepted host sequence.
    pub scroll_sequence: u64,
    /// Logical item count, including unmounted rows.
    pub count: usize,
    /// Measured plus estimated full content extent on the main axis.
    pub total_extent: f64,
    /// Only live wrappers, in logical order, including at most two pinned
    /// rows and, where rows are rebound, the few held past the window for
    /// the rows it needs next (LLP 1078).
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
    /// Portable LE wire version 3 (LLP 1070 H1): u32 version, u32 view, u64
    /// revision, u64 sequence, f64 offset/port_main/port_cross/cross, u32
    /// focus and interaction (zero means none), f64 velocity, u32 limit
    /// (`u32::MAX` means none), u32 flags (bit 0: an ancestor list is
    /// moving; bit 1: build only; bit 2: retire only), u32 count, then count × (u32 wrapper, u64 epoch, f64 size).
    /// Main and cross are the list's axes. No keys, strings, or JSON parsing.
    pub fn encode_with(&self, fill: CollectionFill) -> Result<Vec<u8>, FeedbackError> {
        self.validate()?;
        if !fill.velocity.is_finite() || fill.limit == Some(u32::MAX) {
            return Err(FeedbackError);
        }
        let mut w = Writer::default();
        w.u32(3);
        w.u32(self.view);
        w.u64(self.revision);
        w.u64(self.scroll_sequence);
        for n in [self.offset, self.port_main, self.port_cross, self.cross] {
            w.f64(n);
        }
        w.u32(self.focus_view.unwrap_or(0));
        w.u32(self.interaction_view.unwrap_or(0));
        w.f64(fill.velocity);
        w.u32(fill.limit.unwrap_or(u32::MAX));
        if fill.create_only && fill.no_build {
            return Err(FeedbackError);
        }
        w.u32(
            u32::from(fill.ancestor_moving)
                | u32::from(fill.create_only) << 1
                | u32::from(fill.no_build) << 2,
        );
        w.u32(
            self.measurements
                .len()
                .try_into()
                .map_err(|_| FeedbackError)?,
        );
        for row in &self.measurements {
            w.u32(row.view);
            w.u64(row.epoch);
            w.f64(row.size);
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
            if r.u32()? != 3 {
                return Err(exact_plan::PlanError::BadCount(0));
            }
            let view = r.u32()?;
            let revision = r.u64()?;
            let scroll_sequence = r.u64()?;
            let offset = r.f64()?;
            let port_main = r.f64()?;
            let port_cross = r.f64()?;
            let cross = r.f64()?;
            let focus = r.u32()?;
            let interaction = r.u32()?;
            let velocity = r.f64()?;
            let limit = r.u32()?;
            let flags = r.u32()?;
            if flags > 7 || flags & 6 == 6 {
                return Err(exact_plan::PlanError::BadCount(flags));
            }
            let fill = CollectionFill {
                velocity,
                limit: (limit != u32::MAX).then_some(limit),
                ancestor_moving: flags & 1 != 0,
                create_only: flags & 2 != 0,
                no_build: flags & 4 != 0,
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
                    size: r.f64()?,
                });
            }
            let facts = Self {
                view,
                revision,
                scroll_sequence,
                offset,
                port_main,
                port_cross,
                cross,
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
            || ![self.offset, self.port_main, self.port_cross, self.cross]
                .into_iter()
                .all(valid)
        {
            return Err(FeedbackError);
        }
        let mut seen = std::collections::BTreeSet::new();
        for row in &self.measurements {
            if row.view == 0 || !valid(row.size) || !seen.insert(row.view) {
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
        write!(out, "{{\"view\":{},\"axis\":\"{}\",", c.view, c.axis.name()).unwrap();
        if let Some(parent) = c.parent {
            write!(out, "\"parent\":{parent},").unwrap();
        }
        if c.restored {
            out.push_str("\"restored\":true,");
        }
        if c.seeking {
            out.push_str("\"seeking\":true,");
        }
        write!(out, "\"revision\":\"{}\",\"scrollSequence\":\"{}\",\"count\":{},\"totalExtent\":{},\"rows\":[", c.revision, c.scroll_sequence, c.count, exact_num::Shortest(c.total_extent)).unwrap();
        for (i, row) in c.rows.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write!(out, "{{\"view\":{},\"root\":{},\"index\":{},\"start\":{},\"size\":{},\"epoch\":\"{}\",\"measured\":{}}}", row.view, row.root, row.index, exact_num::Shortest(row.start), exact_num::Shortest(row.size), row.epoch, row.measured).unwrap();
        }
        write!(out, "],\"pending\":{},\"correction\":", c.pending).unwrap();
        if let Some(correction) = c.correction {
            write!(
                out,
                "{{\"scrollSequence\":\"{}\",\"offset\":{}",
                correction.scroll_sequence,
                exact_num::Shortest(correction.offset)
            )
            .unwrap();
            if let Some(from) = correction.from {
                write!(out, ",\"from\":{}", exact_num::Shortest(from)).unwrap();
            }
            if correction.smooth {
                out.push_str(",\"smooth\":true");
            }
            out.push('}');
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

impl super::Collection {
    pub(super) fn snapshot(&self) -> CollectionSnapshot {
        self.snapshot_rows(usize::MAX)
    }
    /// [`Collection::snapshot`] with at most `rows` mounted rows (the first).
    pub(super) fn snapshot_rows(&self, rows: usize) -> CollectionSnapshot {
        CollectionSnapshot {
            view: self.view,
            axis: self.axis,
            parent: self.parent,
            restored: self.restored,
            seeking: self.target.is_some(),
            revision: self.revision,
            scroll_sequence: self.geometry.as_ref().map_or(0, |g| g.scroll_sequence),
            count: self.index.len(),
            total_extent: self.index.total_height(),
            rows: self
                .mounted
                .iter()
                .take(rows)
                .map(|row| CollectionRow {
                    view: row.wrapper,
                    root: super::super::first_root(&row.row.roots).expect("a row has a root"),
                    index: row.position,
                    start: self.index.prefix(row.position).unwrap(),
                    size: self.index.height(row.position).unwrap(),
                    epoch: row.epoch,
                    measured: self.index.is_measured_at(row.position),
                })
                .collect(),
            correction: self.correction,
            pending: self.pending || self.target.is_some(), // an into-view request wants its next report
        }
    }
}
