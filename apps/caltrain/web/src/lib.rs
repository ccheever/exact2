//! Caltrain on the web: the host's five exports over the app's data source
//! and its baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

exact_web::host!(caltrain_data::Caltrain, PLAN);
