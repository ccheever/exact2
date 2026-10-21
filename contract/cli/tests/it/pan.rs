//! @ref LLP 1043.000 §3 D8 — pan is an ordinary typed action, not motion ownership.
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
#[test]
fn pan_commits_curried_deltas_and_refuses_nonfinite_before_state_changes() {
    let plan = contract::compile(
        r#"component App
  state x = 0
  state y = 0
  action move(factor: number, dx: number, dy: number)
    x = x + dx * factor
    y = y + dy * factor
  view
    box testId="pan" pan=move(2) left=x top=y
"#,
    )
    .unwrap();
    let mut runner = Runner::boot(
        plan,
        Empty,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id = runner
        .kernel()
        .arena()
        .iter_live()
        .find_map(|slot| {
            let n = runner
                .kernel()
                .node_by_key(runner.kernel().arena().key(slot))?;
            (n.props.str(exact_kernel::PropId::TestId) == Some("pan")).then_some(n.id)
        })
        .unwrap();
    runner
        .dispatch(id, Event::pan_payload("120,-40").unwrap())
        .unwrap();
    let before = exact_runner::agent::state(&runner);
    assert!(before.contains("240"));
    assert!(before.contains("-80"));
    for value in [f64::INFINITY, f64::NAN, f64::MAX] {
        assert!(runner.dispatch(id, Event::Pan(value, 1.)).is_err());
    }
    assert_eq!(exact_runner::agent::state(&runner), before);
    for payload in ["NaN,0", "0,inf", "1,2,3", "1e39,0"] {
        assert!(Event::pan_payload(payload).is_none());
    }
}
#[test]
fn pan_requires_two_numeric_payload_parameters_including_component_actions() {
    for params in ["dx: number", "dx: string, dy: number"] {
        let src=format!("component App\n  state x = 0\n  action move({params})\n    x = 1\n  view\n    box pan=move\n");
        assert!(contract::compile(&src).is_err());
    }
}

/// `rows()` answers two keys; nothing else answers.
struct Rows;
impl DataSource for Rows {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        match name {
            "rows" => Ok(Value::list(vec![Value::str("a"), Value::str("b")])),
            _ => Err(DataError::UnknownSource(name.into())),
        }
    }
}

/// Builds `src`, sends `event` to the node `testId="go"`, and returns the
/// state the agent reads.
fn drive(src: &str, event: Event) -> String {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}\n{src}"));
    let mut runner = Runner::boot(
        plan,
        Rows,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = runner.kernel().find_by_test_id("go")[0];
    let id = runner.kernel().node_by_key(key).unwrap().id;
    runner.dispatch(id, event).unwrap();
    exact_runner::agent::state(&runner)
}

/// What the type checker made of the root's `action`'s parameters.
fn params(src: &str, action: &str) -> Vec<contract_types::Ty> {
    let file = contract_syntax::parse(src).unwrap();
    let checked = contract_types::check(&file, contract_lower::tags::style)
        .unwrap_or_else(|e| panic!("{e:?}\n{src}"));
    let ai = (file.components[0].actions.iter())
        .position(|a| a.name == action)
        .unwrap();
    checked.types.components[0].actions[ai].clone()
}

/// A child's action calls its parent component's action through an action
/// prop (LLP 1089 D7). That value carries the parent's props as hidden
/// captures, which `pan=` types from what it binds, as `press=` does.
const PANEL: &str = r#"component App
  view
    main padding=16
      Panel(title="Panel")

component Panel
  props
    title: string
  state last = ""
  action note(label: string)
    last = `${title} ${label}`
  view
    column
      text last
      Leaf(label="a", tell=note)

component Leaf
  props
    label: string
    tell: action
  action drag(dx: number, dy: number)
    tell(`${label} ${dx}`)
  view
    box pan=drag width=100 height=40 testId="go"
"#;

