//! The resident dev driver for the Tamber binding host: `tamber-host-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app tamber-host --wasm`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<tamber_host_data::TamberHost>()
}
