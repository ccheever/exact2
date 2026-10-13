//! LLP 1116.007 D5: a route whose scroll is a grouped list is on the grouped
//! background, bar to bottom edge, as a hand-built `UITableViewController`'s
//! screen is. Its unsaid `background-color` is the role
//! `-exact-grouped-background`, which each host resolves; an authored one,
//! on the route or on what sits between the header and the list, wins.
//! Any other scroll leaves the page's.

use exact_kernel::style::roles::role;
use exact_kernel::{ColorValue, Kernel};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// The polls screen of round 9: a header (the bar), a segmented switcher,
/// and the list that is the route's scroll. `ROUTE` and `BAND` are the
/// attributes of the route and of the switcher; `LIST` is the list's tag line.
const SCREEN: &str = "routes nav
  home \"/\"
component App
  view
    main navigationKey=`${top(nav).id}` navigationBack=\"back\" testId=\"root\"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` NAMED ROUTE position=\"absolute\" inset=0 testId=\"route\"
          header
            text \"Polls\" role=\"heading\" aria-level=1
          row role=\"radiogroup\" BAND testId=\"band\"
            button role=\"radio\" aria-checked=true
              text \"Alice\"
          LIST
            section
              row
                text \"One\"
";

fn screen(named: &str, route: &str, band: &str, list: &str) -> String {
    SCREEN
        .replace("NAMED", named)
        .replace("ROUTE", route)
        .replace("BAND", band)
        .replace("LIST", list)
}

fn boot(source: &str) -> Runner<NoData> {
    let plan = contract::compile(source).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A box's own `background-color`, by test id: `None` where it paints nothing.
fn fill(r: &Runner<NoData>, id: &str) -> Option<ColorValue> {
    let key = r.kernel().find_by_test_id(id)[0];
    let fill = r.kernel().node_by_key(key).unwrap().style.background_color;
    fill.filter(|c| c.resolve(false).a() != 0)
}

fn grouped() -> Option<ColorValue> {
    Some(ColorValue::Role(role("-exact-grouped-background").unwrap()))
}

const LIST: &str =
    "list id=\"l\" appearance=\"auto\" listStyle=\"inset-grouped\" flex=1 min-height=0";

#[test]
fn a_route_that_scrolls_a_grouped_list_is_on_the_grouped_background() {
    let r = boot(&screen("navigationScroll=\"l\"", "", "", LIST));
    assert_eq!(fill(&r, "route"), grouped());
    // What sits between the header and the list paints nothing of its own,
    // so the route's shows through it.
    assert_eq!(fill(&r, "band"), None);
}

#[test]
fn the_scroll_is_found_as_the_hosts_find_it() {
    // Unnamed: the shallowest box that scrolls on y.
    let unnamed = LIST.replace(" id=\"l\"", "");
    let r = boot(&screen("", "", "", &unnamed));
    assert_eq!(fill(&r, "route"), grouped());
    // `grouped`, the other list style that draws the grouped grey.
    let flat = LIST.replace("inset-grouped", "grouped");
    let r = boot(&screen("navigationScroll=\"l\"", "", "", &flat));
    assert_eq!(fill(&r, "route"), grouped());
    // Under control flow, which is no box.
    let when = format!("if true\n            {LIST}");
    let r = boot(&screen("navigationScroll=\"l\"", "", "", &when));
    assert_eq!(fill(&r, "route"), grouped());
    // A list written without a style is the default, inset-grouped.
    let bare = "list id=\"l\" appearance=\"auto\" flex=1 min-height=0";
    let r = boot(&screen("navigationScroll=\"l\"", "", "", bare));
    assert_eq!(fill(&r, "route"), grouped());
}

#[test]
fn an_authored_background_wins() {
    // On the route.
    let r = boot(&screen(
        "navigationScroll=\"l\"",
        "background-color=\"#ffffff\"",
        "",
        LIST,
    ));
    assert_ne!(fill(&r, "route"), grouped());
    assert!(fill(&r, "route").is_some(), "the author's white");
    let r = boot(&screen(
        "navigationScroll=\"l\"",
        "background-color=\"-exact-background\"",
        "",
        LIST,
    ));
    assert_eq!(
        fill(&r, "route"),
        Some(ColorValue::Role(role("-exact-background").unwrap()))
    );
    // On what sits between the header and the list: the box paints its own
    // over the route's.
    let r = boot(&screen(
        "navigationScroll=\"l\"",
        "",
        "background-color=\"-exact-background\"",
        LIST,
    ));
    assert_eq!(
        fill(&r, "band"),
        Some(ColorValue::Role(role("-exact-background").unwrap()))
    );
    assert_eq!(fill(&r, "route"), grouped());
}

#[test]
fn any_other_scroll_leaves_the_pages_background() {
    // A plain list, a `scroll`, a list that is not the named one.
    for (named, list) in [
        (
            "navigationScroll=\"l\"",
            "list id=\"l\" appearance=\"auto\" listStyle=\"plain\" flex=1 min-height=0",
        ),
        (
            "navigationScroll=\"l\"",
            "scroll id=\"l\" flex=1 min-height=0\n            text \"x\"",
        ),
        (
            "navigationScroll=\"l\"",
            "list id=\"l\" flex=1 min-height=0\n            row\n              text \"x\"",
        ),
        // Unnamed, and a `scroll` comes before the grouped list.
        (
            "",
            "scroll flex=1 min-height=0\n            text \"x\"\n          list appearance=\"auto\" flex=1 min-height=0",
        ),
        // A name that is computed is not told.
        ("navigationScroll=e.url", LIST),
    ] {
        let r = boot(&screen(named, "", "", list));
        assert_eq!(fill(&r, "route"), None, "{named} {list}");
    }
}

#[test]
fn only_a_route_is_given_it() {
    // The navigation root, and a list's box outside any route.
    let r = boot(&screen("navigationScroll=\"l\"", "", "", LIST));
    assert_eq!(fill(&r, "root"), None);
    let r = boot(
        "component App\n  view\n    column navigationScroll=\"l\" testId=\"route\"\n      list id=\"l\" appearance=\"auto\"\n        section\n          row\n            text \"x\"\n",
    );
    assert_eq!(fill(&r, "route"), None, "no navigationKey: no route");
}

#[test]
fn a_sheet_route_is_given_it_too() {
    let r = boot(&screen(
        "navigationScroll=\"l\"",
        "navigationPresentation=\"modal\"",
        "",
        LIST,
    ));
    assert_eq!(fill(&r, "route"), grouped());
}
