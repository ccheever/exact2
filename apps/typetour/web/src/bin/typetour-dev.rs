//! The resident dev driver for Type Tour: `typetour-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app typetour`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<typetour_data::Tour>()
}
