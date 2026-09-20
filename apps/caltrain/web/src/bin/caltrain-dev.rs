//! The resident dev driver for the Caltrain app: `caltrain-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<caltrain_data::Caltrain>()
}
