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
    let (mut host, _) = booted();
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
    let (mut host, _) = booted();
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

#[test]
fn unfinished_list_settlement_is_owned_by_its_list() {
    let plan = contract::compile(
        r#"shape Item
  id: number
component App
  resource rows = rows() as shape list<Item>
  view
    column
      list estimated-item-height=24 height=240 width=390 testId="slow"
        each item in rows key=item.id
          view height=0
      list estimated-item-height=24 height=240 width=390 testId="fast"
        each item in rows key=item.id
          view height=24
"#,
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        Rows,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        480.0,
    )
    .unwrap();
    let id = |host: &Host<Rows>, name| {
        host.runner()
            .kernel()
            .node_by_key(host.runner().kernel().find_by_test_id(name)[0])
            .unwrap()
            .id
    };
    let slow = id(&host, "slow");
    let fast = id(&host, "fast");
    let viewport = ListViewport {
        height: 240.0,
        width: 390.0,
        ..Default::default()
    };
    let batch = host.list_viewport(slow, viewport);
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert!(
        host.list_pending(slow),
        "zero-height discovery needs another bounded call"
    );
    let batch = host.list_viewport(fast, viewport);
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert!(!host.list_pending(fast));
    assert!(
        host.list_pending(slow),
        "another list cannot clear this continuation"
    );
}

#[test]
fn budgeted_hidden_viewport_and_released_pin_retire_immediately() {
    let (mut host, list) = booted();
    report(&mut host, list, 0.0);
    let kernel = host.runner().kernel();
    let first = kernel
        .node_by_key(kernel.find_by_test_id("row-0")[0])
        .unwrap()
        .id;
    let viewport = ListViewport {
        top: 12000.0,
        height: 240.0,
        width: 390.0,
        pins: [first, 0],
        ..Default::default()
    };
    host.list_viewport_within(list, viewport, Some(1));
    assert!(!host.runner().kernel().find_by_test_id("row-0").is_empty());
    host.list_viewport_within(
        list,
        ListViewport {
            pins: [0, 0],
            ..viewport
        },
        Some(1),
    );
    assert!(
        host.runner().kernel().find_by_test_id("row-0").is_empty(),
        "a released distant pin is not budgeted retirement"
    );
    host.list_viewport_within(
        list,
        ListViewport {
            height: 0.0,
            width: 260.0,
            pins: [0, 0],
            ..viewport
        },
        Some(1),
    );
    assert_eq!(
        mounted(&host, list),
        0,
        "hidden lists release their mounted rows immediately"
    );
    assert!(!host.list_pending(list));
}

/// Spend the existing three-viewport extent in the direction of travel, with
/// one atomic row per report and retirement attached to the same report.
#[test]
fn directional_fill_uses_one_row_reports_and_reverses_within_the_window_budget() {
    let (mut host, _) = booted();
    let list = host.runner().roots()[0];
    let at = |velocity| ListViewport {
        top: 12000.0,
        height: 240.0,
        width: 390.0,
        velocity,
        ..Default::default()
    };
    report(&mut host, list, 0.0);
    host.list_viewport(
        list,
        ListViewport {
            width: 390.0,
            ..Default::default()
        },
    );
    host.list_viewport_within(list, at(12000.0), Some(0));
    let indices = |host: &Host<Rows>| {
        let k = host.runner().kernel();
        let content = k.node(list).unwrap().children()[0];
        k.node(content)
            .unwrap()
            .children()
            .iter()
            .map(|id| {
                let row = k.node(*id).unwrap();
                row.frame.y as i32
            })
            .collect::<Vec<_>>()
    };
    let visible = indices(&host);
    assert!(visible.iter().all(|y| *y >= 11976 && *y < 12240));
    host.list_viewport_within(list, at(12000.0), Some(1));
    let forward = indices(&host);
    assert_eq!(forward.len(), visible.len() + 1);
    assert_eq!(forward.first(), visible.first(), "ahead is admitted first");
    assert!(forward.last() > visible.last());
    for _ in 0..40 {
        if !host.list_pending(list) {
            break;
        }
        host.list_viewport_within(list, at(12000.0), Some(1));
    }
    assert!(!host.list_pending(list));
    let ahead = indices(&host);
    assert!(ahead.len() <= 31, "same three-viewport row allowance");
    assert!(ahead[0] >= 11640, "bounded trailing band");
    host.list_viewport_within(list, at(-12000.0), Some(1));
    let reverse = indices(&host);
    assert!(reverse[0] < ahead[0], "reversal immediately admits behind");
    for _ in 0..40 {
        if !host.list_pending(list) {
            break;
        }
        host.list_viewport_within(list, at(-12000.0), Some(1));
    }
    assert!(!host.list_pending(list));
    assert!(indices(&host).len() <= 31);
    let symmetric = host.list_viewport(list, at(0.0));
    assert!(!symmetric.contains("\"error\":\""));
    assert!(indices(&host).len() <= 31);
}
