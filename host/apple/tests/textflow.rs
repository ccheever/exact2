//! @ref LLP 1043.000 §3 D4, D7 — real batches carry geometry independently of frames.
use exact_apple::{content_region::ContentRegionRegistration, Host};
use exact_kernel::{MonospaceMeasurer, TextMeasurer};
use exact_runner::{DataError, DataSource, Event, Value};
struct Data;
impl DataSource for Data {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
const SOURCE: &str = r#"component App
  state x = 80
  action move writes x
    x = x + 20
  action leave writes x
    x = 900
  view
    column width=400 height=500
      view id="owner" width=400 height=300 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%"
          text "The river carries its quiet story through the garden and beyond the trees." testId="prose" width=400 height=300
          box position="absolute" left=x top=40 width=60 height=60 wrap-flow="both" shape-outside="circle()"
        text "Waiting" id="pending"
      button testId="move" press=move
        text "Move"
      button testId="leave" press=leave
        text "Leave"
"#;
fn id(h: &Host<Data>, name: &str) -> u32 {
    let key = h.runner().kernel().find_by_test_id(name)[0];
    h.runner().kernel().node_by_key(key).unwrap().id
}
#[test]
fn unchanged_frame_gets_flow_and_leaving_shape_gets_empty_clear() {
    let plan = contract::compile(SOURCE).unwrap().encode();
    let (mut h, boot) = Host::boot(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        400.,
        500.,
    )
    .unwrap();
    let prose = id(&h, "prose");
    assert!(
        boot.contains(&format!("\"op\":\"flow\",\"id\":{prose}")),
        "{boot}"
    );
    let frame = h.runner().kernel().node(prose).unwrap().frame;
    let move_id = id(&h, "move");
    let moved = h.dispatch_at(move_id, Event::Press, 16.);
    assert!(
        moved.contains(&format!("\"op\":\"flow\",\"id\":{prose}")),
        "{moved}"
    );
    assert!(
        !moved.contains(&format!("\"op\":\"frame\",\"id\":{prose},")),
        "{moved}"
    );
    assert_eq!(frame, h.runner().kernel().node(prose).unwrap().frame);
    let leave_id = id(&h, "leave");
    let cleared = h.dispatch_at(leave_id, Event::Press, 32.);
    assert!(
        cleared.contains(&format!("\"op\":\"flow\",\"id\":{prose},\"shapes\":[]")),
        "{cleared}"
    );
}
#[test]
fn raster_region_retires_before_flowed_native_ink() {
    let plan = contract::compile(SOURCE).unwrap().encode();
    let (mut h, mut batch) = Host::boot_region(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        400.,
        500.,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    for _ in 0..20 {
        let Some((id, request)) = h.pending_region_request() else {
            break;
        };
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        batch = h.complete_region_text(id, metrics, std::rc::Rc::new(()));
    }
    assert!(
        batch.contains("\"disabled\":\"flowed text uses native fragments\""),
        "{batch}"
    );
    assert!(h.pending_region_request().is_none());
    assert!(h.region_publication_id().is_none());
    let prose = id(&h, "prose");
    assert!(
        batch.contains(&format!("\"op\":\"flow\",\"id\":{prose}")),
        "{batch}"
    );
    assert!(!h
        .runner()
        .kernel()
        .node(prose)
        .unwrap()
        .flow_shapes()
        .is_empty());
}
