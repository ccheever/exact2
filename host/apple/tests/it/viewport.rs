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
