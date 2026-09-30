//! @ref LLP 1039 D3 / LLP 1030 D7 — host facts must not enter kept storage.
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, TypeKind, Value};
use exact_runner::{DataError, DataSource, Runner, Viewport};

#[derive(Default)]
struct Deferred {
    ready: bool,
    queries: Vec<Vec<Value>>,
}
impl DataSource for Deferred {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        assert!(
            self.ready,
            "deferred application source ran before activation"
        );
        assert_eq!(
            source, "value",
            "runner fact reached application data source"
        );
        self.queries.push(args.to_vec());
        Ok(args[0].clone())
    }
    fn ready(&self) -> bool {
        self.ready
    }
}

#[test]
fn runner_facts_never_write_kept_answers() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let string = b.primitive(TypeKind::String);
    let viewport = b.record("Viewport", &[("width", number), ("height", number)]);
    let delivery = b.record("Delivery", &[("stream", string)]);
    b.resource(
        "viewport",
        exact_runner::viewport::SOURCE,
        &[],
        viewport,
        Some(&Value::record(vec![Value::Number(1.), Value::Number(1.)])),
    );
    b.resource(
        "delivery",
        exact_runner::delivery::SOURCE,
        &[],
        delivery,
        Some(&Value::record(vec![Value::str("baked stream")])),
    );
    let zero = b.constant(&Value::Number(0.));
    let revision = b.slot("revision", number, zero);
    let mut arg = Asm::new();
    arg.load_slot(revision);
    let arg = b.code(arg);
    let value = b.resource("value", "value", &[arg], number, Some(&Value::Number(0.)));
    b.set_resource_initial_args(value, &[Value::Number(0.)]);
    b.set_resource_reader(value, true);
    let mut change = Asm::new();
    change.load_param(0).store_slot(revision);
    let change = b.code(change);
    b.action("change", &[("revision", number)], &[revision], change);
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut runner = Runner::boot(
        b.finish().unwrap(),
        Deferred::default(),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    assert!(runner.take_router_change().is_none());
    assert!(runner.carry().keeps_answers);
    assert!(runner.take_store_writes().is_empty());
    // Timers can change arguments while the host is still loading its module.
    // Each change must keep the compiled placeholder, without invoking app code.
    for revision in [1., 2.] {
        runner.act("change", vec![Value::Number(revision)]).unwrap();
        assert_eq!(runner.resource("value"), Some(&Value::Number(0.)));
    }
    assert!(runner.data().queries.is_empty());
    assert!(runner.data_ready().unwrap().is_none());
    assert!(runner.set_viewport(1280.0, 900.0).unwrap().is_some());
    assert_eq!(
        runner.resource("viewport"),
        Some(&Value::record(vec![
            Value::Number(1280.0),
            Value::Number(900.0)
        ]))
    );
    assert!(runner.take_store_writes().is_empty());
    let mut delivery = runner.delivery().clone();
    delivery.stream = "updated".into();
    assert!(runner.set_delivery(delivery).unwrap().is_some());
    assert!(runner.take_store_writes().is_empty());
    assert!(runner.carry().store.is_empty());
    assert_eq!(
        runner.resource("delivery"),
        Some(&Value::record(vec![Value::str("updated")]))
    );
    runner.data().ready = true;
    assert!(runner.data_ready().unwrap().is_some());
    assert_eq!(runner.resource("value"), Some(&Value::Number(2.)));
    assert_eq!(runner.data().queries, vec![vec![Value::Number(2.)]]);
    assert!(runner.data_ready().unwrap().is_none());
    assert_eq!(runner.data().queries.len(), 1);
    assert_eq!(
        runner
            .take_store_writes()
            .iter()
            .map(|write| write.name.as_str())
            .collect::<Vec<_>>(),
        ["exact.kept.value"]
    );
}

#[test]
fn corrupt_unicode_kept_answers_do_not_prevent_boot() {
    for stored in ["€x|00", "é|00", "💬|00", "0500000000|€x", "0500000000|€"] {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let number = b.primitive(TypeKind::Number);
        let mut arg = Asm::new();
        arg.number(0.);
        let arg = b.code(arg);
        let value = b.resource("value", "value", &[arg], number, Some(&Value::Number(0.)));
        b.set_resource_initial_args(value, &[Value::Number(0.)]);
        b.set_resource_reader(value, true);
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        let mut runner = Runner::boot_stored(
            b.finish().unwrap(),
            Deferred::default(),
            Kernel::with_monospace(),
            vec![("exact.kept.value".into(), stored.into())],
            Viewport::default(),
            "/",
        )
        .unwrap();
        assert_eq!(runner.resource("value"), Some(&Value::Number(0.)));
        runner.data().ready = true;
        runner.data_ready().unwrap();
        assert_eq!(runner.data().queries.len(), 1);
    }
}
