//! The walker's integration tests: one binary, so one link and one launch.
//! `tests/benchmark.rs` stays its own binary: it installs a counting global
//! allocator that other tests would contaminate.

mod bands;
mod flow;
mod shapes;
mod support;
mod walker;
