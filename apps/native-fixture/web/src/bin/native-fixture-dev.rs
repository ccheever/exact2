//! The resident dev driver for The native-module fixture: `native-fixture-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app native-fixture`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<native_fixture_data::Fixture>()
}
