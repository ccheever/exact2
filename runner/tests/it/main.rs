//! The runner's integration tests: one binary, so one link and one launch.
//! `tests/surface_record.rs` stays its own binary: it installs a counting
//! global allocator.

mod active_route;
mod app_module;
mod canvas2d;
mod flow_agent;
mod font_axes;
mod font_keyframes;
mod foresee;
mod format;
mod frames_agent;
mod height_binding;
mod incremental;
mod keep_mounted;
mod launch_stand_in;
mod list_layout;
mod listen;
mod live_tick;
mod live_timers;
mod long_list;
mod native_field_resize;
mod now_screen;
mod reach_margin;
mod reorder_codec;
mod replay;
mod router;
mod sockets;
mod tabs;
mod transform_binding;
mod uses;
mod viewport;
