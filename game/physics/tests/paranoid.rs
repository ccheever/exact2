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

#[test]
fn incomplete_v1_refusal_keeps_destination_unchanged() {
    let mut sim = common::scene("stack");
    sim.run(50.0);
    sim.key_down("KeyW");
    let before = sim.save();
    let mut old = before.clone();
    let at = old.windows(8).position(|s| s == b"EXPHYS\0\x02").unwrap();
    old[at + 7] = 1;
    for bound in [false, true] {
        let error = if bound {
            sim.restore_bound(&old)
        } else {
            sim.restore(&old)
        }
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("EXPHYS v2") && error.contains("incomplete"),
            "{error}"
        );
        assert!(
            sim.save() == before,
            "refused restore mutated the destination"
        );
    }
}
