//! @ref LLP 1039 §5 — bake, first frame, resize, carry and invalid facts.
use exact_kernel::{Kernel, PropId};
use exact_runner::{agent, DataError, DataSource, Runner, RunnerError, Value, Viewport};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("host fact reached data: {source}")
    }
    fn ready(&self) -> bool {
        false
    }
}
fn plan() -> exact_plan::Plan {
    contract::bake(
        contract::compile(include_str!("../../corpus/viewport.contract")).unwrap(),
        NoData,
    )
    .unwrap()
}
fn present(r: &Runner<NoData>, id: &str) -> bool {
    !r.kernel().find_by_test_id(id).is_empty()
}
fn state(r: &Runner<NoData>) -> serde_json::Value {
    serde_json::from_str(&agent::state(r)).unwrap()
}

#[test]
fn baked_phone_first_wide_batch_then_one_narrow_commit() {
    let baked = plan();
    let initial = Value::from_bytes(baked.bytes(baked.resources[0].initial)).unwrap();
    assert_eq!(
        initial,
        Value::record(vec![Value::Number(844.0), Value::Number(390.0)])
    );
    assert!(baked.resources[0].reader);
    assert!(!contract::typescript(&baked)
        .unwrap()
        .contains("exactViewport"));
    let mut r = Runner::boot(
        baked,
        NoData,
        Kernel::with_monospace(),
        Viewport {
            width: 1280.0,
            height: 900.0,
        },
        "/",
    )
    .unwrap();
    assert_eq!(r.kernel().epoch(), 1);
    assert!(present(&r, "wide") && !present(&r, "narrow"));
    assert_eq!(
        state(&r)["resources"]["viewport"],
        serde_json::json!({"width":1280,"height":900})
    );
    let rail = r.kernel().find_by_test_id("rail")[0];
    assert_eq!(
        r.kernel()
            .node_by_key(rail)
            .unwrap()
            .props
            .str(PropId::AccessibilityOrientation),
        Some("vertical")
    );
    let receipt = r.set_viewport(390.0, 844.0).unwrap().unwrap();
    assert_eq!(receipt.epoch, 2);
    assert!(present(&r, "narrow") && !present(&r, "wide"));
    assert!(r.set_viewport(390.0, 844.0).unwrap().is_none());
    assert_eq!(
        state(&r)["resources"]["viewport"],
        serde_json::json!({"width":390,"height":844})
    );
    println!("bake 390x844; boot 1280x900 wide at epoch 1; resize 390x844 narrow at epoch {}; identical resize None", receipt.epoch);
    let carried = r.carry();
    let r = Runner::boot_carrying(
        plan(),
        NoData,
        Kernel::with_monospace(),
        &carried,
        Viewport {
            width: 1280.0,
            height: 900.0,
        },
        "/",
    )
    .unwrap();
    assert!(present(&r, "wide"));
}

#[test]
fn invalid_sizes_refuse_without_poisoning_and_unread_sizes_do_not_commit() {
    let mut r = Runner::boot(
        plan(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for (width, height) in [
        (0.0, 844.0),
        (-1.0, 844.0),
        (390.0, 0.0),
        (f64::NAN, 844.0),
        (390.0, f64::INFINITY),
    ] {
        assert!(matches!(
            r.set_viewport(width, height),
            Err(RunnerError::InvalidViewport)
        ));
        assert_eq!(r.viewport(), Viewport::default());
        assert!(present(&r, "narrow"));
    }
    assert!(agent::logs(&r, 0).contains("InvalidViewport"));
    assert_eq!(r.set_viewport(1280.0, 900.0).unwrap().unwrap().epoch, 2);
    let plan = contract::compile("component A\n  view\n    text \"empty\"\n").unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(r.set_viewport(1280.0, 900.0).unwrap().is_none());
    assert_eq!(r.viewport().width, 1280.0);
}

#[test]
fn fields_and_arguments_refuse_at_the_fact_boundary() {
    for source in ["shape V\n  width: string\ncomponent A\n  resource viewport = exactViewport() as shape V\n  view\n    text viewport.width\n", "shape V\n  width: number\ncomponent A\n  resource viewport = exactViewport(1) as shape V\n  view\n    text `${viewport.width}`\n"] {
        let p = contract::compile(source).unwrap();
        assert!(contract::bake(p,NoData).is_err());
    }
}

#[test]
fn every_declared_viewport_is_reanswered_in_one_commit_by_field_name() {
    let src = r#"shape Width
  width: number
component App
  resource a = exactViewport() as shape Width
  resource b = exactViewport() as shape Width
  view
    column
      text `${a.width}` testId="width-a"
      text `${b.width}` testId="width-b"
"#;
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let receipt = r.set_viewport(1280.0, 900.0).unwrap().unwrap();
    assert_eq!(receipt.epoch, 2);
    assert_eq!(state(&r)["resources"]["a"]["width"], 1280);
    assert_eq!(state(&r)["resources"]["b"]["width"], 1280);
    assert!(agent::logs(&r, 0).contains("viewport (2 asked again)"));
}
