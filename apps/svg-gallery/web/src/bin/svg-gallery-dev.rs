//! The resident dev driver for SVG Gallery: `svg-gallery-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app svg-gallery`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<svg_gallery_data::Gallery>()
}
