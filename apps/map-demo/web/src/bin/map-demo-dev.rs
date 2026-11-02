//! The resident dev driver for The map demo: `map-demo-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app map-demo`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<map_demo_data::Map>()
}
