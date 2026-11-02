//! The history swipe a host tracks (LLP 1115.000 D5.5): the navigation
//! root's `historyswipe`, with a `HistorySwipeEvent` to take or leave, and
//! no other element's; and the ways through history the root offers,
//! its `navigationHistory`.

use exact_kernel::{Kernel, PropId};
use exact_runner::{Event, Runner};

const APP: &str = r#"routes nav
  home "/"

component App
  state seen = ""
  action follow(location: string)
    nav = go(nav, location)
  action swiped(e: HistorySwipeEvent)
    seen = `${seen}${e.phase} ${e.direction} ${e.amount} ${e.clientY};`
  view
    main navigationKey="app" navigationBack="" navigate=follow historyswipe=swiped
      text seen testId="seen"
"#;

#[test]
fn the_navigation_root_hears_each_phase_with_its_way_and_amount() {
    let plan = contract::compile(APP).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(plan, (), Kernel::with_monospace(), Default::default(), "/").unwrap();
    let root = r.kernel().roots()[0];
    for payload in [
        "began,back,0.1,5,40",
        "changed,back,0.25,5,40",
        "ended,back,0.25,5,40",
        "cancelled,forward,0,5,40",
    ] {
        r.dispatch(root, Event::of_host_kind(60, payload).unwrap())
            .unwrap();
    }
    let key = r.kernel().find_by_test_id("seen")[0];
    let seen = r.kernel().node_by_key(key).unwrap().id;
    assert_eq!(
        r.kernel().node(seen).unwrap().props.str(PropId::Text),
        Some("began back 0.1 40;changed back 0.25 40;ended back 0.25 40;cancelled forward 0 40;")
    );
}

#[test]
fn historyswipe_belongs_to_the_navigation_root() {
    let src = APP.replace(" historyswipe=swiped", "").replace(
        "text seen testId=\"seen\"",
        "text seen testId=\"seen\" historyswipe=swiped",
    );
    let error = contract::compile(&src).unwrap_err().to_string();
    assert!(
        error.contains("`historyswipe` belongs to the navigation root"),
        "{error}"
    );
}

#[test]
fn navigation_history_takes_the_ways_on_the_navigation_root_alone() {
    let with = |value: &str| {
        APP.replace(
            " historyswipe=swiped",
            &format!(" historyswipe=swiped navigationHistory={value}"),
        )
    };
    for value in [
        "\"back forward\"",
        "\"back\"",
        "\"forward\"",
        "\"none\"",
        "seen",
    ] {
        contract::compile(&with(value)).unwrap_or_else(|e| panic!("{value}: {e}"));
    }
    for value in ["\"\"", "\"forward back\"", "\"both\""] {
        let error = contract::compile(&with(value)).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value") && error.contains("`navigationHistory` takes"),
            "{value}: {error}"
        );
    }
    let src = APP.replace(
        "text seen testId=\"seen\"",
        "text seen testId=\"seen\" navigationHistory=\"none\"",
    );
    let error = contract::compile(&src).unwrap_err().to_string();
    assert!(
        error.contains("`navigationHistory` belongs to the navigation root"),
        "{error}"
    );
}
