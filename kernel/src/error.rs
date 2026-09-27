//! Every failure the kernel can report, as typed values.
//!
//! There is one convention: a fallible operation returns a `Result` whose error
//! names the exact condition. There are no status integers, no `-1`, and no
//! silent fallbacks — an unknown byte on the wire is a [`DecodeError`], an
//! invalid mutation is an [`ApplyError`] that leaves the tree untouched, and a
//! layout request against a missing root is a [`LayoutError`].

use std::fmt;

use crate::generated::{NodeType, OpCode, PropId, PropKind, StyleId};
use crate::id::{NodeKey, ViewId};

/// A masked style contains a value outside the schema's declared domain.
///
/// Structured mutation, EXWF decoding, and EXNODE export all use this one
/// vocabulary so their accepted style state cannot diverge.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleDomainError {
    InvalidLineHeight,
    NonFinite(StyleId),
    AutoNotAdmitted(StyleId),
    TooManyTracks { style: StyleId, count: usize },
    InvalidGridSpan(StyleId),
    InvalidTransition(exact_motion::TransitionError),
    InvalidAnimation(exact_motion::AnimationError),
}

/// A frame or payload could not be decoded. Nothing was applied.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Invalid line-height tag, negative length or ratio.
    InvalidLineHeight,
    /// Fewer bytes remained than the field needs.
    Truncated { needed: usize, available: usize },
    /// A colour row's tag byte named neither a fixed colour nor a
    /// `light-dark()` pair (LLP 1034 D1).
    BadColorValue(u8),
    /// The frame does not start with the EXWF magic.
    BadMagic,
    /// Invalid or unsupported CSS clipping path.
    BadClipPath,
    /// Invalid CSS shape-outside value.
    BadShapeOutside,
    /// Invalid CSS `aspect-ratio` value.
    BadAspectRatio,
    /// Invalid SVG paint (LLP 1055 D2).
    BadPaint,
    /// Invalid SVG `stroke-dasharray` (LLP 1055 D2).
    BadDashArray,
    /// Invalid CSS `transform` list (LLP 1055.000 D5).
    BadTransform,
    /// Invalid CSS `transform-origin` (LLP 1055.000 D5).
    BadTransformOrigin,
    /// Invalid SVG `paint-order` (LLP 1055.000 D7).
    BadPaintOrder,
    /// Invalid SVG `marker-start`/`-mid`/`-end` (LLP 1055.000 D9).
    BadMarker,
    /// Invalid CSS `filter` (LLP 1055.000 D14).
    BadFilter,
    /// An `animation` row carried more entries or keyframes than the wire
    /// admits, or a direction/fill byte outside the table (LLP 1055 D5).
    BadAnimation,
    /// An `animation` row decoded but failed the sampler's validation.
    InvalidAnimation(exact_motion::AnimationError),
    /// The frame revision is not one this kernel reads.
    UnsupportedRevision(u16),
    /// The producer was generated from a different schema than this kernel.
    SchemaDigestMismatch { expected: u64, actual: u64 },
    /// The header length is invalid for this format revision.
    BadHeaderLength(u16),
    /// The declared frame length disagrees with the bytes supplied.
    FrameLengthMismatch { declared: u32, actual: usize },
    /// The frame length is not a multiple of 8.
    FrameNotAligned(u32),
    /// Reserved header or op flags were nonzero.
    ReservedFlags,
    /// A padding byte was nonzero; `offset` is its position in the envelope.
    NonZeroPadding { offset: usize },
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
    TooManyTracks(usize),
    /// A grid placement kind byte is outside the closed grammar.
    UnknownPlacementKind(u8),
    /// A grid span was zero; CSS spans are positive integers.
    InvalidGridSpan,
    /// A `transition` row carried more declarations than the wire admits.
    TooManyTransitions(u8),
    /// A `transition` row named a property discriminant the table lacks.
    UnknownTransitionProperty(u8),
    /// A `transition` row named an easing discriminant the table lacks.
    UnknownEasing(u8),
    /// A `steps()` easing named a step-position discriminant the table lacks.
    UnknownStepPosition(u8),
    /// A `linear()` easing carried more stops than the wire admits.
    TooManyEasingStops(u8),
    /// A `transition` row decoded but failed the evaluator's validation.
    InvalidTransition(exact_motion::TransitionError),
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
    /// An EXNODE envelope did not declare exactly its three sections.
    UnexpectedSectionCount(u32),
    /// An EXNODE directory entry was not the canonical section for its index.
    UnexpectedSection {
        index: u32,
        expected: u32,
        actual: u32,
    },
    /// An EXNODE section did not immediately follow the directory/previous section.
    InvalidSectionLayout { section: u32 },
    /// A decoded EXNODE section had bytes beyond its canonical zero padding.
    TrailingSection { section: u32, remaining: usize },
    /// An EXNODE row set flags outside the declared three bits.
    UnknownRowFlags { row: u32, flags: u8 },
    /// An EXNODE row repeated a wire id.
    DuplicateNode { row: u32, id: ViewId },
    /// An EXNODE row carried a non-finite frame coordinate.
    NonFiniteFrame { row: u32 },
    /// An EXNODE row's parent/depth/root relation disagreed with preorder.
    InvalidTopology { row: u32 },
    /// A wire boolean used a byte other than canonical zero or one.
    NonCanonicalBool(u8),
    /// An EXNODE row repeated one prop id.
    DuplicateProp { row: u32, prop: PropId },
    /// An EXNODE float prop was infinite or NaN.
    NonFiniteProp { row: u32, prop: PropId },
}

