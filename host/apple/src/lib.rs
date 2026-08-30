//! The Apple host.
//!
//! @ref LLP 1008 (Apple host v1)
//! @ref LLP 1001 §5–6 (layout is a host call; text measurement is injected)
//! @ref LLP 1002 D2, §4 (every host but the web runs `exact-motion`)
//!
//! One rule, the web host's: **the view tree mirrors the kernel tree.** The
//! runner and kernel run natively; after every commit the host lays the tree
//! out with the kernel's own layout (Taffy, measuring text through a callback
//! the app registered at boot), seeks the motion engine to the app's clock,
//! and emits a batch — create, props, style, children, destroy, roots, plus
//! `frame` (parent-relative layout frames that changed), `content` (a scroll
//! container's content size), and `present` (a motion property's presentation
//! value) — that makes the presenter's views equal to the tree. Style rows
//! cross as a typed dictionary keyed by the row's name, so a row added to
//! `schema.json` reaches the presenter with no change here. The presenter
//! (AppKit today, `host/apple/macos`) is host glue: it knows nothing about
//! the app, and nothing in it runs per frame unless motion is running.
//!
//! - [`style`] — style rows → the typed dictionary.
//! - [`batch`] — the JSON the presenter applies.
//! - [`measure`] — the text-measurement callback as a kernel [`TextMeasurer`].
//! - [`host`] — the runner wrapped for a presenter: receipts → batches, layout,
//!   motion, events, timers.
//! - [`abi`] — the C exports: the web's buffer discipline over `extern "C"`.
//!
//! [`TextMeasurer`]: exact_kernel::TextMeasurer

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod abi;
pub mod batch;
pub mod executor;
pub mod host;
pub mod measure;
pub mod style;

pub use host::{Host, HostError};
