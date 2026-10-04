//! The Ibex 2 standard library: Rust implementations with JavaScript bindings.
//!
//! Algorithms and host operations shared by Rust callers and the bindings.
//! Platform transport and entropy stay below the Rust semantics; capabilities
//! are admitted at the boundary (LLP 0059.000).

pub mod base64;
pub mod console;
pub mod crypto;
pub mod fetch;
pub mod fs;
#[cfg(all(feature = "hermes", target_os = "linux"))]
pub(crate) mod intl;
#[cfg(all(feature = "hermes", target_os = "linux"))]
pub(crate) mod intl_case;
#[cfg(all(feature = "hermes", target_os = "linux"))]
pub(crate) mod intl_datetime;
pub mod subtle;
pub(crate) mod subtle_abi;
pub mod text;
pub mod timers;
pub mod url;
pub mod websocket;

pub mod abort;
mod fetch_body;

pub mod sqlite;

pub mod app_fs;
#[cfg(windows)]
pub(crate) mod windows_directory;
#[cfg(windows)]
mod windows_fs;
#[cfg(windows)]
pub(crate) mod windows_path;
