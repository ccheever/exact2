//! `harness [--fullscreen] [--size COLSxROWS] [op …]`: the coding agent in
//! this terminal, or headless under the agent's verbs (see
//! [`exact_terminal::cli`]).

fn main() -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect();
    exact_terminal::cli::run_embedded(
        args,
        harness_data::Harness::new(),
        include_bytes!(concat!(env!("OUT_DIR"), "/terminal.plan")),
        exact_terminal::host::Mode::Inline,
    )
}
