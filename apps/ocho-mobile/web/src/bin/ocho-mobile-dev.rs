//! The resident dev driver for Ocho on the phone: `ocho-mobile-dev <source> <out>`, run through
//! `bun host/web/dev.mjs --app ocho-mobile`, which pushes each plan to the page.
//! It bakes with the app's own data source, so the resources the module
//! answers stay pending rather than unknown.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<ocho_mobile_data::OchoMobile>()
}
