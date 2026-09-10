//! The corpus (LLP 1004 D6): every accept fixture compiles byte-identically,
//! round-trips, and runs; every reject fixture is refused with exactly its id.

use exact_kernel::{Dimension, Kernel, NodeType, PropId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

#[test]
fn dynamic_auto_keeps_the_meaning_of_its_style_row() {
    let plan = contract::compile(
        r#"component AutoRows
  state sizing = "auto"
  state bars = "auto"
  action choose(value: string) writes bars
    bars = value
  view
    scroll testId="reader" height=100 width=sizing overscroll-behavior-x=sizing align-self=sizing scrollbar-width=bars
      box width=600 height=100
"#,
    )
    .unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        Schedule,
        Kernel::with_monospace(),
    )
    .unwrap();
    let key = r.kernel().find_by_test_id("reader")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    assert_eq!(node.style.width, Dimension::Auto);
    assert_eq!(
        node.style.overscroll_behavior_x,
        exact_kernel::OverscrollBehavior::Auto
    );
    assert_eq!(node.style.align_self, exact_kernel::AlignSelf::Auto);
    assert_eq!(
        node.style.scrollbar_width,
        exact_kernel::ScrollbarWidth::Auto
    );
    for (value, expected) in [
        ("none", exact_kernel::ScrollbarWidth::None),
        ("thin", exact_kernel::ScrollbarWidth::Thin),
        ("auto", exact_kernel::ScrollbarWidth::Auto),
    ] {
        r.act("choose", vec![Value::str(value)]).unwrap();
        let node = r.kernel().node_by_key(key).unwrap();
        assert_eq!(node.style.scrollbar_width, expected);
        assert_eq!(node.style.width, Dimension::Auto);
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn reject_fixtures_are_refused_by_exactly_their_id() {
    let text = corpus("rejects.txt");
    let mut checked = 0;
    for case in text.split("\n---\n") {
        let case = case.trim();
        let Some(rest) = case.strip_prefix("== ") else {
            continue;
        };
        let (id, src) = rest.split_once('\n').unwrap();
        let src: String = src
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        match contract::compile(&src) {
            Err(e) => assert_eq!(e.id, id, "fixture `{id}` was refused as `{}`: {e}", e.id),
            Ok(_) => panic!("fixture `{id}` compiled"),
        }
        checked += 1;
    }
    assert!(checked >= 26, "{checked} reject fixtures");
}

#[test]
fn a_file_without_a_component_is_a_typed_refusal() {
    for src in ["", "shape S\n  a: number\n"] {
        let error = contract::compile(src).unwrap_err();
        assert_eq!(error.id, "analyze-no-component");
        assert_eq!(error.span, (1, 1));
    }
}

#[test]
fn a_template_with_an_inline_match_runs_through_the_compiler() {
    let src = "component App\n  state choice = some(\"yes\")\n  view\n    text `${match choice { case some(value) => value, case none => \"}\" }}` testId=\"matched\"\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(plan, Schedule, Kernel::with_monospace()).unwrap();
    assert_eq!(text_of(&r, "matched").as_deref(), Some("yes"));
}

#[test]
fn button_primary_text_is_a_real_accessible_text_child() {
    let src = "component App\n  state pressed = false\n  action press writes pressed\n    pressed = true\n  view\n    button \"Post\" press=press testId=\"post\"\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(plan, Schedule, Kernel::with_monospace()).unwrap();
    let key = r.kernel().find_by_test_id("post")[0];
    let button = r.kernel().node_by_key(key).unwrap();
    assert_eq!(button.node_type, NodeType::Pressable);
    assert_eq!(button.props.str(PropId::AccessibilityRole), Some("button"));
    let children = button.children();
    assert_eq!(children.len(), 1);
    let label = r.kernel().node(children[0]).unwrap();
    assert_eq!(label.node_type, NodeType::Text);
    assert_eq!(label.props.str(PropId::Text), Some("Post"));
}

#[test]
fn every_handler_kind_types_and_untyped_payloads_are_inferred() {
    let src = "component App\n  state textValue = \"\"\n  state boolValue = false\n  action noPayload\n  action stringPayload(value) writes textValue\n    textValue = value\n  action boolPayload(value) writes boolValue\n    boolValue = value\n  view\n    column\n      button \"press\" press=noPayload\n      input change=stringPayload key=stringPayload hover=boolPayload focus=noPayload blur=noPayload submit=noPayload\n      iframe \"/guest\" load=noPayload message=stringPayload\n";
    contract::compile(src).unwrap();
}

#[test]
fn dynamic_invalid_integer_props_are_still_refused_at_boot() {
    for value in ["1.5", "9223372036854775808"] {
        let src = format!(
            "component App\n  state level = {value}\n  view\n    column aria-level=level\n      text \"heading\"\n"
        );
        let plan = contract::compile(&src).unwrap();
        assert!(Runner::boot(plan, Schedule, Kernel::with_monospace()).is_err());
    }
}

/// The same miniature data source the runner's hand-built test uses.
#[derive(Default)]
struct Schedule;

fn station(id: &str, name: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(name)])
}

