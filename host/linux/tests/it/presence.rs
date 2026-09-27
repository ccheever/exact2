//! Layout transition on Linux, and its refusal of exit animation (LLP 1063):
//! a sibling that takes a removed node's place is painted from where it was;
//! the removed node leaves at once and the journal says why.
use exact_kernel::MonospaceMeasurer;
use exact_linux::Host;
use exact_runner::{DataError, DataSource, Event};

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

fn boot(source: &str) -> Host<NoData> {
    let plan = contract::compile(source).unwrap().encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0
}

fn view(h: &Host<NoData>, name: &str) -> u32 {
    let k = h.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

#[test]
fn a_sibling_slides_into_a_removed_nodes_place_which_leaves_at_once() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/presence.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let second = view(&h, "second");
    assert_eq!(h.presented(second).layout, [0.0, 0.0, 1.0, 1.0]);
    assert!(!h.motion());
    h.dispatch_at(view(&h, "toggle"), Event::Press, 100.);
    assert!(h.kernel().find_by_test_id("first").is_empty());
    let [_, from, ..] = h.presented(second).layout;
    assert!(from > 0.0, "painted where it was: {from}");
    h.tick(200.);
    let [_, mid, ..] = h.presented(second).layout;
    assert!(mid > 0.0 && mid < from, "{mid} of {from}");
    h.tick(421.);
    assert_eq!(h.presented(second).layout, [0.0, 0.0, 1.0, 1.0]);
    assert!(!h.motion());
    let logs = h.agent("{\"op\":\"logs\"}");
    assert!(logs.contains("exit-animation: refused on Linux"), "{logs}");
}

#[test]
fn a_growing_box_scales_from_its_old_size_and_a_gained_row_animates_its_first_move() {
    let mut h = boot(
        "component App\n  state on = false\n  action go writes on\n    on = true\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      when not on\n        view height=50\n      column testId=\"card\" layout-transition=(on ? \"200ms linear\" : \"none\")\n        text \"Title\"\n        when on\n          view height=150\n",
    );
    let card = view(&h, "card");
    let before = h.kernel().node(card).unwrap().frame.height;
    // One commit gives the card the row, moves it up and grows it.
    h.dispatch_at(view(&h, "go"), Event::Press, 100.);
    let after = h.kernel().node(card).unwrap().frame.height;
    let [dx, dy, sx, sy] = h.presented(card).layout;
    assert_eq!((dx, sx), (0.0, 1.0));
    assert!((dy - 50.0).abs() < 1e-3, "{dy}");
    assert!((sy - before / after).abs() < 1e-6, "{sy}");
    h.tick(301.);
    assert_eq!(h.presented(card).layout, [0.0, 0.0, 1.0, 1.0]);
}
