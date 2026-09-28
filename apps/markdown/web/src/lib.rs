//! The Markdown reader on the web: the host's five exports over the app's
//! data source and its baked plan. It opens what the File System Access
//! API's pickers chose (LLP 1069.010 D2), read through storage.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (LLP 1030 D3a), written beside it.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/entry.rs"));
