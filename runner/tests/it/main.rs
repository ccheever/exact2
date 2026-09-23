//! The runner's integration tests: one binary, so one link and one launch.
//! `tests/surface_record.rs` stays its own binary: it installs a counting
//! global allocator.

mod flow_agent;
mod height_binding;
mod incremental;
mod now_screen;
mod reorder_codec;
mod router;
mod transform_binding;
mod viewport;
