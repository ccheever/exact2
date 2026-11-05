//! Computed presence declarations use the kernel’s values and paint policy.
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
/// Values a binding computes, not as its code names them: unset, with a
/// journal line, whether the kernel refuses one (`bogus`) or takes it (the
/// second, QUEUE.md); and ones that clear the row, as the runner clears them,
/// with none.
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

fn row(plan: &exact_plan::Plan, id: StyleId) -> exact_plan::BindingsRow {
    node_row(plan, id).1
}

fn node_row(plan: &exact_plan::Plan, id: StyleId) -> (NodeType, exact_plan::BindingsRow) {
    plan.nodes
        .iter()
        .flat_map(|n| {
            n.bindings
                .iter()
                .map(|b| (NodeType::from_wire(n.node_type).unwrap(), plan.binding(b)))
                .collect::<Vec<_>>()
        })
        .find(|(_, b)| b.kind == BindingKind::Style && b.id == id as u16)
        .expect("the bound row")
}

#[test]
fn a_bound_layout_transition_compiles_and_is_followed() {
    let js = crate::emit::emit(&plan(), false, false).unwrap().js;
    assert!(js.contains("\"--exact-layout-transition\""), "{js}");
    assert!(
        js.contains(r#";lt(e0,["-exact-spring(300, 24, 1)","1s 200ms linear","260ms ease","320ms cubic-bezier(.32,.72,0,1)"],()=>"#),
        "{js}"
    );
}

#[test]
fn each_named_value_writes_what_the_kernel_writes_and_a_computed_one_is_unset() {
    let plan = plan();
    let values = crate::presence::layout_transitions(
        &plan,
        plan.code(row(&plan, StyleId::LayoutTransition).expr),
    );
    let mut expected = Vec::new();
    for v in VALUES {
        let css = runner_holds(v).then(|| crate::presence::layout_transition(v).unwrap());
        assert_eq!(values.held.get(v), css.as_ref(), "{v:?}");
        expected.push(serde_json::json!(css.flatten()));
    }
    assert_eq!(values.held["260ms ease"].as_deref(), Some("260 0 ease"));
    assert_eq!(values.held["none"], None);
    assert_eq!(values.unsets, ["", "unset"]);
    let map = crate::presence::layout_transition_map(&values, "7", "say");
    let inputs: Vec<serde_json::Value> = VALUES
        .iter()
        .map(|v| serde_json::json!(v))
        .chain(COMPUTED.iter().map(|v| serde_json::json!(v)))
        .chain([serde_json::json!(5), serde_json::Value::Null])
        .collect();
    let got = bun(&format!(
        "const journal=[];const say=l=>journal.push(l);const m={map};console.log(JSON.stringify({{out:{}.map(m),journal}}))",
        serde_json::Value::Array(inputs)
    ));
    expected.extend(std::iter::repeat_n(
        serde_json::Value::Null,
        COMPUTED.len() + 2,
    ));
    assert_eq!(got["out"], serde_json::Value::Array(expected));
    assert_eq!(crate::presence::layout_transition("bogus"), None);
    assert!(runner_holds("260ms  ease"));
    assert!(!runner_holds("  ") && !runner_holds(" UNSET"));
    let why = "; unset (the JS target writes only the values the code names)";
    assert_eq!(
        got["journal"],
        serde_json::json!([
            format!("view 7: invalid layout-transition value \"bogus\"{why}"),
            format!("view 7: invalid layout-transition value \"260ms  ease\"{why}"),
            format!("view 7: invalid layout-transition value 5{why}"),
        ])
    );
}

/// Paint's layout bit (`own_from`'s `holds_layout_transition`, flag 256,
/// with the policy bit 4) is set where the kernel's mask holds the row: a
/// value it names, `none` included; never a blank or `unset`, which clear it,
/// nor a computed one, which this target unsets.
#[test]
fn the_paint_flag_is_set_where_the_kernel_holds_the_row() {
    let plan = plan();
    let (kind, b) = node_row(&plan, StyleId::LayoutTransition);
    let (_, test) = crate::paint::binding(&plan, kind, &b).expect("a paint flag");
    let mut expected: Vec<u32> = VALUES
        .map(|v| if runner_holds(v) { 260 } else { 0 })
        .to_vec();
    expected.extend(COMPUTED.map(|_| 0));
    expected.extend([0, 0]);
    let got = bun(&format!(
        "console.log(JSON.stringify([...{},...{},5,null].map(v=>{test})))",
        serde_json::to_string(&VALUES).unwrap(),
        serde_json::to_string(&COMPUTED).unwrap()
    ));
    assert_eq!(got, serde_json::json!(expected));
}

const EXITS: [&str; 6] = [
    "none",
    "leave 200ms both",
    "leave 200ms 50ms ease-out",
    "__proto__",
    "",
    "unset",
];

fn exit_plan() -> exact_plan::Plan {
    exact_web::link(exact_web_capabilities::ALL);
    let mut value = String::from("\"none\"");
    for (k, text) in EXITS.iter().enumerate().rev() {
        value = format!("(k == {k} ? {text:?} : {value})");
    }
    contract::compile(&format!(
        "keyframes leave\n  to opacity=0 scale=0.75\nkeyframes __proto__\n  to opacity=0\n\ncomponent App\n  state k = 0\n  action next\n    k = k + 1\n  view\n    column press=next -exact-press-scale=0.97 -exact-exit-animation={value}\n      button \"Next\" press=next\n"
    )).unwrap()
}

fn native_exit(plan: &exact_plan::Plan, value: &str, press: bool) -> (bool, Option<String>) {
    let mut style = exact_kernel::StyleProps::default();
    if exact_runner::bridge::unsets(
        StyleId::ExitAnimation as u16,
        &exact_plan::Value::str(value),
    ) || style
        .set_dynamic(
            StyleId::ExitAnimation,
            &exact_kernel::StyleValue::Text(value.into()),
        )
        .is_err()
    {
        return (false, None);
    }
    let _ = exact_runner::bridge::keyframes(plan).resolve(&mut style.rare.exit_animation);
    if press {
        style
            .set_dynamic(StyleId::PressScale, &exact_kernel::StyleValue::Number(0.97))
            .unwrap();
    }
    let (css, _) = exact_web::css::css_text(&style, &[]);
    (
        style.mask.has(StyleId::ExitAnimation),
        css.split(';')
            .find_map(|decl| decl.strip_prefix("--exact-exit-animation:"))
            .map(str::to_string),
    )
}

#[test]
fn exit_values_write_native_css_and_clear_unlisted_values() {
    let plan = exit_plan();
    crate::emit::emit(&plan, false, false).expect("a computed exit builds");
    assert!(native_exit(&plan, "leave  200ms both", false).0);
    let inputs: Vec<_> = EXITS
        .iter()
        .map(|v| serde_json::json!(v))
        .chain([
            serde_json::json!("leave  200ms both"),
            serde_json::json!("bogus"),
            serde_json::json!("leave 1s infinite"),
            serde_json::json!(5),
            serde_json::Value::Null,
            serde_json::json!(" UNSET"),
        ])
        .collect();
    for press in [false, true] {
        let values = crate::presence::exit_animations(
            &plan,
            plan.code(row(&plan, StyleId::ExitAnimation).expr),
            press,
        );
        let map = crate::presence::exit_animation_map(&values, "7", "say");
        let got = bun(&format!("const journal=[];const say=v=>journal.push(v);const m={map};console.log(JSON.stringify({{values:{}.map(m),journal}}))", serde_json::json!(inputs)));
        let mut expected: Vec<_> = EXITS
            .iter()
            .map(|v| serde_json::json!(native_exit(&plan, v, press).1))
            .collect();
        expected.extend(std::iter::repeat_n(serde_json::Value::Null, 6));
        assert_eq!(got["values"], serde_json::json!(expected));
        assert_eq!(got["journal"].as_array().unwrap().len(), 4);
        assert_eq!(values.held["none"], None);
        assert!(values.held["__proto__"].is_some());
    }
}

#[test]
fn an_exit_paint_flag_follows_the_kernel_mask() {
    let plan = exit_plan();
    let (kind, binding) = node_row(&plan, StyleId::ExitAnimation);
    let (_, flag) = crate::paint::binding(&plan, kind, &binding).unwrap();
    let got = bun(&format!(
        "const f=v=>({flag});console.log(JSON.stringify({}.map(f)))",
        serde_json::json!(EXITS)
    ));
    let expected: Vec<_> = EXITS
        .iter()
        .map(|v| if native_exit(&plan, v, false).0 { 4 } else { 0 })
        .collect();
    assert_eq!(got, serde_json::json!(expected));
}
