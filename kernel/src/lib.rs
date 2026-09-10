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
pub mod error;
pub mod export;
pub mod generated;
pub mod id;
pub mod kernel;
pub mod layout;
pub mod motion;
pub mod node;
pub mod props;
pub mod selector;
pub mod style;
pub mod text;
pub mod txn;
pub mod wire;

pub use error::{
    ApplyError, DecodeError, KernelError, LayoutError, StyleDomainError, StyleValueError,
};
pub use generated::*;
pub use id::{AxisOffer, Frame, NodeFlags, NodeKey, Offer, ViewId};
pub use kernel::{Kernel, NodeRef};
pub use layout::LayoutReceipt;
pub use motion::{motion_node, MotionSync};
pub use props::{PropList, PropValue};
pub use style::{
    uses_env, Color, ColorValue, Dimension, Edge, Env, GridLine, GridPlacement, GridTrack,
    GridTracks, RowValue, StyleValue, Transitions, Vec2,
};
pub use text::{
    MonospaceMeasurer, TextMeasureRequest, TextMeasurer, TextMetrics, TextRun, TextStyle,
};
pub use txn::CommitReceipt;
pub use wire::{FrameBuilder, Op};