/// A batch was rejected. The tree, its derived state, and every receipt are
/// exactly as they were before the batch — except after [`ApplyError::Internal`],
/// which names a kernel bug, never a producer error (see its docs).
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// An authored line height was negative.
    InvalidLineHeight { op_index: usize },
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
    /// `auto` was supplied for a dimension row whose grammar does not admit it.
    AutoNotAdmitted { op_index: usize, style: StyleId },
    /// A grid template exceeded the closed grammar's track bound.
    TooManyTracks {
        op_index: usize,
        style: StyleId,
        count: usize,
    },
    /// A grid placement carried a zero span; CSS spans are positive integers.
    InvalidGridSpan { op_index: usize, style: StyleId },
    /// A `SetStyle` patch carried a `transition` row the evaluator refuses.
    InvalidTransition {
        op_index: usize,
        error: exact_motion::TransitionError,
    },
    /// A `SetStyle` patch carried an `animation` row the sampler refuses.
    InvalidAnimation {
        op_index: usize,
        error: exact_motion::AnimationError,
    },
    /// An SVG element's parent is not an `svg` or `g`, or an `svg` or `g`
    /// was given a child that is not an SVG element (LLP 1055 D3).
    SvgContent {
        op_index: usize,
        parent: ViewId,
        child: ViewId,
    },
    /// The batch would leave node `id` `depth` levels below the top of its
    /// tree, past [`MAX_DEPTH`](crate::MAX_DEPTH): layout recurses once per
    /// level and the host's stack is finite. `op_index` attached the subtree
    /// that holds it; nested inline runs count like any other level.
    TooDeep {
        op_index: usize,
        id: ViewId,
        depth: u32,
    },
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
    /// Explicit region registration/publication refused; no fake ready metrics.
    ContentRegion(&'static str),
    /// A definite available-space offer is infinite or NaN.
    InvalidOffer,
    /// The id is not live.
    UnknownView(ViewId),
    /// The id is live but not a root; layout runs per root.
    NotARoot(ViewId),
    /// An intrinsic size was reported for a node that is not an image.
    NotAnImage(ViewId),
    /// An intrinsic size that is not finite and positive on both axes.
    InvalidIntrinsicSize(ViewId),
    /// An environment with a non-finite inset.
    InvalidEnv,
    /// A host text callback returned a non-finite or negative metric.
    InvalidTextMetrics(ViewId),
    /// A sampled CSS height is non-finite or negative.
    InvalidPresentedHeight,
    /// More than one sample supplies the same generational node.
    DuplicatePresentedHeight(NodeKey),
    /// The sample was stamped before/after the current authored epoch.
    StalePresentedHeight { expected: u64, actual: u64 },
    /// The sampled allocation has been removed or its slot reused.
    UnknownPresentedNode(NodeKey),
    /// The sampled node is detached or belongs to a different root.
    PresentedHeightOutsideRoot(NodeKey),
    /// Height is neither nonnegative pixels nor border-box auto, or the node
    /// has no visible independent box (inline text or a display:none ancestor).
    UnsupportedPresentedHeight(NodeKey),
    /// Target measurement currently requires authored border-box sizing.
    UnsupportedHeightMeasurement(NodeKey),
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

impl From<StyleDomainError> for DecodeError {
    fn from(error: StyleDomainError) -> Self {
        match error {
            StyleDomainError::InvalidLineHeight => DecodeError::InvalidLineHeight,
            StyleDomainError::NonFinite(style) => DecodeError::NonFinite(style),
            StyleDomainError::AutoNotAdmitted(style) => DecodeError::AutoNotAdmitted { style },
            StyleDomainError::TooManyTracks { count, .. } => DecodeError::TooManyTracks(count),
            StyleDomainError::InvalidGridSpan(_) => DecodeError::InvalidGridSpan,
            StyleDomainError::InvalidTransition(error) => DecodeError::InvalidTransition(error),
            StyleDomainError::InvalidAnimation(error) => DecodeError::InvalidAnimation(error),
        }
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

/// A dynamic style write (`StyleProps::set_dynamic`) was refused. Nothing changed.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleValueError {
    /// The value's kind cannot fill this row's codec.
    WrongKind {
        style: StyleId,
        expected: &'static str,
    },
    /// The row is an enum and the text is not one of its values.
    UnknownEnumValue {
        style: StyleId,
    },
    /// `auto` on a row that does not admit it.
    AutoNotAdmitted {
        style: StyleId,
    },
    /// The number does not fit the row's integer codec.
    OutOfRange {
        style: StyleId,
    },
    /// A color text was not hex (`#rgb`, `#rrggbb`, `#rrggbbaa`) or `rgb()`.
    BadColor {
        style: StyleId,
    },
    /// The row's codec has no dynamic form (grid tracks, placements, gradients).
    Unsupported {
        style: StyleId,
    },
    /// A `transition` text was not CSS shorthand the evaluator accepts.
    BadTransition {
        style: StyleId,
    },
    BadShapeOutside {
        style: StyleId,
    },
    BadClipPath {
        style: StyleId,
    },
    /// Not CSS `aspect-ratio`: `auto`, a ratio, or both.
    BadAspectRatio {
        style: StyleId,
    },
    /// Not SVG paint: `none`, `currentcolor`, or a colour.
    BadPaint {
        style: StyleId,
    },
    /// Not SVG `stroke-dasharray`: `none` or non-negative numbers.
    BadDashArray {
        style: StyleId,
    },
    /// Not a CSS or SVG transform list.
    BadTransform {
        style: StyleId,
    },
    /// Not CSS `transform-origin`.
    BadTransformOrigin {
        style: StyleId,
    },
    /// Not SVG `paint-order`.
    BadPaintOrder {
        style: StyleId,
    },
    /// Not `none` or `url(#id)`.
    BadMarker {
        style: StyleId,
    },
    /// Not `none`, `url(#id)` or filter functions.
    BadFilter {
        style: StyleId,
    },
    /// Not CSS `animation` shorthand.
    BadAnimation {
        style: StyleId,
    },
}

impl fmt::Display for StyleValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StyleValueError {}
impl std::error::Error for DecodeError {}
impl std::error::Error for ApplyError {}
impl std::error::Error for LayoutError {}
impl std::error::Error for KernelError {}
