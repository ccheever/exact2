//! The resident dev driver for Reflow: `dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app reflow`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<reflow_data::Reflow>()
}
