//! CSS inheritance across the Apple host's batch (LLP 1035.000 slice 1): a
//! run or an editor carries the computed rows it measures with; an
//! ancestor's change re-sends exactly the descendants that follow it; and
//! `layout <node>`'s runner half (LLP 1035.002 D1) says where each value
//! came from.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn count(batch: &str, op: &str) -> usize {
    batch.matches(&format!("\"op\":\"{op}\"")).count()
}

fn op(batch: &str, id: u32) -> String {
    batch
        .split("{\"op\":")
        .find(|part| part.contains(&format!("\"id\":{id},")))
        .unwrap_or_else(|| panic!("missing {id} in {batch}"))
        .to_owned()
}

const SRC: &str = r##"component Type
  state big = false
  action toggle writes big
    big = not big
  view
    column font-size=(big ? 24 : 20) line-height=24 testId="root"
      button press=toggle testId="toggle"
        text "Toggle"
      text testId="paragraph"
        text "plain" testId="plain"
        text "bold" font-weight=700 testId="bold"
        text "small" font-size=12 testId="small"
      input testId="field" value="Input"
      column font-size=14
        text "nested" testId="nested"
"##;

#[test]
fn inherited_text_rows_reach_runs_and_editors_as_computed_values() {
    let plan = contract::compile(SRC).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let plain = view(&host, "plain");
    let bold = view(&host, "bold");
    let small = view(&host, "small");
    let field = view(&host, "field");
    let nested = view(&host, "nested");
    // A run or an editor carries the computed rows it measures with; an own
    // row stays its own; a box node carries only its colour.
    for id in [plain, bold, field] {
        assert!(
            op(&first, id).contains("\"font_size\":20"),
            "{}",
            op(&first, id)
        );
    }
    assert!(op(&first, plain).contains("\"line_height\":24"));
    assert!(op(&first, bold).contains("\"font_weight\":700"));
    assert!(op(&first, small).contains("\"font_size\":12"));
    assert!(op(&first, small).contains("\"line_height\":24"));
    assert!(op(&first, nested).contains("\"font_size\":14"));
    assert!(!op(&first, view(&host, "toggle")).contains("font_size"));
    // The ancestor's change re-sends exactly the descendants that follow it.
    let changed = host.dispatch_at(view(&host, "toggle"), Event::Press, 0.0);
    for id in [plain, bold, field] {
        assert!(
            op(&changed, id).contains("\"font_size\":24"),
            "{}",
            op(&changed, id)
        );
    }
    for id in [small, nested] {
        assert!(
            !changed.contains(&format!("\"op\":\"style\",\"id\":{id},")),
            "{id} overrides the row and must not be re-sent: {changed}"
        );
    }
    assert_eq!(count(&host.resize(402.0, 874.0), "style"), 0);
}

#[test]
fn a_node_read_names_where_each_value_came_from() {
    let plan = contract::compile(SRC).unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let root = view(&host, "root");
    let small = view(&host, "small");
    let reply = host.agent(&format!("{{\"op\":\"node\",\"id\":{small}}}"));
    assert!(
        reply.contains("\"epoch\":") && reply.contains("\"incarnation\":1"),
        "{reply}"
    );
    // Its own size is authored; the line height is its paragraph's parent's
    // — the root column — by inheritance; letter spacing is initial.
    assert!(
        reply.contains("\"font_size\":{\"value\":12,\"source\":\"authored\"}"),
        "{reply}"
    );
    assert!(
        reply.contains(&format!(
            "\"line_height\":{{\"value\":24,\"source\":\"inherited\",\"from\":{root}}}"
        )),
        "{reply}"
    );
    assert!(
        reply.contains("\"letter_spacing\":{\"value\":0,\"source\":\"initial\"}"),
        "{reply}"
    );
    assert!(
        reply.contains("\"text_color\":{\"value\":\"#000000\",\"source\":\"initial\"}"),
        "{reply}"
    );
    // A box row never appears unless authored.
    assert!(!reply.contains("\"width\":"), "{reply}");
    assert!(
        reply.contains("\"props\":{\"text\":\"small\",\"testId\":\"small\"}")
            || reply.contains("\"testId\":\"small\""),
        "{reply}"
    );
    assert!(
        reply.contains("\"site\":") && reply.contains("\"frame\":{"),
        "{reply}"
    );
    // A stale id is refused by name, never answered from a reused slot.
    let stale = host.agent("{\"op\":\"node\",\"id\":9999}");
    assert!(stale.contains("stale node #9999"), "{stale}");
}
