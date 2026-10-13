//! LLP 1115.002 D4 (the rule is LLP 1116.004 D1.2's): a stack splits when a
//! row of its root route's list pushes, read from the push and not from a
//! chevron (`Runner::stack_split`). The Mac's split view reads it; the web's
//! own rule (`host/web/navigation.js` `splitOf`) is its own until it moves.

use exact_kernel::{Kernel, ViewId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner, StackSplit};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A stack whose root route holds `rows` (written under a `list` with the
/// attributes `list`) and a header button, and whose pushed routes are an
/// item's and a form's. `route` is the root route's own attributes.
fn app(route: &str, list: &str, rows: &str) -> String {
    format!(
        r#"routes nav
  home "/"
    item "/item/:id"
    add "/add"

component App
  state picked = ""
  state hidden = false
  action hide
    hidden = true
  action follow(url: string)
    nav = go(nav, url)
  action open(id: string)
    nav = push(nav, path("item", id))
  action pick(id: string)
    picked = id
  action openAdd
    nav = push(nav, "/add")
  action back
    nav = back(nav)
  view
    main navigationKey=`${{top(nav).id}}` navigationBack="back" navigate=follow width="100%" height="100%"
      each e in stack(nav) key=e.id
        when e.name == "home"
          column navigationKey=`${{e.id}}` testId="home" position="absolute" inset=0 {route}
            header
              text "Home" role="heading"
              button press=openAdd testId="add"
                text "Add"
              button press=hide testId="hide"
                text "Hide"
            list testId="rows" {list}
{rows}
        when e.name == "item"
          column navigationKey=`${{e.id}}` testId=`item-${{e.params.id}}` position="absolute" inset=0
            text e.params.id
            button id="back" press=back testId="item-back"
              text "Back"
        when e.name == "add"
          column navigationKey=`${{e.id}}` navigationPresentation="modal" testId="form" position="absolute" inset=0
            text "New"
"#
    )
}

const LINKS: &str = r#"              each id in ["a", "b", "c"] key=id
                link href=path("item", id) testId=`row-${id}`
                  text id
                  button press=pick(id) testId=`star-${id}`
                    text "Star""#;

fn boot(source: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(source).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view(r: &Runner<NoData>, test_id: &str) -> ViewId {
    let k = r.kernel();
    let found = k.find_by_test_id(test_id);
    assert_eq!(found.len(), 1, "{test_id}");
    k.node_by_key(found[0]).unwrap().id
}

fn split(r: &Runner<NoData>) -> Option<StackSplit> {
    r.stack_split(view(r, "home"), None)
}

#[test]
fn a_list_whose_rows_link_to_a_route_splits_and_names_each_rows_detail() {
    let mut r = boot(&app("", "", LINKS));
    let found = split(&r).expect("rows that are links to a route push");
    assert_eq!(found.list, Some(view(&r, "rows")));
    assert_eq!(found.width, None);
    assert!(found.rows.is_empty(), "nothing is pushed yet");

    // A row's link is followed: its route is that row's detail.
    let root = r.kernel().roots()[0];
    r.dispatch(root, Event::Navigate("/item/b".into())).unwrap();
    let found = split(&r).unwrap();
    assert_eq!(found.rows, vec![(view(&r, "item-b"), view(&r, "row-b"))]);

    // Another row's, pushed over the first: each is its own row's.
    r.dispatch(root, Event::Navigate("/item/c".into())).unwrap();
    let found = split(&r).unwrap();
    assert_eq!(
        found.rows,
        vec![
            (view(&r, "item-b"), view(&r, "row-b")),
            (view(&r, "item-c"), view(&r, "row-c")),
        ]
    );
    // A pushed route is no stack's root, and a button in a row is not the row.
    assert_eq!(r.stack_split(view(&r, "item-c"), None), None);
    assert_eq!(r.stack_split(view(&r, "star-a"), None), None);
}

#[test]
fn a_row_whose_press_writes_the_router_splits_and_one_that_writes_none_does_not() {
    let pressed = |action: &str| {
        format!(
            "              each id in [\"a\", \"b\"] key=id\n                button press={action}(id) testId=`row-${{id}}`\n                  text id"
        )
    };
    let r = boot(&app("", "", &pressed("open")));
    let found = split(&r).expect("a press that writes the router pushes");
    assert_eq!(found.list, Some(view(&r, "rows")));
    assert!(found.rows.is_empty(), "a press names no location");

    // Rows that select into state push nothing: the header's button, which
    // does write the router, is no row of the list.
    let r = boot(&app("", "", &pressed("pick")));
    assert_eq!(split(&r), None);
}

#[test]
fn rows_of_text_a_sideways_list_and_a_hidden_list_do_not_split() {
    let text = "              each id in [\"a\", \"b\"] key=id\n                text id";
    assert_eq!(split(&boot(&app("", "", text))), None);
    assert_eq!(
        split(&boot(&app(
            "",
            "display=\"flex\" flex-direction=\"row\"",
            LINKS
        ))),
        None,
        "a row of cards is a carousel"
    );
    // A list hidden as authored shows no rows.
    let mut r = boot(&app("", "display=(hidden ? \"none\" : \"block\")", LINKS));
    assert!(split(&r).is_some());
    r.dispatch(view(&r, "hide"), Event::Press).unwrap();
    assert_eq!(split(&r), None);
    // A link out of the app is no push.
    let out = "              each id in [\"a\", \"b\"] key=id\n                link href=`https://example.com/${id}`\n                  text id";
    assert_eq!(split(&boot(&app("", "", out))), None);
}

#[test]
fn navigation_split_on_the_root_route_opts_out_names_the_width_and_opts_in() {
    assert_eq!(
        split(&boot(&app("navigationSplit=\"none\"", "", LINKS))),
        None
    );
    let r = boot(&app("navigationSplit=280", "", LINKS));
    let found = split(&r).unwrap();
    assert_eq!(
        (found.list, found.width),
        (Some(view(&r, "rows")), Some(280.0))
    );
    // A width splits a stack whose list the rule does not find.
    let text = "              each id in [\"a\", \"b\"] key=id\n                text id";
    let found = split(&boot(&app("navigationSplit=300", "", text))).unwrap();
    assert_eq!((found.list, found.width), (None, Some(300.0)));
}

#[test]
fn a_retained_list_still_obeys_the_rule_and_only_keeps_nonactionable_rows() {
    let cases = [
        ("", "", true),
        ("", "              text \"No Results\"", true),
        ("display=\"flex\" flex-direction=\"row\"", "", false),
        ("display=\"none\"", "", false),
        ("virtualized=true display=\"flex\" height=132 overflow-x=\"scroll\" overflow-y=\"hidden\" estimated-item-width=100", LINKS, false),
        (
            "",
            "              button press=pick(\"a\")\n                text \"Pick\"",
            false,
        ),
        (
            "",
            "              link href=\"https://example.com\"\n                text \"Outside\"",
            false,
        ),
    ];
    for (attributes, rows, retained) in cases {
        let r = boot(&app("", attributes, rows));
        let list = view(&r, "rows");
        assert_eq!(
            r.stack_split(view(&r, "home"), Some(list)).is_some(),
            retained,
            "{attributes}: {rows}"
        );
    }
    let mut r = boot(&app(
        "navigationSplit=(hidden ? \"\" : \"300\")",
        "",
        "              text \"No list navigation\"",
    ));
    assert_eq!(split(&r).unwrap().width, Some(300.0));
    r.dispatch(view(&r, "hide"), Event::Press).unwrap();
    assert_eq!(split(&r), None, "the current authored width is read fresh");
}

/// A host that shows a stack's top in a pane of its own shows the list
/// beside it: the list is then no covered screen (`Runner::inactive`), as
/// the driver's `tree` and a testId's resolution read it. Under a sheet,
/// whose block is a maximum, and with no block, it is covered as before.
#[test]
fn the_list_beside_a_route_in_a_hosts_pane_is_not_inactive() {
    use exact_kernel::{BlockAxis, HostBlock};
    let mut r = boot(&app("", "", LINKS));
    let root = r.kernel().roots()[0];
    r.dispatch(root, Event::Navigate("/item/b".into())).unwrap();
    let (row, item) = (view(&r, "row-a"), view(&r, "item-b"));
    assert!(r.inactive(row), "a pushed route covers its list");
    let pane = HostBlock {
        width: BlockAxis::Is(500.0),
        height: BlockAxis::Is(400.0),
    };
    r.kernel_mut().set_host_block(item, Some(pane)).unwrap();
    assert!(!r.inactive(row), "beside a pane the list is live");
    assert!(!r.inactive(view(&r, "item-back")));
    // Another row's detail over the first, in the pane too: the first is covered, the list is not.
    r.dispatch(root, Event::Navigate("/item/c".into())).unwrap();
    let top = view(&r, "item-c");
    assert!(r.inactive(row), "the new top has no pane yet");
    r.kernel_mut().set_host_block(top, Some(pane)).unwrap();
    assert!(!r.inactive(row));
    assert!(r.inactive(item), "a route under the pane's top is covered");
    // A sheet's block is a maximum: what is under it is covered.
    let sheet = HostBlock {
        width: BlockAxis::AtMost(500.0),
        height: BlockAxis::AtMost(400.0),
    };
    r.kernel_mut().set_host_block(top, Some(sheet)).unwrap();
    assert!(r.inactive(row));
    r.kernel_mut().set_host_block(top, None).unwrap();
    assert!(r.inactive(row));
}
