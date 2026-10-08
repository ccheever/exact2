//! The executor's integration tests: one binary, so one link and one launch.
//! `tests/fixtures/` stays put: build.rs compiles those to bytecode.

mod background;
mod caltrain;
mod castle;
mod compress;
mod ecdsa;
mod entropy;
mod forget;
mod inputs;
mod interrupt;
mod pinned_engines;
mod pure;
mod render;
mod storage;
mod superseded;
mod windows_lean;
