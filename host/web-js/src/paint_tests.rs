//! The emitted program, both server documents and adoption run in Chrome.
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::{builder::PlanBuilder, BindingKind, BindingsRow, Plan, Value};
use std::collections::BTreeMap;

fn bun_command() -> std::process::Command {
    std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
}

#[cfg(unix)]
#[test]
fn bun_command_uses_the_named_binary() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("exact-bun-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("bun");
    let marker = dir.join("invoked");
    std::fs::write(
        &stub,
        "#!/bin/sh\nprintf invoked > \"$EXACT_BUN_MARKER\"\nexec \"$EXACT_BUN_REAL\" \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "paint_tests::dynamic_backdrop_grammar_agrees_with_the_kernel",
            "--nocapture",
        ])
        .env("BUN", &stub)
        .env(
            "EXACT_BUN_REAL",
            std::env::var_os("BUN").unwrap_or_else(|| "bun".into()),
        )
        .env("EXACT_BUN_MARKER", &marker)
        .output()
        .unwrap();
    let invoked = marker.exists();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(invoked, "the BUN executable was not invoked");
}

#[test]
fn native_field_templates_keep_the_marker_for_runtime_and_rendering() {
    let plan = contract::compile(include_str!("../conformance/native-fields.contract")).unwrap();
    let sites = crate::emit::Sites::new(&plan).unwrap();
    let parts = crate::style::project(&plan, &sites, &mut Vec::new()).unwrap();
    let mut fields = 0;
    for part in parts.into_iter().flatten() {
        let Some(name) = part.props.get("data-testid") else {
            continue;
        };
        fields += 1;
        assert_eq!(
            part.props.contains_key("data-native"),
            !matches!(name.as_str(), "bare" | "devolved" | "markdown"),
            "{name}"
        );
    }
    assert_eq!(fields, 13);
    let emitted = crate::emit::emit(&plan, false, false).unwrap();
    assert!(emitted.js.contains("data-native"));
}

#[test]
fn dynamic_backdrop_grammar_agrees_with_the_kernel() {
    use exact_kernel::style::BackdropFilter;
    exact_kernel::style::link_backdrop_filter();
    let cases: Vec<_> = [
        "none",
        "blur()",
        "blur(0)",
        "blur(+0e5)",
        "blur(2.5PX)",
        "saturate()",
        "saturate(0)",
        "saturate(180%)",
        "SATURATE(+1.14)",
        "blur(12px) saturate(1.14)",
        "saturate(1.8) blur(4px)",
        "saturate(-1)",
        "saturate(-1e-50)",
        "saturate(1.)",
        "saturate(1 %)",
        "saturate(2px)",
        "saturate(NaN)",
        "saturate(inf)",
        "saturate(1e100)",
        "saturate(1) saturate(2)",
        "blur(1px) blur(2px)",
        "blur(1e-100)",
        "blur(3.4028235e38px)",
        "blur(1.)",
        "blur(-1px)",
        "brightness(1.2)",
        "none saturate(1)",
        "saturate (1)",
        "saturate(1) none",
        "",
        "4",
    ]
    .into_iter()
    .map(|css| match BackdropFilter::check(css) {
        Ok(value) => (css, Some(value.css()), None),
        Err(reason) => (css, None, Some(reason)),
    })
    .collect();
    let output = bun_command()
        .args(["-e", "const {backdropValue}=await import('./backdrop.js');for(const [input,expected,reason] of JSON.parse(process.argv[1])){let why=null;const actual=backdropValue(input,n=>why=n);if(actual!==expected||why!==reason)throw Error(JSON.stringify({input,expected,actual,reason,why}));}"])
        .arg(serde_json::to_string(&cases).unwrap())
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture_plan(root: &str, nodes: &str) -> Plan {
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let mut ids = BTreeMap::new();
    let nodes = nodes.split(" & ").map(|node| {
        let mut parts = node.splitn(3, '>');
        (
            parts.next().unwrap().parse::<u32>().unwrap(),
            parts.next().unwrap().parse::<u32>().unwrap(),
            parts.next().unwrap(),
        )
    });
    for (id, parent, css) in std::iter::once((1, 0, root)).chain(nodes) {
        let mut bindings: Vec<_> = css
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|decl| {
                let (name, value) = decl.split_once(':').unwrap();
                let row = match name {
                    "position" => StyleId::PositionType,
                    "z-index" => StyleId::ZIndex,
                    "margin-top" => StyleId::MarginTop,
                    "margin-left" => StyleId::MarginLeft,
                    name => StyleId::from_name(name)
                        .unwrap_or_else(|| panic!("unknown fixture row {name}")),
                };
                let value = value
                    .trim_end_matches("px")
                    .parse()
                    .map(Value::Number)
                    .unwrap_or_else(|_| Value::str(value));
                BindingsRow {
                    kind: BindingKind::Style,
                    id: row as u16,
                    expr: builder.constant(&value),
                    written: true,
                }
            })
            .collect();
        bindings.push(BindingsRow {
            kind: BindingKind::Prop,
            id: PropId::TestId as u16,
            expr: builder.constant(&Value::str(&format!("n{id}"))),
            written: true,
        });
        let node = builder.node(
            NodeType::View as u8,
            ids.get(&parent).copied(),
            None,
            id,
            &bindings,
            &[],
            None,
        );
        ids.insert(id, node);
    }
    builder.finish().unwrap()
}

