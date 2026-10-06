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
//! - [`host`] — the runner wrapped: layout, focus, dialogs, the mouse; keys
//!   and paste in `keys`.
//! - [`image`] — PNGs decoded once; kitty, iTerm2 or half-blocks.
//! - [`term`] — raw mode, vte, the loop, restoration.
//! - [`vt`] — the headless screen: a terminal emulator the agent reads.
//! - [`pointer`] — clicks and the wheel against what was presented.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod cli;
pub mod grid;
pub mod host;
pub mod image;
mod keys;
pub mod measure;
pub mod paint;
pub mod pointer;
pub mod term;
pub mod vt;
