//! The resident dev driver for Metronome: `metronome-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app metronome`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<metronome_data::Metronome>()
}
