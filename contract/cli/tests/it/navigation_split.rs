//! LLP 1116.006 D2: `navigationSplit="placeholder"` marks a box in a stack's
//! root route as the detail pane's while nothing is selected. Only a wide
//! window's split shows it (the web shell, over its `display`); every other
//! host lays it out as no box, so a phone's list never draws it. On a route
//! it would hide the route, so it is refused there.
//!
//! LLP 1116.007 D3.7: the web's split reads a row that draws the disclosure
//! chevron as one that pushes, so a route whose every chevron row presses an
//! action that writes no router is written `navigationSplit="none"`.

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

/// A stack whose root route holds `rows`, with actions that select and that push.
fn rows(route: &str, rows: &str) -> String {
    let rows: String = rows.lines().map(|l| format!("          {l}\n")).collect();
    format!(
        "routes nav\n  home \"/\"\n    item \"/item\"\ncomponent App\n  state picked = \"\"\n  action pick(id: string)\n    picked = id\n  action open(id: string)\n    nav = push(nav, \"/item\")\n  action choose(id: string)\n    picked = id\n    open(id)\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\"\n      each e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}` testId=`route-${{e.name}}` position=\"absolute\" inset=0 {route}\n{rows}"
    )
}

/// What the home route says for `navigationSplit` once lowered.
fn split_of(src: &str) -> Option<String> {
    let plan = contract::bake(
        contract::compile(src).unwrap_or_else(|e| panic!("{e}\n{src}")),
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
    k.node_by_key(k.find_by_test_id("route-home")[0])
        .unwrap()
        .props
        .str(PropId::NavigationSplit)
        .map(str::to_owned)
}

#[test]
fn a_chevron_on_rows_that_push_nowhere_is_decoration_and_keeps_one_column() {
    // event-rsvp (round 9): invitations that select into a panel, a chevron
    // on each, and a Create button elsewhere that does push.
    let decorative = [
        "column press=pick(\"a\") testId=\"row\"\n  text \"A\"\n  image \"symbol:forward-chevron\"",
        "button press=pick(\"a\")\n  text \"A\"\n  image \"symbol:forward-chevron\"",
        // Another spelling of the chevron is the same chevron.
        "button press=pick(\"a\")\n  text \"A\"\n  image \"symbol:chevron-right\"",
        "button press=pick(\"a\")\n  text \"A\"\n  image \"symbol:sf/chevron.forward\"",
        // A row nested in a box, under a `when`, beside a button that does push.
        "button press=open(\"x\")\n  text \"Create\"\ncolumn\n  when true\n    row press=pick(\"a\")\n      text \"A\"\n      image \"symbol:forward-chevron\"",
    ];
    for body in decorative {
        assert_eq!(split_of(&rows("", body)).as_deref(), Some("none"), "{body}");
    }
}

#[test]
fn a_row_that_navigates_keeps_the_hosts_split() {
    for body in [
        "button press=open(\"a\")\n  text \"A\"\n  image \"symbol:forward-chevron\"",
        // An action that selects and then pushes (through a call) pushes.
        "button press=choose(\"a\")\n  text \"A\"\n  image \"symbol:forward-chevron\"",
        "link href=\"/item\"\n  text \"A\"\n  image \"symbol:forward-chevron\"",
        // One row that pushes among rows that do not: the list's rows push.
        "button press=pick(\"a\")\n  text \"A\"\n  image \"symbol:forward-chevron\"\nbutton press=open(\"b\")\n  text \"B\"\n  image \"symbol:forward-chevron\"",
        // No chevron, or a chevron no pressable holds, is no list the rule reads.
        "button press=pick(\"a\")\n  text \"A\"",
        "row\n  text \"A\"\n  image \"symbol:forward-chevron\"",
    ] {
        assert_eq!(split_of(&rows("", body)), None, "{body}");
    }
    // What the author said wins, a width included.
    let body = "button press=pick(\"a\")\n  text \"A\"\n  image \"symbol:forward-chevron\"";
    assert_eq!(
        split_of(&rows("navigationSplit=\"280\"", body)).as_deref(),
        Some("280")
    );
}