#[test]
fn a_pan_action_calls_an_action_prop_that_carries_captures() {
    let pan = Event::Pan(3., 4.);
    assert!(drive(PANEL, pan.clone()).contains("\"Panel a 3\""));
    // Two components down: `Leaf` hands `drag("k")` to `Grip`'s `pan`.
    let grip = PANEL
        .replace(
            "action drag(dx: number, dy: number)\n    tell(`${label} ${dx}`)",
            "action drag(k: string, dx: number, dy: number)\n    tell(`${label} ${k} ${dx}`)",
        )
        .replace(
            "    box pan=drag width=100 height=40 testId=\"go\"\n",
            "    Grip(move=drag(\"k\"))\n\ncomponent Grip\n  props\n    move: action\n  view\n    box pan=move width=100 height=40 testId=\"go\"\n",
        );
    assert!(drive(&grip, pan.clone()).contains("\"Panel a k 3\""));
    // The root's own action, curried where it is handed down.
    let curried = r#"component App
  state last = ""
  action note(pre: string, label: string)
    last = `${pre} ${label}`
  view
    main padding=16
      text last
      Leaf(label="a", tell=note("pre"))

component Leaf
  props
    label: string
    tell: action
  action drag(dx: number, dy: number)
    tell(label)
  view
    box pan=drag width=100 height=40 testId="go"
"#;
    assert!(drive(curried, pan).contains("\"pre a\""));
}

#[test]
fn pan_types_its_actions_untyped_parameters() {
    use contract_types::Ty::{Number, String};
    let root = |action: &str, binding: &str| {
        format!("component App\n  state n = 0\n  action {action}\n    n = n + 1\n  view\n    box pan={binding} width=100 height=40\n")
    };
    assert_eq!(
        params(&root("drag(x, y)", "drag"), "drag"),
        [Number, Number]
    );
    assert_eq!(
        params(
            &root("drag(k, x: number, y: number)", "drag(\"k\")"),
            "drag"
        ),
        [String, Number, Number]
    );
}

/// A handler whose payload does not fit its action's count is refused at
/// the binding with analysis's arity refusal, whether the action's
/// parameters are typed or not: analysis never runs after the type stage
/// refuses, so the type stage says it for an action the miscount leaves
/// untyped, ahead of a body that reads such a parameter.
#[test]
fn a_miscounted_handler_is_refused_at_its_binding_typed_or_not() {
    let all = |src: &str| {
        contract::compile_path_source_all(std::path::Path::new("app.contract"), src, false)
            .map(|_| ())
            .expect_err(src)
    };
    let app = |actions: &str, view: &str| {
        format!("component App\n  state n = 0\n  resource items = rows() as shape string[]\n{actions}  view\n    {view}\n")
    };
    let reorder = "list id=\"arrange\" virtualized=true height=200 reorderdrop=drop\n      for x in items key=x\n        column reorderFor=\"arrange\"\n          text x";
    let cases = [
        // (untyped, typed, view, the binding's column)
        (
            "act(a, b: number)",
            "act(a: string, b: number)",
            "input change=act",
            11,
        ),
        (
            "act(k, extra: number)",
            "act(k: string, extra: number)",
            "input key=act",
            11,
        ),
        (
            "rel(a, vx, vy: string)",
            "rel(a: number, vx: number, vy: string)",
            "box panrelease=rel",
            9,
        ),
        ("drag(x)", "drag(x: number)", "box pan=drag", 9),
        (
            "drop(item, before, e: number)",
            "drop(item: string, before: option<string>, e: number)",
            reorder,
            51,
        ),
    ];
    for (untyped, typed, view, col) in cases {
        let typed = all(&app(&format!("  action {typed}\n    n = 1\n"), view));
        let src = app(&format!("  action {untyped}\n    n = 1\n"), view);
        let untyped = all(&src);
        assert_eq!(untyped.len(), 1, "{src}\n{untyped:#?}");
        assert_eq!(
            untyped[0].id, "analyze-handler-arity",
            "{src}\n{untyped:#?}"
        );
        assert_eq!(
            (untyped[0].span.line, untyped[0].span.col),
            (7, col),
            "{src}"
        );
        assert_eq!(untyped[0].message, typed[0].message, "{src}");
    }
    let reads = app(
        "  action act(a, b: number)\n    n = a.x\n",
        "input change=act",
    );
    let reads = all(&reads);
    assert_eq!(reads[0].id, "analyze-handler-arity", "{reads:#?}");
    let drop = all(&app(
        "  action drop(item, before, e: number)\n    n = 1\n",
        reorder,
    ));
    assert_eq!(
        drop[0].message,
        "`reorderdrop=drop` calls `drop` with `item` (string), `before` (option<string>); declare `action drop(item: string, before: option<string>)`, or `action drop(item: string, before: option<string>, event: ReorderEvent)` for its `ReorderEvent`"
    );
}

