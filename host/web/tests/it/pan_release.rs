//! `panrelease` through the web's bridge (LLP 1057 §10.6 phase 2): the page's
//! pointer samples go to motion ops 13 and 14, which answer the engine
//! tracker's velocity; ABI kind 28 delivers it to the action.
use exact_runner::{DataError, DataSource, Value};
use exact_web::abi::Bridge;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const SOURCE: &str = r#"component App
  state x = 0
  state vx = 0
  state vy = 0
  action move(dx: number, dy: number)
    x = x + dx
  action release(sx: number, sy: number)
    vx = sx
    vy = sy
  view
    box testId="card" pan=move panrelease=release width=100 height=100 touch-action="none"
"#;

fn motion(bridge: &mut Bridge<NoData>, op: u32, view: u32, token: u64, xy: [f64; 3]) -> String {
    let mut bytes = Vec::new();
    for word in [1u32, op, view, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(token.to_le_bytes());
    for value in xy {
        bytes.extend(value.to_le_bytes());
    }
    bridge.input_write(&bytes);
    let len = bridge.motion(bytes.len());
    String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap()
}

fn dispatch(bridge: &mut Bridge<NoData>, view: u32, kind: u32, payload: &str) -> String {
    bridge.input_write(payload.as_bytes());
    let len = bridge.dispatch(view, kind, payload.len(), 100.0);
    String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap()
}

#[test]
fn the_pages_samples_release_at_the_trackers_velocity_through_kind_28() {
    exact_web::link(exact_web_capabilities::ALL);
    let plan = contract::compile(SOURCE).unwrap().encode();
    let mut bridge = Bridge::new();
    bridge.boot(&plan, NoData, 400.0, 800.0, "/");
    let card = {
        let host = exact_web::Host::boot(&plan, NoData, Default::default(), "/")
            .unwrap()
            .0;
        let k = host.runner().kernel();
        k.node_by_key(k.find_by_test_id("card")[0]).unwrap().id
    };
    // Six samples 16 ms apart, 20 px each: 1250 px/s to the right.
    for i in 0..6 {
        let t = 1000.0 + 16.0 * i as f64;
        let reply = motion(
            &mut bridge,
            13,
            card,
            u64::from(i == 0),
            [20.0 * i as f64, 0.0, t],
        );
        assert!(reply.contains("\"accepted\":true"), "{reply}");
    }
    let reply = motion(&mut bridge, 14, card, 0, [0.0, 0.0, 1080.0]);
    let vx: f64 = reply
        .split("\"vx\":")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .unwrap()
        .parse()
        .unwrap();
    assert!((vx - 1250.0).abs() < 1.0, "{reply}");
    assert!(motion(&mut bridge, 14, card, 0, [0.0, 0.0, 1080.0]).contains("\"vx\":0,"));

    let batch = dispatch(&mut bridge, card, 20, "120,0");
    assert!(batch.contains("\"error\":null"), "{batch}");
    let batch = dispatch(&mut bridge, card, 28, &format!("{vx},0"));
    assert!(batch.contains("\"error\":null"), "{batch}");
    let refused = dispatch(&mut bridge, card, 28, "NaN,0");
    assert!(
        refused.contains("invalid pan release velocity"),
        "{refused}"
    );
}
