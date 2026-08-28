//! The web host.
//!
//! @ref LLP 1007 (web host v1)
//! @ref LLP 1002 D2 (on the web the browser executes motion; the host emits
//! the `transition` row as CSS and runs nothing per frame)
//!
//! One rule: **the DOM mirrors the kernel tree.** The runner and kernel run
//! in wasm exactly as they do natively; after every commit the host reads the
//! kernel's receipt and emits a batch — create, props, style, children,
//! destroy — that makes the page equal to the tree. CSS is computed here, in
//! Rust, from the kernel's style rows ([`css`]), so the browser is the layout
//! engine and the motion engine and this crate is neither. The JavaScript
//! side (`glue.js`) is ~150 lines that apply batches, forward events, and
//! tick the clock; it is host glue, not app code, and the plan is data baked
//! into the wasm.
//!
//! - [`css`] — style rows → CSS declarations, once.
//! - [`batch`] — the JSON the glue applies.
//! - [`host`] — the runner wrapped for a DOM: receipts → batches, events,
//!   timers.
//! - [`abi`] — the wasm exports, with no `unsafe`: the glue writes into a
//!   host-owned buffer and reads from another.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod abi;
pub mod batch;
pub mod css;
pub mod host;

pub use host::{Host, HostError};
