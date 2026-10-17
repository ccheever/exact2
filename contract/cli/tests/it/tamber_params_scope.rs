//! @ref LLP 1121 D15 (stage 7 A, review round 1) — where a component may
//! name its params, and what the build and the IR make of them.

use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-scope-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

const DESIGN: &str = "tamber t\n\ntheme\n  accent.fill = #0a7aff\n";

fn build(name: &str, src: &str) -> Result<exact_plan::Plan, contract::CompileError> {
    contract::compile_path(&app(name, &[("app.contract", src), ("app.tamber", DESIGN)]))
}

#[test]
fn a_param_named_where_no_component_owns_it_is_refused() {
    let style = r##"style Card
  padding = "param.inset"
component App
  params
    inset: length = "8px"
  view
    column class=Card margin="param.inset"
      text "x"
"##;
    assert_eq!(build("style", style).unwrap_err().id, "tamber-param-scope");
    let arg = r##"component Label
  props
    label: string
  view
    text label
component App
  params
    fill: color = "#ffffff"
  view
    column background-color="param.fill"
      Label(label="param.fill")
"##;
    assert_eq!(build("arg", arg).unwrap_err().id, "tamber-param-scope");
    let state = r##"component App
  params
    fill: color = "#ffffff"
  state c = "param.fill"
  view
    column background-color="param.fill"
      text c
"##;
    assert_eq!(build("state", state).unwrap_err().id, "tamber-param-scope");
}

#[test]
fn a_row_names_a_param_anywhere_in_it_and_other_text_is_left_alone() {
    let src = r##"fn id(x: string): string = x
component App
  params
    fill: color = "#ffffff"
  view
    column background-color=(id("param.fill")) testId="param.fill"
      text "param.fill"
"##;
    let plan = build("call", src).unwrap_or_else(|e| panic!("{e:?}"));
    let strings = format!("{:?}", plan.strings);
    assert!(
        strings.contains("\"param.fill\""),
        "the testId and the text stay: {strings}"
    );
    let icon = r##"component App
  params
    base: icon = "icon(sf \"heart\")"
    glyph: icon = "param.base"
  view
    column
      image "symbol:param.glyph"
"##;
    build("icon", icon).unwrap_or_else(|e| panic!("{e:?}"));
    let options = r##"component App
  params
    quiet: color = "#ffffff"
    fill: color = "accent.fill"  control="select"  options="param.quiet|accent.fill"
  view
    column background-color="param.fill" border-color="param.quiet"
      text "x"
"##;
    build("options", options).unwrap_or_else(|e| panic!("{e:?}"));
    let field = r##"component App
  params
    if: color = "#ffffff"
  view
    column background-color="param.if"
      text "x"
"##;
    build("field", field).unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn ranges_and_every_refusal_name_the_param_in_full() {
    let colour = r##"component App
  params
    a: color = "#ffffff"  range="0..1"
  view
    column background-color="param.a"
      text "x"
"##;
    assert_eq!(
        build("range-colour", colour).unwrap_err().id,
        "tamber-param-range"
    );
    let number = r##"component App
  params
    a: number = "5"  range="0..1"
  view
    column opacity="param.a"
      text "x"
"##;
    assert_eq!(
        build("range-number", number).unwrap_err().id,
        "tamber-param-range"
    );
    let kind = r##"component App
  params
    a: colour = "#fff"
  view
    column background-color="param.a"
      text "x"
"##;
    let path = app(
        "kind-all",
        &[("app.contract", kind), ("app.tamber", DESIGN)],
    );
    let all = contract::compile_path_all(&path, false).err().unwrap();
    assert!(
        all.iter().all(|e| !e.message.contains("param.f")),
        "{all:?}"
    );
}

#[test]
fn a_short_name_is_a_components_a_text_has_an_identity_and_icons_fold() {
    let a = r##"component Card
  params
    radius: length = "4px"
  view
    column border-radius="param.radius"
      text "a"
"##;
    let b = r##"component Card
  params
    padding: length = "4px"
  view
    column padding="param.padding"
      text "b"
"##;
    let root = "use Card from \"./a.contract\"\nuse Card as Other from \"./b.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n      Other()\n";
    let path = app(
        "disjoint",
        &[("app.contract", root), ("a.contract", a), ("b.contract", b)],
    );
    let params = contract::tamber_params::params_of(&path);
    let e = contract_lower::params::resolve(&params, "Card.radius").unwrap_err();
    assert_eq!(e.id, "tamber-param-ambiguous");
    let text = "component App\n  params\n    a: length = \"6px\"\n  view\n    column padding=\"param.a\"\n      text \"x\"\n";
    let plan = contract::compile(text).unwrap();
    let token = contract_lower::params::token_name("app.contract#App", "a");
    assert!(plan.theme.iter().any(|t| plan.str(t.name) == token));
    let src = r##"component App
  params
    glyph: icon = "icon(sf \"heart\")"
    r: length = "calc(4px * roundness)"
  view
    column border-radius="param.r"
      image "symbol:param.glyph"
"##;
    let design = "tamber t\n\ndials\n  roundness: number = 1 in 0..2\n";
    let path = app("dial", &[("app.contract", src), ("app.tamber", design)]);
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let params = contract::tamber_params::params_of(&path);
    let block = contract::tamber_params::components(
        &params,
        Some(&exact_tamber::parse(design).unwrap()),
        &contract_lower::tamber_vocab(),
    );
    let glyph = block[0]["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["param"] == "glyph")
        .unwrap();
    assert_eq!(glyph["values"][0]["sf"], "heart");
}
