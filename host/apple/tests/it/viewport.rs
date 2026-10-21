use exact_runner::{DataError, Value};

// @ref LLP 1039 §5 — native layout and the resource share one viewport.
#[test]
fn viewport_resize_reanswers_before_native_layout() {
    struct FactsOnly;
    impl exact_runner::DataSource for FactsOnly {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("host fact reached {name}")
        }
    }
    let plan = contract::bake(
        contract::compile(include_str!(
            "../../../../contract/corpus/viewport.contract"
        ))
        .unwrap(),
        FactsOnly,
    )
    .unwrap();
    let (mut host, _) = exact_apple::Host::boot(
        &plan.encode(),
        FactsOnly,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        1280.0,
        900.0,
    )
    .unwrap();
    assert_eq!(host.runner().kernel().epoch(), 1);
    assert!(!host.runner().kernel().find_by_test_id("wide").is_empty());
    host.resize(390.0, 844.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
    assert!(!host.runner().kernel().find_by_test_id("narrow").is_empty());
    assert_eq!(
        host.runner().viewport(),
        exact_runner::Viewport::sized(390.0, 844.0)
    );
    host.resize(390.0, 844.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
    host.resize(f32::NAN, 0.0);
    assert_eq!(host.runner().viewport().width, 390.0);
    host.resize(1280.0, 900.0);
    assert_eq!(host.runner().kernel().epoch(), 3);
}

#[test]
fn percentage_root_and_nested_pane_follow_keyboard_height_restoration() {
    struct FactsOnly;
    impl exact_runner::DataSource for FactsOnly {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("host fact reached {name}")
        }
    }
    let mut failures = Vec::new();
    for reads_viewport in [false, true] {
        let fact = if reads_viewport {
            "  resource viewport = exactViewport() as shape Viewport\n"
        } else {
            ""
        };
        let pane_width = if reads_viewport {
            "viewport.width"
        } else {
            "\"100%\""
        };
        let source = format!(
            "shape Viewport\n  width: number\n  height: number\n\ncomponent App\n{fact}  view\n    main testId=\"root\" width=\"100%\" height=\"100%\" display=\"flex\" flex-direction=\"row\" interactive-widget=\"resizes-content\"\n      column testId=\"pane\" width={pane_width} height=\"100%\" flex-shrink=0\n        view testId=\"dock\" position=\"absolute\" left=0 right=0 bottom=0 height=64\n"
        );
        let plan = contract::compile(&source).unwrap().encode();
        let (mut host, _) = exact_apple::Host::boot(
            &plan,
            FactsOnly,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            1024.,
            1366.,
        )
        .unwrap();
        for height in [999., 1366., 973., 1366.] {
            let wire = host.resize(1024., height);
            let batch: serde_json::Value = serde_json::from_str(&wire).unwrap();
            if !batch["error"].is_null() {
                failures.push(format!("reader={reads_viewport} height={height}: {wire}"));
            }
            for (id, y, h) in [
                ("root", 0., height),
                ("pane", 0., height),
                ("dock", height - 64., 64.),
            ] {
                let k = host.runner().kernel();
                let node = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
                if (node.frame.y, node.frame.height) != (y, h) {
                    failures.push(format!(
                        "reader={reads_viewport} height={height} {id}: {:?}",
                        node.frame
                    ));
                }
                if id != "dock" {
                    let emitted = batch["ops"].as_array().unwrap().iter().rev().find(|op| {
                        op["op"] == "frame" && op["id"].as_u64() == Some(u64::from(node.id))
                    });
                    if emitted.is_none_or(|op| op["h"].as_f64() != Some(f64::from(height))) {
                        failures.push(format!("reader={reads_viewport} height={height} {id}: missing restored frame in {wire}"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// @ref LLP 1061 D4 — the preferences are told after boot and on change: one
// commit that carries the app's own choice (its entry animation dropped).
#[test]
fn preferences_reanswer_the_viewport_and_the_app_drops_its_motion() {
    struct FactsOnly;
    impl exact_runner::DataSource for FactsOnly {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("host fact reached {name}")
        }
    }
    let plan = contract::bake(
        contract::compile(include_str!(
            "../../../../contract/corpus/motion-feel.contract"
        ))
        .unwrap(),
        FactsOnly,
    )
    .unwrap();
    let (mut host, _) = exact_apple::Host::boot(
        &plan.encode(),
        FactsOnly,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let batch = host.set_preferences(exact_runner::Preferences::from_bits(1));
    assert_eq!(host.runner().kernel().epoch(), 2);
    assert!(host.runner().viewport().preferences.reduced_motion);
    assert!(batch.contains("\"ops\""), "{batch}");
    let k = host.runner().kernel();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap();
    assert!(root.style.animation.0.is_empty());
    host.set_preferences(exact_runner::Preferences::from_bits(1));
    assert_eq!(host.runner().kernel().epoch(), 2);
}
