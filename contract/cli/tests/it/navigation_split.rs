//! LLP 1116.006 D2: `navigationSplit="placeholder"` marks a box in a stack's
//! root route as the detail pane's while nothing is selected. Only a wide
//! window's split shows it (the web shell, over its `display`); every other
//! host lays it out as no box, so a phone's list never draws it. On a route
//! it would hide the route, so it is refused there.

use exact_kernel::{Display, Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A stack of two routes; `route` and `inner` are attributes of each route
/// and of a column inside it beside the route's text.
fn stack(route: &str, inner: &str) -> String {
    format!(
        "routes nav\n  home \"/\"\n    item \"/item\"\ncomponent App\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\"\n      each e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}` testId=`route-${{e.name}}` position=\"absolute\" inset=0 {route}\n          text e.name\n          column testId=`own-${{e.name}}` {inner}\n            text \"Pick one\"\n"
    )
}

#[test]
fn the_placeholder_reaches_the_host_and_is_no_box_where_no_split_shows_it() {
    let plan = contract::bake(
        contract::compile(&stack("", "navigationSplit=\"placeholder\" gap=8")).unwrap(),
        NoData,
    )
    .unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let own = k.node_by_key(k.find_by_test_id("own-home")[0]).unwrap();
    assert_eq!(own.props.str(PropId::NavigationSplit), Some("placeholder"));
    assert_eq!(own.style.display, Display::None);
    assert_eq!((own.frame.width, own.frame.height), (0.0, 0.0));
    // An unmarked box keeps its `column`.
    let plan = contract::bake(contract::compile(&stack("", "")).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    assert_ne!(
        k.node_by_key(k.find_by_test_id("own-home")[0])
            .unwrap()
            .style
            .display,
        Display::None
    );
}

#[test]
fn on_a_route_the_placeholder_is_refused() {
    let error = contract::compile(&stack("navigationSplit=\"placeholder\"", "")).unwrap_err();
    assert_eq!(error.id, "lower-attr-value", "{error}");
    assert!(
        error.message.contains("on a route it would hide the route"),
        "{error}"
    );
    // Any other word is refused as before, naming the new one.
    let error = contract::compile(&stack("", "navigationSplit=\"empty\"")).unwrap_err();
    assert!(
        error.message.contains("\"placeholder\" on a box in it"),
        "{error}"
    );
}

#[test]
fn a_button_in_the_placeholder_is_no_zero_size_button_to_the_bake() {
    // Laid out as no box at the lint's phone width, as the platform intends:
    // not the hidden button the bake refuses (it is pressed in the pane).
    let src = stack("", "navigationSplit=\"placeholder\"").replace(
        "            text \"Pick one\"\n",
        "            text \"Pick one\"\n            button \"Add\" press=add testId=\"add\"\n",
    );
    let src = src.replace(
        "  view\n",
        "  action add\n    nav = push(nav, \"/item\")\n  view\n",
    );
    let plan = contract::compile(&src).unwrap();
    contract::check(&plan).unwrap();
    contract::bake(plan, NoData).unwrap();
}
