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
//! - [`bridge`] — values to kernel props and style rows, through the kernel's
//!   own `set_dynamic`.
//! - [`instance`] — the instance tree: nodes, `when`/`match` arms, keyed
//!   `each` rows, and the ops that keep the kernel equal to it.
//! - [`runner`] — boot, actions, events, resources, timers, the clock.
//! - [`agent`] — the agent API's read operations (`tree`, `state`, `logs`),
//!   answered from the runner and kernel for every host.
//!
//! Time is a number the host supplies (`Runner::advance`); timers fire from it,
//! so an agent seeks instead of waiting — the same clock discipline as
//! `exact-motion`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod bridge;
pub mod instance;
pub mod request;
pub mod runner;
pub mod stdlib;
pub mod store;
pub mod vm;

pub use exact_plan::Value;
pub use instance::SurfaceUpdate;
pub use request::{Answer, FailureKind, Outcome, Request, RequestOut, Response};
pub use runner::{
    Advanced, Carried, Command, DataError, DataSource, Event, Runner, RunnerError, Timed,
    JOURNAL_RING, MAX_CLOCK_MS, TIMER_FIRE_LIMIT,
};
pub use store::{Store, StoreError, StoreWrite};
pub use vm::Trap;
