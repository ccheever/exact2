//! The resident dev driver for Sparkline: `sparkline-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app sparkline`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<sparkline_data::Spark>()
}
