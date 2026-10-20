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
fn a_param_named_where_no_component_owns_it_is_refused_and_one_passed_is_the_childs() {
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
    let arg = r##"component App
  params
    fill: color = "#ffffff"
  view
    column background-color="param.fill"
      Label(label="param.fill")
component Label
  props
    label: string
  view
    text label
"##;
    let e = build("arg", arg).unwrap_err();
    assert_eq!(
        e.id, "tamber-param-scope",
        "a param passed to a prop drawn as text: {e:?}"
    );
    assert!(e.message.contains("app.contract#App.fill"), "{}", e.message);
    // Passed to a prop the child draws a row with, it is the child's row's.
    let glyph = r##"component App
  params
    tint: color = "#0a7aff"
  view
    column
      Glyph(tint="param.tint")
component Glyph
  props
    tint: string
  view
    column background-color=tint
      text "g"
"##;
    let plan = build("pass", glyph).unwrap_or_else(|e| panic!("{e:?}"));
    let token = contract_lower::params::token_name("app.contract#App", "tint");
    assert!(plan.theme.iter().any(|t| plan.str(t.name) == token));
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
fn a_param_is_a_rows_value_and_what_a_row_computes_from_it_is_refused() {
    // A row's value: a string, a template, a branch, an arm, a `+`.
    let src = r##"component App
  params
    fill: color = "#ffffff"
    fade: duration = "200ms"
  view
    column background-color=(true ? "param.fill" : "#000000") testId="param.fill" transition=("opacity " + "param.fade")
      text "param.fill"
      image "https://example.com/param.fill"
"##;
    let plan = build("value", src).unwrap_or_else(|e| panic!("{e:?}"));
    let strings = format!("{:?}", plan.strings);
    assert!(
        strings.contains("\"param.fill\""),
        "the testId and the text stay: {strings}"
    );
    assert!(
        strings.contains("https://example.com/param.fill"),
        "an image's URL is no row: {strings}"
    );
    // A function's argument, a condition, a callback: data, refused.
    for (name, row) in [
        ("call", r##"background-color=(id("param.fill"))"##),
        (
            "predicate",
            r##"background-color=(is("param.fill") ? "param.fill" : "#000000")"##,
        ),
        (
            "arrow",
            r##"color=(["x"].map(x => "param.fill").join(""))"##,
        ),
        ("motion-call", r##"transition=(id("opacity param.fade"))"##),
    ] {
        let src = format!(
            "fn id(x: string): string = x\nfn is(x: string): bool = x == \"param.fill\"\ncomponent App\n  params\n    fill: color = \"#ffffff\"\n    fade: duration = \"200ms\"\n  view\n    column background-color=\"param.fill\" transition=\"opacity param.fade\"\n      text \"x\" {row}\n"
        );
        let e = build(name, &src).unwrap_err();
        assert_eq!(e.id, "tamber-param-scope", "{name}: {e:?}");
    }
    // A quoted name across a template's hole stays as written.
    let quoted = r##"component App
  params
    fill: color = "#ffffff"
  state face = "Example"
  view
    column background-color="param.fill"
      text "x" font-family=`"${face} param.fill", system-ui`
"##;
    let plan = build("quoted", quoted).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        !format!("{:?}", plan.strings).contains("Example param.f"),
        "{:?}",
        plan.strings
    );
    // Passed to a child that computes with it: refused there too.
    let child = r##"component App
  params
    tint: color = "#0a7aff"
  view
    column
      Glyph(tint="param.tint")
component Glyph
  props
    tint: string
  view
    column background-color=(tint == "x" ? "#000000" : tint)
      text "g"
"##;
    assert_eq!(build("child", child).unwrap_err().id, "tamber-param-scope");
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
    let e = contract_lower::params::resolve(&params.tokens, &params.identities, "Card.radius")
        .unwrap_err();
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
