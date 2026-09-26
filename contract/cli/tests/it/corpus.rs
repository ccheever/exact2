//! The corpus (LLP 1004 D6): every accept fixture compiles byte-identically,
//! round-trips, and runs; every reject fixture is refused with exactly its id.

use exact_kernel::{Dimension, Kernel, NodeType, PropId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

#[test]
fn symbols_admit_roles_and_refuse_platform_names_or_misspellings() {
    for role in [
        "back",
        "close",
        "compose",
        "add",
        "microphone",
        "send",
        "search",
        "copy",
        "select-text",
        "more",
        "delete",
        "forward",
        "home",
        "person",
        "messages",
        "notifications",
        "settings",
        "repeat",
        "activity",
        "bookmark",
        "bookmark-fill",
        "document",
        "select",
        "select-fill",
        "reorder",
        "sort",
        "filter",
    ] {
        contract::compile(&format!("component App\n  view\n    image \"symbol:{role}\" tint-color=\"light-dark(#123456,#abcdef)\"\n")).unwrap();
    }
    contract::compile("component App\n  state selected = true\n  view\n    button role=\"tab\" aria-selected=selected\n      text \"Questions\"\n").unwrap();
    for role in [
        "",
        "chevron.backward",
        "sf/plus",
        "Search",
        "serach",
        "bookmark.fill",
        "checkmark-circle",
    ] {
        let error = contract::compile(&format!(
            "component App\n  view\n    image \"symbol:{role}\"\n"
        ))
        .unwrap_err();
        assert!(error.to_string().contains("not a role"), "{error}");
    }
    // A dynamic source is checked by the host after it resolves.
    contract::compile(
        "component App\n  state source = \"symbol:unknown\"\n  view\n    image source\n",
    )
    .unwrap();
}

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
        Default::default(),
        "/",
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

#[test]
fn a_style_attributes_branches_may_mix_a_length_and_a_keyword() {
    let plan = contract::compile(&corpus("style-branches.contract")).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = r.kernel().find_by_test_id("sheet")[0];
    let style = |r: &Runner<Schedule>| r.kernel().node_by_key(key).unwrap().style.clone();
    let closed = style(&r);
    assert_eq!(closed.margin_top, Dimension::Auto);
    assert_eq!(closed.width, Dimension::Percent(50.0));
    assert_eq!(closed.height, Dimension::Auto);
    r.act("set", vec![Value::Bool(true)]).unwrap();
    let open = style(&r);
    assert_eq!(open.margin_top, Dimension::Points(-14.5));
    assert_eq!(open.width, Dimension::Points(100.0));
    assert_eq!(open.height, Dimension::Points(200.0));
    // Only a style row is one value space: a prop, a state, or a derive
    // still needs one type.
    for (declarations, attr) in [
        ("", "testId=(open ? 1 : \"a\")"),
        ("  derive top = open ? -14.5 : \"auto\"\n", "margin-top=top"),
        ("", "margin-top=(open ? 1 : true)"),
    ] {
        let source = format!(
            "component App\n  state open = false\n{declarations}  view\n    text \"a\" {attr}\n"
        );
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "type-branches", "{source}");
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
            Ok(plan) if id.starts_with("bake-") => {
                let error = contract::bake(plan, Schedule).unwrap_err();
                assert!(error.to_string().starts_with(&format!("[{id}]")), "{error}");
            }
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
        assert_eq!(error.span, contract_syntax::Span::point(1, 1));
    }
}

#[test]
fn a_template_with_an_inline_match_runs_through_the_compiler() {
    let src = "component App\n  state choice = some(\"yes\")\n  view\n    text `${match choice { case some(value) => value, case none => \"}\" }}` testId=\"matched\"\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "matched").as_deref(), Some("yes"));
}

#[test]
fn state_initializers_keep_earlier_bindings_after_local_shadowing() {
    let src = r#"component App
  state value = 7
  state wrapped = some(value)
  state next = match wrapped { case some(value) => value + 1, case none => value }
  state again = value + next
  view
    text `${value} ${next} ${again}` testId="values"
"#;
    let r = Runner::boot(
        contract::compile(src).unwrap(),
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "values").as_deref(), Some("7 8 15"));
}

