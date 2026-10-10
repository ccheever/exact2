//! Reuse the shared bake for the Android app.

#[path = "../build.rs"]
mod bake;

fn main() {
    bake::main();
}
