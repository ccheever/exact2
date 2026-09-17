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
    bridge.boot(&plan.encode(), NoData, 390., 844., "/");
    let mut publish = |text: &[u8]| {
        let n = bridge.input_write(text);
        let n = bridge.surface_record(n);
        String::from_utf8_lossy(bridge.output_bytes(n as usize)).into_owned()
    };
    assert!(publish(b"world\0{\"beacons\":2}").contains("\"error\":null"));
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
