//! Media events and properties survive compilation and the host boundary.
use exact_runner::{Event, Viewport};
use exact_web::Host;
#[test]
fn media_properties_events_and_rejections() {
    let source = r#"component App
  state seconds = 0
  action update(value: number)
    seconds = value
  view
    column interactive-widget="resizes-content"
      video "assets/movie.mp4" testId="video" controls=false muted=true playbackVisibilityThreshold=0.5 currentTime=seconds timeupdate=update
      text `${seconds}`
"#;
    let plan = contract::compile(source).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Viewport::default(),
        "/",
    )
    .unwrap();
    assert!(first.contains("\"tag\":\"video\""), "{first}");
    assert!(
        first.contains("\"interactiveWidget\":\"resizes-content\""),
        "{first}"
    );
    assert!(first.contains("\"controls\":\"false\""), "{first}");
    assert!(
        first.contains("\"playbackVisibilityThreshold\":\"0.5\""),
        "{first}"
    );
    let key = host.runner().kernel().find_by_test_id("video")[0];
    let id = host.runner().kernel().node_by_key(key).unwrap().id;
    // Media and binary reorder dispatch occupy distinct web ABI kinds.
    let mut bridge = exact_web::abi::Bridge::new();
    bridge.boot(&plan.encode(), caltrain_data::Caltrain, 320., 200., "/");
    let payload = b"timeupdate\n12.5";
    bridge.input_write(payload);
    let n = bridge.dispatch(id, 19, payload.len(), 100.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("12.5"), "{out}");
    bridge.input_write(payload);
    let n = bridge.dispatch(id, 18, payload.len(), 100.);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid reorder event"));
    let event = Event::media_payload("timeupdate\n12.5").unwrap();
    let batch = host.dispatch(id, event);
    assert!(batch.contains("12.5"), "{batch}");
    assert!(!batch.contains("\"op\":\"create\""), "{batch}");
    assert!(Event::media_payload("timeupdate\nNaN").is_none());
    assert!(Event::media_payload("press\n").is_none());
    assert!(Event::media_payload("pan\n").is_none());
    for attribute in [
        "volume=2",
        "playbackRate=0",
        "preload=\"sometimes\"",
        "playbackVisibilityThreshold=-0.1",
        "playbackVisibilityThreshold=1.1",
    ] {
        assert!(
            contract::compile(&format!("component App\n  view\n    video {attribute}\n")).is_err()
        );
    }
}
