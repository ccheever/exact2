//! The executor's fixture (LLP 1027 §9 stage 1): Caltrain's data crate and
//! its TypeScript twin (`fixtures/caltrain.ts`, compiled by `build.rs` with
//! Rolldown and hermesc) answer the same bytes on every source and every
//! error path — the byte-equality test LLP 1026 D3 named as the ABI's
//! fixture, now this seam's.

#![cfg(exact_js_engine)]

use caltrain_data::{Caltrain, DAY_START_MS, DEFAULT_LOCATION};
use exact_js::{abi_supported, Module, ABI};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource};
use std::path::Path;

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/caltrain.hbc"));

fn plan() -> Plan {
    contract::compile_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../apps/caltrain/app.contract"
    )))
    .expect("app.contract compiles")
}

fn module() -> Module {
    let mut m = Module::loaded(HBC.to_vec(), "com.exact.caltrain", "").expect("the twin loads");
    // Functional fixtures carry no wall-clock budget; it is not a stable
    // gate on a shared test machine. Budget tests set their own.
    m.set_budget_ms(f64::INFINITY);
    m.bind(&plan());
    m
}

fn loc(lat: f64, lon: f64) -> Value {
    Value::record(vec![Value::Number(lat), Value::Number(lon)])
}

fn cases() -> Vec<(&'static str, Vec<Value>)> {
    let (lat, lon) = DEFAULT_LOCATION;
    let noon = DAY_START_MS + 12.0 * 3_600_000.0;
    let s = Value::str;
    let n = Value::Number;
    vec![
        ("defaultLocation", vec![]),
        ("stations", vec![loc(lat, lon)]),
        ("nearest", vec![loc(lat, lon), n(3.0)]),
        ("nearest", vec![loc(37.7, -122.4), n(9.0)]),
        ("station", vec![s("paloalto"), loc(lat, lon)]),
        ("board", vec![s("mv"), s("north"), n(noon)]),
        (
            "board",
            vec![s("mv"), s("south"), n(DAY_START_MS + 7.0 * 3_600_000.0)],
        ),
        ("board", vec![s("sf"), s("north"), n(noon)]),
        ("board", vec![s("sj"), s("north"), n(DAY_START_MS)]),
        ("search", vec![s("Palo"), loc(lat, lon)]),
        ("search", vec![s("san"), loc(lat, lon)]),
        ("search", vec![s(""), loc(lat, lon)]),
        // The error paths, message for message.
        ("nearest", vec![loc(lat, lon), n(10.0)]),
        ("nearest", vec![loc(lat, lon), n(1.5)]),
        ("stations", vec![]),
        ("stations", vec![loc(91.0, 0.0)]),
        ("station", vec![s("nowhere"), loc(lat, lon)]),
        ("board", vec![s("mv"), s("east"), n(noon)]),
        ("board", vec![n(7.0), s("north"), n(noon)]),
        ("bogus", vec![]),
    ]
}

#[test]
fn the_twin_answers_the_crates_bytes_on_every_case() {
    let mut twin = module();
    let mut native = Caltrain;
    for (source, args) in cases() {
        let ts = twin.query(source, &args);
        let rs = native.query(source, &args);
        match (&ts, &rs) {
            (Ok(a), Ok(b)) => assert_eq!(a.to_bytes(), b.to_bytes(), "{source} {args:?}"),
            (Err(a), Err(b)) => assert_eq!(a, b, "{source} {args:?}"),
            _ => panic!("{source} {args:?}: ts={ts:?} rs={rs:?}"),
        }
    }
}

#[test]
fn identity_and_grants_are_the_bakes_and_the_modules_agree() {
    let m = module();
    assert_eq!(m.app_id(), "com.exact.caltrain");
    assert_eq!(m.grants(), "");
    // The fixture exports ABI 1 (it draws nothing); the executor speaks
    // ABI 2 (LLP 1056 D1) and runs both.
    assert!(abi_supported("1") && abi_supported(&ABI.to_string()));
    let mut names = m.sources();
    names.sort_unstable();
    // `exactDelivery` (LLP 1030 D7) and `exactViewport` (the TV fit,
    // 1e6cb3bb0) are the plan's too: the runner answers them before the
    // data seam, so the module is never asked for them and need not
    // export them.
    assert_eq!(
        names,
        [
            "board",
            "defaultLocation",
            "exactDelivery",
            "exactViewport",
            "nearest",
            "search",
            "station",
            "stations"
        ]
    );
    // The bake's word and the module's must agree, by name.
    let wrong = Module::loaded(HBC.to_vec(), "com.exact.other", "");
    assert!(wrong.is_err(), "a module claiming another app is refused");
    let wrong = Module::loaded(HBC.to_vec(), "com.exact.caltrain", "net.fetch https://x\n");
    assert!(
        wrong.is_err(),
        "grants the module did not declare are refused"
    );
}

#[test]
fn unloaded_is_unavailable_by_name_and_load_is_after_the_host_says_so() {
    let mut m = Module::new(HBC.to_vec(), "com.exact.caltrain", "");
    m.bind(&plan());
    assert!(!m.is_loaded());
    assert!(matches!(
        m.query("defaultLocation", &[]),
        Err(DataError::Unavailable(ref e)) if e.contains("not loaded")
    ));
    m.load().unwrap();
    assert!(m.query("defaultLocation", &[]).is_ok());
    m.unload();
    assert!(!m.is_loaded());
    assert!(matches!(
        m.query("defaultLocation", &[]),
        Err(DataError::Unavailable(_))
    ));
}

#[test]
fn the_executor_refuses_what_the_plan_does_not_declare_and_what_does_not_fit() {
    let mut m = module();
    // Unbound: no plan, no signatures — every source is unknown.
    let mut unbound = Module::loaded(HBC.to_vec(), "com.exact.caltrain", "").unwrap();
    assert!(matches!(
        unbound.query("stations", &[loc(0.0, 0.0)]),
        Err(DataError::UnknownSource(ref s)) if s == "stations"
    ));
    // An argument outside its declared shape never reaches the module.
    assert!(matches!(
        m.query("station", &[Value::str("mv"), Value::str("not a location")]),
        Err(DataError::BadArguments(ref e)) if e == "argument 1"
    ));
    // Arity is the plan's to check, with the crate's exact words.
    assert!(matches!(
        m.query("station", &[Value::str("mv")]),
        Err(DataError::BadArguments(ref e)) if e == "expected 2 arguments, got 1"
    ));
}

#[test]
fn console_lines_are_collected_for_the_runners_logs() {
    let mut m = module();
    assert!(m.take_logs().is_empty());
    m.query("defaultLocation", &[]).unwrap();
    let logs = m.take_logs();
    assert_eq!(
        logs,
        ["defaultLocation {\"lat\":37.3947,\"lon\":-122.0763}"]
    );
    assert!(m.take_logs().is_empty(), "taken once");
}

#[test]
fn a_call_over_budget_is_unavailable_and_counted() {
    let mut m = module();
    m.set_budget_ms(0.0);
    let (lat, lon) = DEFAULT_LOCATION;
    assert!(matches!(
        m.query("stations", &[loc(lat, lon)]),
        Err(DataError::Unavailable(ref e)) if e.contains("over the 0 ms budget")
    ));
    assert_eq!(m.overruns(), 1);
}
