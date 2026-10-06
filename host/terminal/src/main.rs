//! `exact-terminal <app.contract | app.plan> [--size COLSxROWS] [--inline] [op …]`:
//! a terminal entry with no data module (see [`exact_terminal::cli`]).

fn main() -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect();
    exact_terminal::cli::run(args, (), None, exact_terminal::host::Mode::Fullscreen)
}
