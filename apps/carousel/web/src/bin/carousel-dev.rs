//! The resident dev driver for Carousel: `carousel-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app carousel`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<carousel_data::Cards>()
}
