//! The resident dev driver for Orientation Fixture: `orientation-fixture-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app orientation-fixture`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<orientation_fixture_data::Fixture>()
}
