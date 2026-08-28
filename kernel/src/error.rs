//! Every failure the kernel can report, as typed values.
//!
//! There is one convention: a fallible operation returns a `Result` whose error
//! names the exact condition. There are no status integers, no `-1`, and no
//! silent fallbacks — an unknown byte on the wire is a [`DecodeError`], an
//! invalid mutation is an [`ApplyError`] that leaves the tree untouched, and a
//! layout request against a missing root is a [`LayoutError`].

use std::fmt;

use crate::generated::{NodeType, OpCode, PropId, PropKind, StyleId};
use crate::id::ViewId;

/// A frame or payload could not be decoded. Nothing was applied.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer bytes remained than the field needs.
    Truncated { needed: usize, available: usize },
    /// The frame does not start with the EXWF magic.
    BadMagic,
    /// The frame revision is not one this kernel reads.
    UnsupportedRevision(u16),
    /// The producer was generated from a different schema than this kernel.
    SchemaDigestMismatch { expected: u64, actual: u64 },
    /// The header length is smaller than the fixed header or not 8-aligned.
    BadHeaderLength(u16),
    /// The declared frame length disagrees with the bytes supplied.
    FrameLengthMismatch { declared: u32, actual: usize },
    /// The frame length is not a multiple of 8.
    FrameNotAligned(u32),
    /// Reserved header or op flags were nonzero.
    ReservedFlags,
    /// The opcode is not in the closed list.
    UnknownOpcode(u16),
    /// The node type is not in the closed list.
    UnknownNodeType(u8),
    /// The prop id is not in the table.
    UnknownProp(u16),
    /// The prop value kind byte is not in the closed list.
    UnknownPropKind(u8),
    /// The wire carried a value of a kind other than the prop's declared kind.
    PropKindMismatch {
        prop: PropId,
        expected: PropKind,
        actual: PropKind,
    },
    /// An enum byte is outside its vocabulary.
    UnknownEnumValue { style: StyleId, value: u8 },
    /// `auto` was encoded on a row whose grammar does not admit it.
    AutoNotAdmitted { style: StyleId },
    /// A dimension kind byte is not auto/points/percent.
    UnknownDimensionKind(u8),
    /// A grid track kind byte is outside the closed grammar.
    UnknownTrackKind(u8),
    /// More grid tracks than the closed grammar allows.
    TooManyTracks(u8),
    /// A grid placement kind byte is outside the closed grammar.
    UnknownPlacementKind(u8),
    /// A style mask set bits above the last row.
    ReservedMaskBits,
    /// A child list exceeds the bound.
    TooManyChildren(u32),
    /// A string field is not UTF-8.
    InvalidUtf8,
    /// A string field exceeds the bound.
    StringTooLong(u32),
    /// An op payload had bytes left over after its fields were read.
    TrailingPayload { opcode: OpCode, remaining: usize },
    /// An op's declared payload length runs past the frame.
    PayloadOverrun { opcode: OpCode, declared: u32 },
    /// A number that must be finite was not.
    NonFinite(StyleId),
    /// A section directory or row count claims more than the envelope holds.
    SectionOverrun { declared: u32 },
    /// A section's checksum does not match its bytes.
    ChecksumMismatch { section: u32 },
    /// A required section is missing from the directory.
    MissingSection { section: u32 },
}

/// A batch was rejected. The tree, its derived state, and every receipt are
/// exactly as they were before the batch — except after [`ApplyError::Internal`],
/// which names a kernel bug, never a producer error (see its docs).
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// The op targets an id that is not live.
    UnknownView { op_index: usize, id: ViewId },
    /// `CreateView` on a live id of a different type.
    TypeMismatch {
        op_index: usize,
        id: ViewId,
        existing: NodeType,
        requested: NodeType,
    },
    /// The op targets an id destroyed earlier in the same batch.
    DestroyedInBatch { op_index: usize, id: ViewId },
    /// A child appears twice in one `SetChildren`.
    DuplicateChild {
        op_index: usize,
        parent: ViewId,
        child: ViewId,
    },
    /// A node was listed as its own child.
    SelfChild { op_index: usize, id: ViewId },
    /// Adopting the child would make the parent its own descendant.
    Cycle {
        op_index: usize,
        parent: ViewId,
        child: ViewId,
    },
    /// A root was listed as somebody's child.
    RootAsChild {
        op_index: usize,
        parent: ViewId,
        child: ViewId,
    },
    /// `AttachRoot` on a node that has a parent.
    RootHasParent { op_index: usize, id: ViewId },
    /// The value kind does not match the prop's declared kind.
    PropKindMismatch {
        op_index: usize,
        prop: PropId,
        expected: PropKind,
        actual: PropKind,
    },
    /// A `SetChildren` targets a node type that cannot hold children.
    LeafCannotHoldChildren {
        op_index: usize,
        id: ViewId,
        node_type: NodeType,
    },
    /// A `Text` was given a child that is not a `Text`. A text node's children
    /// are its inline runs; anything else has no place in a measured leaf.
    InlineRunNotText {
        op_index: usize,
        parent: ViewId,
        child: ViewId,
        node_type: NodeType,
    },
    /// A style row carried an infinite or NaN number.
    NonFiniteStyle { op_index: usize, style: StyleId },
    /// No representable slot index remains.
    SlotSpaceExhausted,
    /// Validation accepted an op the apply phase could not perform. This is a
    /// kernel defect: the batch stopped at `op_index`, earlier ops in it were
    /// applied, no receipt was published, and the host should `reset()` and
    /// re-snapshot rather than trust the tree.
    Internal { op_index: usize, what: &'static str },
}

/// A layout request could not run.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// The id is not live.
    UnknownView(ViewId),
    /// The id is live but not a root; layout runs per root.
    NotARoot(ViewId),
    /// The layout engine reported an error (a kernel bug, never a producer error).
    Engine(String),
}

/// Any kernel failure.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// Frame or payload decoding.
    Decode(DecodeError),
    /// Batch validation.
    Apply(ApplyError),
    /// Layout.
    Layout(LayoutError),
}

impl From<DecodeError> for KernelError {
    fn from(e: DecodeError) -> Self {
        KernelError::Decode(e)
    }
}

impl From<ApplyError> for KernelError {
    fn from(e: ApplyError) -> Self {
        KernelError::Apply(e)
    }
}

impl From<LayoutError> for KernelError {
    fn from(e: LayoutError) -> Self {
        KernelError::Layout(e)
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "decode: {self:?}")
    }
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "apply rejected: {self:?}")
    }
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout: {self:?}")
    }
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::Decode(e) => e.fmt(f),
            KernelError::Apply(e) => e.fmt(f),
            KernelError::Layout(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for DecodeError {}
impl std::error::Error for ApplyError {}
impl std::error::Error for LayoutError {}
impl std::error::Error for KernelError {}
