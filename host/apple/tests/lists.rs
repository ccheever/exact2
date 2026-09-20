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

fn reader() -> Host<Rows> {
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
    Host::boot(
        &plan.encode(),
        Rows,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        240.0,
    )
    .unwrap()
    .0
}

#[test]
fn native_measurements_fill_the_window_and_retire_rows() {
    let mut host = reader();
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

/// A created row is an estimate until the kernel lays it out. The host settles
/// that inside the one report, so the presenter's batch already has final
/// frames and a second report of the same geometry has nothing to add.
#[test]
fn one_report_settles_the_window() {
    let mut host = reader();
    let list = host.runner().roots()[0];
    let viewport = ListViewport {
        height: 240.0,
        width: 390.0,
        ..Default::default()
    };
    let first = host.list_viewport(list, viewport);
    assert!(!first.contains("\"error\":\""), "{first}");
    assert!(first.contains("\"op\":\"create\""), "{first}");
    let kernel = host.runner().kernel();
    let content = kernel.node(list).unwrap().children()[0];
    assert_eq!(kernel.node(content).unwrap().frame.height, 24056.0);
    let second = kernel
        .node_by_key(kernel.find_by_test_id("row-1")[0])
        .unwrap();
    assert_eq!(
        second.frame.y, 80.0,
        "the measured first row is in this batch"
    );
    // The window is the scrollport and one more below it, 480px: the 80px row
    // and seventeen of 24px, the last straddling the edge. At the 24px
    // estimate the first round admits twenty; the measured 80px retires two.
    let mounted = kernel.node(content).unwrap().children().len();
    assert_eq!(mounted, 18);
    let again = host.list_viewport(list, viewport);
    assert!(
        !again.contains("\"op\":"),
        "a settled window is silent: {again}"
    );
}

fn mounted(host: &Host<Rows>, list: u32) -> usize {
    let kernel = host.runner().kernel();
    let content = kernel.node(list).unwrap().children()[0];
    kernel.node(content).unwrap().children().len()
}

/// A host that scrolls on the thread that lays out fills its window a few rows
/// at a time, between frames. The rows the scrollport shows are never
/// rationed; the overscan is, and the host is told when more is owed.
#[test]
fn a_rationed_report_fills_the_scrollport_now_and_the_overscan_in_turns() {
    let mut host = reader();
    let list = host.runner().roots()[0];
    let viewport = ListViewport {
        height: 240.0,
        width: 390.0,
        ..Default::default()
    };
    let first = host.list_viewport_within(list, viewport, Some(2));
    assert!(!first.contains("\"error\":\""), "{first}");
    // The 240px scrollport: the 80px row and seven of 24px, the last
    // straddling its edge. Two more are this report's whole budget, however
    // many rounds it took to settle the measured first row.
    assert_eq!(mounted(&host, list), 10);
    assert!(host.list_pending(list));
    let second = host
        .runner()
        .kernel()
        .node_by_key(host.runner().kernel().find_by_test_id("row-1")[0])
        .unwrap();
    assert_eq!(second.frame.y, 80.0, "a rationed report still settles");
    let mut reports = 1;
    while host.list_pending(list) {
        let batch = host.list_viewport_within(list, viewport, Some(2));
        assert!(!batch.contains("\"error\":\""), "{batch}");
        assert!(
            batch.contains("\"op\":\"create\""),
            "a pending window creates: {batch}"
        );
        reports += 1;
        assert!(reports <= 8, "the window must fill");
    }
    // The same eighteen rows the unrationed report mounts, four reports later.
    assert_eq!((mounted(&host, list), reports), (18, 5));
    let again = host.list_viewport_within(list, viewport, Some(2));
    assert!(
        !again.contains("\"op\":"),
        "a full window is silent: {again}"
    );
}

/// Rows that have left the window go with a report that creates, a few at a
/// time, never in a report of their own; the last of them go once nothing is
/// left to create.
#[test]
fn retirement_rides_with_creation_and_finishes_when_the_window_is_full() {
    let mut host = reader();
    let list = host.runner().roots()[0];
    let at = |top: f64| ListViewport {
        top,
        height: 240.0,
        width: 390.0,
        ..Default::default()
    };
    host.list_viewport(list, at(0.0));
    assert_eq!(mounted(&host, list), 18);
    // A jump: none of the mounted rows is in the new window.
    let jump = host.list_viewport_within(list, at(12000.0), Some(2));
    assert!(!jump.contains("\"error\":\""), "{jump}");
    let kernel = host.runner().kernel();
    assert!(
        !kernel.find_by_test_id("row-497").is_empty(),
        "the scrollport's own rows are there at once"
    );
    // Rows 497 to 507 cross the scrollport: eleven, and two more, created;
    // four of the old eighteen retired.
    assert_eq!(mounted(&host, list), 13 + 14);
    assert!(host.list_pending(list));
    let mut reports = 1;
    while host.list_pending(list) {
        host.list_viewport_within(list, at(12000.0), Some(2));
        reports += 1;
        assert!(reports <= 16, "the window must settle");
    }
    assert!(host.runner().kernel().find_by_test_id("row-0").is_empty());
    // One scrollport above, the scrollport, one below: 720px of 24px rows,
    // and the row straddling each end — rows 487 to 517.
    assert_eq!(mounted(&host, list), 31);
}
