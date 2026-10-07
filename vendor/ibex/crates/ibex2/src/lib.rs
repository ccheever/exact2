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
//! process. The optional `bindings` feature projects the library into a JSI
//! runtime supplied by the caller and links no VM.
//!
//! @ref LLP 0057#2-the-inversion — the three-category split this crate implements
//! @ref LLP 0067#1-five-properties — authority is carried, not inferred

#[cfg(feature = "bindings")]
pub mod bindings;
// No Rust item from hermes-lean-sys is used here, so name the crate to make
// rustc link it: its ICU features carry the archives the Linux engine and Intl
// shims call.
#[cfg(feature = "bindings")]
extern crate hermes_lean_sys as _;
pub mod boundary;
#[cfg(feature = "bindings")]
pub mod boundary_abi;
pub mod grant;
pub mod host;
#[cfg(feature = "bindings")]
mod host_opcodes;
pub mod kv;
#[cfg(feature = "bindings")]
pub mod pool;
pub mod secrets;
pub mod stdlib;
#[cfg(feature = "bindings")]
pub mod task;
pub mod transport;

#[cfg(feature = "bindings")]
mod sqlite_abi;
