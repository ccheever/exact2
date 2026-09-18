//! Typed tuples are validated independently of host binding/geometry/token admission.
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, EventKind, Plan, TypeKind, Value};
use exact_runner::{agent, DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
}
fn release(v: [f64; 6]) -> Event {
    Event::TransformRelease {
        x: v[0],
        y: v[1],
        scale: v[2],
        vx: v[3],
        vy: v[4],
        vscale: v[5],
    }
}
fn geometry(v: [f64; 4]) -> Event {
    Event::TransformGeometry {
        box_width: v[0],
        box_height: v[1],
        port_width: v[2],
        port_height: v[3],
    }
}
fn boot() -> Runner<NoData> {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.0));
    let names = ["a", "b", "c", "d", "e", "f"];
    let slots: Vec<_> = names.iter().map(|s| b.slot(s, number, zero)).collect();
    let mut handlers = Vec::new();
    for (event, n) in [
        (EventKind::Transformgeometry, 4),
        (EventKind::Transformrelease, 6),
    ] {
        let mut body = Asm::new();
        for (i, slot) in slots[..n].iter().enumerate() {
            body.load_param(i as u16).store_slot(*slot);
        }
        let body = b.code(body);
        let params: Vec<_> = names[..n].iter().map(|name| (*name, number)).collect();
        let action = b.action(event.name(), &params, &slots[..n], body);
        handlers.push((event, action, &[][..]));
    }
    b.node(NodeType::View as u8, None, None, 0, &[], &handlers, None);
    let plan = Plan::decode(&b.finish().unwrap().encode()).unwrap();
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
fn exact_tuples_allow_signed_release_zero_geometry_and_positive_subnormal_scale() {
    assert_eq!(
        Event::transform_geometry_payload("800,400,800,400"),
        Some(geometry([800.0, 400.0, 800.0, 400.0]))
    );
    assert_eq!(
        Event::transform_geometry_payload("0,0,0,0"),
        Some(geometry([0.0; 4]))
    );
    assert_eq!(
        Event::transform_release_payload("-3,4,1.3,-200,300,-4"),
        Some(release([-3.0, 4.0, 1.3, -200.0, 300.0, -4.0]))
    );
    let max = f32::MAX as f64;
    let tiny = f32::from_bits(1) as f64;
    assert!(Event::transform_release_payload(&format!(
        "{max},-{max},{tiny},{},{},0",
        f64::MAX,
        -f64::MAX
    ))
    .is_some());
    for invalid in [
        "",
        "1,2,3",
        "1,2,3,4,5",
        " 1,2,3,4",
        "1,2,3,4 ",
        "1,,3,4",
        "NaN,2,3,4",
        "1,inf,3,4",
        "1,-2,3,4",
        "1e39,2,3,4",
    ] {
        assert_eq!(
            Event::transform_geometry_payload(invalid),
            None,
            "{invalid}"
        );
    }
    for invalid in [
        "",
        "1,2,3,4,5",
        "1,2,3,4,5,6,7",
        "1,2,0,4,5,6",
        "1,2,-1,4,5,6",
        "1,2,1e-100,4,5,6",
        "1,2,NaN,4,5,6",
        "1e39,2,1,4,5,6",
        "1,2,1,inf,5,6",
        "1,2,1,4,NaN,6",
        "1,2,1,4,5,-inf",
        "1,2,1,4,5,6 ",
    ] {
        assert_eq!(Event::transform_release_payload(invalid), None, "{invalid}");
    }
}

#[test]
fn handbuilt_plan_roundtrips_exports_and_runs_both_synchronous_actions() {
    let mut r = boot();
    let view = r.roots()[0];
    assert_eq!(EventKind::Transformgeometry as u8, 15);
    assert_eq!(EventKind::Transformrelease as u8, 16);
    assert_eq!(EventKind::from_wire(18), None);
    r.dispatch(view, geometry([800.0, 400.0, 600.0, 300.0]))
        .unwrap();
    assert_eq!(r.slot("a"), Some(&Value::Number(800.0)));
    assert_eq!(r.slot("d"), Some(&Value::Number(300.0)));
    r.dispatch(view, release([-12.0, 17.0, 1.5, -300.0, 600.0, 0.0]))
        .unwrap();
    for (name, v) in ["a", "b", "c", "d", "e", "f"]
        .into_iter()
        .zip([-12.0, 17.0, 1.5, -300.0, 600.0, 0.0])
    {
        assert_eq!(r.slot(name), Some(&Value::Number(v)));
    }
    let tree = agent::tree(&r);
    assert!(tree.contains("transformgeometry"));
    assert!(tree.contains("transformrelease"));
}

#[test]
fn malformed_typed_fields_preserve_every_slot_epoch_and_clock_without_poisoning() {
    let mut r = boot();
    r.advance(123.0).unwrap();
    let view = r.roots()[0];
    let epoch = r.kernel().epoch();
    let mut events = Vec::new();
    for i in 0..4 {
        for bad in [-1.0, f64::NAN, f64::INFINITY, 1e39] {
            let mut v = [1.0; 4];
            v[i] = bad;
            events.push(geometry(v));
        }
    }
    for i in 0..6 {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut v = [1.0; 6];
            v[i] = bad;
            events.push(release(v));
        }
    }
    for (i, bad) in [
        (0, 1e39),
        (1, -1e39),
        (2, 0.0),
        (2, -1.0),
        (2, 1e-100),
        (2, 1e39),
    ] {
        let mut v = [1.0; 6];
        v[i] = bad;
        events.push(release(v));
    }
    for e in events {
        assert!(r.dispatch(view, e).is_err());
        assert_eq!(r.kernel().epoch(), epoch);
        assert_eq!(r.now_ms(), 123.0);
        for name in ["a", "b", "c", "d", "e", "f"] {
            assert_eq!(r.slot(name), Some(&Value::Number(0.0)));
        }
    }
    r.dispatch(view, release([0.0, 0.0, 1.0, 0.0, 0.0, 0.0]))
        .unwrap();
}
