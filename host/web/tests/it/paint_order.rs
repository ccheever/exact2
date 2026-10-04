//! Live batches must write exactly the kernel's isolation (LLP 1083.000 W1).
use exact_kernel::{NodeType, StyleId, ViewId};
use exact_plan::{builder::PlanBuilder, BindingKind, BindingsRow, Plan, Value};
use exact_runner::Event;
use exact_web::Host;
use std::collections::{BTreeMap, BTreeSet};

type Web = Host<caltrain_data::Caltrain>;

fn boot(plan: &Plan) -> (Web, String) {
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap()
}

fn apply(css: &mut BTreeMap<ViewId, String>, batch: &str) {
    for op in batch.split("{\"op\":").skip(1) {
        let kind = op.split('"').nth(1).unwrap();
        if !matches!(kind, "create" | "style" | "destroy") {
            continue;
        }
        let id = op.split("\"id\":").nth(1).unwrap();
        let id: ViewId = id.split([',', '}']).next().unwrap().parse().unwrap();
        if kind == "destroy" {
            css.remove(&id);
        } else {
            let value = op
                .split("\"css\":\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            css.insert(id, value.into());
        }
    }
}

fn agrees(host: &Web, css: &BTreeMap<ViewId, String>, label: &str) {
    let want: BTreeSet<_> = host
        .runner()
        .kernel()
        .paint_order()
        .into_iter()
        .filter_map(|(id, p)| (p.isolated || p.policy).then_some(id))
        .collect();
    let got: BTreeSet<_> = css
        .iter()
        .filter_map(|(&id, css)| css.contains("isolation:isolate;").then_some(id))
        .collect();
    assert_eq!(got, want, "{label}: {css:?}");
}

/// Build literal fixture rows directly so the compiler cannot add a
/// containing block that the Chrome fixture did not declare.
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
        let bindings: Vec<_> = css
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
fn every_chrome_paint_case_writes_the_kernels_isolated_and_policy_sets() {
    let cases = include_str!("../../../../kernel/tests/it/fixtures/browser_paint_order.tsv");
    let mut count = 0;
    for line in cases.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<_> = line.split('\t').collect();
        let plan = fixture_plan(fields[1], fields[2]);
        let (host, batch) = boot(&plan);
        let mut css = BTreeMap::new();
        apply(&mut css, &batch);
        agrees(&host, &css, fields[0]);
        let document = host.document().unwrap();
        let tree = host.runner().document_tree().unwrap();
        let (detached, _) =
            exact_web::document::project_tree(&tree, host.runner(), Default::default()).unwrap();
        assert_eq!(detached.root, document.root, "{}: DocTree", fields[0]);
        for (&id, css) in &css {
            let open = document
                .root
                .split(&format!("data-view=\"{id}\""))
                .nth(1)
                .unwrap()
                .split('>')
                .next()
                .unwrap();
            let style = open
                .split("style=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert_eq!(style, css, "{}: view {id}", fields[0]);
        }

        // The adoption path reuses the server projection's CSS. It must
        // replace that path's isolation with the live kernel's decisions.
        let page = exact_web::document::checkpoint(host.runner(), "/");
        let digest =
            exact_web::document::digest(&plan.encode(), "/", &page, &host.document().unwrap().root);
        let (adopted, batch) = Host::boot_checkpoint(
            &plan.encode(),
            caltrain_data::Caltrain,
            &page,
            &digest,
            vec![],
            None,
            Default::default(),
            "/",
        )
        .unwrap();
        assert!(batch.contains("\"adopted\":true"), "{batch}");
        let mut css = BTreeMap::new();
        apply(&mut css, &batch);
        agrees(&adopted, &css, fields[0]);
        count += 1;
    }
    assert_eq!(count, 18);
}

fn view(host: &Web, name: &str) -> ViewId {
    let kernel = host.runner().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

fn drive(source: &str, actions: &[&str]) {
    let (mut host, batch) = boot(&contract::compile(source).unwrap());
    let mut css = BTreeMap::new();
    apply(&mut css, &batch);
    agrees(&host, &css, "boot");
    for action in actions {
        let id = view(&host, action);
        apply(&mut css, &host.dispatch(id, Event::Press));
        agrees(&host, &css, action);
    }
}

#[test]
fn dirty_lists_follow_rows_parent_display_and_inserted_descendants() {
    drive(
        r#"component App
  state changed = false
  action toggle
    changed = not changed
  view
    box
      button "Toggle" testId="toggle" press=toggle
      box position=(changed ? "static" : "relative")
      box
        box opacity=(changed ? 0.5 : 1)
          box position="absolute" z-index=(changed ? -4 : 2)
      box
      box display=(changed ? "flex" : "block")
        box z-index=2
        box
          box position="relative"
      box
        when changed
          box position="absolute" z-index=3
      box
"#,
        &["toggle", "toggle", "toggle"],
    );
}

#[test]
fn dirty_lists_follow_layout_policy_and_text_flow_structure() {
    drive(
        r#"component App
  state changed = false
  action toggle
    changed = not changed
  view
    box
      button "Toggle" testId="toggle" press=toggle
      box
        when changed
          box layout-transition="200ms linear"
      box position="relative"
        text "paragraph"
        when changed
          box position="absolute" width=20 height=20 shape-outside="circle(50%)" wrap-flow="both"
      box transition=(changed ? "opacity 1s" : "color 1s")
      box
"#,
        &["toggle", "toggle"],
    );
}

#[test]
fn authored_isolation_keeps_its_value() {
    // Isolation is a kernel row, currently authored only on SVG in
    // Contract. Exercise the live host directly with a literal plan.
    let mut plan = fixture_plan("", "2>1>isolation:auto & 3>2>position:absolute;z-index:1 & 4>1>isolation:isolate & 5>1>isolation:auto & 6>1>opacity:0.5");
    for i in [4, 5] {
        plan.nodes[i].node_type = NodeType::Canvas as u8;
    }
    let (host, batch) = boot(&plan);
    let mut css = BTreeMap::new();
    apply(&mut css, &batch);
    let root = host.runner().roots()[0];
    let children = host.runner().kernel().node(root).unwrap().children();
    assert!(css[&root].contains("isolation:isolate;"));
    for id in [children[0], children[2]] {
        let value = &css[&id];
        assert!(value.contains("isolation:auto;"), "{id}: {value}");
        assert!(!value.contains("isolation:isolate;"), "{id}: {value}");
    }
    assert!(css[&children[1]].contains("isolation:isolate;"));
    assert!(!css[&children[3]].contains("isolation:"));
}

#[test]
fn z_index_is_an_exact_integer_below_the_ghost_and_lift() {
    use exact_kernel::{paint_order::Z_MAX, StyleProps, StyleValue};
    for z in [
        0,
        16_777_217,
        -16_777_217,
        Z_MAX,
        -Z_MAX,
        i32::MAX,
        i32::MIN,
    ] {
        let mut rows = StyleProps::default();
        rows.set_dynamic(StyleId::ZIndex, &StyleValue::Number(f64::from(z)))
            .unwrap();
        let (css, skipped) = exact_web::css::css_text(&rows, &[]);
        assert!(skipped.is_empty());
        assert_eq!(css, format!("z-index:{};", z.clamp(-Z_MAX, Z_MAX)));
    }
}
