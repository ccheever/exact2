//! Media events and properties survive compilation and the host boundary.
use exact_runner::Event;
use exact_web::Host;
#[test]
fn media_properties_events_and_rejections() {
    let source = r#"component App
  state seconds = 0
  action update(value: number) writes seconds
    seconds = value
  view
    column interactive-widget="resizes-content"
      video "assets/movie.mp4" testId="video" controls=false muted=true currentTime=seconds timeupdate=update
      text `${seconds}`
"#;
    let plan = contract::compile(source).unwrap();
    let (mut host, first) = Host::boot(&plan.encode(), caltrain_data::Caltrain).unwrap();
    assert!(first.contains("\"tag\":\"video\""), "{first}");
    assert!(
        first.contains("\"interactiveWidget\":\"resizes-content\""),
        "{first}"
    );
    assert!(first.contains("\"controls\":\"false\""), "{first}");
    let key = host.runner().kernel().find_by_test_id("video")[0];
    let id = host.runner().kernel().node_by_key(key).unwrap().id;
    let event = Event::media_payload("timeupdate\n12.5").unwrap();
    let batch = host.dispatch(id, event);
    assert!(batch.contains("12.5"), "{batch}");
    assert!(!batch.contains("\"op\":\"create\""), "{batch}");
    assert!(Event::media_payload("timeupdate\nNaN").is_none());
    assert!(Event::media_payload("press\n").is_none());
    for attribute in ["volume=2", "playbackRate=0", "preload=\"sometimes\""] {
        assert!(
            contract::compile(&format!("component App\n  view\n    video {attribute}\n")).is_err()
        );
    }
}
