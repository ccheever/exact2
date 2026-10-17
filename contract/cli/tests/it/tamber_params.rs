//! @ref LLP 1121 D15 (stage 7 A) — a component's `params` section: each
//! param a token its component owns, named by the component's identity (its
//! declaring file and name), `param.x` in its rows and defaults rewritten to
//! it, folded with the design, checked (kind, reach, range, controls),
//! retained as any token, and listed in the IR's `components` block.

use exact_plan::Plan;
use std::path::PathBuf;

/// An app in a fresh directory: its files, `app.contract` first.
fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-params-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir.join("app.contract")
}

fn theme(plan: &Plan) -> Vec<(String, String)> {
    plan.theme
        .iter()
        .map(|t| (plan.str(t.name).to_string(), plan.str(t.value).to_string()))
        .collect()
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  radius.md = 12px\n  space.row = 8px\n    when density=compact: 30px\n  accent.fill = #0a7aff\n  duration.fast = 150ms\n";

const BUTTON: &str = "component PostButton\n  props\n    label: string\n  params\n    fill: color = \"accent.fill\"  control=\"color\"  group=\"Colour\"  priority=1\n    radius: length = \"radius.md\"  range=\"0..40\"  step=1  control=\"slider\"  group=\"Shape\"  priority=1  label=\"Corner radius\"\n    padding: length = \"12px\"  range=\"8..32\"  step=2  control=\"slider\"\n    pressed: color = \"color-mix(in oklab, param.fill 88%, black)\"  control=\"none\"\n    fade: duration = \"duration.fast\"\n  view\n    button background-color=\"param.pressed\" border-radius=\"param.radius\" padding=\"param.padding\" transition=\"opacity param.fade\"\n      text label\n";

#[test]
fn a_param_is_a_token_its_component_owns() {
    let root = "use PostButton as Btn from \"./post.contract\"\n\ncomponent App\n  view\n    column\n      Btn(label=\"One\")\n      Btn(label=\"Two\")\n";
    let path = app(
        "owned",
        &[
            ("app.contract", root),
            ("post.contract", BUTTON),
            ("app.tamber", DESIGN),
        ],
    );
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let rows = theme(&plan);
    // One token a param, by the declaring file's identity, the import's
    // alias aside.
    let named = |param: &str| {
        rows.iter()
            .find(|(n, _)| n.starts_with("param.") && n.ends_with(&format!(".PostButton.{param}")))
            .cloned()
    };
    let token = contract_lower::params::token_name("post.contract#PostButton", "radius");
    assert_eq!(
        named("radius"),
        Some((token.clone(), "12px".into())),
        "{rows:?}"
    );
    // A param only another's default reads folds into it, and the plan
    // keeps what its rows read (D7).
    assert!(named("pressed").is_some(), "{rows:?}");
    assert!(named("fill").is_none(), "{rows:?}");
    // Two instances, one token: the rows of both read it.
    assert_eq!(rows.iter().filter(|(n, _)| *n == token).count(), 1);
    // A duration param is written into its row at build, as a motion token.
    assert!(named("fade").is_none(), "a motion param is no plan row");
    // The IR's components block: by identity, each param by its full name.
    let params = contract::tamber_params::params_of(&path);
    let design = exact_tamber::parse(DESIGN).unwrap();
    let block = contract::tamber_params::components(
        &params,
        Some(&design),
        &contract_lower::tamber_vocab(),
    );
    let c = &block[0];
    assert_eq!(c["identity"], "post.contract#PostButton");
    let radius = c["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["param"] == "radius")
        .unwrap();
    assert_eq!(radius["name"], "post.contract#PostButton.radius");
    assert_eq!(radius["short"], "PostButton.radius");
    assert_eq!(radius["control"], "slider");
    assert_eq!(radius["group"], "Shape");
    assert_eq!(radius["priority"], 1.0);
    assert_eq!(radius["range"], serde_json::json!([0.0, 40.0]));
    assert_eq!(radius["values"], serde_json::json!(["12px", "12px"]));
    let pressed = c["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["param"] == "pressed")
        .unwrap();
    assert_eq!(
        pressed["default"], "color-mix(in oklab, post.contract#PostButton.fill 88%, black)",
        "a default names a param by its full name, never the hash"
    );
}

#[test]
fn a_short_name_two_components_share_is_ambiguous() {
    let card = |file: &str| {
        format!("component Card\n  params\n    inset: length = \"4px\"\n  view\n    column padding=\"param.inset\"\n      text \"{file}\"\n")
    };
    let root = "use Card from \"./a.contract\"\nuse Card as Other from \"./b/c.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n      Other()\n";
    let a = card("a");
    let b = card("b");
    let path = app(
        "ambiguous",
        &[
            ("app.contract", root),
            ("a.contract", &a),
            ("b/c.contract", &b),
        ],
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let params = contract::tamber_params::params_of(&path);
    assert_eq!(params.len(), 2);
    let e = contract_lower::params::resolve(&params, "Card.inset").unwrap_err();
    assert_eq!(e.id, "tamber-param-ambiguous");
    assert!(
        e.message.contains("a.contract#Card.inset")
            && e.message.contains("b/c.contract#Card.inset"),
        "{}",
        e.message
    );
    let full = contract_lower::params::resolve(&params, "b/c.contract#Card.inset").unwrap();
    assert_eq!(full.identity, "b/c.contract#Card");
    assert_ne!(params[0].token, params[1].token, "two files, two tokens");
}

#[test]
fn a_param_that_would_not_hold_is_refused_named_in_full() {
    let refused = |name: &str, params: &str, row: &str, design: &str| {
        let src = format!(
            "component App\n  params\n{params}  view\n    column {row}\n      text \"x\"\n"
        );
        let path = app(name, &[("app.contract", &src), ("app.tamber", design)]);
        let e = contract::compile_path(&path).unwrap_err();
        assert!(
            !e.message.contains("param.f"),
            "a refusal never names the hash: {}",
            e.message
        );
        (e.id, e.message)
    };
    let (id, m) = refused("unused", "    lonely: length = \"4px\"\n", "", DESIGN);
    assert_eq!(id, "tamber-param-unused");
    assert!(
        m.contains("app.contract#App.lonely") && m.contains("stage 8"),
        "{m}"
    );
    let (id, _) = refused(
        "unknown",
        "    a: length = \"4px\"\n",
        "padding=\"param.b\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-unknown");
    let (id, _) = refused(
        "kind",
        "    a: colour = \"#fff\"\n",
        "background-color=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-kind");
    let (id, _) = refused(
        "rowkind",
        "    a: length = \"4px\"\n",
        "background-color=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-kind");
    let (id, _) = refused(
        "rem",
        "    a: length = \"1rem\"  range=\"0..40\"\n",
        "padding=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-range-unit");
    // The default, folded for every tuple, inside its range: `space.row` is
    // 30px when compact.
    let (id, m) = refused(
        "range",
        "    a: length = \"space.row\"  range=\"0..20\"\n",
        "padding=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-range");
    assert!(m.contains("30px"), "{m}");
    let (id, _) = refused(
        "control",
        "    a: number = \"0.5\"  control=\"color\"\n",
        "opacity=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-control");
    let (id, _) = refused(
        "select",
        "    a: length = \"4px\"  control=\"select\"\n",
        "padding=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-control");
    let (id, m) = refused(
        "options",
        "    a: length = \"4px\"  control=\"select\"  options=\"radius.md|accent.fill\"\n",
        "padding=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-options");
    assert!(
        m.contains("accent.fill") && m.contains("color token"),
        "{m}"
    );
    let (id, _) = refused(
        "priority",
        "    a: length = \"4px\"  priority=4\n",
        "padding=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-priority");
    let (id, _) = refused(
        "step",
        "    a: length = \"4px\"  step=0\n",
        "padding=\"param.a\"",
        DESIGN,
    );
    assert_eq!(id, "tamber-param-step");
    let (id, _) = refused(
        "name",
        "    a: length = \"4px\"\n",
        "padding=\"param.a\"",
        "tamber t\n\ntheme\n  param.x = 4px\n",
    );
    assert_eq!(id, "tamber-param-name");
    // A select's options of its kind are accepted.
    let src = "component App\n  params\n    a: length = \"radius.md\"  control=\"select\"  options=\"radius.md|space.row\"\n  view\n    column padding=\"param.a\"\n      text \"x\"\n";
    let path = app(
        "options-ok",
        &[("app.contract", src), ("app.tamber", DESIGN)],
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn an_app_with_params_and_no_design_folds_them() {
    let src = "component App\n  params\n    fill: color = \"#0a7aff\"\n    inset: length = \"param.gap\"\n    gap: length = \"6px\"\n  view\n    column background-color=\"param.fill\" padding=\"param.inset\"\n      text \"x\"\n";
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    let rows = theme(&plan);
    assert!(
        rows.iter()
            .any(|(n, v)| n.ends_with(".App.inset") && v == "6px"),
        "{rows:?}"
    );
}

#[test]
fn contract_fmt_keeps_a_params_section_as_written() {
    let formatted = contract_syntax::fmt::format(BUTTON).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        formatted.contains("  params\n    fill: color = \"accent.fill\""),
        "{formatted}"
    );
    assert_eq!(contract_syntax::fmt::format(&formatted).unwrap(), formatted);
}
