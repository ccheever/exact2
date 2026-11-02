//! The resident dev driver for The geofencing fixture: `geofence-fixture-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app geofence-fixture`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<geofence_fixture_data::Fences>()
}
