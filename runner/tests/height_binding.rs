//! Typed release validation is independent of platform gesture/token admission.

use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, EventKind, Plan, TypeKind, Value};
use exact_runner::{agent, DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("event queried {name}")
    }
}

fn boot() -> Runner<NoData> {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.0));
    let height = b.slot("height", number, zero);
    let velocity = b.slot("velocity", number, zero);
    let mut body = Asm::new();
    body.load_param(0)
        .store_slot(height)
        .load_param(1)
        .store_slot(velocity);
    let body = b.code(body);
    let action = b.action(
        "release",
        &[("height", number), ("velocity", number)],
        &[height, velocity],
        body,
    );
    b.node(
        NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[(EventKind::Heightrelease, action, &[])],
        None,
    );
    let plan = b.finish().unwrap();
    assert_eq!(EventKind::Heightrelease as u8, 14);
    assert_eq!(EventKind::Navigate as u8, 13);
    assert_eq!(EventKind::from_wire(18), None);
    let plan = Plan::decode(&plan.encode()).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn release_parser_accepts_exact_finite_pair_and_signed_velocity() {
    for (text, height, velocity) in [
        ("0,-2000", 0.0, -2000.0),
        ("360.5,0", 360.5, 0.0),
        ("1e2,2e3", 100.0, 2000.0),
    ] {
        assert_eq!(
            Event::height_release_payload(text),
            Some(Event::HeightRelease { height, velocity })
        );
    }
    let max = format!("{},{}", f32::MAX as f64, f64::MAX);
    assert!(Event::height_release_payload(&max).is_some());
    for text in [
        "", "1", "1,2,3", ",2", "1,", " 1,2", "1,2 ", "NaN,0", "0,NaN", "inf,0", "0,-inf",
        "-0.01,2", "1e39,0", "[1,2]",
    ] {
        assert_eq!(Event::height_release_payload(text), None, "{text}");
    }
}

#[test]
fn handbuilt_plan_roundtrips_dispatches_and_exports_release() {
    let mut r = boot();
    let root = r.roots()[0];
    r.dispatch(
        root,
        Event::HeightRelease {
            height: 271.5,
            velocity: -500.0,
        },
    )
    .unwrap();
    assert_eq!(r.slot("height"), Some(&Value::Number(271.5)));
    assert_eq!(r.slot("velocity"), Some(&Value::Number(-500.0)));
    assert_eq!(r.handlers_of(root), vec![EventKind::Heightrelease]);
    assert!(agent::tree(&r).contains("\"handlers\":[\"heightrelease\"]"));
}

#[test]
fn malformed_typed_release_cannot_mutate_slots_kernel_or_clock() {
    let mut r = boot();
    r.advance(123.0).unwrap();
    let epoch = r.kernel().epoch();
    let now = r.now_ms();
    for (height, velocity) in [
        (f64::NAN, 0.0),
        (f64::INFINITY, 0.0),
        (-1.0, 0.0),
        (f32::MAX as f64 * 2.0, 0.0),
        (1.0, f64::NEG_INFINITY),
        (1.0, f64::NAN),
    ] {
        assert!(r
            .dispatch(r.roots()[0], Event::HeightRelease { height, velocity })
            .is_err());
        assert_eq!(r.slot("height"), Some(&Value::Number(0.0)));
        assert_eq!(r.slot("velocity"), Some(&Value::Number(0.0)));
        assert_eq!(r.kernel().epoch(), epoch);
        assert_eq!(r.now_ms(), now);
    }
    // Refusal did not poison the runner.
    r.dispatch(
        r.roots()[0],
        Event::HeightRelease {
            height: 0.0,
            velocity: -10.0,
        },
    )
    .unwrap();
}
