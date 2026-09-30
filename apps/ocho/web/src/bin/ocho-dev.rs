//! The resident dev driver for Ocho: `ocho-dev <source> <out>`, run through
//! `bun host/web/dev.mjs --app ocho`, which pushes each plan to the page.
//! It bakes with the app's own data source, so the resources the module
//! answers stay pending rather than unknown.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<ocho_data::Ocho>()
}
