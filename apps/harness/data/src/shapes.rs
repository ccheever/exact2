//! The seam's types, generated at build time from `../shapes.contract` by
//! `contract rust` (see `build.rs`).

#![allow(missing_docs, clippy::all)]

include!(concat!(env!("OUT_DIR"), "/shapes.rs"));
