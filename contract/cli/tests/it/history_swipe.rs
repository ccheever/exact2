//! The history swipe a host tracks (LLP 1115.000 D5.5): the navigation
//! root's `historyswipe`, with a `HistorySwipeEvent` to take or leave, and
//! no other element's.

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