#[test]
fn tvos_focus_guides_are_ignored_on_the_web_even_when_bound() {
    for guide in ["", "focusGuide=\"auto\"", "focusGuide=guide"] {
        let plan = contract::compile(&format!(
            "component FocusGuide\n  state guide = \"auto\"\n  view\n    column {guide}\n      button \"Focusable\" testId=\"control\"\n"
        ))
        .unwrap();
        let out = crate::emit::emit(&plan, false, false).unwrap();
        assert!(out.js.contains("control"));
        assert!(!out.js.contains("focusGuide"));
    }
}

#[test]
fn plain_plans_omit_the_mirror_but_keep_root_isolation() {
    for (style, needed) in [
        ("", false),
        ("opacity=1", false),
        ("opacity=(changed ? 0.5 : 1)", true),
        ("position=\"relative\"", true),
        ("z-index=(changed ? 2 : 0)", true),
    ] {
        let plan = contract::compile(&format!(
            "component Paint\n  state changed = false\n  view\n    box\n      box {style}\n"
        ))
        .unwrap();
        let out = crate::emit::emit(&plan, false, false).unwrap();
        assert_eq!(out.js.contains("from\"./paint.js\""), needed, "{style}");
        if !needed {
            assert!(out.css.contains("isolation:isolate"));
            assert!(!out.js.contains("data-exact-f"));
        }
    }
}