fn departure(id: &str, train: f64, at_ms: f64) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::Number(train),
        Value::Number(at_ms),
    ])
}

impl DataSource for Schedule {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "stations" => Ok(Value::list(vec![
                station("mv", "Mountain View"),
                station("pa", "Palo Alto"),
            ])),
            "departures" => match args.first().and_then(Value::as_str) {
                Some("mv") => Ok(Value::list(vec![
                    departure("d1", 101.0, 600_000.0),
                    departure("d2", 103.0, 1_500_000.0),
                ])),
                Some("pa") => Ok(Value::list(vec![
                    departure("d2", 103.0, 1_200_000.0),
                    departure("d1", 101.0, 300_000.0),
                    departure("d9", 109.0, 9_000_000.0),
                ])),
                other => Err(DataError::Unavailable(format!("{other:?}"))),
            },
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn text_of(r: &Runner<Schedule>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}

fn ids(r: &Runner<Schedule>, prefix: &str) -> Vec<(String, u32)> {
    let k = r.kernel();
    let mut out = Vec::new();
    for root in k.roots() {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let n = k.node(id).unwrap();
            if let Some(t) = n.props.str(PropId::TestId) {
                if t.starts_with(prefix) {
                    out.push((t.to_string(), id));
                }
            }
            let mut c = n.children();
            c.reverse();
            stack.extend(c);
        }
    }
    out
}

#[test]
fn the_now_screen_fixture_compiles_and_behaves_like_the_hand_built_plan() {
    let src = corpus("now-screen.contract");
    let plan = contract::compile(&src).unwrap();
    assert_eq!(
        contract::compile(&src).unwrap().encode(),
        plan.encode(),
        "byte-identical"
    );
    let plan = Plan::decode(&plan.encode()).unwrap();
    let baked = contract::bake(plan, Schedule).unwrap();
    assert!(baked.resources.iter().all(|r| r.initial.len > 0));

    let mut r = Runner::boot(baked, Schedule, Kernel::with_monospace()).unwrap();
    assert_eq!(text_of(&r, "count").as_deref(), Some("2 trains"));
    assert_eq!(text_of(&r, "nearest").as_deref(), Some("nearest"));
    let rows = ids(&r, "dep-");
    assert_eq!(
        rows.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["dep-d1", "dep-d2"]
    );
    let (d1, d2) = (rows[0].1, rows[1].1);

    // Press → the curried row id → the source refuses "d2" → rolled back.
    assert!(r.dispatch(d2, Event::Press).is_err());
    assert_eq!(r.slot("stationId"), Some(&Value::NONE));
    r.act("selectStation", vec![Value::str("pa")]).unwrap();
    assert_eq!(text_of(&r, "count").as_deref(), Some("3 trains"));
    assert_eq!(text_of(&r, "selected").as_deref(), Some("at pa"));
    let rows = ids(&r, "dep-");
    assert_eq!(
        rows.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["dep-d2", "dep-d1", "dep-d9"]
    );
    assert_eq!(
        (rows[0].1, rows[1].1),
        (d2, d1),
        "keyed rows keep their views"
    );

    // Search flips the `when`; the timer ticks under the clock.
    let search = ids(&r, "search")[0].1;
    r.dispatch(search, Event::Change("pal".into())).unwrap();
    assert_eq!(text_of(&r, "searching").as_deref(), Some("searching"));
    r.dispatch(search, Event::Change(String::new())).unwrap();
    assert_eq!(ids(&r, "dep-").len(), 3);
    assert_eq!(r.advance(2_500.0).unwrap().len(), 2);
    assert_eq!(r.slot("nowMs"), Some(&Value::Number(2_000.0)));

    r.act("setDark", vec![Value::str("dark")]).unwrap();
    assert_eq!(r.take_commands()[0].name, "setScheme");
}

