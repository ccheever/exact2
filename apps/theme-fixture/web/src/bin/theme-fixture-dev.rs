//! The resident dev driver for the theme fixture: `theme-fixture-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app theme-fixture --wasm`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<theme_fixture_data::Fixture>()
}
