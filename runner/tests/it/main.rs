//! The runner's integration tests: one binary, so one link and one launch.
//! `tests/surface_record.rs` stays its own binary: it installs a counting
//! global allocator.

mod active_route;
mod app_module;
mod canvas2d;
mod flow_agent;
mod foresee;
mod format;
mod frames_agent;
mod height_binding;
mod incremental;
mod list_layout;
mod live_tick;
mod native_field_resize;
mod now_screen;
mod reorder_codec;
mod replay;
mod router;
mod transform_binding;
mod uses;
mod viewport;
