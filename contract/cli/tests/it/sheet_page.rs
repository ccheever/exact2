//! LLP 1116.009 D3: a sheet is never transparent. A route that
//! `navigationPresentation="modal"` presents is on the page's own ground, the
//! role `-exact-background`, where its author wrote none, so the route it
//! covers is not seen through it (a web or Mac host keeps that route in view;
//! UIKit's sheet has the system background). An authored `background-color`,
//! on the route or in its class, wins; a sheet that scrolls a grouped list is
//! on that grey (LLP 1116.007 D5); a route that is no sheet is left alone.

use exact_kernel::style::roles::role;
use exact_kernel::{ColorValue, Kernel};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A navigation root with one route, the form of round 11's habit-tracker: a
/// header and a plain `scroll`. `STYLES` precede the app; `ROUTE` is the route's
/// own attributes; `SCROLL` the tag line of what scrolls.
const SCREEN: &str = "STYLESroutes nav
  home \"/\"
component App
  view
    main navigationKey=`${top(nav).id}` navigationBack=\"back\" testId=\"root\"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` ROUTE position=\"absolute\" inset=0 testId=\"route\"
          header
            text \"New habit\" role=\"heading\" aria-level=2
          SCROLL
";

fn screen(styles: &str, route: &str, scroll: &str) -> String {
    SCREEN
        .replace("STYLES", styles)
        .replace("ROUTE", route)
        .replace("SCROLL", scroll)
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
    let _roles = r.bind_roles(); // Its own: the test's other runners live too.
    let key = r.kernel().find_by_test_id(id)[0];
    let fill = r.kernel().node_by_key(key).unwrap().style.background_color;
    fill.filter(|c| c.resolve(false).a() != 0)
}

fn role_of(name: &str) -> Option<ColorValue> {
    Some(ColorValue::Role(role(name).unwrap()))
}

const SCROLL: &str = "scroll id=\"form\" flex=1 min-height=0\n            text \"Name\"";
const MODAL: &str = "navigationPresentation=\"modal\"";

#[test]
fn a_modal_route_is_on_the_pages_background() {
    let r = boot(&screen("", MODAL, SCROLL));
    assert_eq!(fill(&r, "route"), role_of("-exact-background"));
    // With the scroll named, as the recipes write it, and a detent.
    let r = boot(&screen(
        "",
        "navigationPresentation=\"modal\" navigationDetent=\"fit-content\" navigationScroll=\"form\"",
        SCROLL,
    ));
    assert_eq!(fill(&r, "route"), role_of("-exact-background"));
    // The header and the scroll paint nothing of their own: the route's is theirs.
    assert_eq!(fill(&r, "root"), None);
}

#[test]
fn an_authored_background_wins() {
    // On the route.
    let r = boot(&screen(
        "",
        &format!("{MODAL} background-color=\"#336699\""),
        SCROLL,
    ));
    assert!(fill(&r, "route").is_some());
    assert_ne!(fill(&r, "route"), role_of("-exact-background"));
    let r = boot(&screen(
        "",
        &format!("{MODAL} background-color=\"-exact-secondary-background\""),
        SCROLL,
    ));
    assert_eq!(fill(&r, "route"), role_of("-exact-secondary-background"));
    // Written transparent, a sheet that means to show what it covers does.
    let r = boot(&screen(
        "",
        &format!("{MODAL} background-color=\"transparent\""),
        SCROLL,
    ));
    assert_eq!(fill(&r, "route"), None);
    // In a class of the route.
    let r = boot(&screen(
        "style Panel\n  background-color=\"#336699\"\n",
        &format!("{MODAL} class=Panel"),
        SCROLL,
    ));
    assert!(fill(&r, "route").is_some());
    assert_ne!(fill(&r, "route"), role_of("-exact-background"));
}

#[test]
fn a_sheet_that_scrolls_a_grouped_list_is_on_that_grey() {
    let list =
        "list id=\"form\" appearance=\"auto\" listStyle=\"inset-grouped\" flex=1 min-height=0";
    let r = boot(&screen(
        "",
        &format!("{MODAL} navigationScroll=\"form\""),
        list,
    ));
    assert_eq!(fill(&r, "route"), role_of("-exact-grouped-background"));
}

#[test]
fn a_route_that_is_no_sheet_is_left_alone() {
    // A page, a fullscreen cover (the route under it is hidden, not shown through it),
    // and a presentation the compiler cannot tell.
    for route in [
        "",
        "navigationPresentation=\"fullscreen\"",
        "navigationPresentation=\"\"",
        "navigationPresentation=(e.id == 0 ? \"modal\" : \"\")",
    ] {
        let r = boot(&screen("", route, SCROLL));
        assert_eq!(fill(&r, "route"), None, "{route}");
    }
}

#[test]
fn a_presentation_off_a_route_is_refused() {
    // No navigation root at all, and round 34's gradebook: the stack draws a
    // form component whose root says the presentation but has no key, so no
    // host presented it and iOS drew its header under the status bar.
    let form = "routes nav
  home \"/\"
    rename \"/rename\"
component App
  view
    main navigationKey=`${top(nav).id}` navigationBack=\"back\"
      for e in stack(nav) key=e.id
        if e.name == \"home\"
          column navigationKey=`${e.id}` position=\"absolute\" inset=0
            text \"Home\"
        if e.name == \"rename\"
          RenameForm()
component RenameForm
  view
    column navigationPresentation=\"modal\" position=\"absolute\" inset=0
      text \"Rename\"
";
    for (source, line) in [
        ("component App\n  view\n    column navigationPresentation=\"modal\"\n      text \"x\"\n".to_string(), 3),
        (form.to_string(), 15),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-route-place", "{error}");
        assert!(error.message.contains("no host presents it") && error.message.contains("edit-sheet"), "{error}");
        assert_eq!(error.span.line, line, "{error}");
    }
    // Moved onto the route, as the message says, it is a sheet.
    let fixed = form
        .replace("        if e.name == \"rename\"\n          RenameForm()", "        if e.name == \"rename\"\n          column navigationKey=`${e.id}` navigationPresentation=\"modal\" position=\"absolute\" inset=0\n            RenameForm()")
        .replace("    column navigationPresentation=\"modal\" position=\"absolute\" inset=0\n      text \"Rename\"", "    column flex=1\n      text \"Rename\"");
    contract::compile(&fixed).unwrap_or_else(|e| panic!("{e}"));
}
