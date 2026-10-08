//! Fixed collection-owned Arrange capability and bounded host snapshots.
use exact_kernel::{CommitReceipt, NodeKey};

/// Generational source binding; the actual string key stays private.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReorderBinding {
    /// Authored grip.
    pub handle: NodeKey,
    /// Strict ancestor virtual List with the terminal handler.
    pub list: NodeKey,
    /// Runner-owned flow wrapper.
    pub wrapper: NodeKey,
    /// Authored row root inside the wrapper.
    pub root: NodeKey,
    /// Current measured row generation.
    pub row_epoch: u64,
}
/// Exact latest accepted scroll facts, not merely a revision number.
#[derive(Debug, Clone, PartialEq)]
pub struct ReorderGeometry {
    /// Live List allocation.
    pub list: NodeKey,
    /// Collection publication revision.
    pub revision: u64,
    /// Latest accepted user scroll sequence.
    pub scroll_sequence: u64,
    /// Actual clamped scroll position.
    pub scroll_top: f64,
    /// Actual scrollport width.
    pub port_width: f64,
    /// Actual scrollport height.
    pub port_height: f64,
    /// Actual measured wrapper width.
    pub row_width: f64,
    /// Full logical extent, including unmounted estimates.
    pub total_extent: f64,
}
/// Opaque, process-unique single-owner token. Hosts also bind runtime incarnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReorderToken {
    pub(crate) list: NodeKey,
    pub(crate) serial: u64,
}
impl ReorderToken {
    /// Exact integer handle for lossless host transport; never convert to JS Number.
    pub fn serial(self) -> u64 {
        self.serial
    }
    /// Generational List owner.
    pub fn list(self) -> NodeKey {
        self.list
    }
}
/// Successful admission; no action or data query occurred.
#[derive(Debug)]
pub struct ReorderStart {
    /// Single preview capability.
    pub token: ReorderToken,
    /// Wrapper transition/target changes, if any.
    pub receipt: Option<CommitReceipt>,
}
/// A current measured gap, an unproved boundary, or an obsolete request.
// Returned once per drag sample and never stored: the receipt stays inline.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum ReorderProgress {
    /// No mutation; check identity before validating any numeric sample.
    Stale,
    /// Keep displayed targets, but clear terminal eligibility until a new accepted
    /// sample. Request ordinary bounded collection feedback.
    NeedsMeasurement,
    /// Absolute wrapper targets have been accepted.
    Accepted {
        /// Changed styles, if the gap differs.
        receipt: Option<CommitReceipt>,
    },
}
/// One surviving mounted wrapper for host-side C0 presentation rebasing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReorderWrapper {
    /// Private wrapper allocation, never an item key.
    pub wrapper: NodeKey,
    /// Current authored root allocation.
    pub root: NodeKey,
    /// Untranslated logical content top; host adds actual port/scroll mapping.
    pub top: f64,
    /// Absolute Runner-authored translate target.
    pub offset: f64,
}
/// Bounded before/after snapshot; no presentation samples or N-key export.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReorderFrame {
    /// Action eligibility is consumed; finish remains required.
    pub terminal: bool,
    /// Surviving mounted wrappers only.
    pub wrappers: Vec<ReorderWrapper>,
    /// Where the session stands (LLP 1094 D4, D8).
    pub phase: ReorderPhase,
    /// How a drop's hold ended, once it has (LLP 1094 D8).
    pub ending: Option<ReorderEnding>,
    /// The list a drop lands in: the source's own until another grouped
    /// list takes the dragged row's centre (LLP 1094 D7).
    pub target: Option<NodeKey>,
    /// The mounted wrapper that now holds the dragged row, in whichever
    /// grouped list holds it: where a ghost lands (LLP 1094 D6, D8).
    pub row: Option<NodeKey>,
}

/// Where a reorder session stands (LLP 1094 D4, D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReorderPhase {
    /// The preview follows the contact (or the keys).
    #[default]
    Active,
    /// Dropped; the action's move has not shown yet. Both previews stay,
    /// the pin is kept, and Escape does nothing.
    Holding,
    /// Cancelled before a drop: the previews close and no action ran.
    Cancelling,
    /// The drop showed, or its hold ended: the previews closed, and the
    /// host's return runs until `finish_reorder`.
    Settling,
}

