//! The resident dev driver for Video Player: `dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app video-player`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<video_player_data::Player>()
}
