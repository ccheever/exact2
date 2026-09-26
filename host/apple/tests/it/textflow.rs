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
fn id<D: DataSource>(h: &Host<D>, name: &str) -> u32 {
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

#[test]
fn list_settlement_publishes_final_mounted_flow_and_clears_disappearing_shapes() {
    struct Rows;
    impl DataSource for Rows {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(
                (0..100).map(|n| Value::Number(n as f64)).collect(),
            ))
        }
    }
    let plan = contract::compile(r#"component App
  state holes = true
  resource rows = rows() as shape list<number>
  action clear writes holes
    holes = false
  view
    column width=240 height=200
      button testId="clear" press=clear
        text "Clear"
      list testId="list" estimated-item-height=80 width=240 height=160
        each row in rows key=row
          view width=240 height=80
            text `Paragraph ${row} around the circle` testId=`row-${row}` width=240 height=80
            when holes
              box position="absolute" left=80 top=10 width=40 height=40 wrap-flow="both" shape-outside="circle()"
"#).unwrap().encode();
    let (mut h, _) = Host::boot(
        &plan,
        Rows,
        Box::new(MonospaceMeasurer::default()),
        240.,
        200.,
    )
    .unwrap();
    let list = id(&h, "list");
    let old: std::collections::BTreeSet<_> = h
        .runner()
        .kernel()
        .arena()
        .iter_live()
        .map(|slot| h.runner().kernel().arena().key(slot))
        .collect();
    let batch = h.list_viewport(
        list,
        exact_apple::ListViewport {
            top: 400.,
            height: 160.,
            width: 240.,
            ..Default::default()
        },
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    let mut flowed = Vec::new();
    let mut mounted = 0;
    for slot in h.runner().kernel().arena().iter_live() {
        let key = h.runner().kernel().arena().key(slot);
        let node = h.runner().kernel().node_by_key(key).unwrap();
        if node.flow_shapes().is_empty() {
            continue;
        }
        flowed.push(node.id);
        if old.contains(&key) {
            continue;
        }
        mounted += 1;
        let mut expected = format!("{{\"op\":\"flow\",\"id\":{},\"shapes\":[", node.id);
        for (i, shape) in node.flow_shapes().iter().enumerate() {
            if i > 0 {
                expected.push(',');
            }
            shape.write_json(&mut expected);
        }
        expected.push_str("]}");
        assert!(
            batch.contains(&expected),
            "missing final flow {expected}: {batch}"
        );
        let prefix = format!("{{\"op\":\"flow\",\"id\":{},", node.id);
        assert_eq!(
            batch.matches(&prefix).count(),
            1,
            "intermediate geometry must not escape"
        );
    }
    assert!(
        mounted > 0,
        "the report must mount paragraphs under exclusions"
    );
    let clear = id(&h, "clear");
    let batch = h.dispatch_at(clear, Event::Press, 16.);
    for view in flowed {
        let Some(node) = h.runner().kernel().node(view) else {
            continue;
        };
        assert!(node.flow_shapes().is_empty());
        assert!(
            batch.contains(&format!("{{\"op\":\"flow\",\"id\":{view},\"shapes\":[]}}")),
            "{batch}"
        );
    }
    let repeated = h.resize(240., 200.);
    assert!(
        !repeated.contains("\"op\":\"flow\""),
        "unchanged clears are not repeated: {repeated}"
    );
}

// @ref LLP 1043.000 §8 — the Reader's drop cap: an auto-height paragraph is
// measured through the seam around the shape it is painted around.
#[test]
fn a_drop_cap_measures_its_auto_height_paragraph_around_the_shape() {
    use std::{cell::RefCell, rc::Rc};
    struct Seen(Rc<RefCell<Vec<usize>>>);
    impl TextMeasurer for Seen {
        fn measure(
            &mut self,
            r: &exact_kernel::TextMeasureRequest<'_>,
        ) -> exact_kernel::TextMetrics {
            self.0.borrow_mut().push(r.exclusions.len());
            MonospaceMeasurer::default().measure(r)
        }
    }
    let boot = |wrap: &str| {
        let plan = contract::compile(&format!(
            r#"component App
  view
    box width=360 padding=20 position="relative"
      box position="absolute" left=20 top=20 width=48 height=48 wrap-flow="{wrap}" shape-outside="inset(0)" shape-margin=6
        text "T" font-size=48 line-height=1
      text "There is an hour when the garden belongs to neither day nor night. The visitors have gone, but the birds have not yet settled." testId="lede"
      text "After." testId="after"
"#
        ))
        .unwrap()
        .encode();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (h, batch) = Host::boot(&plan, Data, Box::new(Seen(seen.clone())), 400., 600.).unwrap();
        let seen = seen.borrow().clone();
        (h, batch, seen)
    };
    let (plain, _, _) = boot("auto");
    let (h, batch, seen) = boot("both");
    let (lede, after) = (id(&h, "lede"), id(&h, "after"));
    let kernel = h.runner().kernel();
    let (l, a) = (
        kernel.node(lede).unwrap().frame,
        kernel.node(after).unwrap().frame,
    );
    assert!(seen.contains(&1), "no request carried the shape: {seen:?}");
    assert!(
        batch.contains(&format!("\"op\":\"flow\",\"id\":{lede}")),
        "{batch}"
    );
    assert_eq!(kernel.node(lede).unwrap().flow_refusal(), None);
    assert!(l.height > plain.runner().kernel().node(lede).unwrap().frame.height);
    assert_eq!(a.y, l.y + l.height);
}
