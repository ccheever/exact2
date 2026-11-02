//! A world save refuses an older EXGAME format by name, and a refused load
//! leaves the world as it was.
use exact_game::*;

#[test]
fn world_v4_is_refused_by_name() {
    let error = World::new(60, 0)
        .load(b"EXGAME\0\x04")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("EXGAME v4") && error.contains("tableInputs"),
        "{error}"
    );
}

#[test]
fn world_v5_is_refused_by_name_atomically() {
    let mut world = World::new(60, 0);
    let saved = world.save();
    let mut old = saved.clone();
    old[7] = 5;
    let error = world.load(&old).unwrap_err().to_string();
    assert!(
        error.contains("EXGAME v5 predates v6 tableInputs"),
        "{error}"
    );
    assert_eq!(world.save(), saved);
}
