//! The executor's integration tests: one binary, so one link and one launch.
//! `tests/fixtures/` stays put: build.rs compiles those to bytecode.

mod caltrain;
mod castle;
mod inputs;
mod interrupt;
mod pure;
mod render;
mod storage;
