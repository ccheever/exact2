//! GitHub #809: a state-dependent `-exact-layout-transition` in the JS
//! target, written and painted as the kernel takes each value.
use exact_kernel::{NodeType, StyleId};
use exact_plan::BindingKind;

/// Values a binding can push: curves, a spring, `none`, a transition of no
/// layout, and ones that clear the row (lowering refuses a literal the kernel
/// would).
const VALUES: [&str; 8] = [
    "none",
    "260ms ease",
    "320ms cubic-bezier(.32,.72,0,1)",
    "-exact-spring(300, 24, 1)",
    "opacity 1s",
    "1s 200ms linear",
    "",
    "unset",
];
/// Computed strings follow the same grammar as named values.
const COMPUTED: [&str; 4] = ["bogus", "260ms  ease", "  ", " UNSET"];

/// Whether the runner holds the row for `v`: not cleared before parsing
/// (`bridge::unsets`), and parsed.
fn runner_holds(v: &str) -> bool {
    let mut p = exact_kernel::StyleProps::default();
    !exact_runner::bridge::unsets(StyleId::LayoutTransition as u16, &exact_plan::Value::str(v))
        && p.set_dynamic(
            StyleId::LayoutTransition,
            &exact_kernel::StyleValue::Text(v.into()),
        )
        .is_ok()
        && p.mask.has(StyleId::LayoutTransition)
}

fn plan() -> exact_plan::Plan {
    let mut chain = String::from("\"none\"");
    for (k, v) in VALUES.iter().enumerate().rev() {
        chain = format!("(k == {k} ? {v:?} : {chain})");
    }
    contract::compile(&format!(
        "component App\n  state k = 0\n  action next\n    k = k + 1\n  view\n    column testId=\"c\" -exact-layout-transition={chain}\n      button press=next testId=\"b\"\n        text \"Next\"\n"
    ))
    .unwrap()
}

fn bun(script: &str) -> serde_json::Value {
    let out = std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
        .args(["-e", script])
        .output()
        .expect("bun");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn node_row(plan: &exact_plan::Plan) -> (NodeType, exact_plan::BindingsRow) {
    plan.nodes
        .iter()
        .flat_map(|n| {
            n.bindings
                .iter()
                .map(|b| (NodeType::from_wire(n.node_type).unwrap(), plan.binding(b)))
                .collect::<Vec<_>>()
        })
        .find(|(_, b)| b.kind == BindingKind::Style && b.id == StyleId::LayoutTransition as u16)
        .expect("the bound row")
}

#[test]
fn a_bound_layout_transition_compiles_and_is_followed() {
    let js = crate::emit::emit(&plan(), false, false).unwrap().js;
    assert!(js.contains("\"--exact-layout-transition\""), "{js}");
    assert!(js.contains("pr(e0);"), "{js}");
}

#[test]
fn named_and_computed_values_write_the_kernel_css() {
    let inputs: Vec<serde_json::Value> = VALUES
        .iter()
        .chain(COMPUTED.iter())
        .map(|v| serde_json::json!(v))
        .chain([serde_json::json!(5), serde_json::Value::Null])
        .collect();
    let expected: Vec<_> = inputs
        .iter()
        .map(|v| {
            let mut props = exact_kernel::StyleProps::default();
            if let Some(v) = v.as_str().filter(|v| runner_holds(v)) {
                props
                    .set_dynamic(
                        StyleId::LayoutTransition,
                        &exact_kernel::StyleValue::Text(v.into()),
                    )
                    .unwrap();
            }
            let (css, _) = exact_web::css::css_text(&props, &[]);
            css.split(';')
                .find_map(|d| d.strip_prefix("--exact-layout-transition:"))
                .map(str::to_string)
        })
        .collect();
    let map = &*crate::style::LAYOUT_TRANSITION_MAP;
    let got = bun(&format!(
        "const journal=[];const m={map};console.log(JSON.stringify({{out:{}.map(v=>m(v,()=>journal.push(JSON.stringify(v)))),journal}}))",
        serde_json::Value::Array(inputs)
    ));
    assert_eq!(got["out"], serde_json::json!(expected));
    assert_eq!(got["journal"], serde_json::json!(["\"bogus\"", "5"]));
    assert!(runner_holds("260ms  ease"));
    assert!(!runner_holds("  ") && !runner_holds(" UNSET"));
}

/// Paint's layout bit (`own_from`'s `holds_layout_transition`, flag 256,
/// with the policy bit 4) is set where the kernel's mask holds the row: a
/// parsed value, `none` included; never a blank, `unset` or an invalid value.
#[test]
fn the_paint_flag_is_set_where_the_kernel_holds_the_row() {
    let plan = plan();
    let (kind, b) = node_row(&plan);
    let (_, test) = crate::paint::binding(&plan, kind, &b).expect("a paint flag");
    let mut expected: Vec<u32> = VALUES
        .map(|v| if runner_holds(v) { 260 } else { 0 })
        .to_vec();
    expected.extend(COMPUTED.map(|v| if runner_holds(v) { 260 } else { 0 }));
    expected.extend([0, 0]);
    let got = bun(&format!(
        "console.log(JSON.stringify([...{},...{},5,null].map(v=>{test})))",
        serde_json::to_string(&VALUES).unwrap(),
        serde_json::to_string(&COMPUTED).unwrap()
    ));
    assert_eq!(got, serde_json::json!(expected));
}
