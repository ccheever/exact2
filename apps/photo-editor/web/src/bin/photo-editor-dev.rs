//! The resident dev driver for The photo editor: `photo-editor-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app photo-editor`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<photo_editor_data::Photo>()
}
