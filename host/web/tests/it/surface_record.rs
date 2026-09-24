use exact_runner::{DataError, DataSource, Value};
use exact_web::abi::Bridge;
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("runner fact reached {source}")
    }
}
#[test]
fn surface_record_abi_distinguishes_an_invalid_empty_record_from_disposal() {
    let plan = contract::compile("shape Hud\n  beacons: number\ncomponent App\n  resource hud = exactSurface(\"world\") as shape Hud\n  view\n    text `${hud.beacons}`\n").unwrap();
    let mut bridge = Bridge::new();
    exact_web::link(exact_web_capabilities::ALL);
    bridge.boot(&plan.encode(), NoData, 390., 844., "/");
    let mut publish = |text: &[u8]| {
        let n = bridge.input_write(text);
        let n = bridge.surface_record(n);
        String::from_utf8_lossy(bridge.output_bytes(n as usize)).into_owned()
    };
    assert!(publish(b"world\0{\"beacons\":2}").contains("\"error\":null"));
    assert!(publish(b"world\0{\"beacons\":9,\"extra\":\"\xff\"}").contains("UTF-8"));
    assert!(publish(b"wor\xffld").contains("UTF-8"));
    assert!(publish(b"world\0{\"beacons\":2}").contains("\"ops\":[]"));
    let refused = publish(b"world\0");
    assert!(
        refused.contains("hud") && refused.contains("expected a value"),
        "{refused}"
    );
    assert!(publish(b"world").contains("\"error\":null"));
    let n = bridge.input_write(br#"{"op":"state"}"#);
    let n = bridge.agent(n);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize))
        .contains("\"hud\":{\"beacons\":0}"));
}

mod exported {
    use super::NoData;
    const PLAN: &[u8] = &[];
    const COMPAT: &str = "{}";
    const EXACT_LINKED: exact_web::Linked = exact_web::Linked::CORE;
    const EXACT_REPLACEMENT: bool = false;
    fn app_data() -> NoData {
        NoData
    }
    exact_web::host!(NoData, PLAN, COMPAT, app_data);
    exact_web::surface_exports!();

    #[test]
    fn nested_surface_export_is_refused_without_a_refcell_panic() {
        EXACT_BRIDGE.with(|cell| {
            let _busy = cell.borrow_mut();
            assert_eq!(exact_surface_record(0), 0);
        });
    }
}
