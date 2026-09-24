//! The canvas seam (LLP 1009 D2/D3): a `canvas` node reaches the page as a
//! `<canvas>` with the web's default size, and its surface's inputs —
//! arguments evaluated against state — arrive as `surface` ops, at boot
//! and whenever they change, never otherwise.

use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot() -> (Host<NoData>, String) {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/canvas.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn a_canvas_is_a_canvas_element_with_the_webs_default_size() {
    let (host, batch) = boot();
    let bare = view(&host, "bare");
    assert!(
        batch.contains(&format!(
            "\"op\":\"create\",\"id\":{bare},\"tag\":\"canvas\""
        )),
        "{batch}"
    );
    let at = batch
        .find(&format!("\"id\":{bare},\"tag\":\"canvas\""))
        .unwrap();
    let create = &batch[at..at + 200];
    assert!(
        create.contains("width:300px;height:150px;"),
        "a bare canvas is 300×150, as on the web: {create}"
    );
    let map = view(&host, "map");
    assert!(
        batch.contains(&format!(
            "\"op\":\"surface\",\"id\":{map},\"name\":\"map\",\"values\":[1,\"caltrain\"]}}"
        )),
        "{batch}"
    );
    // The sky (LLP 1014): a canvas with children, its surface's one input
    // a string from state.
    let sky = view(&host, "sky");
    assert!(
        batch.contains(&format!(
            "\"op\":\"surface\",\"id\":{sky},\"name\":\"aurora\",\"values\":[\"1\"]}}"
        )),
        "{batch}"
    );
    assert_eq!(
        batch.matches("\"op\":\"surface\"").count(),
        2,
        "the bare canvas has no surface; `map` and `sky` do"
    );
}

#[test]
fn surface_inputs_are_published_when_they_change_and_only_then() {
    let (mut host, _) = boot();
    let map = view(&host, "map");
    let zoom = view(&host, "zoom");
    let quiet = host.advance(1000.0);
    assert!(
        !quiet.contains("\"op\":\"surface\""),
        "nothing changed: {quiet}"
    );
    let batch = host.dispatch(zoom, Event::Press);
    assert!(
        batch.contains(&format!(
            "\"op\":\"surface\",\"id\":{map},\"name\":\"map\",\"values\":[2,\"caltrain\"]}}"
        )),
        "{batch}"
    );
    assert_eq!(host.runner().plan().surfaces.len(), 2);
}

#[test]
fn the_caltrain_app_boots_with_both_surfaces() {
    let plan = caltrain::build().unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    let (_, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        batch.contains("\"name\":\"glass\",\"values\":[\"glass\",\"mv\"]"),
        "{}",
        &batch[batch.len().saturating_sub(600)..]
    );
    assert!(
        batch.contains("\"name\":\"map\",\"values\":[[["),
        "the map's inputs start with the station list"
    );
    assert_eq!(batch.matches("\"op\":\"surface\"").count(), 2);
}

#[test]
fn r13_named_empty_and_short_contract_calls_survive_web_batch() {
    for (call, values) in [
        (
            "world(restart=false, seed=7, paused=true)",
            r#"{"restart":false,"seed":7,"paused":true}"#,
        ),
        ("world()", "{}"),
        ("world(7)", "[7]"),
    ] {
        let plan = contract::compile(&format!(
            "component App\n  view\n    canvas surface={call}\n"
        ))
        .unwrap();
        exact_web::link(exact_web_capabilities::ALL);
        let (_, batch) =
            exact_web::Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
        assert!(batch.contains(&format!("\"values\":{values}")), "{batch}");
    }
}
