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
