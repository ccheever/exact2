//! Exact kernel.
//!
//! A copied-in, borrow-parsed binary command stream with transactional apply,
//! Taffy layout, and columnar binary exports. The design is RFC 0491's
//! (`llp/research/0491-kernel-refresh-program.rfc.md`), built fresh:
//!
//! - **One declaration authority.** `tables/schema.json` generates every node
//!   type, prop id and kind, style row, enum vocabulary, opcode, and the schema
//!   digest a frame must carry ([`generated`]).
//! - **Typed props in a columnar arena.** Nodes are slots; attributes are
//!   columns; identity is generation-checked ([`arena`], [`id`], [`props`]).
//! - **One wire, one write path.** EXWF frames and in-process ops both enter
//!   the validate-then-apply engine; a rejection changes nothing ([`wire`],
//!   [`txn`]).
//! - **Layout proportional to change.** Per-node dirty flags, an epoch, and
//!   changed-geometry receipts ([`layout`]).
//! - **One crossing per sync.** EXNODE exports the tree as typed rows or one
//!   sectioned envelope ([`export`]).
//! - **One seam to motion.** A commit restated as what the motion engine needs
//!   to hear — new targets and `transition` rows, nothing else ([`motion`]).
//! - **Injected host services.** Text measurement is a per-kernel trait object,
//!   never a process-global callback ([`text`]).
//!
//! The crate adds no threads, links no platform libraries, and builds for
//! `wasm32-unknown-unknown`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod arena;
pub mod clip;
pub mod control;
pub mod corner;
pub mod error;
pub mod export;
mod flow;
pub mod fragment;
pub mod generated;
pub mod gradient;
pub mod grouped;
pub mod handoff;
pub mod id;
pub mod kernel;
pub mod layout;
pub mod motion;
pub mod node;
pub mod paint_order;
pub mod props;
pub mod ratio;
pub mod region;
mod replaced;
pub mod selector;
mod shared_style;
pub mod sorted;
pub mod style;
pub mod svg;
pub mod text;
pub mod timeline;
pub mod transform;
pub mod txn;
pub mod wire;

pub use control::{Choice, ControlKind, PressFace, Range};
pub use error::{
    ApplyError, DecodeError, KernelError, LayoutError, StyleDomainError, StyleValueError,
};
pub use generated::*;
pub use grouped::{Accessory, GroupedList, GroupedRow, GroupedSection};
pub use handoff::Handoff;
pub use id::{AxisOffer, Frame, NodeFlags, NodeKey, Offer, ViewId};
pub use kernel::{HostCover, Kernel, NodeRef, PresentedHeight, StickyConstraint};
pub use layout::LayoutReceipt;
pub use motion::{motion_node, MotionSync};
pub use node::NodeFacts;
pub use props::{PropList, PropValue};
pub use sorted::{SortedMap, SortedSet};
pub use style::{
    link_segments, uses_env, BoxShadow, Color, ColorValue, Dimension, Edge, Env, EnvRefusal,
    GridFitContent, GridLine, GridPlacement, GridRepeat, GridRepeatCount, GridTrack,
    GridTrackComponent, GridTrackMax, GridTrackMin, GridTracks, LineHeight, Rect, RowValue,
    SegmentVar, StyleValue, Transitions, Vec2, ViewportUnit,
};
pub use text::case::link as link_text_transform;
pub use text::case::{linked_lowercase, lowercase_bounded, Lowercase};
pub use text::{
    Markup, MonospaceMeasurer, ParagraphStamp, TextMeasureRequest, TextMeasurer, TextMetrics,
    TextRun, TextStyle,
};
pub use transform::TransformDragBinding;
pub use txn::{CommitReceipt, Exit, MAX_DEPTH};
pub use wire::{FrameBuilder, Op};

pub use region::{
    ContentRegion, RegionArtifact, RegionFrame, RegionInputs, RegionLayoutReceipt,
    RegionPublication, RegionSelection, RegionTextRequest, RegionTextSource, RegionTicket,
};

/// CSS authored and resolved exclusion geometry (LLP 1043.000).
pub use exact_textflow::{FlowShape, ShapeOutside};
pub use flow::FlowRefusal;