#[test]
fn state_initializers_refuse_later_names_and_leaked_locals() {
    for (declarations, id, line) in [
        (
            "  state next = later\n  state later = 1\n",
            "type-unknown-name",
            2,
        ),
        ("  state own = own\n", "type-unknown-name", 2),
        (
            "  state value = 1\n  state value = 2\n",
            "type-duplicate-name",
            3,
        ),
        (
            "  state value = match some(1) { case some(local) => local, case none => 0 }\n  state leaked = local\n",
            "type-unknown-name",
            3,
        ),
    ] {
        let source = format!("component App\n{declarations}  view\n    text \"value\"\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, id, "{source}");
        assert_eq!(error.span.line, line, "{source}");
    }
}

#[test]
fn button_primary_text_is_a_real_accessible_text_child() {
    let src = "component App\n  state pressed = false\n  action press writes pressed\n    pressed = true\n  view\n    button \"Post\" press=press testId=\"post\"\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
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

/// A `button` is a pressable `column` (Charlie, 2026-09-23: "One native
/// button, flex column"; LLP 1006 §3): its two rows are fixed, and an
/// author's own row wins, as on any tag.
#[test]
fn a_button_is_a_pressable_column_whose_rows_an_author_overrides() {
    let src = "component App\n  state n = 0\n  action bump writes n\n    n = n + 1\n  view\n    column\n      button \"Save\" press=bump testId=\"save\"\n      button press=bump flex-direction=\"row\" testId=\"row\"\n        text \"Row\"\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let node = |id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        r.kernel().node_by_key(key).unwrap()
    };
    let save = node("save");
    assert_eq!(save.node_type, NodeType::Pressable);
    assert_eq!(save.style.display, exact_kernel::Display::Flex);
    assert_eq!(
        save.style.flex_direction,
        exact_kernel::FlexDirection::Column
    );
    let row = node("row");
    assert_eq!(row.style.display, exact_kernel::Display::Flex);
    assert_eq!(row.style.flex_direction, exact_kernel::FlexDirection::Row);
}

#[test]
fn every_handler_kind_types_and_untyped_payloads_are_inferred() {
    let src = "component App\n  state textValue = \"\"\n  state boolValue = false\n  action noPayload\n  action stringPayload(value) writes textValue\n    textValue = value\n  action boolPayload(value) writes boolValue\n    boolValue = value\n  view\n    column\n      button \"press\" press=noPayload\n      input change=stringPayload key=stringPayload hover=boolPayload focus=noPayload blur=noPayload submit=noPayload\n      iframe \"/guest\" load=noPayload message=stringPayload\n";
    contract::compile(src).unwrap();
}

#[test]
fn dynamic_invalid_integer_props_are_unset_and_journaled_at_boot() {
    for value in ["1.5", "9223372036854775808"] {
        let src = format!(
            "component App\n  state level = {value}\n  view\n    column aria-level=level\n      text \"heading\"\n"
        );
        let plan = contract::compile(&src).unwrap();
        // CSS's invalid value at computed-value time: the row is unset.
        let r = Runner::boot(
            plan,
            Schedule,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let column = r.kernel().node(r.roots()[0]).unwrap();
        assert!(column
            .props
            .iter()
            .all(|(prop, _)| !prop.name().to_lowercase().contains("level")));
        assert!(r
            .journal()
            .any(|l| l.contains("invalid") && l.to_lowercase().contains("level")));
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

    let mut r = Runner::boot(
        baked,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // @ref LLP 1038 D5 — bake records exactly the arguments this unchanged boot evaluates.
    for row in &r.plan().resources {
        let name = r.plan().str(row.name);
        assert_eq!(
            Value::from_bytes(r.plan().bytes(row.initial_args)).unwrap(),
            Value::list(r.resource_args(name).unwrap().to_vec())
        );
    }
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
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
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
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
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
      textarea value=draft change=write field-sizing="content" font-size=16 line-height="20px" width=160 min-height=28 max-height=88 testId="composer"
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
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
        Default::default(),
        "/",
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
        button id="delete" press=choose aria-label="Delete" destructive=alternate testId="delete" width=50 height=50
"#;
    let plan = contract::compile(src).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
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
            .bool(PropId::Destructive),
        Some(true)
    );
    r.dispatch(id, Event::Press).unwrap();
    assert_eq!(
        r.kernel()
            .node_by_key(delete)
            .unwrap()
            .props
            .bool(PropId::Destructive),
        Some(false)
    );
}

#[test]
fn css_line_height_literals_and_dynamic_lengths_use_the_existing_value_grammar() {
    for value in ["1.5", "0", "\"0px\"", "\"24px\"", "\"normal\""] {
        contract::compile(&format!(
            "component App\n  view\n    text \"hello\" line-height={value}\n"
        ))
        .unwrap();
    }
    for value in [
        "-1",
        "\"-2px\"",
        "\"1em\"",
        "\"150%\"",
        "\"NaNpx\"",
        "\"24\"",
    ] {
        assert!(
            contract::compile(&format!(
                "component App\n  view\n    text \"hello\" line-height={value}\n"
            ))
            .is_err(),
            "{value}"
        );
    }
    contract::compile(
        r#"component App
  state height = 24
  state natural = false
  view
    text "hello" line-height=(natural ? "normal" : `${height}px`)
"#,
    )
    .unwrap();
}

#[test]
fn inlined_literal_string_templates_cost_the_same_as_literal_text() {
    let source = |body: &str| format!("component App\n  view\n    {body}\n");
    for (template, literal) in [
        (r#"`hello ${"世界"}!`"#, r#""hello 世界!""#),
        (r#"`${""}${""}`"#, r#""""#),
        (r#"``"#, r#""""#),
    ] {
        assert_eq!(
            contract::compile(&source(&format!("text {template}")))
                .unwrap()
                .encode(),
            contract::compile(&source(&format!("text {literal}")))
                .unwrap()
                .encode()
        );
    }
    let reused = r#"component App
  view
    Label(prefix="reply-typing")
component Label
  props
    prefix: string
  view
    text "dot" testId=`${prefix}-dot-0`
"#;
    let literal = source(r#"text "dot" testId="reply-typing-dot-0""#);
    assert_eq!(
        contract::compile(reused).unwrap().encode(),
        contract::compile(&literal).unwrap().encode()
    );
}

#[test]
fn template_folding_preserves_dynamic_values_and_string_conversion() {
    let source = r#"component App
  state name = "one"
  state n = 2
  action change writes name, n
    name = "two"
    n = 3
  view
    Label(prefix="前", name=name, n=n)
component Label
  props
    prefix: string
    name: string
    n: number
  view
    text `${prefix}:${name}:${n}:${true}` testId="result"
"#;
    let mut runner = Runner::boot(
        contract::compile(source).unwrap(),
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for expected in ["前:one:2:true", "前:two:3:true"] {
        let key = runner.kernel().find_by_test_id("result")[0];
        assert_eq!(
            runner
                .kernel()
                .node_by_key(key)
                .unwrap()
                .props
                .str(PropId::Text),
            Some(expected)
        );
        runner.act("change", vec![]).unwrap();
    }
}

#[test]
fn the_motion_fixtures_after_task_fires_once_and_its_timer_is_then_spent() {
    let plan = contract::compile(&corpus("motion.contract")).unwrap();
    assert_eq!(plan.timers.len(), 2);
    assert!(!plan.timers[0].once && plan.timers[1].once);
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "launch").as_deref(), Some("Launching"));
    assert_eq!(r.timer_due_ms(), Some(60.0));
    assert_eq!(r.advance(60.0).unwrap().len(), 1);
    assert_eq!(text_of(&r, "launch").as_deref(), Some("Ready"));
    // Spent: only the repeating ticker's deadline remains.
    assert_eq!(r.timer_due_ms(), Some(1_000.0));
    assert_eq!(r.advance(120.0).unwrap().len(), 0);
    assert_eq!(r.advance(180.0).unwrap().len(), 0);
    assert_eq!(r.advance(2_500.0).unwrap().len(), 2);
}

#[test]
fn a_child_binder_never_captures_a_name_its_parent_passes_in() {
    let boot = |name: &str| {
        let plan = contract::compile(&corpus(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let plan = Plan::decode(&plan.encode()).unwrap();
        Runner::boot(
            plan,
            Schedule,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    };
    let r = boot("capture-match.contract");
    assert_eq!(text_of(&r, "viaexpr").as_deref(), Some("parent-x"));
    assert_eq!(text_of(&r, "vianode").as_deref(), Some("parent-x"));
    let r = boot("capture-derive.contract");
    assert_eq!(text_of(&r, "viaderive").as_deref(), Some("parent-x"));
    // A root `count__1` and the lifted state of `Child`'s `count` are two slots.
    let mut r = boot("capture-lifted.contract");
    assert_eq!(text_of(&r, "root").as_deref(), Some("100"));
    assert_eq!(text_of(&r, "child").as_deref(), Some("5"));
    let key = r.kernel().find_by_test_id("bump")[0];
    let bump = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(bump, Event::Press).unwrap();
    assert_eq!(text_of(&r, "root").as_deref(), Some("100"));
    assert_eq!(text_of(&r, "child").as_deref(), Some("6"));
}

#[test]
fn roster_calls_the_runner_cannot_answer_are_refused_while_checking() {
    let app = |expr: &str| {
        format!("shape Station\n  id: string\n  name: string\ncomponent App\n  resource items = stations() as shape list<Station>\n  view\n    text toString({expr}) testId=\"out\"\n")
    };
    for (expr, expects) in [
        (
            "length(5)",
            "argument 1 of `length` expects `string | list`, given `number`",
        ),
        (
            "isEmpty(true)",
            "argument 1 of `isEmpty` expects `string | list`, given `bool`",
        ),
        (
            "toString(items)",
            "argument 1 of `toString` expects `number | string | bool`, given `list<Station>`",
        ),
    ] {
        let error = contract::compile(&app(expr)).unwrap_err();
        assert_eq!(
            (error.id.as_str(), error.message.as_str()),
            ("type-argument", expects)
        );
    }
    let error = contract::compile(&app("length()")).unwrap_err();
    assert!(
        error.message.ends_with("expected `length(string | list)`"),
        "{error}"
    );
    for (expr, shown) in [
        ("length(\"héllo\")", "5"),
        ("length(items)", "2"),
        ("isEmpty(\"\")", "true"),
        ("toString(true)", "true"),
    ] {
        let r = Runner::boot(
            contract::compile(&app(expr)).unwrap(),
            Schedule,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(text_of(&r, "out").as_deref(), Some(shown), "{expr}");
    }
}

#[test]
fn chains_of_derives_and_functions_compile_in_linear_size() {
    let derives = |n: usize| {
        let mut src = String::from("component App\n  state a3 = 100\n  view\n    main\n      Child(x=1, y=a3)\ncomponent Child\n  props\n    x: number\n    y: number\n  derive a0 = x + x\n");
        for i in 1..n {
            src.push_str(&format!("  derive a{i} = a{} + a{}\n", i - 1, i - 1));
        }
        src + &format!(
            "  derive last = a{} + y\n  view\n    text `${{last}}` testId=\"out\"\n",
            n - 1
        )
    };
    let fns = |n: usize| {
        let mut src = String::from("fn f0(x: number): number = x + x\n");
        for i in 1..n {
            src.push_str(&format!(
                "fn f{i}(x: number): number = f{}(x) + f{}(x)\n",
                i - 1,
                i - 1
            ));
        }
        src + &format!("component App\n  state v = 1\n  view\n    main\n      text `${{f{}(v)}}` testId=\"out\"\n", n - 1)
    };
    for (source, n, shown) in [
        (derives(16), 16, "65636"),
        (derives(32), 32, "4294967396"),
        (fns(16), 16, "65536"),
        (fns(32), 32, "4294967296"),
    ] {
        let plan = contract::compile(&source).unwrap();
        // Each link adds a bounded number of bytes; a copy would double them.
        assert!(
            plan.code.len() < 40 * n,
            "{n}: {} code bytes",
            plan.code.len()
        );
        let r = Runner::boot(
            plan,
            Schedule,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        // The parent's `a3` (100) reaches `last` through `y`, past the child's own `a3`.
        assert_eq!(text_of(&r, "out").as_deref(), Some(shown));
    }
}

#[test]
fn deep_expressions_are_refused_by_name_on_a_small_stack() {
    let app = |derive: String| {
        format!("component App\n  state n = 1\n  derive d = {derive}\n  view\n    main\n      text `${{d}}` testId=\"out\"\n")
    };
    let chain = |terms: usize| vec!["n"; terms].join(" + ");
    let nested = |depth: usize| format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    let cases = [
        (app(chain(100)), None),
        (app(nested(60)), None),
        (app(chain(101)), Some("more than 100 operations deep")),
        (app(chain(200_000)), Some("more than 100 operations deep")),
        (app(nested(100_000)), Some("nest more than 64")),
        (
            app(format!("{}n", "-".repeat(100_000))),
            Some("nest more than 64"),
        ),
        (
            app(format!("some({})", nested(70))),
            Some("nest more than 64"),
        ),
    ];
    // Every pass recurses over the tree; all of it fits a 2 MB thread, as
    // the test harness gives, even unoptimized.
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            for (source, refused) in cases {
                match (contract::compile(&source), refused) {
                    (Ok(_), None) => {}
                    (Err(e), Some(message)) => {
                        assert_eq!(e.id, "syntax-expression-depth");
                        assert!(e.message.contains(message), "{e}");
                    }
                    (result, _) => panic!("{:?}", result.map(|_| ()).map_err(|e| e.to_string())),
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_shared_child_value_is_computed_only_where_its_readers_computed_it() {
    let plan = contract::compile(&corpus("lazy-derive.contract")).unwrap();
    let mut r = Runner::boot(
        plan,
        Schedule,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .expect("no reader evaluates the refused path");
    for (id, shown) in [
        ("branch", "fallback"),
        ("through", "through fallback"),
        ("short", "false"),
        ("arm", "arm fallback"),
        ("fn", "fn fallback"),
    ] {
        assert_eq!(text_of(&r, id).as_deref(), Some(shown), "{id}");
    }
    // Once the path is valid and every reader takes its branch, each reads
    // the value twice, computed once.
    let key = r.kernel().find_by_test_id("arrive")[0];
    let arrive = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(arrive, Event::Press).unwrap();
    for id in ["branch", "through", "arm", "fn"] {
        assert_eq!(text_of(&r, id).as_deref(), Some("/post/7/post/7"), "{id}");
    }
    assert_eq!(text_of(&r, "short").as_deref(), Some("true"));
}
