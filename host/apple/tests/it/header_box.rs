//! LLP 1075.003 §9.10: a drawn face's box (a filled, fixed-size box with one
//! child: a title's avatar) carries its authored points as `faceBoxSize`,
//! since a header the bar replaces is laid out as `display: none`. The test
//! reads the box alone, so a box moved into a header, or nine wrappers deep,
//! keeps it; a box that stops being one loses it.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

/// The JSON object of the op for `id` in a batch, and its `props`.
fn props_of(batch: &str, id: u32) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(batch)
        .map_err(|e| eprintln!("not JSON: {e}"))
        .ok()?;
    v["ops"]
        .as_array()?
        .iter()
        .find(|op| op["id"] == id && op.get("props").is_some())
        .map(|op| op["props"].clone())
}

#[test]
fn a_face_box_carries_its_authored_size_and_loses_it() {
    let src = "component App\n  state wide = false\n  action widen\n    wide = not wide\n  view\n    column\n      header\n        row press=widen testId=\"go\"\n          column\n            column\n              column\n                column\n                  column\n                    column\n                      column\n                        column\n                          column width=(wide ? \"auto\" : 40) height=40 background-color=\"#dcfce7\" testId=\"avatar\"\n                            text \"MC\"\n          text \"Maya\" aria-level=2\n      column width=40 height=40 background-color=\"transparent\" testId=\"empty\"\n        text \"no fill\"\n      column width=40 height=40 testId=\"plain\"\n        text \"unstyled\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let id = |host: &Host<NoData>, t: &str| {
        let k = host.runner().kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let avatar = id(&host, "avatar");
    let props = props_of(&first, avatar).expect("the avatar is created with props");
    assert_eq!(
        props["faceBoxSize"], "40x40",
        "nine wrappers below the header: {first}"
    );
    let empty =
        props_of(&first, id(&host, "empty")).expect("the unfilled box is created with props");
    assert!(
        empty.get("faceBoxSize").is_none(),
        "a transparent box is no face"
    );
    let plain =
        props_of(&first, id(&host, "plain")).expect("the unstyled box is created with props");
    assert!(
        plain.get("faceBoxSize").is_none(),
        "nor is a box with no background-color"
    );
    // A width that stops being authored points clears it.
    let go = id(&host, "go");
    let next = host.dispatch_at(go, Event::Press, 0.0);
    let v: serde_json::Value = serde_json::from_str(&next).unwrap();
    let cleared = v["ops"].as_array().unwrap().iter().any(|op| {
        op["op"] == "props"
            && op["id"] == avatar
            && op["clear"]
                .as_array()
                .is_some_and(|c| c.iter().any(|k| k == "faceBoxSize"))
    });
    assert!(cleared, "no longer a fixed box: {next}");
}
