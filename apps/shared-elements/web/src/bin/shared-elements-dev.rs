//! The resident dev driver for Shared Elements: `shared-elements-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app shared-elements`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<shared_elements_data::Shared>()
}