#[test]
fn the_iframe_fixture_lowers_and_records_its_events() {
    let src = corpus("iframe.contract");
    let plan = contract::compile(&src).unwrap();
    assert_eq!(contract::compile(&src).unwrap().encode(), plan.encode());
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(plan, Schedule, Kernel::with_monospace()).unwrap();
    let key = r.kernel().find_by_test_id("deck")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    assert_eq!(node.node_type, NodeType::WebView);
    assert_eq!(node.props.str(PropId::Src), Some("/deck/index.html"));
    assert_eq!(node.props.str(PropId::Sandbox), Some("allow-scripts"));
    assert_eq!(node.style.width, Dimension::Points(300.0));
    assert_eq!(node.style.height, Dimension::Points(150.0));
    let id = node.id;
    assert_eq!(r.handlers_of(id), vec![EventKind::Load, EventKind::Message]);
    r.dispatch(id, Event::Load).unwrap();
    r.dispatch(id, Event::Message("deck-ready".into())).unwrap();
    assert_eq!(r.slot("loaded"), Some(&Value::Bool(true)));
    assert_eq!(r.slot("received"), Some(&Value::str("deck-ready")));
}

#[test]
fn contextmenu_and_double_click_keep_their_authored_arguments_and_do_not_take_a_press() {
    let src = r#"component App
  state selected = ""
  state magnify = false
  action choose(value: string) writes selected, magnify
    selected = value
    magnify = value == "context"
  view
    column
      button contextmenu=choose("context") dblclick=choose("double") swiperight=choose("right") testId="bubble"
        text "A message"
      column contextTarget="bubble" contextMagnify=magnify testId="preview"
        text "A message"
"#;
    let plan = contract::compile(src).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(plan, Schedule, Kernel::with_monospace()).unwrap();
    let node = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("bubble")[0])
        .unwrap()
        .id;
    assert_eq!(
        r.handlers_of(node),
        vec![
            EventKind::Contextmenu,
            EventKind::Dblclick,
            EventKind::Swiperight
        ]
    );
    assert!(r.dispatch(node, Event::Press).is_err());
    assert_eq!(r.slot("selected"), Some(&Value::str("")));
    let preview = r.kernel().find_by_test_id("preview")[0];
    let magnifies = |r: &Runner<Schedule>| {
        r.kernel()
            .node_by_key(preview)
            .unwrap()
            .props
            .bool(PropId::ContextMagnify)
    };
    assert_eq!(magnifies(&r), Some(false));
    r.dispatch(node, Event::Contextmenu).unwrap();
    assert_eq!(r.slot("selected"), Some(&Value::str("context")));
    assert_eq!(magnifies(&r), Some(true));
    r.dispatch(node, Event::Dblclick).unwrap();
    assert_eq!(r.slot("selected"), Some(&Value::str("double")));
    assert_eq!(magnifies(&r), Some(false));
    r.dispatch(node, Event::Swiperight).unwrap();
    assert_eq!(r.slot("selected"), Some(&Value::str("right")));
}

