#[path = "../bake.rs"]
mod bake;
fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    bake::build(platform);
}
