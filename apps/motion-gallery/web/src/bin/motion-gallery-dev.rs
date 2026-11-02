//! The resident dev driver for Motion Gallery: `motion-gallery-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app motion-gallery`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<motion_gallery_data::Gallery>()
}