#[test]
fn server_client_and_adopted_paint_agree_on_the_chrome_corpus() {
    exact_web::link(exact_web_capabilities::ALL);
    let dir = std::env::temp_dir().join(format!("exact-js-paint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cases = include_str!("../../../kernel/tests/it/fixtures/browser_paint_order.tsv");
    let mut labels = Vec::new();
    for (i, line) in cases.lines().enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        let plan = fixture_plan(fields[1], fields[2]);
        write_case(&dir.join(i.to_string()), &plan, &[]);
        labels.push(fields[0]);
    }
    assert_eq!(labels.len(), 18);
    let dynamic = contract::compile(
        r#"shape Item
  id: number
component Paint
  resource items = items() as shape Item[]
  state changed = false
  derive keys = items.map(x => changed ? 5 - x.id : x.id)
  action toggle
    changed = not changed
  view
    box testId="root"
      button "Toggle" testId="toggle" press=toggle
      button "Native style" testId="native-button" appearance="auto" disabled=true buttonStyle=(changed ? "plain" : "glass")
      box testId="position" position=(changed ? "static" : "relative")
      box testId="holder"
        box testId="fade" opacity=(changed ? 0.5 : 1)
          box position="absolute" z-index=(changed ? -4 : 2)
      box testId="plain"
      box testId="flex" display=(changed ? "flex" : "block")
        box testId="zitem" z-index=(changed ? -2147483648 : 2147483647)
        box testId="item-holder"
          box position="relative"
      box testId="arm-holder"
        if changed
          box position="absolute" z-index=3
      box testId="arm-follower"
      box testId="motion" transition=(changed ? "1s opacity" : "1s color")
      box testId="filter" filter=(changed ? "blur(0px)" : "none")
      box testId="backdrop" backdrop-filter=(changed ? "saturate(1)" : "none")
      box testId="backdrop-pair" backdrop-filter=(changed ? "blur(4px) saturate(1.8)" : "saturate(0) blur(4px)")
      box testId="backdrop-refused" backdrop-filter="saturate(" + (changed ? -1 : 0).toString() + ")"
      box testId="identity" scale=(changed ? 1 : 2)
      box testId="copies"
        for n in keys.filter(n => n != (changed ? 2 : 4)) key=n
          box testId=`row-${n}`
            if n == 1
              box position="relative"
"#,
    )
    .unwrap();
    let dynamic = contract::bake(dynamic, Items).unwrap();
    write_case(
        &dir.join(labels.len().to_string()),
        &dynamic,
        &["toggle", "toggle", "toggle"],
    );
    labels.push("bound facts, arm swaps and keyed `for` edits");
    let long = contract::compile(
        r#"shape Item
  id: number
component Paint
  resource items = items() as shape Item[]
  view
    box
      box testId="waiting-list"
        for item in items key=item.id
          box testId=`row-${item.id}`
            box position="absolute" z-index=2
"#,
    )
    .unwrap();
    let long = contract::bake(long, ManyItems).unwrap();
    write_case(&dir.join(labels.len().to_string()), &long, &[]);
    labels.push("sliced keyed adoption preserves server paint facts");
    compare_cases(&dir, &labels);
}

#[test]
fn waiting_rows_recompute_parent_and_sibling_facts_against_kernel() {
    exact_web::link(exact_web_capabilities::ALL);
    let dir = std::env::temp_dir().join(format!("exact-js-waiting-paint-{}", std::process::id()));
    let mut labels = Vec::new();
    for display in ["block", "flex", "grid"] {
        let source = format!(
            r#"shape Item
  id: number
component Paint
  resource items = items() as shape Item[]
  state display = "{display}"
  action flex
    display = "flex"
  action grid
    display = "grid"
  action block
    display = "block"
  view
    box
      button "Flex" testId="flex" press=flex
      button "Grid" testId="grid" press=grid
      button "Block" testId="block" press=block
      box testId="waiting-list" display=display
        box testId="positioned" position="relative"
        for item in items key=item.id
          box testId=`row-${{item.id}}` position="static" z-index=(item.id == 198 ? 0 : item.id == 199 ? 2147483647 : item.id == 200 ? -2147483648 : item.id <= 100 ? 2 : -2)
"#
        );
        let plan = contract::bake(contract::compile(&source).unwrap(), ManyItems).unwrap();
        write_case(
            &dir.join(labels.len().to_string()),
            &plan,
            &["flex", "block", "grid", "block", display],
        );
        labels.push(format!("sliced keyed z-index initially {display}"));
    }
    for exclusion in [false, true] {
        let source = format!(
            r#"shape Item
  id: number
component Paint
  resource items = items() as shape Item[]
  state exclusion = {exclusion}
  action toggle
    exclusion = not exclusion
  view
    box
      button "Toggle" testId="toggle" press=toggle
      box testId="waiting-list" position="relative"
        box testId="exclusion" position=(exclusion ? "absolute" : "static") wrap-flow="both" width=20 height=20
        for item in items key=item.id
          text "paragraph" testId=`row-${{item.id}}`
"#
        );
        let plan = contract::bake(contract::compile(&source).unwrap(), ManyItems).unwrap();
        write_case(
            &dir.join(labels.len().to_string()),
            &plan,
            &["toggle", "toggle"],
        );
        labels.push(format!(
            "sliced keyed text initially beside exclusion {exclusion}"
        ));
    }
    compare_cases(&dir, &labels);
}

fn compare_cases(dir: &std::path::Path, labels: &[impl AsRef<str>]) {
    std::fs::write(
        dir.join("cases.json"),
        serde_json::to_string(&labels.iter().map(AsRef::as_ref).collect::<Vec<_>>()).unwrap(),
    )
    .unwrap();
    let output = bun_command()
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../web/tests/paint-order/compare.mjs"
        ))
        .arg(dir)
        .output()
        .expect("Bun runs the browser parity fixture");
    assert!(
        output.status.success(),
        "{}\n{}\nfixtures: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        dir.display()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

fn write_case(dir: &std::path::Path, plan: &Plan, steps: &[&str]) {
    std::fs::create_dir_all(dir).unwrap();
    let emitted = crate::emit::emit(plan, false, false).unwrap();
    let (mut host, _) = exact_web::Host::boot(&plan.encode(), (), Default::default(), "/").unwrap();
    let document = host.document().unwrap();
    match host.runner().document_tree() {
        Ok(tree) => {
            let (direct, _) =
                exact_web::document::project_tree(&tree, host.runner(), Default::default())
                    .unwrap();
            assert_eq!(direct.root, document.root);
        }
        // Platform text styles now use relative lengths; the production
        // renderer takes the same kernel path for plans the direct fold refuses.
        Err(exact_runner::DocTreeError::Unsupported(_)) => {}
        Err(error) => panic!("document tree: {error:?}"),
    }
    for (name, value) in [
        ("app.js", emitted.js),
        ("paint.js", emitted.paint),
        ("app.css", emitted.css),
        ("names.js", emitted.names),
        ("rust.html", document.root),
    ] {
        std::fs::write(dir.join(name), value).unwrap();
    }
    let mut snapshots = Vec::new();
    for action in steps {
        let kernel = host.runner().kernel();
        let id = kernel
            .node_by_key(kernel.find_by_test_id(action)[0])
            .unwrap()
            .id;
        host.dispatch(id, exact_runner::Event::Press);
        let kernel = host.runner().kernel();
        let isolation: BTreeMap<_, _> = kernel
            .paint_order()
            .into_iter()
            .filter_map(|(id, p)| {
                let node = kernel.node(id).unwrap();
                node.props.str(PropId::TestId).map(|name| {
                    (
                        name.to_owned(),
                        if p.isolated
                            || p.policy
                            || node.style.isolation == exact_kernel::Isolation::Isolate
                        {
                            "isolate"
                        } else {
                            "auto"
                        },
                    )
                })
            })
            .collect();
        let z_index: BTreeMap<_, _> = kernel
            .paint_order()
            .into_iter()
            .filter_map(|(id, _)| {
                let node = kernel.node(id).unwrap();
                node.props.str(PropId::TestId).map(|name| {
                    (
                        name.to_owned(),
                        if node.style.mask.has(StyleId::ZIndex) {
                            node.style
                                .z_index
                                .clamp(
                                    -exact_kernel::paint_order::Z_MAX,
                                    exact_kernel::paint_order::Z_MAX,
                                )
                                .to_string()
                        } else {
                            "auto".into()
                        },
                    )
                })
            })
            .collect();
        let paint: BTreeMap<_, _> = kernel
            .paint_order()
            .into_iter()
            .filter_map(|(id, p)| {
                let node = kernel.node(id).unwrap();
                let own = exact_kernel::paint_order::own(kernel.arena(), node.key.index);
                node.props.str(PropId::TestId).map(|name| {
                    (
                        name.to_owned(),
                        serde_json::json!({"positioned": own.positioned, "stacks": own.stacks,
                            "z": own.z, "policy": p.policy, "isolated": p.isolated, "rank": p.rank}),
                    )
                })
            })
            .collect();
        let backdrop: BTreeMap<_, _> = kernel
            .paint_order()
            .into_iter()
            .filter_map(|(id, _)| {
                let node = kernel.node(id).unwrap();
                node.props
                    .str(PropId::TestId)
                    .map(|name| (name.to_owned(), node.style.backdrop_filter.css()))
            })
            .collect();
        snapshots.push(serde_json::json!({"action": action, "isolation": isolation,
            "zIndex": z_index, "paint": paint, "backdrop": backdrop}));
    }
    std::fs::write(
        dir.join("steps.json"),
        serde_json::to_string(&snapshots).unwrap(),
    )
    .unwrap();
}

struct Items;
impl exact_runner::DataSource for Items {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Ok(Value::list(
            (1..=4)
                .map(|i| Value::record(vec![Value::Number(f64::from(i))]))
                .collect(),
        ))
    }
}

