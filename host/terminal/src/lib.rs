//! The terminal host (LLP 1101, a spike).
//!
//! @ref LLP 1101 (terminal apps) / LLP 1015 (the painter host it repeats)
//!
//! The runner and kernel run natively; after every commit the host lays the
//! tree out with the kernel's own layout in a fixed 8×16 cell and the painter
//! walks the kernel tree into a grid of character cells — the kernel is the
//! display list, with no mirror. The grid goes to a terminal as escape
//! sequences this crate constructs itself, or nowhere at all: headless, the
//! same host answers the agent's operations and writes `.txt`/`.ans`
//! screenshots.
//!
//! - [`measure`] — text in cells: one wrap, for the measurer and the painter.
//! - [`grid`] — the cells, their text and SGR forms, and the diff.
//! - [`paint`] — the walk.
//! - [`host`] — the runner wrapped: layout, focus, keys, the mouse.
//! - [`term`] — raw mode, vte, the loop, restoration.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod grid;
pub mod host;
pub mod measure;
pub mod paint;
pub mod term;
