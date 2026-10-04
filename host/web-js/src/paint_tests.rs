//! The emitted program, both server documents and adoption run in Chrome.
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::{builder::PlanBuilder, BindingKind, BindingsRow, Plan, Value};
use std::collections::BTreeMap;

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
                }
            })
            .collect();
        bindings.push(BindingsRow {
            kind: BindingKind::Prop,
            id: PropId::TestId as u16,
            expr: builder.constant(&Value::str(&format!("n{id}"))),
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
  resource items = items() as shape list<Item>
  state changed = false
  derive keys = map(items, x => changed ? 5 - x.id : x.id)
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
        when changed
          box position="absolute" z-index=3
      box testId="arm-follower"
      box testId="motion" transition=(changed ? "1s opacity" : "1s color")
      box testId="filter" filter=(changed ? "blur(0px)" : "none")
      box testId="identity" scale=(changed ? 1 : 2)
      box testId="copies"
        each n in filter(keys, n => n != (changed ? 2 : 4)) key=n
          box testId=`row-${n}`
            when n == 1
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
    labels.push("bound facts, arm swaps and keyed each edits");
    let long = contract::compile(
        r#"shape Item
  id: number
component Paint
  resource items = items() as shape list<Item>
  view
    box
      box testId="waiting-list"
        each item in items key=item.id
          box testId=`row-${item.id}`
            box position="absolute" z-index=2
"#,
    )
    .unwrap();
    let long = contract::bake(long, ManyItems).unwrap();
    write_case(&dir.join(labels.len().to_string()), &long, &[]);
    labels.push("sliced keyed adoption preserves server paint facts");
    std::fs::write(
        dir.join("cases.json"),
        serde_json::to_string(&labels).unwrap(),
    )
    .unwrap();
    let output = std::process::Command::new("bun")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../web/tests/paint-order/compare.mjs"
        ))
        .arg(&dir)
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
    let tree = host.runner().document_tree().unwrap();
    let (document, _) =
        exact_web::document::project_tree(&tree, host.runner(), Default::default()).unwrap();
    assert_eq!(document.root, host.document().unwrap().root);
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
        snapshots
            .push(serde_json::json!({"action": action, "isolation": isolation, "zIndex": z_index}));
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
    let values = [
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
    ];
    let plan = fixture_plan("", "2>1>");
    let (_, expression) = crate::paint::binding(
        &plan,
        NodeType::View,
        &BindingsRow {
            kind: BindingKind::Style,
            id: StyleId::Transition as u16,
            expr: Default::default(),
        },
    )
    .unwrap();
    let script = format!(
        "const values={};console.log(JSON.stringify(values.map(v=>({expression})!==0)));",
        serde_json::to_string(&values).unwrap()
    );
    let output = std::process::Command::new("bun")
        .args(["-e", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
    for (value, actual) in values.into_iter().zip(actual) {
        let mut style = exact_kernel::StyleProps::default();
        let _ = style.set_dynamic(
            StyleId::Transition,
            &exact_kernel::StyleValue::Text(value.into()),
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
        assert_eq!(actual, own.policy, "transition: {value}");
    }
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
