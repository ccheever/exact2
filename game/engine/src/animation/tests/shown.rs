use super::*;

// rig.model with no declaration: as a streamed model or one loaded on sight.
fn streamed() -> World {
    let mut w = world();
    w.assets.declared.clear();
    w
}

#[test]
fn shown_samples_an_undeclared_model_at_this_tick_and_the_one_before() {
    let mut w = streamed();
    assert!(w.model("rig.model").is_none(), "simulation cannot read it");
    let e = w.spawn((Transform::default(), Mesh::asset("rig.model")));
    let rest = bind_pose(drawn_model(&w, "rig.model").unwrap());
    let mut shown = ShownPose::default();
    assert!(
        !shown.sample(&w, e, &rest).unwrap(),
        "no ShownClips, nothing drawn"
    );
    // "slow" moves node 0 one unit along x over one second.
    w.insert(e, ShownClips::clip("slow", 0.5).speed(2.));
    assert!(shown.sample(&w, e, &rest).unwrap());
    assert_eq!(shown.local[0], 0.5);
    assert!((shown.previous[0] - (0.5 - 2. / 60.)).abs() < 1e-6);
    // Looping wraps; once holds the end.
    w.insert(e, ShownClips::clip("slow", 1.25));
    shown.sample(&w, e, &rest).unwrap();
    assert!((shown.local[0] - 0.25).abs() < 1e-6);
    w.insert(e, ShownClips::clip("slow", 1.25).once());
    shown.sample(&w, e, &rest).unwrap();
    assert_eq!(shown.local[0], 1.);
    // A later clip mixes over the pose so far by its weight.
    w.insert(e, ShownClips::clip("slow", 0.5).and("fast", 0., 0.5));
    shown.sample(&w, e, &rest).unwrap();
    assert!((shown.local[0] - 0.25).abs() < 1e-6);
    // In place: the node keeps its bind translation.
    w.insert(e, ShownClips::clip("slow", 0.5).in_place(""));
    shown.sample(&w, e, &rest).unwrap();
    assert_eq!(shown.local[..3], rest[..3]);
    // Nothing until the model arrives.
    w.assets.models.remove("rig.model");
    assert!(!shown.sample(&w, e, &rest).unwrap());
}

#[test]
fn shown_refusals_are_named_and_inspected() {
    let mut w = streamed();
    let e = w.spawn((Transform::default(), Mesh::asset("rig.model")));
    let rest = bind_pose(drawn_model(&w, "rig.model").unwrap());
    let mut shown = ShownPose::default();
    w.insert(e, ShownClips::clip("moonwalk", 0.));
    let error = shown.sample(&w, e, &rest).unwrap_err();
    assert!(error.contains("moonwalk"), "{error}");
    assert!(status_json(&w, e).unwrap().contains("moonwalk"));
    w.insert(e, ShownClips::clip("slow", 0.25));
    assert!(status_json(&w, e).unwrap().contains(r#""state":"drawn""#));
    // Inspection reads the drawn pose of an undeclared model.
    assert!(pose_json(&w, e).is_ok());
    w.assets.models.remove("rig.model");
    assert!(status_json(&w, e)
        .unwrap()
        .contains(r#""state":"waiting for its model""#));
}

#[test]
fn shown_is_not_simulation_state() {
    let mut w = streamed();
    let e = w.spawn((Transform::default(), Mesh::asset("rig.model")));
    let before = w.hash();
    w.insert(e, ShownClips::clip("slow", 0.5));
    assert_eq!(w.hash(), before, "presentation is outside the hash");
    w.begin_tick();
    for read in [
        (|w: &World| {
            drawn_model(w, "rig.model");
        }) as fn(&World),
        |w| {
            let e = w.entities().next().unwrap();
            ShownPose::default().sample(w, e, &[]).ok();
        },
    ] {
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read(&w)));
        assert!(caught.is_err(), "a tick cannot read what arrival decides");
    }
}
