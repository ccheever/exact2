//! Ibex 2 — a Rust standard library with JavaScript bindings.
//!
//! This crate is the staging ground for the inversion described in LLP 0057:
//! capabilities implemented in Rust, reached through one host-call boundary,
//! with the JavaScript engine demoted from foundation to component.
//!
//! It is a strangler, not a fork. The end state is that this crate replaces
//! the root `ibex-runtime` crate — not that the two coexist indefinitely.
//!
//! Two consumers, one standard library. A JavaScript module reaches it through
//! the engine adapter and the bindings; a Rust consumer — Exact 2's plan
//! runner — reaches it through `host` (LLP 0068), with no engine in the
//! process. The `hermes` feature is the engine; everything else builds and
//! runs without it.
//!
//! @ref LLP 0057#2-the-inversion — the three-category split this crate implements
//! @ref LLP 0067#1-five-properties — authority is carried, not inferred

pub mod bindings;
pub mod boundary;
pub mod boundary_abi;
pub mod bytecode;
pub mod engine;
#[cfg(feature = "loader")]
pub mod esm;
pub mod grant;
pub mod host;
pub mod kv;
pub mod loader;
pub mod pool;
pub mod receipt;
pub mod secrets;
pub mod stdlib;
pub mod task;
pub mod transport;
#[cfg(feature = "loader")]
pub mod typescript;

mod sqlite_abi;
