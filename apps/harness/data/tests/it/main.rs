//! The harness data module's integration tests: one binary, so one link and
//! one launch. `tests/http.rs` stays its own binary: it sets `OLLAMA_HOST` and
//! `EXACT_HARNESS_OLLAMA_MODEL`, which the module reads while other tests run it.

mod openrouter;
mod runner;
mod screen;
mod startup;