#[test]
fn bound_transition_stacking_matches_kernel() {
    let mut values: Vec<String> = [
        "200ms linear(0, 1)",
        "width 1s, opacity 1s",
        "width 1s",
        "margin 1s",
        "transform 200ms",
        "none",
        "",
        "color 1s",
        "border-color 200ms",
        "all 1s",
        "opacity 1s",
        "1s opacity",
        "translate 1s",
        "scale 1s",
        "rotate 1s",
        "200ms",
        "200ms ease-in",
        "color 1s, opacity 200ms",
        "opacity",
        "all",
        "ease",
        "-exact-spring()",
        "linear(0, 1)",
        "opacity scale 1s",
        "opacity 1s ease linear",
        "opacity 1s 2s 3s",
        "opacity -1s",
        "opacity 1s -2s",
        "opacity 1e309s",
        "opacity NaNs",
        "opacity 0x1s",
        "opacity 1\ns",
        "opacity \u{feff}1s",
        "opacity \u{85}1s",
        "opacity +.1E+2ms",
        "opacity\t1s",
        " opacity 1s ",
        "opacity 1s,",
        ",opacity 1s",
        "none, opacity 1s",
        "opacity 1s (",
        "opacity 1s)",
        "opacity 1s cubic-bezier(0, 0, 1, 1))",
        "layout 1s",
        "-exact-tint-color 1s",
        "--exact-tint 1s",
        "--exact-shadow-color 1s",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    // Every authorable property, including aliases, and every easing branch.
    let properties = exact_motion::Property::ALL
        .into_iter()
        .flat_map(|p| [p.name(), p.css_name()])
        .chain(["all", "border-color", "width", "", "opacity scale"]);
    let easings = [
        "linear",
        "ease",
        "ease-in",
        "ease-out",
        "ease-in-out",
        "step-start",
        "step-end",
        "cubic-bezier(0, -2, 1, 3)",
        "cubic-bezier(-.1, 0, 1, 1)",
        "cubic-bezier(0, 0, 1)",
        "cubic-bezier(0, NaN, 1, 1)",
        "steps(1)",
        "steps(+2, start)",
        "steps(2, end)",
        "steps(2, jump-start)",
        "steps(2, jump-end)",
        "steps(2, jump-none)",
        "steps(2, jump-both)",
        "steps(0)",
        "steps(1, jump-none)",
        "steps(65535)",
        "steps(65536)",
        "steps(1.0)",
        "steps(2, invalid)",
        "steps(2, end, ignored)",
        "linear(0, 1)",
        "linear(0, .5, 1)",
        "linear(0 0% 20%, 1 80% 100%)",
        "linear(0 80%, .5 20%, 1)",
        "linear(0 0% 100%)",
        "linear(0)",
        "linear()",
        "linear(0 -1%, 1)",
        "linear(0, 1 101%)",
        "linear(0, NaN)",
        "-exact-spring()",
        "-exact-spring(300, 30, 1)",
        "-exact-spring(0, 30, 1)",
        "-exact-spring(300, -1, 1)",
        "-exact-spring(300, 30, 0)",
        "-exact-spring(300, 30)",
        "-exact-spring(300, 30, 1e309)",
        "-exact-spring(100, 1e200, 1)",
    ];
    for property in properties {
        for easing in easings {
            for times in ["", "200ms", "0s 100ms", "0s -100ms"] {
                values.push(format!("{property} {times} {easing}"));
            }
        }
    }
    for count in [8, 9] {
        values.push(vec!["opacity 1s"; count].join(", "));
    }
    values.push("opacity 1s linear, color -exact-spring(100, 1e200, 1)".into());
    for count in [64, 65] {
        values.push(format!("200ms linear({})", vec!["0"; count].join(",")));
    }
    let plan = fixture_plan("", "2>1>");
    let (_, expression) = crate::paint::binding(
        &plan,
        NodeType::View,
        &BindingsRow {
            kind: BindingKind::Style,
            id: StyleId::Transition as u16,
            expr: Default::default(),
            written: true,
        },
    )
    .unwrap();
    let script = format!(
        "const values={};console.log(JSON.stringify(values.map(v=>({expression})!==0)));",
        serde_json::to_string(&values).unwrap()
    );
    use std::io::Write;
    use std::process::Stdio;
    let mut child = bun_command()
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
    let mut disagreements = Vec::new();
    for (value, actual) in values.into_iter().zip(actual) {
        let mut style = exact_kernel::StyleProps::default();
        let _ = style.set_dynamic(
            StyleId::Transition,
            &exact_kernel::StyleValue::Text(value.as_str().into()),
        );
        let own = exact_kernel::paint_order::own_from(exact_kernel::paint_order::Facts {
            style: &style,
            props: &Default::default(),
            kind: NodeType::View,
            root: false,
            parent_display: None,
            beside_exclusion: false,
            holds_layout_transition: false,
        });
        if actual != own.policy {
            disagreements.push(format!("{value:?}: JS {actual}, kernel {}", own.policy));
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} disagreements:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}

struct ManyItems;
impl exact_runner::DataSource for ManyItems {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Ok(Value::list(
            (1..=200)
                .map(|i| Value::record(vec![Value::Number(f64::from(i))]))
                .collect(),
        ))
    }
}

#[test]
fn grouped_separators_follow_visible_siblings_without_dom_boxes() {
    exact_web::link(exact_web_capabilities::ALL);
    let source = r##"keyframes ink
  from border-bottom-color="#0000ff"
  to border-bottom-color="#ff0000"

component App
  state changed = false
  state inset = "calc(10% + 2px)"
  action toggle
    changed = not changed
    inset = changed ? "calc(10% + 2px)" : "calc(20% + 2px)"
  view
    column
      button "Toggle" press=toggle testId="toggle"
      button appearance="auto" testId="standalone"
        text "Native button"
      row testId="ordinary" padding-left=(changed ? 24 : 16)
        text "Ordinary"
      list appearance="auto" width=320 height=280
        section
          row width="100%" position="static" testId="first"
            text "Custom row"
            box flex=1 height=8 background-color="#cccccc"
            text "42"
          row width="100%" testId="middle" transition="border-bottom-color 1s linear" padding=(changed ? 24 : 16) border-color=(changed ? "#ff0000" : "#0000ff")
            text "Middle"
          image "symbol:info" height=52 border-bottom-color="#008000" testId="image-row"
          row padding=0 height=52 border-bottom-color="#0000ff" testId="covered" animation="ink 1s linear -500ms paused"
            box width="100%" height="100%" background-color="#ff0000"
          button appearance="auto" testId="last" display=(changed ? "none" : "flex")
            text "Native button"
      list appearance="auto" width=332 height=170 flex-shrink=0 testId="percent-list"
        section
          row width=100 height=52 padding-left=inset padding-right=(changed ? "calc(93px - 20%)" : "calc(93px - 10%)") border-width=0 border-left-width=2 border-right-width=3 border-style="solid" background-color="#ffffff" border-bottom-color="#0000ff" testId="percent-row"
          row width=100 height=52 padding-left="10%" padding-right="calc(100px - 10%)" background-color="#ffffff" border-bottom-color="#0000ff" testId="static-percent"
          row width=100 height=52 testId="percent-last"
          if changed
            row width=100 height=52 padding-left="10%" testId="conditional-percent"
"##;
    let plan = contract::compile(source).unwrap();
    let output = crate::emit::emit(&plan, false, false).unwrap();
    let ordinary =
        contract::compile(&source.replace("appearance=\"auto\"", "appearance=\"none\"")).unwrap();
    let ordinary = crate::emit::emit(&ordinary, false, false).unwrap();
    assert!(!ordinary.js.contains("grouped-row-hidden"));
    assert!(!ordinary.js.contains("--exact-grouped-inset"));
    assert!(output.js.contains("grouped-row-hidden"));
    let dir = std::env::temp_dir().join(format!("exact-js-grouped-{}", std::process::id()));
    write_case(&dir, &plan, &[]);
    let roles: Vec<_> = exact_kernel::generated::SYMBOL_ROLES
        .iter()
        .filter_map(|role| {
            exact_kernel::generated::symbol(role).map(|(_, path, filled)| (role, (path, filled)))
        })
        .collect();
    std::fs::write(
        dir.join("symbol-roles.js"),
        format!(
            "export default {};",
            serde_json::to_string(&roles.into_iter().collect::<BTreeMap<_, _>>()).unwrap()
        ),
    )
    .unwrap();
    let program = r#"
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,readdirSync,copyFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {rolldown} from 'rolldown';
import {chromium as playwright} from 'playwright-core';
import {chromium} from './scripts/agent-launch.mjs';
import {decodePng} from './scripts/png.mjs';
const dir=process.argv[1],root=process.cwd(),gen=resolve(dir,'.gen');
mkdirSync(gen,{recursive:true});
for(const folder of ['host/web','host/web-js'])for(const f of readdirSync(resolve(root,folder)).filter(f=>f.endsWith('.js')))copyFileSync(resolve(root,folder,f),resolve(gen,f));
for(const f of ['app.js','names.js','paint.js','symbol-roles.js'])copyFileSync(resolve(dir,f),resolve(gen,f));
writeFileSync(resolve(gen,'entry.js'),`import app from './app.js';app();globalThis.ready=true;`);
const bundle=await rolldown({input:resolve(gen,'entry.js'),logLevel:'silent',external:['./draw.js']});
await bundle.write({file:resolve(dir,'client.js'),format:'iife',codeSplitting:false});await bundle.close();
const base=readFileSync(resolve(root,'host/web/index.html'),'utf8').match(/<style>([\s\S]*?)<\/style>/)[1];
const css=base+readFileSync(resolve(dir,'app.css'),'utf8');
const shell=content=>`<!doctype html><html><style>${css}</style><div id="exact-root">${content}</div>`;
const rustMarkup=readFileSync(resolve(dir,'rust.html'),'utf8');
writeFileSync(resolve(dir,'rust-page.html'),shell(rustMarkup)+`<script type="module">import {observeGroupedInset} from './.gen/grouped-inset.js';for(const e of document.querySelectorAll('[data-grouped-row-separator]'))observeGroupedInset(e,e.style.getPropertyValue('--exact-grouped-inset'));</script>`);
writeFileSync(resolve(dir,'adopted-page.html'),shell(rustMarkup)+'<script type="application/vnd.exact.checkpoint">{"time":0,"answers":[]}</script><script src="./client.js"></script>');
writeFileSync(resolve(dir,'client-page.html'),shell('')+'<script src="./client.js"></script>');
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch:req=>{const path=new URL(req.url).pathname;return path==='/favicon.ico'?new Response(null,{status:204}):new Response(Bun.file(resolve(dir,'.'+path)))}});
const {executable,unavailable}=chromium();if(unavailable)throw Error(unavailable);
const browser=await playwright.launch({executablePath:executable,headless:true,args:['--no-sandbox']});
try{
 const page=await browser.newPage();
 await page.addInitScript(()=>{const Native=ResizeObserver;globalThis.insetObservers={created:0,delivered:0,active:new Set()};globalThis.ResizeObserver=class extends Native{constructor(f){super(e=>{insetObservers.delivered++;f(e)});insetObservers.created++}observe(e,o){insetObservers.active.add(e);super.observe(e,o)}unobserve(e){insetObservers.active.delete(e);super.unobserve(e)}}});
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const settled=()=>page.waitForFunction(()=>{const e=document.querySelector('[data-testid="percent-row"]'),s=getComputedStyle(e);return s.getPropertyValue('--exact-grouped-inset-resolved').trim()===s.paddingLeft});
 const percentage=async(id,border)=>{const e=page.locator(`[data-testid="${id}"]`),r=await e.boundingBox();const expected=await e.evaluate(e=>parseFloat(getComputedStyle(e).paddingLeft));assert.equal(r.width,100);assert.equal(await e.evaluate(e=>e.clientWidth-parseFloat(getComputedStyle(e).paddingLeft)-parseFloat(getComputedStyle(e).paddingRight))<=0,true,'the content width stays zero');const first=Math.ceil(expected+border);assert.deepEqual(await pixel(id,first-1),[255,255,255],'before resolved padding');assert.deepEqual(await pixel(id,first),[0,0,255],'separator starts at resolved padding, inside the border');};
 const snapshot=()=>page.evaluate(()=>Object.fromEntries(['first','middle','image-row','covered','last'].map(id=>{const e=document.querySelector(`[data-testid="${id}"]`),s=getComputedStyle(e);return[id,{separator:s.backgroundImage.startsWith('linear-gradient('),color:s.getPropertyValue('--exact-grouped-separator').trim(),children:e.children.length,hidden:e.getAttribute('data-grouped-row-hidden'),inset:s.getPropertyValue('--exact-grouped-inset').trim()}]})));
 const pixel=async(id,dx)=>{const r=await page.locator(`[data-testid="${id}"]`).boundingBox(),p=decodePng(await page.screenshot());const x=Math.floor(dx<0?r.x+r.width+dx:r.x+dx),y=Math.floor(r.y+r.height)-1;return [...p.data.slice((y*p.width+x)*4,(y*p.width+x)*4+3)]};
 await page.goto(`${server.url}rust-page.html`);await settled();await percentage('percent-row',2);await percentage('static-percent',0);const rust=await snapshot();
 assert.equal(rust.first.separator,true);assert.equal(rust.middle.separator,true);assert.equal(rust.last.separator,false);
 assert.equal(rust.first.inset,'16px');assert.equal(rust.middle.color,'rgb(0, 0, 255)');assert.equal(rust.last.inset,'16px');
 await page.goto(`${server.url}client-page.html`);await page.waitForFunction(()=>globalThis.ready);await settled();await percentage('percent-row',2);await percentage('static-percent',0);
 const resizeCounts=await page.evaluate(async()=>{const e=document.querySelector('[data-testid="percent-row"]');const parent=e.parentElement;parent.style.width='400px';await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return [parent.clientWidth,getComputedStyle(e).getPropertyValue('--exact-grouped-inset-resolved').trim()]});assert.deepEqual(resizeCounts,[400,'42px'],'a resized containing block updates padding even with zero-width row content');await percentage('percent-row',2);
 await page.evaluate(()=>document.querySelector('[data-testid="percent-row"]').parentElement.style.removeProperty('width'));await settled();
 assert.deepEqual(await page.evaluate(()=>[insetObservers.created,[...insetObservers.active].filter(e=>e.dataset.testid).map(e=>e.dataset.testid).sort()]),[1,['percent-row','static-percent']],'one observer only watches percentage rows and their parent');
 const idle=await page.evaluate(async()=>{await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const before=insetObservers.delivered;for(let i=0;i<5;i++)await new Promise(requestAnimationFrame);return insetObservers.delivered-before});assert.equal(idle,0,'inset observation does no work in idle frames');
 assert.deepEqual(await snapshot(),rust,'the fresh JS client agrees with the Rust document');
 assert.deepEqual(await pixel('middle',-2),[0,0,255],'the separator reaches the inner trailing edge');
 assert.deepEqual(await pixel('image-row',-2),[0,128,0],'a replaced image row draws its separator');
 assert.deepEqual(await pixel('covered',-2),[255,0,0],'opaque row content covers the separator');
 assert.equal(await page.evaluate(()=>{const style=id=>getComputedStyle(document.querySelector(`[data-testid="${id}"]`));return parseFloat(style('last').paddingLeft)-parseFloat(style('standalone').paddingLeft)}),16,'native face fitting includes exactly one leading inset');

 await page.evaluate(()=>document.querySelector('[data-testid="toggle"]').click());
 await settled();await percentage('percent-row',2);
 assert.equal(await page.evaluate(()=>[...insetObservers.active].some(e=>e.dataset.testid==='conditional-percent')),true);
 const colours=await page.evaluate(async()=>{await new Promise(requestAnimationFrame);const e=document.querySelector('[data-testid="middle"]');for(const a of e.getAnimations()){a.pause();a.currentTime=500;}const s=getComputedStyle(e);return[s.borderBottomColor,s.getPropertyValue('--exact-grouped-separator').trim()]});
 assert.deepEqual(colours,['rgb(128, 0, 128)','rgb(128, 0, 128)'],'a compiled bound border colour transitions separator ink');
 await page.evaluate(()=>{for(const a of document.querySelector('[data-testid="middle"]').getAnimations())a.finish()});
 const hidden=await snapshot();assert.equal(hidden.first.separator,true);assert.equal(hidden.middle.separator,true);assert.equal(hidden.covered.separator,false);assert.equal(hidden.last.hidden,'true');assert.equal(hidden.middle.inset,'24px');assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('[data-testid="middle"]')).getPropertyValue('--exact-grouped-separator').trim()),'rgb(255, 0, 0)');
 assert.deepEqual(Object.values(hidden).map(v=>v.children),Object.values(rust).map(v=>v.children),'separator updates add no child boxes');
 await page.evaluate(()=>document.querySelector('[data-testid="toggle"]').click());await page.evaluate(()=>{getComputedStyle(document.querySelector('[data-testid="middle"]')).borderBottomColor;for(const a of document.querySelector('[data-testid="middle"]').getAnimations())a.finish()});assert.deepEqual(await snapshot(),rust,'showing the last row restores its predecessor separator');assert.equal(await page.evaluate(()=>[...insetObservers.active].some(e=>e.dataset.testid==='conditional-percent')),false,'unmount releases the percentage row observation');
 const animated=await page.evaluate(()=>{const s=getComputedStyle(document.querySelector('[data-testid="covered"]'));return[s.borderBottomColor,s.getPropertyValue('--exact-grouped-separator').trim()]});assert.deepEqual(animated,['rgb(128, 0, 128)','rgb(128, 0, 128)'],'compiled keyframes animate separator ink with the border');
 const fallback=await page.evaluate(()=>{const e=document.querySelector('[data-testid="middle"]');e.style.transition='none';e.style.colorScheme='dark';e.style.removeProperty('--exact-grouped-separator');return getComputedStyle(e).getPropertyValue('--exact-grouped-separator').trim()});assert.equal(fallback,'rgba(84, 84, 88, 0.5)','removing an authored ink restores the dark system separator');
 await page.goto(`${server.url}adopted-page.html`);await page.waitForFunction(()=>globalThis.ready);await settled();await percentage('percent-row',2);await percentage('static-percent',0);
 assert.deepEqual(errors,[],'percentage inset observations do not create resize delivery loops');
 await page.close();
}finally{await browser.close();server.stop(true)}
"#;
    let result = bun_command()
        .args(["-e", program])
        .arg(&dir)
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("Bun runs the browser separator regression");
    assert!(
        result.status.success(),
        "{}\n{}\nfixture: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
        dir.display()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
