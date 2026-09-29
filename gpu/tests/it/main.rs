//! The GPU runtime's integration tests: one binary, so one link and one launch.
//! `tests/fixture.rs` stays its own binary: it sets EXACT_GPU_OUT, and wgpu reads
//! the environment while other tests create devices.

mod frame;
mod idle;
mod module;
mod shaders;
