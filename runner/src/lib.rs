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
//! - [`compare`] — value identity, substitution and `==`, once.
//! - [`bridge`] — values to kernel props and style rows, through the kernel's
//!   own `set_dynamic`.
//! - [`delivery`] — what this binary and its update store know about
//!   delivery (LLP 1030 D7): one resource the runner answers itself.
//! - [`instance`] — the instance tree: nodes, `when`/`match` arms, keyed
//!   `each` rows, and the ops that keep the kernel equal to it.
//! - [`runner`] — boot, actions, events, resources, timers, the clock.
//! - [`agent`] — the agent API's read operations (`tree`, `state`, `logs`),
//!   answered from the runner and kernel for every host.
//! - [`head`] — the document's head: the active `head` elements' fields,
//!   for every host's page, window or scene title (LLP 1048.003 D1).
//!
//! Time is a number the host supplies (`Runner::advance`); timers fire from it,
//! so an agent seeks instead of waiting — the same clock discipline as
//! `exact-motion`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod bridge;
pub mod compare;
pub mod delivery;
pub mod head;
pub mod instance;
pub mod request;
pub mod runner;
pub mod stdlib;
pub mod store;
pub mod surface_record;
pub mod viewport;
pub mod vm;

pub use delivery::Delivery;
pub use exact_plan::Value;
pub use head::Head;
pub use instance::collection::{
    AnchorCorrection, CollectionFeedback, CollectionRow, CollectionSnapshot, FeedbackError,
    ReorderBinding, ReorderFrame, ReorderGeometry, ReorderProgress, ReorderStart, ReorderToken,
    ReorderWrapper, RowMeasurement,
};
pub use instance::SurfaceUpdate;
pub use request::{
    io_grants, Answer, Dispatch, FailureKind, HttpScheduling, Outcome, Placement, Reply, Request,
    RequestOut, Response, SurfaceOutcome, SurfaceRequest, Work, MAX_HOST_WORK_BYTES,
};
pub use runner::{
    Advanced, Carried, Checkpoint, Command, DataError, DataSource, Event, InFlight, Interrupt,
    ListStatus, ListTextPosition, ListViewport, RouterChange, Runner, RunnerError, Target, Timed,
    JOURNAL_RING, MAX_CLOCK_MS, TIMER_FIRE_LIMIT,
};
pub use store::{Store, StoreError, StoreWrite};
pub use viewport::Viewport;
pub use vm::Trap;