impl ReorderPhase {
    /// The name hosts and `state.reorder` read.
    pub fn name(self) -> &'static str {
        match self {
            ReorderPhase::Active => "active",
            ReorderPhase::Holding => "holding",
            ReorderPhase::Cancelling => "cancelling",
            ReorderPhase::Settling => "settling",
        }
    }
}

/// How a drop's hold ended (LLP 1094 D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderEnding {
    /// The row is in a grouped list: where it was dropped, or where the
    /// action put it instead. A ghost springs onto it.
    Landed,
    /// The row is in no grouped list: a ghost fades.
    Gone,
    /// A second passed on the session clock: a ghost springs onto wherever
    /// the row is now.
    Timeout,
}

impl ReorderEnding {
    /// The name hosts and `state.reorder` read.
    pub fn name(self) -> &'static str {
        match self {
            ReorderEnding::Landed => "landed",
            ReorderEnding::Gone => "gone",
            ReorderEnding::Timeout => "timeout",
        }
    }
}

/// One keyboard or custom-action move (LLP 1094 D9): the gap one row up or
/// down, or the same index in the previous or next grouped list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderStep {
    /// One row earlier (Up, "Move earlier").
    Earlier,
    /// One row later (Down, "Move later").
    Later,
    /// The previous grouped list in tree order (Left).
    PreviousList,
    /// The next grouped list in tree order (Right).
    NextList,
}

impl ReorderStep {
    /// From DOM's key name: the four arrows.
    pub fn from_key(key: &str) -> Option<ReorderStep> {
        Some(match key {
            "ArrowUp" => ReorderStep::Earlier,
            "ArrowDown" => ReorderStep::Later,
            "ArrowLeft" => ReorderStep::PreviousList,
            "ArrowRight" => ReorderStep::NextList,
            _ => return None,
        })
    }
}

impl ReorderGeometry {
    /// Fixed 68-byte LE v1: u32 version, u32 key index/generation, u64 revision
    /// and sequence, then f64 top/port-width/port-height/row-width/total-extent.
    /// Integer identities never pass through floating point or JSON Number.
    pub fn encode(&self) -> Result<Vec<u8>, super::FeedbackError> {
        self.validate()?;
        let mut w = exact_plan::bytes::Writer::default();
        w.u32(1);
        w.u32(self.list.index);
        w.u32(self.list.generation);
        w.u64(self.revision);
        w.u64(self.scroll_sequence);
        for n in [
            self.scroll_top,
            self.port_width,
            self.port_height,
            self.row_width,
            self.total_extent,
        ] {
            w.f64(n);
        }
        Ok(w.into_vec())
    }
    /// Decode/validate the common geometry wire without mutating Runner state.
    pub fn decode(bytes: &[u8]) -> Result<Self, super::FeedbackError> {
        let mut r = exact_plan::bytes::Reader::new(bytes);
        let out = (|| -> Result<Self, exact_plan::PlanError> {
            if r.u32()? != 1 {
                return Err(exact_plan::PlanError::BadCount(0));
            }
            Ok(Self {
                list: NodeKey {
                    index: r.u32()?,
                    generation: r.u32()?,
                },
                revision: r.u64()?,
                scroll_sequence: r.u64()?,
                scroll_top: r.f64()?,
                port_width: r.f64()?,
                port_height: r.f64()?,
                row_width: r.f64()?,
                total_extent: r.f64()?,
            })
        })()
        .map_err(|_| super::FeedbackError)?;
        if !r.is_empty() {
            return Err(super::FeedbackError);
        }
        out.validate()?;
        Ok(out)
    }
    fn validate(&self) -> Result<(), super::FeedbackError> {
        // `scroll_top` is a collection's offset: negative in the padding
        // before the first row (LLP 1010 §6.9).
        if self.scroll_top.is_finite()
            && self.scroll_top.abs() <= f32::MAX as f64
            && [
                self.port_width,
                self.port_height,
                self.row_width,
                self.total_extent,
            ]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=f32::MAX as f64).contains(&v))
        {
            Ok(())
        } else {
            Err(super::FeedbackError)
        }
    }
}
