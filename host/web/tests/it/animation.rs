//! Keyframe animations on the web (LLP 1057 D5): the browser plays them. The
//! host sends each `@keyframes` rule once, named by its content, ahead of the
//! `animation` declaration that uses it; nothing runs per frame.

use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot() -> (Host<NoData>, String) {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/keyframes.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

/// Each `keyframes` op's name and rule, in batch order. Neither holds a
/// quote or a backslash, so the JSON text is the value.
fn rules(batch: &str) -> Vec<(String, String)> {
    batch
        .split("{\"op\":\"keyframes\",\"name\":\"")
        .skip(1)
        .map(|op| {
            let (name, rest) = op.split_once("\",\"css\":\"").unwrap();
            let (css, _) = rest.split_once("\"}").unwrap();
            (name.to_string(), css.to_string())
        })
        .collect()
}

#[test]
fn each_rule_reaches_the_page_once_before_the_declaration_naming_it() {
    let (mut host, first) = boot();
    let sent = rules(&first);
    let [(breathe, breathe_rule), (appear, appear_rule)] = sent.as_slice() else {
        panic!("{first}")
    };
    assert!(breathe.starts_with("breathe-") && breathe.len() == "breathe-".len() + 8);
    assert_eq!(
        breathe_rule,
        &format!("@keyframes {breathe}{{0%{{opacity:0.4;scale:0.9;}}50%{{opacity:1;}}100%{{opacity:0.4;}}}}")
    );
    assert_eq!(
        appear_rule,
        &format!("@keyframes {appear}{{0%{{opacity:0;translate:0px 8px;animation-timing-function:ease-out;}}}}")
    );
    let declaration =
        format!("animation:{breathe} 1.6s ease-in-out 0s infinite normal none running;");
    assert!(first.contains(&declaration), "{first}");
    assert!(
        first.find(breathe_rule.as_str()) < first.find(&declaration),
        "the rule precedes its first use"
    );
    assert!(first.contains(&format!(
        "animation:{appear} 0.3s ease 0s 1 normal both running;"
    )));
    assert!(
        !first.contains("\"op\":\"animate\""),
        "the browser plays it; no frames"
    );

    // A projected document carries the same rules in its head.
    let document = host.document().unwrap();
    assert_eq!(document.keyframes, format!("{appear_rule}{breathe_rule}"));
    assert!(document.root.contains(&declaration));

    // `none` takes the declaration away; back again, the rule is already there.
    let k = host.runner().kernel();
    let toggle = k.node_by_key(k.find_by_test_id("toggle")[0]).unwrap().id;
    let off = host.dispatch_at(toggle, Event::Press, 100.0);
    assert!(!off.contains(&declaration), "{off}");
    assert!(rules(&off).is_empty());
    let on = host.dispatch_at(toggle, Event::Press, 200.0);
    assert!(on.contains(&declaration), "{on}");
    assert!(rules(&on).is_empty(), "sent once: {on}");
}
