mod common;
#[path = "../../paranoid-test.rs"]
mod paranoid;
#[test]
fn physics_snapshot_survives_every_tick_reconstruction() {
    paranoid::compare(
        || common::scene("stack"),
        |sim| {
            sim.run(2000.0);
        },
    );
}
