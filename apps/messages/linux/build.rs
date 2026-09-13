#[path = "../bake.rs"]
mod bake;
fn main() {
    bake::build("linux");
}
