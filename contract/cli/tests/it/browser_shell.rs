//! A browser shell's `iframe` (LLP 1020.002 D1–D3, D5, D6, §9): the accept
//! fixture lowers its props, zoom and nine events, a host's
//! `FrameNavigation`, `FrameNewWindow`, `FrameDownload`, `FrameCapture` and
//! `FrameUserPrompt`
//! reach an action as its record, and the frame commands reach the host by
//! position.
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
            EventKind::Navigateerror,
            EventKind::Newwindow,
            EventKind::Download,
            EventKind::Capture,
            EventKind::Userpromptopened
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
    r.dispatch(
        id,
        event("newwindow\n{\"url\":\"https://example.com/b\",\"disposition\":\"background-tab\"}"),
    )
    .unwrap();
    assert_eq!(
        r.slot("opened"),
        Some(&Value::str("background-tab https://example.com/b"))
    );
    r.dispatch(id, event("download\n{\"url\":\"https://example.com/f\",\"suggestedFilename\":\"f.txt\",\"state\":\"completed\",\"path\":\"app:/tmp/downloads/f.txt\"}")).unwrap();
    assert_eq!(
        r.slot("saved"),
        Some(&Value::str("completed f.txt app:/tmp/downloads/f.txt"))
    );
    r.dispatch(
        id,
        event("capture\n{\"path\":\"app:/tmp/shot.png\",\"width\":400,\"height\":200}"),
    )
    .unwrap();
    assert_eq!(
        r.slot("shot"),
        Some(&Value::str("app:/tmp/shot.png 400x200 "))
    );
    r.dispatch(
        id,
        event("capture\n{\"path\":\"app:/tmp/shot.png\",\"error\":\"NotSupportedError\"}"),
    )
    .unwrap();
    assert_eq!(
        r.slot("shot"),
        Some(&Value::str("app:/tmp/shot.png 0x0 NotSupportedError"))
    );
    r.dispatch(
        id,
        event("userpromptopened\n{\"type\":\"prompt\",\"message\":\"Name?\",\"defaultValue\":\"Ada\"}"),
    )
    .unwrap();
    assert_eq!(r.slot("prompted"), Some(&Value::str("prompt Name? Ada")));
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
    // A clip is a `DOMRect`, its fields in the constructor's order; none is the whole viewport.
    let c = press(&mut r, "snap");
    assert_eq!(
        (c[0].name.as_str(), c[0].args.clone()),
        (
            "captureScreenshot",
            vec![
                Value::str("tab"),
                Value::str("app:/tmp/shot.png"),
                Value::record(vec![
                    Value::Number(10.0),
                    Value::Number(20.0),
                    Value::Number(200.0),
                    Value::Number(100.0)
                ])
            ]
        )
    );
    let c = press(&mut r, "whole");
    assert_eq!(c[0].args.len(), 3);
    assert_eq!(c[0].args[2], Value::Option(None));
    let c = press(&mut r, "inspect");
    assert_eq!(
        (c[0].name.as_str(), c[0].args.clone()),
        ("openDevTools", vec![Value::str("tab")])
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
    let e = contract::compile(
        "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    column newwindow=go\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "lower-attr-tag", "{e}");
    // An unpartitioned frame keeps LLP 1020's bridge and sandbox.
    contract::compile("component A\n  action got(d: string)\n    let x = d\n  view\n    iframe \"https://a.test\" sandbox=\"allow-scripts\" message=got\n").unwrap();
    // A computed source is the host's to refuse at run time.
    contract::compile("component A\n  state u = \"https://a.test\"\n  view\n    iframe src=u partition=\"work\"\n").unwrap();
    // A capture writes the app's own files, by a clip of Geometry's `DOMRect`, and answers on an iframe.
    let command = |body: &str| {
        contract::compile(&format!("component A\n  action go\n    {body}\n  view\n    column\n      iframe \"https://a.test\" id=\"tab\"\n      button \"go\" press=go\n"))
    };
    assert_eq!(
        command("captureScreenshot(\"tab\", \"shot.png\")")
            .unwrap_err()
            .id,
        "lower-frame-capture-path"
    );
    assert_eq!(
        command("captureScreenshot(\"tab\", \"app:/tmp/../x.png\")")
            .unwrap_err()
            .id,
        "lower-frame-capture-path"
    );
    assert_eq!(
        command("captureScreenshot(\"page\", \"app:/tmp/x.png\")")
            .unwrap_err()
            .id,
        "lower-frame-command"
    );
    assert_eq!(
        command("captureScreenshot(\"tab\", \"app:/tmp/x.png\", clip=4)")
            .unwrap_err()
            .id,
        "type-frame-command"
    );
    command("captureScreenshot(\"tab\", \"app:/cache/x.png\", clip=DOMRect(x=0, y=0, width=10, height=10))").unwrap();
    // The inspector is any iframe's, by its id alone.
    command("openDevTools(\"tab\")").unwrap();
    assert_eq!(
        command("openDevTools(\"page\")").unwrap_err().id,
        "lower-frame-command"
    );
    assert_eq!(
        command("openDevTools(\"tab\", true)").unwrap_err().id,
        "type-frame-command"
    );
    let e = contract::compile("component A\n  action got(e: FrameCapture)\n    let x = e.path\n  view\n    column capture=got\n").unwrap_err();
    assert_eq!(e.id, "lower-attr-tag", "{e}");
    // The root's `navigate=` keeps its location; an iframe's has none.
    let e = contract::compile("component A\n  action go(location: string)\n    let x = location\n  view\n    iframe \"https://a.test\" partition=\"w\" navigate=go\n").unwrap_err();
    assert_eq!(e.id, "analyze-handler-arity", "{e}");
}

#[test]
fn a_guests_requests_are_held_for_the_agent_on_its_frame() {
    use exact_runner::agent;
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
    let id = r.kernel().node_by_key(key).unwrap().id;
    let mut hold = |capability: &str, args: &str| {
        let line = format!(
            r#"{{"op":"frameRequest","node":{id},"capability":"{capability}","args":{args}}}"#
        );
        agent::answer(&mut r, &line).unwrap().0
    };
    let ticket = |reply: &str| {
        let v: serde_json::Value = serde_json::from_str(reply).unwrap();
        v["ticket"].as_u64().unwrap_or_else(|| panic!("{reply}"))
    };
    let alert = ticket(&hold(
        "dialog",
        r#"{"type":"alert","message":"hi","origin":"a.test"}"#,
    ));
    let prompt = ticket(&hold(
        "dialog",
        r#"{"type":"prompt","message":"name?","origin":"a.test","defaultText":""}"#,
    ));
    let file = ticket(&hold(
        "open-file",
        r#"{"multiple":false,"directories":false}"#,
    ));
    let save = ticket(&hold("save-file", r#"{"suggestedFilename":"f.txt"}"#));
    assert!(hold("camera", "{}").contains("not a frame request"));
    let holds = r.device_holds().to_vec();
    assert_eq!(holds.len(), 4);
    assert!(holds.iter().all(|h| h.node == Some(id)));
    assert!(
        holds[0].args.contains("\"message\":\"hi\""),
        "{:?}",
        holds[0]
    );
    let mut answer = |line: String| agent::answer(&mut r, &line).unwrap().0;
    // An alert is answered by tap, a prompt by type too.
    assert!(answer(format!(r#"{{"op":"type","ticket":{alert},"text":"x"}}"#)).contains("error"));
    assert!(
        answer(format!(r#"{{"op":"tap","ticket":{alert},"choice":"ok"}}"#))
            .contains("\"answered\":\"ok\"")
    );
    assert!(
        answer(format!(r#"{{"op":"type","ticket":{prompt},"text":"Ada"}}"#))
            .contains("\"answered\":\"value\"")
    );
    assert!(answer(format!(
        r#"{{"op":"type","ticket":{file},"text":"relative"}}"#
    ))
    .contains("absolute path"));
    assert!(
        answer(format!(r#"{{"op":"tap","ticket":{save},"choice":"ok"}}"#))
            .contains("\"capability\":\"save-file\"")
    );
    assert_eq!(r.device_holds().len(), 1);
}
