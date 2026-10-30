use super::*;
use exact_game::{Game, Input, Sim};

struct Empty;
impl Game for Empty {
    const ID: &'static str = "host-frame-publication";
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

#[test]
fn encoding_is_not_publication_and_submission_is_consumed_once() {
    let sim = Sim::<Empty>::new(()).unwrap();
    let world = sim.world();
    let mut publication = Publication::default();
    assert_eq!(
        publication
            .record(None, publication.ready(world))
            .as_deref(),
        Some(r#"{"frame_ready":false}"#)
    );
    publication.encode(world, true, false);
    publication.submitted(world);
    assert!(!publication.ready(world));
    publication.encode(world, true, true);
    assert!(!publication.ready(world));
    assert!(publication.record(None, false).is_none());
    publication.submitted(world);
    assert!(publication.ready(world));
    assert_eq!(
        publication.record(None, true).as_deref(),
        Some(r#"{"frame_ready":true}"#)
    );
    publication.submitted(world);
    assert!(publication.record(None, true).is_none());
}

#[test]
fn an_unsubmitted_or_foreign_frame_cannot_make_a_world_ready() {
    let first = Sim::<Empty>::new(()).unwrap();
    let second = Sim::<Empty>::new(()).unwrap();
    assert_eq!(
        first.world().presentation_generation(),
        second.world().presentation_generation()
    );
    let mut publication = Publication::default();
    publication.encode(first.world(), true, true);
    publication.discard_encoded();
    publication.submitted(first.world());
    assert!(!publication.ready(first.world()));
    publication.encode(first.world(), true, true);
    publication.submitted(second.world());
    assert!(!publication.ready(first.world()));
    assert!(!publication.ready(second.world()));
    publication.submitted(first.world());
    assert!(!publication.ready(first.world()));
}

#[test]
fn retained_drawable_frames_stay_available_but_missing_imagery_does_not() {
    let sim = Sim::<Empty>::new(()).unwrap();
    let world = sim.world();
    let mut publication = Publication::default();
    publication.encode(world, true, true);
    publication.submitted(world);
    publication.encode(world, true, false);
    publication.submitted(world);
    assert!(publication.ready(world));
    publication.encode(world, false, false);
    assert!(publication.ready(world));
    publication.submitted(world);
    assert!(!publication.ready(world));
    publication.encode(world, true, false);
    publication.submitted(world);
    assert!(!publication.ready(world));
}

#[test]
fn invalidation_discards_both_presented_and_queued_generations() {
    let sim = Sim::<Empty>::new(()).unwrap();
    let world = sim.world();
    let mut publication = Publication::default();
    publication.encode(world, true, true);
    publication.submitted(world);
    publication.record(Some(r#"{"score":7}"#.into()), true);
    publication.encode(world, true, true);
    publication.invalidate();
    publication.submitted(world);
    assert!(!publication.ready(world));
    assert_eq!(
        publication.record(None, false).as_deref(),
        Some(r#"{"frame_ready":false,"score":7}"#)
    );
    publication.encode(world, true, true);
    publication.submitted(world);
    assert!(publication.ready(world));
}

#[test]
fn host_fact_overrides_only_its_reserved_field_without_rounding_game_values() {
    let mut publication = Publication::default();
    let game = r#"{"big":340282366920938463463374607431768211455,"frame_ready":true,"nested":{"frame_ready":false,"option":[{"label":"\"quoted\""}]},"tiny":1.234567890123456789e-90}"#;
    assert_eq!(
        publication.record(Some(game.into()), false).unwrap(),
        r#"{"big":340282366920938463463374607431768211455,"frame_ready":false,"nested":{"frame_ready":false,"option":[{"label":"\"quoted\""}]},"tiny":1.234567890123456789e-90}"#
    );
    assert_eq!(publication.record(None, true).unwrap(), game);
    assert!(publication.record(None, true).is_none());
    assert_eq!(
        publication
            .record(Some(r#"{"replacement":[]}"#.into()), true)
            .as_deref(),
        Some(r#"{"frame_ready":true,"replacement":[]}"#)
    );
}

#[test]
fn escaped_field_names_and_null_publications_survive_host_updates() {
    let mut publication = Publication::default();
    let game = r#"{"a\"key":null,"record":["héllo",[7,true],"present"]}"#;
    assert_eq!(
        publication.record(Some(game.into()), false).as_deref(),
        Some(r#"{"a\"key":null,"frame_ready":false,"record":["héllo",[7,true],"present"]}"#)
    );
    assert_eq!(
        publication.record(None, true).as_deref(),
        Some(r#"{"a\"key":null,"frame_ready":true,"record":["héllo",[7,true],"present"]}"#)
    );
}
