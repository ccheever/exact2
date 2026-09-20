use exact_runner::{Answer, DataSource, Store, Value};
use tally_data::TallySource;
fn answer(d: &mut TallySource, s: &mut Store, name: &str, args: &[Value]) -> Value {
    match d.answer(s, name, args).unwrap() {
        Answer::Now(v) => v,
        _ => panic!("synchronous"),
    }
}
#[test]
fn saved_pending_presses_and_failed_tick_recovery() {
    let mut a = TallySource::default();
    let mut store = Store::new(a.grants(), []);
    for action in ["hold", "draw", "draw"] {
        answer(
            &mut a,
            &mut store,
            "press",
            &[Value::Number(0.), Value::str(action)],
        );
    }
    answer(&mut a, &mut store, "save", &[Value::Number(0.)]);
    let mut b = TallySource::default();
    let mut restored = Store::new(b.grants(), store.snapshot());
    let expected = answer(&mut a, &mut store, "world", &[Value::Number(100.)]);
    answer(&mut b, &mut restored, "world", &[Value::Number(0.)]);
    assert_eq!(
        expected,
        answer(&mut b, &mut restored, "world", &[Value::Number(100.)])
    );
    assert_eq!(a.0.checkpoint().unwrap(), b.0.checkpoint().unwrap());
    answer(
        &mut b,
        &mut restored,
        "press",
        &[Value::Number(100.), Value::str("fail")],
    );
    answer(&mut b, &mut restored, "world", &[Value::Number(200.)]);
    assert!(b.0.checkpoint().is_err());
    answer(&mut b, &mut restored, "restart", &[Value::Number(200.)]);
    assert!(b.0.checkpoint().is_ok());
}
#[test]
fn absolute_clock_is_independent_of_query_cadence() {
    let mut a = TallySource::default();
    let mut b = TallySource::default();
    for t in (0..=1000).step_by(10) {
        a.query("world", &[Value::Number(t as f64)]).unwrap();
    }
    for t in (0..=1000).step_by(100) {
        b.query("world", &[Value::Number(t as f64)]).unwrap();
    }
    assert_eq!(a.0.sim.world().tick(), 61);
    assert_eq!(a.0.checkpoint().unwrap(), b.0.checkpoint().unwrap());
}
