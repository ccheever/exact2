//! @ref LLP 1039 D3 / LLP 1030 D7 — host facts must not enter kept storage.
use exact_kernel::{Kernel, NodeType};
use exact_plan::{builder::PlanBuilder, TypeKind, Value};
use exact_runner::{DataError, DataSource, Runner, Viewport};

struct Deferred;
impl DataSource for Deferred {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("runner fact reached data source {source}")
    }
    fn ready(&self) -> bool {
        false
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
        None,
    );
    b.resource(
        "delivery",
        exact_runner::delivery::SOURCE,
        &[],
        delivery,
        None,
    );
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut runner = Runner::boot(
        b.finish().unwrap(),
        Deferred,
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    assert!(runner.take_router_change().is_none());
    assert!(runner.carry().keeps_answers);
    assert!(runner.take_store_writes().is_empty());
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
}
