//! The plan runner.
//!
//! @ref LLP 1004 D4 (data comes from a Rust data source through one seam)
//! @ref LLP 1004 D5 (a dev reload is a restart)
//! @ref LLP 0485 §8 (the update loop; research)
//!
//! A [`Runner`] loads one validated plan against one [`DataSource`] and drives
//! one kernel. Nothing here is app-specific: the plan is data, the data source
//! is the app's Rust crate behind one trait, and the kernel is the consumer of
//! every op the runner emits.
//!
//! - [`vm`] — the expression VM: a stack machine over [`Value`]s, one
//!   dispatch loop, typed traps, never UB.
//! - [`stdlib`] — the roster's implementations, once.
//! - [`strings`] — strings as JavaScript has them: order, `slice` and
//!   `replaceAll` over UTF-16 code units.
//! - [`compare`] — value identity, substitution and `==`, once.
//! - [`held`] — a settled resource's value; a compiled one no one else
//!   holds is released to the plan's bytes.
//! - [`bridge`] — values to kernel props and style rows, through the kernel's
//!   own `set_dynamic`.
//! - [`delivery`] — what this binary and its update store know about
//!   delivery (LLP 1030 D7): one resource the runner answers itself.
//! - [`device`] — `device.*` grants and the one table every host's
//!   permission spelling derives from (LLP 1069.008).
//! - [`instance`] — the instance tree: nodes, `if`/`match` arms, keyed
//!   `for` rows, and the ops that keep the kernel equal to it.
//! - [`runner`] — boot, actions, events, resources, timers, the clock.
//! - [`agent`] — the agent API's read operations (`tree`, `state`, `logs`),
//!   answered from the runner and kernel for every host.
//! - [`head`] — the document's head: the active `head` elements' fields,
//!   for every host's page, window or scene title (LLP 1048.003 D1).
//! - [`uses`] — what a plan uses beyond the core, from its bytes (LLP 1047
//!   D2): what a host must link to run it.
//!
//! Time is a number the host supplies (`Runner::advance`); timers fire from it,
//! so an agent seeks instead of waiting — the same clock discipline as
//! `exact-motion`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod auth;
pub mod bridge;
pub mod commands;
pub mod compare;
mod conform;
pub mod decorations;
pub mod delivery;
pub mod design;
pub mod device;
pub mod failure;
pub mod file_pickers;
mod format;
pub mod geometry;
pub mod grants;
pub mod hardened;
pub mod head;
pub mod held;
pub mod instance;
pub mod lists;
pub mod notify;
pub mod page;
pub mod perf;
pub mod request;
pub mod runner;
pub mod save_file;
/// The picker's helpers a host shares (LLP 1069.002): types by name,
/// `accept` matching, the HEIC rule, a `type @t` answer's paths.
pub use runner::picker as picker_support;
pub mod machine;
pub mod midi;
pub mod share;
pub mod sound;
pub mod stdlib;
pub mod store;
pub mod stream_timing;
pub mod strings;
pub mod surface_record;
pub mod time;
pub mod transport;
pub mod uses;
pub mod viewport;
pub mod vm;
pub mod wakelock;

pub use delivery::Delivery;
pub use exact_canvas;
pub use exact_plan::{Items, Str, Value};
pub use format::formatting;
pub use head::Head;
pub use instance::collection::{set_bootstrap_extent, set_lead_scale};
pub use instance::collection::{
    AnchorCorrection, CollectionFeedback, CollectionFill, CollectionRow, CollectionSnapshot,
    FeedbackError, ListAxis, ReorderBinding, ReorderEnding, ReorderFrame, ReorderGeometry,
    ReorderPhase, ReorderProgress, ReorderStart, ReorderStep, ReorderToken, ReorderWrapper,
    RowMeasurement,
};
pub use instance::{DocNode, DocTree, DocTreeError, ListLinks, SurfaceUpdate, LISTS};
pub use page::Page;
pub use request::{
    io_grants, Answer, Dispatch, FailureKind, HttpScheduling, Message, Outcome, Placement,
    Redirect, Reply, Request, RequestOut, Response, SocketOp, SocketOrigin, SocketSend,
    SurfaceOutcome, SurfaceRequest, Work, MAX_BODY_FROM_BYTES, MAX_BODY_FROM_PATH,
    MAX_HOST_WORK_BYTES, MAX_TIMEOUT_MS, NATIVE_URL,
};
pub use runner::{
    canvas_engine, press_writes_router, routing, virtual_frame, Advanced, Announce, AuthLinks,
    BackgroundState, BlockPlacement, CanvasEngine, CanvasLink, CanvasList, Carried, Checkpoint,
    Command, ControlValue, DataError, DataSource, DeviceLinks, DrawReply, DrawRequest, Drawn,
    DropEvent, Event, FieldSelection, FormatLink, Geometry, GeometryLink, Hold, HoldAnswer,
    InFlight, InputDetails, Interrupt, Intersection, KeyModifiers, KeyboardEvent, Limits,
    ListTextPolicy, ListTextPosition, Native, NativeCall, NativeHandler, Overlaid, Picked,
    PickerLinks, PickerRequest, PointerEvent, PreloadWake, ResizeRect, RouterChange, RouterLink,
    Routing, Runner, RunnerError, RunnerLinks, ScrollEvent, SelectionDirection, SnapshotModule,
    SocketReader, SocketState, StackSplit, StreamCount, SurfaceAnswer, Target, Timed, WheelEvent,
    Write, BACKGROUND, JOURNAL_RING, MAX_CLOCK_MS, PICKED, QUEUE_BOUND, RESIZE_UNDELIVERED,
    TIMER_FIRE_LIMIT, VIRTUAL_FRAME_MS,
};
pub use runner::{LiveApplied, LiveRefusal, Place, Tamber};
/// The binding core's names (LLP 1121 D12): what `contract tamber gen rust`
/// writes (`pub const SURFACE_CARD: Token<Color>`) and resolves.
pub mod tamber {
    pub use crate::runner::{
        ColorKind as Color, Font, Kind, LengthKind as Length, NumberKind as Number, Place, Tamber,
        Token, TypeKind as Type,
    };
}
pub use store::{Store, StoreChange, StoreError, StoreWrite};
pub use uses::{svg_filters, svg_islands, uses, Capability, Uses};
pub use viewport::{Contrast, Fold, Gamut, Hover, Pointer, Posture, Preferences, Viewport};
pub use vm::Trap;
