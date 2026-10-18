//! The GPU runtime's integration tests: one binary, so one link and one launch.
//! `tests/fixture.rs` stays its own binary: it sets EXACT_GPU_OUT, and wgpu reads
//! the environment while other tests create devices. So does
//! `tests/readback_shaders.rs`: it registers a shader in the module's process-wide
//! registry, which `shaders` clears.

mod frame;
mod idle;
mod module;
mod shaders;
mod uniform;
