//! `harness [--fullscreen] [--size COLSxROWS] [op …]`: the coding agent in
//! this terminal, or headless under the agent's verbs (see
//! [`exact_terminal::cli`]).

fn main() -> std::process::ExitCode {
    let entry = concat!(env!("CARGO_MANIFEST_DIR"), "/../terminal.contract");
    let args = std::env::args().skip(1).collect();
    exact_terminal::cli::run(
        args,
        harness_data::Harness::new(),
        Some(entry),
        exact_terminal::host::Mode::Inline,
    )
}
