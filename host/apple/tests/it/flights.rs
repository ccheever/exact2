//! Shared-element flights on Apple (LLP 1013.000 D4): a handoff sends
//! `flight` before the leaver's destroy, then the curve's progress as
//! `present … "flight"`, then `land` once it settles.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> Option<u32> {
    let k = host.runner().kernel();
    let key = *k.find_by_test_id(test_id).first()?;
    Some(k.node_by_key(key).unwrap().id)
}

fn progress(batch: &str, id: u32) -> Option<f64> {
    let marker = format!("\"op\":\"present\",\"id\":{id},\"property\":\"flight\",\"x\":");
    let at = batch.rfind(&marker)?;
    batch[at + marker.len()..]
        .split([',', '}'])
        .next()?
        .parse()
        .ok()
}

fn boot() -> Host<NoData> {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/shared-elements.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert!(!first.contains("\"op\":\"flight\""), "{first}");
    host
}

#[test]
fn a_handoff_flies_on_the_leavers_curve_and_lands() {
    let mut host = boot();
    let (thumb, toggle) = (
        view(&host, "thumb").unwrap(),
        view(&host, "toggle").unwrap(),
    );
    let open = host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    let flight = format!("{{\"op\":\"flight\",\"id\":{large},\"from\":{thumb}}}");
    let destroy = format!("{{\"op\":\"destroy\",\"id\":{thumb}}}");
    let (at, gone) = (open.find(&flight), open.find(&destroy));
    assert!(at.is_some(), "{open}");
    assert!(
        gone.is_none_or(|g| at.unwrap() < g),
        "captured before the destroy: {open}"
    );
    assert!(open.contains("\"motion\":true"), "{open}");
    let mid = host.tick(250.0);
    let p = progress(&mid, large).expect("progress mid-flight");
    assert!(p > 0.3 && p < 0.9, "ease at half time: {p}");
    let done = host.tick(401.0);
    assert!(
        done.contains(&format!("{{\"op\":\"land\",\"id\":{large}}}")),
        "{done}"
    );
    let after = host.tick(500.0);
    assert!(!after.contains("\"op\":\"land\""), "lands once: {after}");
}

#[test]
fn closing_mid_flight_ends_the_first_flight_and_starts_the_reverse() {
    let mut host = boot();
    let toggle = view(&host, "toggle").unwrap();
    host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    host.tick(200.0);
    let back = host.dispatch_at(toggle, Event::Press, 220.0);
    let thumb = view(&host, "thumb").unwrap();
    assert!(
        back.contains(&format!(
            "{{\"op\":\"flight\",\"id\":{thumb},\"from\":{large}}}"
        )),
        "{back}"
    );
    assert!(
        !back.contains(&format!("{{\"op\":\"land\",\"id\":{large}}}")),
        "a destroyed arriver doesn't land: {back}"
    );
    let done = host.tick(521.0);
    assert!(
        done.contains(&format!("{{\"op\":\"land\",\"id\":{thumb}}}")),
        "{done}"
    );
}
