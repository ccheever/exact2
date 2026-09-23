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
//! engine and the motion engine. CSS Exclusions is the one exception: the
//! optional glue measures segments and places the separate textflow wasm's
//! fragments (@ref LLP 1043.000 §3 D7); browser frames and CSS feed Rust geometry. The JavaScript
//! side (`glue.js`) is ~150 lines that apply batches, forward events, and
//! tick the clock; it is host glue, not app code, and the plan is data baked
//! into the wasm.
//!
//! - [`css`] — style rows → CSS declarations, once.
//! - [`batch`] — the JSON the glue applies.
//! - [`motion`] — springs: the one curve CSS cannot play, lowered to frames
//!   by the same engine every native host runs, once per release.
//! - [`host`] — the runner wrapped for a DOM: receipts → batches, events,
//!   timers.
//! - [`document`] — the same DOM as HTML, before browser layout: what a
//!   page is before its runtime starts (LLP 1048.000 D1).
//! - [`parity`] — the browser-driven parity harness: cases a real browser
//!   runs, and the check that holds the engine to what it recorded.
//! - [`dev`] — the resident dev driver: a source change observed → the
//!   plan ready, in one long-lived process; the page restarts from it.
//! - [`abi`] — the wasm exports, with no `unsafe`: the glue writes into a
//!   host-owned buffer and reads from another.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod abi;
pub mod batch;
pub mod css;
#[cfg(not(target_arch = "wasm32"))]
pub mod dev;
pub mod host;
pub mod motion;
pub mod parity;
#[cfg(test)]
mod textflow_tests;

pub use host::{document, Host, HostError};
