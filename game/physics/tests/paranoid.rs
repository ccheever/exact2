mod common;
#[path = "compare.rs"]
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
fn obsolete_snapshot_refusal_keeps_destination_unchanged() {
    let mut sim = common::scene("stack");
    sim.run(50.0);
    sim.key_down("KeyW");
    let before = sim.save().unwrap();
    let mut old = before.clone();
    let at = old.windows(8).position(|s| s == b"EXPHYS\0\x03").unwrap();
    for (version, bound) in [(1, false), (2, false), (1, true), (2, true)] {
        old[at + 7] = version;
        let error = if bound {
            sim.restore_bound(&old)
        } else {
            sim.restore(&old)
        }
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("EXPHYS v3") && error.contains("predate"),
            "{error}"
        );
        assert!(
            sim.save().unwrap() == before,
            "refused restore mutated the destination"
        );
    }
}

#[test]
fn live_scheduler_and_save_metadata_match_seekable_continuation() {
    use exact_game::{Clock, Paranoid};
    let mut continuous = common::scene("stack").paranoid(Paranoid::Off);
    let mut saved = common::scene("stack").paranoid(Paranoid::Save);
    let mut fresh = common::scene("stack").paranoid(Paranoid::FreshGame);
    for frame in 1..=120 {
        for sim in [&mut continuous, &mut saved, &mut fresh] {
            sim.frame_period(1000.0 / 144.0);
            sim.advance(frame as f64 * 1000.0 / 144.0, Clock::Live);
        }
        assert_eq!(
            continuous.world().hash(),
            saved.world().hash(),
            "save frame {frame}"
        );
        assert_eq!(
            continuous.world().hash(),
            fresh.world().hash(),
            "fresh frame {frame}"
        );
    }
    let tick = continuous.world().tick();
    let hash = continuous.world().hash();
    let bytes = continuous.save().unwrap();
    assert_eq!(continuous.world().tick(), tick);
    assert_eq!(
        continuous.world().hash(),
        hash,
        "save cannot invalidate capture metadata"
    );
    assert!(bytes == saved.save().unwrap() && bytes == fresh.save().unwrap());
    let mut reopened = common::scene("stack");
    reopened.restore(&bytes).unwrap();
    assert_eq!(reopened.world().hash(), hash);
    continuous.run(1000.0);
    reopened.run(1000.0);
    assert_eq!(continuous.world().hash(), reopened.world().hash());
    assert!(continuous.save().unwrap() == reopened.save().unwrap());
}
