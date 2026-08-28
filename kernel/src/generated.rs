//! Types generated from `tables/schema.json` by `build.rs`.
//!
//! Node types, prop ids and kinds, enum vocabularies, style rows and the style
//! mask, `StyleProps` with its patch/clear/codec, the opcode list, and the
//! schema digest. Never edit by hand; edit the table.

#![allow(missing_docs, clippy::all)]

include!(concat!(env!("OUT_DIR"), "/schema.rs"));
