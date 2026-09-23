//! Caltrain's pages as HTML documents (LLP 1048.000 D1, D9):
//! `caltrain-render [--plan <app.plan>] [--deadline <ms>] (--build |
//! <location>…)`, one JSON line each. The web build and the document parity
//! check run it. It is a binary of its own over the Linux host's crates, not
//! a mode of `caltrain-linux`, whose painter comes up at startup (D10).

/// The baked plan, written by `build.rs`.
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));

fn main() -> std::process::ExitCode {
    exact_render::main(PLAN, caltrain_data::Caltrain::default)
}
