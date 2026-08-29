//! Caltrain on Apple platforms: the host's C exports over the app's data
//! source and its baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

exact_apple::host!(caltrain_data::Caltrain, PLAN);
