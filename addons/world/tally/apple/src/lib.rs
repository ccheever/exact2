//! Tally on Apple platforms: the host's C exports over the app's
//! data source and its baked plan.

#![deny(missing_docs)]

/// The baked plan, written by `build.rs`.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

/// The compatibility id and its inputs (exact2 LLP 1030 D3a), written by
/// `build.rs` beside the plan; the runner answers the `delivery` resource
/// from it.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

// @ref ../exact2/llp/1030-delivery-unified.rfc.md §D4 — match the baked store level.
include!(concat!(env!("OUT_DIR"), "/entry.rs"));

// Measure pre-main initialization -> first world tick without changing the host.
extern "C" fn experiment_entry() {
    tally_data::metrics::entry();
}
#[used]
#[link_section = "__DATA,__mod_init_func"]
static EXPERIMENT_ENTRY: extern "C" fn() = experiment_entry;
