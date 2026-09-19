//! Native list measurements must come from the kernel's actual row boxes.
use exact_apple::{Host, ListViewport};
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Value};

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..1000)
                .map(|id| Value::record(vec![Value::Number(id as f64)]))
                .collect(),
        ))
    }
}

#[test]
fn native_measurements_fill_the_window_and_retire_rows() {
    let plan = contract::compile(
        r#"shape Item
  id: number
component App
  resource rows = rows() as shape list<Item>
  view
    list estimated-item-height=24 height=240 width=390 testId="list"
      each item in rows key=item.id
        view height=(item.id == 0 ? 80 : 24) testId=`row-${item.id}`
          text `${item.id}`
"#,
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        Rows,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        240.0,
    )
    .unwrap();
    let list = host.runner().roots()[0];
    assert!(host.runner().kernel().live_count() < 20);
    for _ in 0..4 {
        let batch = host.list_viewport(
            list,
            ListViewport {
                height: 240.0,
                width: 390.0,
                ..Default::default()
            },
        );
        assert!(!batch.contains("\"error\":\""), "{batch}");
    }
    let kernel = host.runner().kernel();
    let content = kernel.node(list).unwrap().children()[0];
    assert_eq!(kernel.node(content).unwrap().frame.height, 24056.0);
    let second = kernel
        .node_by_key(kernel.find_by_test_id("row-1")[0])
        .unwrap();
    assert_eq!(
        second.frame.y, 80.0,
        "row 1 follows the measured 80px first row"
    );
    assert!(kernel.live_count() < 70);
    let batch = host.list_viewport(
        list,
        ListViewport {
            top: 12000.0,
            height: 240.0,
            width: 390.0,
            ..Default::default()
        },
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert!(batch.contains("\"op\":\"destroy\""));
    assert!(host.runner().kernel().find_by_test_id("row-0").is_empty());
    assert!(host.runner().kernel().live_count() <= 95);
}

fn booted() -> (Host<Rows>, u32) {
    let plan = contract::compile(
        r#"shape Item
  id: number
component App
  resource rows = rows() as shape list<Item>
  view
    list estimated-item-height=24 height=240 width=390 testId="list"
      each item in rows key=item.id
        view height=(item.id == 0 ? 80 : 24) testId=`row-${item.id}`
          text `${item.id}`
"#,
    )
    .unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        Rows,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        240.0,
    )
    .unwrap();
    let list = host.runner().roots()[0];
    (host, list)
}

fn report(host: &mut Host<Rows>, list: u32, top: f64) -> String {
    let batch = host.list_viewport(
        list,
        ListViewport {
            top,
            height: 240.0,
            width: 390.0,
            ..Default::default()
        },
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    batch
}

/// The window settles inside the report: the presenter gets one batch whose
/// rows already sit where their measured heights put them, and a second
/// report of the same geometry has nothing left to say (LLP 1044 F7).
#[test]
fn one_report_settles_the_window() {
    let (mut host, list) = booted();
    let first = report(&mut host, list, 0.0);
    assert!(first.contains("\"op\":\"create\""));
    let kernel = host.runner().kernel();
    let content = kernel.node(list).unwrap().children()[0];
    assert_eq!(kernel.node(content).unwrap().frame.height, 24056.0);
    let second = kernel
        .node_by_key(kernel.find_by_test_id("row-1")[0])
        .unwrap();
    assert_eq!(second.frame.y, 80.0, "corrected within the same report");
    let again = report(&mut host, list, 0.0);
    assert!(
        !again.contains("\"op\":\""),
        "nothing left to settle: {again}"
    );
}

/// A row that has left the window is not worth a relayout of its own: it is
/// retired by the next report that mounts a row (LLP 1044 F7).
#[test]
fn a_row_past_the_window_waits_for_the_next_pass_that_mounts_one() {
    let (mut host, list) = booted();
    report(&mut host, list, 0.0);
    // Window [90, 810): rows 1..=31.
    let moved = report(&mut host, list, 330.0);
    assert!(moved.contains("\"op\":\"create\"") && moved.contains("\"op\":\"destroy\""));
    assert!(host.runner().kernel().find_by_test_id("row-0").is_empty());
    // Window [104, 824): row 1 is past it and no new row is inside it.
    let idle = report(&mut host, list, 344.0);
    assert!(
        !idle.contains("\"op\":\""),
        "no pass for a retirement alone: {idle}"
    );
    assert!(!host.runner().kernel().find_by_test_id("row-1").is_empty());
    // Window [106, 826): row 32 enters, and row 1 goes with that pass.
    let next = report(&mut host, list, 346.0);
    assert!(next.contains("\"op\":\"create\"") && next.contains("\"op\":\"destroy\""));
    assert!(host.runner().kernel().find_by_test_id("row-1").is_empty());
    assert!(!host.runner().kernel().find_by_test_id("row-32").is_empty());
}
