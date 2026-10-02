//! @ref LLP 1039 §5 — bake, first frame, resize, carry and invalid facts.
use exact_kernel::{Kernel, PropId};
use exact_runner::{agent, DataError, DataSource, Runner, RunnerError, Value, Viewport};

struct NoData;

#[test]
fn window_toolbar_intention_survives_compile_bake_and_boot() {
    let plan = contract::compile(
        "component App\n  view\n    row role=\"toolbar\" toolbarPlacement=\"window\" testId=\"commands\"\n      text \"Home\" role=\"heading\" aria-level=1\n",
    )
    .unwrap();
    let runner = Runner::boot(
        contract::bake(plan, NoData).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = runner.kernel().find_by_test_id("commands")[0];
    let props = &runner.kernel().node_by_key(key).unwrap().props;
    assert_eq!(props.str(PropId::ToolbarPlacement), Some("window"));
    assert_eq!(props.str(PropId::AccessibilityRole), Some("toolbar"));
}

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
        contract::compile(include_str!("../../../corpus/viewport.contract")).unwrap(),
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
        Viewport::sized(1280.0, 900.0),
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
        Viewport::sized(1280.0, 900.0),
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

/// LLP 1076 D2: the fold's three fields ride `exactViewport` — the bake
/// answers `continuous`, 1, 1; a host's `set_fold` re-answers every reader
/// in one commit; an unknown or mistyped field is refused at the bake.
#[test]
fn fold_fields_are_viewport_fields_the_bake_answers_flat() {
    let src = r#"shape Fold
  devicePosture: string
  horizontalViewportSegments: number
  verticalViewportSegments: number
component App
  resource m = exactViewport() as shape Fold
  derive two = m.horizontalViewportSegments == 2
  view
    column
      text `${m.devicePosture} ${m.horizontalViewportSegments}x${m.verticalViewportSegments}` testId="fold"
      when two
        text "two panes" testId="panes"
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
    let text = |r: &Runner<NoData>, id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(exact_kernel::PropId::Text)
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(text(&r, "fold"), "continuous 1x1");
    assert!(r.kernel().find_by_test_id("panes").is_empty());
    let state: serde_json::Value = serde_json::from_str(&agent::state(&r)).unwrap();
    assert_eq!(state["device"]["devicePosture"], "continuous");
    assert_eq!(state["device"]["horizontalViewportSegments"], 1);
    let folded = exact_runner::Fold {
        posture: exact_runner::Posture::Folded,
        cols: 2,
        rows: 1,
    };
    let receipt = r.set_fold(folded).unwrap().unwrap();
    assert_eq!(receipt.epoch, 2, "one commit");
    assert_eq!(text(&r, "fold"), "folded 2x1");
    assert_eq!(r.kernel().find_by_test_id("panes").len(), 1);
    assert!(r.set_fold(folded).unwrap().is_none(), "the same fold again");
    assert!(matches!(
        r.set_fold(exact_runner::Fold { cols: 0, ..folded }),
        Err(RunnerError::InvalidViewport)
    ));
    assert_eq!(r.viewport().fold, folded, "nothing changed");
    // A resize re-answers the same resource (one source, LLP 1039 D2) and
    // keeps the fold.
    assert!(r.set_viewport(900.0, 600.0).unwrap().is_some());
    assert_eq!(r.viewport().fold, folded, "a resize keeps the fold");
    assert_eq!(text(&r, "fold"), "folded 2x1");
    let state: serde_json::Value = serde_json::from_str(&agent::state(&r)).unwrap();
    assert_eq!(state["device"]["devicePosture"], "folded");
    assert_eq!(state["device"]["horizontalViewportSegments"], 2);
    for wrong in [
        "shape F\n  devicePosture: number\ncomponent A\n  resource m = exactViewport() as shape F\n  view\n    text `${m.devicePosture}`\n",
        "shape F\n  hingeAngle: number\ncomponent A\n  resource m = exactViewport() as shape F\n  view\n    text `${m.hingeAngle}`\n",
    ] {
        assert!(contract::bake(contract::compile(wrong).unwrap(), NoData).is_err(), "{wrong}");
    }
}