#[test]
fn content_sized_composer_grows_wraps_and_stops_at_its_maximum() {
    let source = r#"component App
  state draft = ""
  action write(value) writes draft
    draft = value
  view
    column width=200 height=400 testId="root"
      textarea value=draft field-sizing="fixed" width=70 testId="fixed-composer"
      textarea value=draft change=write field-sizing="content" font-size=16 line-height=20 width=160 min-height=28 max-height=88 testId="composer"
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Schedule,
        Kernel::with_monospace(),
    )
    .unwrap();
    let root = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("root")[0])
        .unwrap()
        .id;
    let input = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("composer")[0])
        .unwrap()
        .id;
    let height = |r: &mut Runner<Schedule>| {
        r.kernel_mut()
            .compute_layout(root, exact_kernel::Offer::definite(200.0, 400.0))
            .unwrap();
        r.kernel().node(input).unwrap().frame.height
    };
    let empty = height(&mut r);
    let fixed = r.kernel().find_by_test_id("fixed-composer")[0];
    let fixed_height = r.kernel().node_by_key(fixed).unwrap().frame.height;
    r.dispatch(input, Event::Change("one\ntwo\nthree".into()))
        .unwrap();
    let lines = height(&mut r);
    assert!(lines > empty, "{empty} -> {lines}");
    r.dispatch(
        input,
        Event::Change("A long message that must wrap onto several lines. ".repeat(12)),
    )
    .unwrap();
    assert_eq!(height(&mut r), 88.0);
    assert_eq!(
        r.kernel().node_by_key(fixed).unwrap().frame.height,
        fixed_height
    );
    r.dispatch(input, Event::Change(String::new())).unwrap();
    assert_eq!(height(&mut r), empty);
}

#[test]
fn scroll_events_append_two_numeric_offsets_after_authored_arguments() {
    let src = r#"component App
  state name = ""
  state left = 0
  state top = 0
  action moved(id: string, x, y) writes name, left, top
    name = id
    left = x
    top = y
  view
    scroll height=100 scroll=moved("row") testId="port"
      box width=600 height=600
"#;
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        Schedule,
        Kernel::with_monospace(),
    )
    .unwrap();
    let id = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("port")[0])
        .unwrap()
        .id;
    r.dispatch(id, Event::scroll_payload("12.5,-3.25").unwrap())
        .unwrap();
    assert_eq!(r.slot("name"), Some(&Value::str("row")));
    assert_eq!(r.slot("left"), Some(&Value::Number(12.5)));
    assert_eq!(r.slot("top"), Some(&Value::Number(-3.25)));
    for bad in ["NaN,0", "0,inf", "0", "1,2,3", "bad,2"] {
        assert!(Event::scroll_payload(bad).is_none());
    }
    for params in [
        "id: string, x: string, y: number",
        "id: string, x: number, y: bool",
    ] {
        let bad = src.replace("id: string, x, y", params);
        assert_eq!(
            contract::compile(&bad).unwrap_err().id,
            "type-handler-payload"
        );
    }
    assert_eq!(
        contract::compile(&src.replace("scroll=moved(\"row\")", "scroll=moved"))
            .unwrap_err()
            .id,
        "analyze-handler-arity"
    );
}

#[test]
fn native_swipe_bindings_keep_authored_ids_through_plan_roundtrip_and_updates() {
    let src = r#"component App
  state alternate = false
  action choose writes alternate
    alternate = not alternate
  view
    scroll swipeContent="row" swipeLeading=(alternate ? "second" : "first") swipeTrailing="delete" width=300 height=80
      row
        button id="first" press=choose aria-label="First" width=50 height=50
        button id="second" press=choose aria-label="Second" width=50 height=50
        button id="row" press=choose width=300 height=80 testId="row"
          text "Row"
        button id="delete" press=choose aria-label="Delete" swipeDestructive=true testId="delete" width=50 height=50
"#;
    let plan = contract::compile(src).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(plan, Schedule, Kernel::with_monospace()).unwrap();
    let row = r.kernel().find_by_test_id("row")[0];
    let id = r.kernel().node_by_key(row).unwrap().id;
    let root = r.kernel().roots()[0];
    assert_eq!(
        r.kernel()
            .node(root)
            .unwrap()
            .props
            .str(PropId::SwipeContent),
        Some("row")
    );
    assert_eq!(
        r.kernel()
            .node(root)
            .unwrap()
            .props
            .str(PropId::SwipeLeading),
        Some("first")
    );
    r.dispatch(id, Event::Press).unwrap();
    assert_eq!(
        r.kernel()
            .node(root)
            .unwrap()
            .props
            .str(PropId::SwipeLeading),
        Some("second")
    );
    let delete = r.kernel().find_by_test_id("delete")[0];
    assert_eq!(
        r.kernel()
            .node_by_key(delete)
            .unwrap()
            .props
            .bool(PropId::SwipeDestructive),
        Some(true)
    );
}
