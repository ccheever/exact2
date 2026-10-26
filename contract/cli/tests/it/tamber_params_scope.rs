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
fn a_style_owned_in_its_file_and_applied_from_another_is_shared() {
    // `Look` is Card's in card.contract; the app applying it too makes it
    // shared, its `param.` names no one's (LLP 1139 D3).
    let card = "style Look\n  padding = \"param.inset\"\ncomponent Card\n  params\n    inset: length = \"8px\"\n  view\n    column class=Look\n      text \"c\"\n";
    let app_src = "use Card, Look from \"./card.contract\"\ncomponent App\n  view\n    column class=Look\n      Card()\n";
    let path = app(
        "owned-elsewhere",
        &[
            ("app.contract", app_src),
            ("card.contract", card),
            ("app.tamber", DESIGN),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-param-scope", "{e:?}");
    assert!(e.message.contains("`App` applies it too"), "{}", e.message);
    // Card alone applying it: Card's.
    let alone = "use Card from \"./card.contract\"\ncomponent App\n  view\n    Card()\n";
    let path = app(
        "owned-alone",
        &[
            ("app.contract", alone),
            ("card.contract", card),
            ("app.tamber", DESIGN),
        ],
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
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
    // One component alone applies the style: it reads that component's
    // params (LLP 1139 D3, stage 12 C3). A second applying it: no one's.
    build("style", style).unwrap();
    let shared =
        format!("{style}component Other\n  view\n    column class=Card\n      text \"y\"\n");
    assert_eq!(
        build("style-shared", &shared).unwrap_err().id,
        "tamber-param-scope"
    );
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

/// Many ranged params, each given a value in the design's `params` block,
/// build in moments: the lines resolved by one index and their values folded
/// together, a probe per value index, not a fold of the design per value
/// (the checker's work in param counts, Astra's round 1).
#[test]
fn many_ranged_params_set_by_the_design_build_in_moments() {
    let n = 1500;
    let mut src = String::from("component App\n  params\n");
    for k in 0..n {
        src += &format!("    p{k}: number = \"0.5\"  range=\"0..1\"\n");
    }
    src += "  view\n    column\n";
    for k in 0..n {
        src += &format!("      column opacity=\"param.p{k}\"\n        text \"x\"\n");
    }
    let mut design = String::from("tamber t\n\ntheme\n  accent.fill = #0a7aff\n\nparams\n");
    for k in (0..n).rev() {
        let v = if k == 7 { "2" } else { "0.7" };
        design += &format!("  App.p{k} = {v}\n");
    }
    let path = app(
        "many-ranged",
        &[("app.contract", &src), ("app.tamber", &design)],
    );
    let began = std::time::Instant::now();
    let built = contract::compile_path(&path);
    let took = began.elapsed();
    assert!(took < std::time::Duration::from_secs(20), "{took:?}");
    // `p7`'s value, out of its range, is refused as before.
    let e = built.unwrap_err();
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
}

/// A ranged line whose value reads a param through a theme token is folded
/// alone, with that param's `when` lines as written (Grok's round 2 on the
/// counts: folded beside its siblings, the other line's dark value was
/// pinned to its default and the out-of-range value went unseen).
#[test]
fn a_value_reading_a_param_through_a_token_is_checked_against_the_design() {
    let src = r##"component App
  params
    p0: number = "0.5"  range="0..5"
    p1: number = "0.5"  range="0..1"
  view
    column opacity="param.p0"
      column opacity="param.p1"
        text "x"
"##;
    // (a token names a param by its token name)
    let p0 = contract_lower::params::token_name("app.contract#App", "p0");
    let design = format!("tamber t\n\naxes\n  tone = a | b\n\ntheme\n  accent.fill = #0a7aff\n  via: number = {p0}\n\nparams\n  App.p0 = 0.5\n    when tone=b: 3\n  App.p1 = via\n");
    let design = design.as_str();
    let path = app(
        "range-via",
        &[("app.contract", src), ("app.tamber", design)],
    );
    let e = contract::compile_path(&path).unwrap_err();
    // `p1` through `via` is 3 where tone=b: past its 0..1 (`p0`'s 0..5 holds it).
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(e.message.contains("p1"), "{e:?}");
}

/// Ranged params each with a `when` line reading one dial fold together
/// too: the batch reads the dial beside them, so it stays moving a token
/// (Astra's round 2 on the counts: cleared `when` lines left it inert and
/// every value folded alone).
#[test]
fn many_ranged_params_reading_a_dial_build_in_moments() {
    let n = 1500;
    let mut src = String::from("component App\n  params\n");
    for k in 0..n {
        src += &format!("    p{k}: number = \"0.5\"  range=\"0..1\"\n");
    }
    src += "  view\n    column\n";
    for k in 0..n {
        src += &format!("      column opacity=\"param.p{k}\"\n        text \"x\"\n");
    }
    let mut design = String::from(
        "tamber t\n\naxes\n  tone = a | b\n\ndials\n  d: number = 0.3  in 0..1\n\ntheme\n  accent.fill = #0a7aff\n\nparams\n",
    );
    for k in 0..n {
        let v = if k == 7 { "2" } else { "0.5" };
        design += &format!("  App.p{k} = {v}\n    when tone=b: d\n");
    }
    let path = app(
        "many-dial",
        &[("app.contract", &src), ("app.tamber", &design)],
    );
    let began = std::time::Instant::now();
    let built = contract::compile_path(&path);
    let took = began.elapsed();
    assert!(took < std::time::Duration::from_secs(20), "{took:?}");
    let e = built.unwrap_err();
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(e.message.contains("p7"), "{e:?}");
}

/// Ranged params whose values name a theme token that reads no param fold
/// together, each line's k-th value in one probe: only a token that reads a
/// param, directly or through another, folds its values alone (Astra's and
/// Grok's round 3 on the counts: every token-valued line was a fold of the
/// design).
#[test]
fn many_ranged_params_naming_a_token_build_in_moments() {
    let n = 5000;
    let mut src = String::from("component App\n  params\n");
    for k in 0..n {
        src += &format!("    p{k}: number = \"0.5\"  range=\"0..1\"\n");
    }
    src += "  view\n    column\n";
    for k in 0..n {
        src += &format!("      column opacity=\"param.p{k}\"\n        text \"x\"\n");
    }
    let mut design = String::from(
        "tamber t\n\ntheme\n  accent.fill = #0a7aff\n  half: number = 0.5\n  most: number = half\n  two: number = 2\n\nparams\n",
    );
    for k in 0..n {
        let v = if k == 7 {
            "two"
        } else if k % 2 == 0 {
            "half"
        } else {
            "most"
        };
        design += &format!("  App.p{k} = {v}\n");
    }
    let path = app(
        "many-named",
        &[("app.contract", &src), ("app.tamber", &design)],
    );
    let began = std::time::Instant::now();
    let built = contract::compile_path(&path);
    let took = began.elapsed();
    assert!(took < std::time::Duration::from_secs(20), "{took:?}");
    let e = built.unwrap_err();
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(e.message.contains("p7"), "{e:?}");
}

/// The dials' readers a batch adds never take an authored token's place: a
/// design with tokens under the readers' first stems is checked as written
/// (Astra's round 3 on the counts).
#[test]
fn a_batchs_dial_readers_take_no_authored_name() {
    let src = r##"component App
  params
    p0: number = "0.5"  range="0..1"
    p1: number = "0.5"  range="0..1"
  view
    column opacity="param.p0"
      column opacity="param.p1"
        text "x"
"##;
    let design = "tamber t\n\naxes\n  tone = a | b\n\ndials\n  d: number = 0.3  in 0..1\n\ntheme\n  accent.fill = #0a7aff\n  exact.probe.dial0: number = 2\n  exact.probe0.dial0: number = 2\n\nparams\n  App.p0 = 0.5\n    when tone=b: d\n  App.p1 = exact.probe0.dial0\n    when tone=b: d\n";
    let path = app(
        "probe-names",
        &[("app.contract", src), ("app.tamber", design)],
    );
    let e = contract::compile_path(&path).unwrap_err();
    // `p1` is the authored 2, past its 0..1.
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(e.message.contains("p1"), "{e:?}");
}

/// An icon's name is taken too: the readers' stem skips it, so the batch
/// folds and a range error in a `when` value reading the dial is still found
/// (Astra's narrow re-check: a reader named as the icon failed the batch,
/// and the value alone left the dial inert, so the error went unseen).
#[test]
fn a_batchs_dial_readers_take_no_icons_name() {
    let src = r##"component App
  params
    p0: number = "0.5"  range="0..1"
    p1: number = "0.5"  range="0..1"
  view
    column opacity="param.p0"
      column opacity="param.p1"
        text "x"
"##;
    let design = "tamber t\n\naxes\n  tone = a | b\n\ndials\n  d: number = 0.3  in 0..1\n\ntheme\n  accent.fill = #0a7aff\n\nicons\n  exact.probe0.dial0 = icon(sf \"house\")\n\nparams\n  App.p0 = 2\n    when tone=b: d\n  App.p1 = 0.5\n";
    let path = app(
        "probe-icon",
        &[("app.contract", src), ("app.tamber", design)],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(e.message.contains("p0"), "{e:?}");
}

/// A design read again unchanged through `fold_with` (what `describe` and
/// `resolve` read) is answered from the last few checked, not folded again
/// (both reviewers' round 3 on the counts).
#[test]
fn fold_with_answers_an_unchanged_design_from_the_last() {
    let src = "component App\n  params\n    p0: number = \"0.5\"  range=\"0..1\"\n  view\n    column opacity=\"param.p0\"\n      text \"x\"\n";
    let mut design = String::from("tamber t\n\ndials\n");
    for k in 0..800 {
        design += &format!("  d{k}: number = 0.1  in 0..0.5\n");
    }
    design += "\ntheme\n  accent.fill = #0a7aff\n  t0 = oklch(from #3366cc calc(l + d0) c h)\n";
    for k in 1..800 {
        design += &format!("  t{k} = oklch(from t{} calc(l + d{k}) c h)\n", k - 1);
    }
    let path = app(
        "fold-cached",
        &[("app.contract", src), ("app.tamber", &design)],
    );
    let params = contract::tamber_params::params_of(&path);
    assert!(!params.failed && params.tokens.len() == 1, "{params:?}");
    let vocab = contract_lower::tamber_vocab();
    let began = std::time::Instant::now();
    let first = contract::tamber_params::fold_with(&design, &params, &vocab)
        .map(|(f, _)| f.tokens.len())
        .ok();
    let cold = began.elapsed();
    assert!(first.is_some());
    // (the fastest of a few: another test's designs may push it out once)
    let warm = (0..5)
        .map(|_| {
            let began = std::time::Instant::now();
            let again = contract::tamber_params::fold_with(&design, &params, &vocab)
                .map(|(f, _)| f.tokens.len())
                .ok();
            assert_eq!(again, first);
            began.elapsed()
        })
        .min()
        .unwrap();
    assert!(warm * 2 < cold, "again {warm:?}, the first {cold:?}");
}
