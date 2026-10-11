//! `state … persist` (LLP 1116 D5): a root state the store keeps across
//! launches — restored before the first frame where it still fits its type,
//! written after every commit that stands.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Runner, StoreWrite};

const SRC: &str = r#"
component App
  state tipPercent = 18 persist
  state units = "metric" persist
  state seen = none persist
  state recent = [1, 2] persist
  state zero = 0
  action tip(n: number)
    tipPercent = n
  action convert
    units = units == "metric" ? "imperial" : "metric"
    seen = some(units)
  action add
    recent = concat(recent, [-1.5])
  action refuse
    tipPercent = 99
    zero = 1 / zero
  view
    column
      text "{tipPercent}% {units}" testId="t"
"#;

fn boot(snapshot: Vec<(&str, &str)>) -> Runner<()> {
    let plan = contract::compile(SRC).unwrap();
    let snapshot = snapshot
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Runner::boot_stored(
        plan,
        (),
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap()
}

fn kept(writes: Vec<StoreWrite>) -> Vec<(String, String)> {
    writes
        .into_iter()
        .map(|w| {
            (
                w.name,
                w.value.expect("a persisted state is never forgotten"),
            )
        })
        .collect()
}

fn pair(name: &str, value: &str) -> (String, String) {
    (format!("exact.state.{name}"), value.to_string())
}

#[test]
fn a_fresh_launch_keeps_the_declared_values() {
    let mut r = boot(vec![]);
    assert_eq!(r.slot("tipPercent"), Some(&Value::Number(18.0)));
    assert_eq!(
        kept(r.take_store_writes()),
        vec![
            pair("tipPercent", "18"),
            pair("units", "\"metric\""),
            pair("seen", "null"),
            pair("recent", "[1,2]"),
        ],
        "after boot the store holds every persisted state; `zero` is not one"
    );
    assert!(r.journal().any(|l| l.ends_with("store exact.state.units")));
}

#[test]
fn a_commit_writes_what_changed_and_a_refused_one_writes_nothing() {
    let mut r = boot(vec![]);
    r.take_store_writes();
    r.act("tip", vec![Value::Number(22.0)]).unwrap();
    assert_eq!(kept(r.take_store_writes()), vec![pair("tipPercent", "22")]);
    r.act("tip", vec![Value::Number(22.0)]).unwrap();
    assert!(
        r.take_store_writes().is_empty(),
        "an unchanged value is not written again"
    );
    assert!(r.act("refuse", vec![]).is_err());
    assert!(r.take_store_writes().is_empty());
    assert_eq!(r.persisted("tipPercent"), Some(Value::Number(22.0)));
}

#[test]
fn the_next_launch_reads_back_what_the_last_kept() {
    let mut first = boot(vec![]);
    first.act("tip", vec![Value::Number(-0.0)]).unwrap();
    first.act("convert", vec![]).unwrap();
    first.act("add", vec![]).unwrap();
    let snapshot = first.store().snapshot();
    assert!(snapshot.contains(&pair("tipPercent", "-0")));
    assert!(snapshot.contains(&pair("recent", "[1,2,-1.5]")));
    let plan = contract::compile(SRC).unwrap();
    let second = Runner::boot_stored(
        plan,
        (),
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap();
    let tip = second.slot("tipPercent").unwrap().as_number().unwrap();
    assert!(tip == 0.0 && tip.is_sign_negative(), "{tip}");
    assert_eq!(second.slot("units"), Some(&Value::str("imperial")));
    assert_eq!(
        second.slot("seen"),
        Some(&Value::Option(Some(Value::str("metric").into())))
    );
    assert_eq!(second.persisted("recent"), second.slot("recent").cloned());
}

#[test]
fn a_stored_value_of_another_shape_keeps_the_initial_value_and_says_so() {
    let mut r = boot(vec![
        ("exact.state.tipPercent", "\"eighteen\""),
        ("exact.state.units", "\"imperial\""),
        ("exact.state.recent", "[1,\"x\"]"),
        ("exact.state.seen", "{not json"),
        ("exact.state.gone", "1"),
    ]);
    assert_eq!(r.slot("tipPercent"), Some(&Value::Number(18.0)));
    assert_eq!(r.slot("units"), Some(&Value::str("imperial")));
    assert_eq!(r.slot("seen"), Some(&Value::Option(None)));
    let lines: Vec<&str> = r
        .journal()
        .filter(|l| l.contains("persisted state"))
        .collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].ends_with(
        "persisted state tipPercent: the stored value does not fit number: the initial value stands"
    ));
    assert!(lines[1].contains("seen: the stored value does not fit option<string>"));
    assert!(lines[2].contains("recent: the stored value does not fit list<number>"));
    // The store mirrors the state again from the boot on.
    assert_eq!(
        kept(r.take_store_writes()),
        vec![
            pair("tipPercent", "18"),
            pair("seen", "null"),
            pair("recent", "[1,2]")
        ]
    );
}

#[test]
fn a_value_over_sixteen_kilobytes_is_not_kept() {
    let src = r#"
component App
  state note = "" persist
  action fill(s: string)
    note = s
  view
    text note
"#;
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(plan, (), Kernel::with_monospace(), Default::default(), "/").unwrap();
    r.take_store_writes();
    r.act("fill", vec![Value::str(&"x".repeat(20_000))])
        .unwrap();
    assert!(r.take_store_writes().is_empty());
    assert_eq!(r.persisted("note"), Some(Value::str("")));
    assert!(r
        .journal()
        .any(|l| l.contains("persisted state note: 20002 bytes is over the 16384")));
}
