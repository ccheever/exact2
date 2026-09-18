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
        contract::compile(include_str!("../../../contract/corpus/viewport.contract")).unwrap(),
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
        exact_runner::Viewport {
            width: 390.0,
            height: 844.0
        }
    );
    host.resize(390.0, 844.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
    host.resize(f32::NAN, 0.0);
    assert_eq!(host.runner().viewport().width, 390.0);
    host.resize(1280.0, 900.0);
    assert_eq!(host.runner().kernel().epoch(), 3);
}
