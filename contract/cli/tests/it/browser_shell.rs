//! A browser shell's `iframe` (LLP 1020.002 D1–D3, D6, §9): the accept
//! fixture lowers its props, zoom and four events, a host's
//! `FrameNavigation` reaches an action as its record, and the frame
//! commands reach the host by position.
use exact_kernel::{Kernel, NodeType, PropId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct Schedule;

impl DataSource for Schedule {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../corpus/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_browser_shell_fixture_lowers_and_hears_its_guest() {
    let plan = contract::compile(&corpus("browser-shell.contract")).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = r.kernel().find_by_test_id("tab")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    assert_eq!(node.node_type, NodeType::WebView);
    assert_eq!(node.props.str(PropId::Partition), Some("persist:work"));
    assert_eq!(
        node.props.str(PropId::AppleInactiveSchedulingPolicy),
        Some("throttle")
    );
    assert_eq!(node.style.rare.zoom, 1.5);
    let id = node.id;
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, exact_kernel::Offer::definite(800.0, 600.0))
        .unwrap();
    // `zoom: 1.5` on a 400 px frame: a 600 px box (Chrome, LLP 1020.002 D6).
    assert_eq!(r.kernel().node(id).unwrap().frame.width, 600.0);
    assert_eq!(
        r.handlers_of(id),
        vec![
            EventKind::Navigate,
            EventKind::Currententrychange,
            EventKind::Navigatesuccess,
            EventKind::Navigateerror
        ]
    );
    let event = |payload: &str| Event::of_host_kind(58, payload).unwrap();
    r.dispatch(id, event("navigate\n{\"destination\":\"https://example.com/a\",\"loading\":true,\"navigationId\":1}")).unwrap();
    assert_eq!(r.slot("loading"), Some(&Value::Bool(true)));
    r.dispatch(id, event("currententrychange\n{\"url\":\"https://example.com/a\",\"title\":\"A\",\"canGoBack\":true,\"loading\":true}")).unwrap();
    assert_eq!(
        r.slot("address"),
        Some(&Value::str("https://example.com/a"))
    );
    assert_eq!(r.slot("title"), Some(&Value::str("A")));
    assert_eq!(r.slot("back"), Some(&Value::Bool(true)));
    r.dispatch(
        id,
        event("navigatesuccess\n{\"url\":\"https://example.com/a\"}"),
    )
    .unwrap();
    assert_eq!(r.slot("loading"), Some(&Value::Bool(false)));
    r.dispatch(id, event("navigateerror\n{\"error\":\"AbortError\"}"))
        .unwrap();
    assert_eq!(r.slot("failure"), Some(&Value::str("AbortError")));
    assert!(Event::of_host_kind(58, "load\n{}").is_err());
}

#[test]
fn frame_commands_reach_the_host_by_position() {
    let plan = contract::compile(&corpus("browser-shell.contract")).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let press = |r: &mut Runner<Schedule>, label: &str| {
        let key = r.kernel().find_by_test_id(label)[0];
        let id = r.kernel().node_by_key(key).unwrap().id;
        r.dispatch(id, Event::Press).unwrap();
        r.take_commands()
    };
    let c = press(&mut r, "go");
    assert_eq!(
        (c[0].name.as_str(), c[0].args.clone()),
        (
            "navigate",
            vec![Value::str("tab"), Value::str("https://example.com/next")]
        )
    );
    let c = press(&mut r, "reload");
    assert_eq!(
        (c[0].name.as_str(), c[0].args.clone()),
        ("reload", vec![Value::str("tab"), Value::Bool(true)])
    );
    let c = press(&mut r, "stop");
    assert_eq!(
        (c[0].name.as_str(), c[0].args.clone()),
        ("stopLoading", vec![Value::str("tab")])
    );
}

#[test]
fn a_partition_refuses_what_would_reach_the_app_and_the_words_are_an_iframes() {
    let refused = |view: &str| {
        contract::compile(&format!(
            "component A\n  action got(d: string)\n    let x = d\n  view\n    {view}\n"
        ))
        .unwrap_err()
        .id
    };
    assert_eq!(
        refused("iframe \"app:/data/a.html\" partition=\"work\""),
        "lower-iframe-partition-src"
    );
    assert_eq!(
        refused("iframe src=\"https://a.test\" partition=\"work\" sandbox=\"\""),
        "lower-iframe-partition-sandbox"
    );
    assert_eq!(refused("column zoom=1.5"), "lower-attr-tag");
    // An unpartitioned frame keeps LLP 1020's bridge and sandbox.
    contract::compile("component A\n  action got(d: string)\n    let x = d\n  view\n    iframe \"https://a.test\" sandbox=\"allow-scripts\" message=got\n").unwrap();
    // A computed source is the host's to refuse at run time.
    contract::compile("component A\n  state u = \"https://a.test\"\n  view\n    iframe src=u partition=\"work\"\n").unwrap();
    // The root's `navigate=` keeps its location; an iframe's has none.
    let e = contract::compile("component A\n  action go(location: string)\n    let x = location\n  view\n    iframe \"https://a.test\" partition=\"w\" navigate=go\n").unwrap_err();
    assert_eq!(e.id, "analyze-handler-arity", "{e}");
}
