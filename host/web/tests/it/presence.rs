//! Exit animation and layout transition on the web (LLP 1063): the browser
//! plays both. The host writes each row as a custom property the page's
//! presence module reads, sends an exit's `@keyframes` while its node lives,
//! and names the view that leaves ahead of the destroys that remove it.

use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn the_rows_reach_the_page_as_custom_properties_and_an_exit_precedes_its_destroy() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/presence.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    let (mut host, first) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    assert!(
        first.contains("--exact-layout-transition:320 0 cubic-bezier(0.32,0.72,0,1);"),
        "{first}"
    );
    let leave = first
        .split("{\"op\":\"keyframes\",\"name\":\"")
        .nth(1)
        .and_then(|op| op.split_once('"'))
        .map(|(name, _)| name.to_string())
        .expect("the exit's rule, sent while its node lives");
    assert!(leave.starts_with("leave-"), "{leave}");
    assert!(
        first.contains(&format!("--exact-exit-animation:{leave} 0.2s cubic-bezier(0.32,0.72,0,1) 0s 1 normal both running;")),
        "{first}"
    );
    assert!(
        !first.contains("transition:all 0.32"),
        "not a CSS transition"
    );

    let view = |name| {
        let k = host.runner().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let (gone, toggle) = (view("first"), view("toggle"));
    let off = host.dispatch_at(toggle, Event::Press, 100.0);
    let exit = off.find(&format!("{{\"op\":\"exit\",\"id\":{gone}}}"));
    let destroy = off.find(&format!("{{\"op\":\"destroy\",\"id\":{gone}}}"));
    assert!(exit.is_some() && exit < destroy, "{off}");
}

#[test]
fn a_spring_layout_transition_is_its_curve_as_linear() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      text \"a\" layout-transition=\"spring(300, 30, 1)\"\n",
    )
    .unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    let (_, first) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    let at = first.find("--exact-layout-transition:").expect("declared");
    let value = &first[at..first[at..].find(';').unwrap() + at];
    assert!(value.contains(" 0 linear(0 0%,"), "{value}");
    assert!(value.ends_with("1 100%)"), "{value}");
}