/// The other drags and the drop: each types its action as `pan` does, and
/// a child's action bound to it calls its parent component's action.
#[test]
fn the_drags_and_the_drop_type_their_actions_as_pan_does() {
    use contract_types::Ty::{self, Number};
    let key = Ty::String;
    let before = Ty::Option(Box::new(Ty::String));
    let height = "column heightDragFor=\"sheet\" heightrelease=release testId=\"go\"\n";
    let transform = "column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release testId=\"go\"\n";
    let reorder = "list id=\"arrange\" virtualized=true height=200 reorderdrop=drop testId=\"go\"\n      for x in items key=x\n        column reorderFor=\"arrange\"\n          text x\n";
    // The root's untyped parameters take the payload's types.
    let root = |actions: &[&str], view: &str| {
        let actions: std::string::String = (actions.iter())
            .map(|a| format!("  action {a}\n    n = n + 1\n"))
            .collect();
        format!("component App\n  state n = 0\n  resource items = rows() as shape string[]\n{actions}  view\n    {view}")
    };
    let src = root(&["release(height, velocity)"], height);
    assert_eq!(params(&src, "release"), [Number, Number]);
    let src = root(
        &[
            "geometry(bw, bh, pw, ph)",
            "release(x, y, scale, vx, vy, vs)",
        ],
        transform,
    );
    assert_eq!(params(&src, "geometry"), vec![Number; 4]);
    assert_eq!(params(&src, "release"), vec![Number; 6]);
    let src = root(&["drop(item, before)"], reorder);
    assert_eq!(params(&src, "drop"), [key, before]);
    // A child's action calls `Owner`'s `note` through `tell`, carrying
    // `Owner`'s `title` as a hidden capture.
    let owned = |actions: &[&str], view: &str| {
        let actions: std::string::String = (actions.iter())
            .map(|a| format!("  action {a}\n    tell(label)\n"))
            .collect();
        format!("component App\n  resource rows = rows() as shape string[]\n  view\n    column\n      Owner(title=\"t\", items=rows)\ncomponent Owner\n  props\n    title: string\n    items: string[]\n  state last = \"\"\n  action note(label: string)\n    last = `${{title}} ${{label}}`\n  view\n    column\n      Grip(label=\"g\", items=items, tell=note)\ncomponent Grip\n  props\n    label: string\n    items: string[]\n    tell: action\n{actions}  view\n    {view}")
    };
    let src = owned(&["release(height: number, velocity: number)"], height);
    let event = Event::HeightRelease {
        height: 200.,
        velocity: 0.,
    };
    assert!(drive(&src, event).contains("\"t g\""), "{src}");
    let src = owned(
        &[
            "geometry(bw: number, bh: number, pw: number, ph: number)",
            "release(x: number, y: number, scale: number, vx: number, vy: number, vs: number)",
        ],
        transform,
    );
    let event = Event::TransformGeometry {
        box_width: 10.,
        box_height: 10.,
        port_width: 20.,
        port_height: 20.,
    };
    assert!(drive(&src, event).contains("\"t g\""), "{src}");
    let event = Event::TransformRelease {
        x: 0.,
        y: 0.,
        scale: 1.,
        vx: 0.,
        vy: 0.,
        vscale: 0.,
    };
    assert!(drive(&src, event).contains("\"t g\""), "{src}");
    let src = owned(&["drop(item: string, before: option<string>)"], reorder);
    let event = Event::ReorderDrop {
        item: "a".into(),
        before: None,
    };
    assert!(drive(&src, event).contains("\"t g\""), "{src}");
}
