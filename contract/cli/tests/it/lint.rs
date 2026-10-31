//! LLP 1017 P1: what the compiler refuses about values and names, and what
//! bake's layout lint refuses that the compiler cannot see.

use contract::BakeError;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn an_old_spelling_is_refused_with_the_css_name() {
    let src = "component A\n  view\n    text \"a\" size=13\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "lower-unknown-attr");
    assert!(e.message.contains("`font-size`"), "{e}");
    let src = "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    button press=go label=\"Go\"\n      text \"a\"\n";
    let e = contract::compile(src).unwrap_err();
    assert!(e.message.contains("`aria-label`"), "{e}");
}

/// A state part on an element (LLP 1121 D8, `:hover` then its rows) is linted
/// as lowering folds it: a row its state takes passes, checked as the row it
/// lowers to (`:active`'s colour as the press's, which inherits nothing); a
/// row it takes none of, or a name that is no row, is refused (Excord's
/// emoji button, f8b012ef1e).
#[test]
fn an_elements_state_part_is_linted_as_lowering_folds_it() {
    let src = |state: &str, row: &str| {
        format!("component A\n  state n = 0\n  action go\n    n = 1\n  view\n    button appearance=\"none\" press=go aria-label=\"Go\" opacity=0.6\n      :{state}\n        {row}\n      text \"a\"\n")
    };
    let lint = |state: &str, row: &str| {
        contract_lower::lint(&contract_syntax::parse(&src(state, row)).unwrap())
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    };
    for (state, row) in [
        ("hover", "opacity=1"),
        ("hover", "opacity = 1"),
        ("active", "opacity=0.5"),
        ("active", "color=\"#000000\""),
    ] {
        assert!(
            lint(state, row).is_empty(),
            "{state} {row}: {:?}",
            lint(state, row)
        );
        contract::compile(&src(state, row)).unwrap_or_else(|e| panic!("{state} {row}: {e}"));
    }
    // A prop put in before the fold: a row that reads one is the fold's to judge, not the lint's (Astra's re-check).
    let param = "component App\n  view\n    Dim(level=0.8)\ncomponent Dim\n  props\n    level: number\n  view\n    column opacity=0.6\n      :hover\n        opacity=level\n      text \"a\"\n";
    let found = contract_lower::lint(&contract_syntax::parse(param).unwrap());
    assert!(found.is_empty(), "{found:?}");
    contract::compile(param).unwrap_or_else(|e| panic!("{e}"));
    for (state, row, id) in [
        ("hover", "opacty=1", "lower-state-row"),
        ("hover", "width=10", "lower-state-row"),
        ("hover", "color=\"#000000\"", "lower-state-row"),
        ("active", "-exact-press-opacity-to=0.5", "lower-state-row"),
        ("active", "color=\"inherit\"", "lower-attr-value"),
    ] {
        let found = lint(state, row);
        assert_eq!(found.first().copied(), Some(id), "{state} {row}: {found:?}");
        assert!(
            contract::compile(&src(state, row)).is_err(),
            "{state} {row} compiles"
        );
    }
}

/// The lint runs on the unexpanded file when an earlier pass refuses. A
/// shorthand arm that is a child's prop is lowering's to judge once
/// expansion puts the caller's value in; the lint judges the other arms, a
/// derive by its body, and `font`'s type tokens across them as lowering does.
#[test]
fn a_shorthand_read_from_a_name_is_left_to_lowering() {
    let src = |extra: &str| {
        format!(
            "component App\n  state kind = \"heading\"\n  view\n    column\n      Runs(kind=kind)\n{extra}\
             component Runs\n  props\n    kind: string\n  derive face = kind == \"heading\" ? \"-exact-title2\" : \"-exact-body\"\n  view\n    \
             Run(face=face, edge=\"1px solid red\")\n\
             component Run\n  props\n    face: string\n    edge: string\n  derive own = face == \"-exact-body\" ? \"-exact-footnote\" : \"-exact-body\"\n  view\n    \
             text \"a\" font=face border=edge\n    text \"b\" font=own\n    text \"c\" font=(WHICH ? \"-exact-caption1\" : \"-exact-caption2\")\n    \
             text \"d\" font=(WHICH ? \"-exact-body\" : \"twelve\")\n    text \"e\" border=`${{edge}} red`\n"
        )
        .replace("WHICH", "face == \"x\"")
    };
    let lint = |source: &str| {
        contract_lower::lint(&contract_syntax::parse(source).unwrap())
            .into_iter()
            .map(|e| (e.id, e.span.line))
            .collect::<Vec<_>>()
    };
    // Only the literal that is no shorthand, and the computed string, which
    // no expansion makes literal.
    let refused = [("lower-css-shorthand", 21), ("lower-css-shorthand", 22)];
    assert_eq!(lint(&src("")), refused);
    // With an unrelated type error: that error and the same two.
    let found = compile_all("shorthand-name", &src("      text missingName\n"));
    let ids: Vec<(&str, u32)> = found
        .iter()
        .map(|(id, line, _)| (id.as_str(), *line))
        .collect();
    assert_eq!(
        ids,
        [
            ("type-unknown-name", 6),
            ("lower-css-shorthand", 22),
            ("lower-css-shorthand", 23)
        ],
        "{found:#?}"
    );
    // Without the two, it compiles.
    let fine = src("")
        .replace(
            "    text \"d\" font=(face == \"x\" ? \"-exact-body\" : \"twelve\")\n",
            "",
        )
        .replace("    text \"e\" border=`${edge} red`\n", "");
    contract::compile(&fine).unwrap_or_else(|e| panic!("{e}"));
    // Type tokens across the known arms, a prop's arm left out and a
    // derive's body in its place.
    let design =
        "tamber t\n\ntheme\n  type.a = 700 1rem Georgia\n  type.b: type = 1rem\n  space.md = 8px\n";
    let lint_designed = |view: &str| {
        let source = format!(
            "component App\n  view\n    Run(face=\"type.a\", on=true)\ncomponent Run\n  props\n    face: string\n    on: bool\n  state title = \"-exact-body\"\n  derive good = on ? face : \"-exact-body\"\n  derive bad = on ? \"twelve\" : good\n  view\n    {view}\n"
        );
        let mut file = contract_syntax::parse(&source).unwrap();
        file.design = Some(contract_syntax::DesignSource {
            path: "app.tamber".into(),
            home: "app.tamber".into(),
            text: design.into(),
            source_id: 1,
            bindable: false,
            package: None,
            root: None,
        });
        contract_lower::lint(&file)
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    };
    let font = |value: &str| format!("text \"x\" font={value}");
    for (view, refused) in [
        (font("(on ? face : \"type.a\")"), None),
        (font("good"), None),
        (font("bad"), Some("lower-css-shorthand")),
        // A name no expansion replaces (a state's, a binder's, though it
        // shadows a prop) is no literal at any stage.
        (
            font("(on ? \"-exact-body\" : title)"),
            Some("lower-css-shorthand"),
        ),
        (
            font("(match pick { case some(face) => face, case none => \"-exact-body\" })"),
            Some("lower-css-shorthand"),
        ),
        (
            format!("for face in faces key=face\n      {}", font("face")),
            Some("lower-css-shorthand"),
        ),
        (
            font("(on ? \"type.a\" : \"type.b\")"),
            Some("tamber-type-longhands"),
        ),
        (
            font("(on ? \"type.a\" : \"bold 12px serif\")"),
            Some("tamber-kind"),
        ),
        (
            font("(on ? \"-exact-body\" : \"space.md\")"),
            Some("lower-css-shorthand"),
        ),
    ] {
        assert_eq!(lint_designed(&view), Vec::from_iter(refused), "{view}");
    }
    // A chain of derives each reading the last twice is read within a budget,
    // and a derive read in its own body is the type pass's to refuse.
    let chain: String = (1..40)
        .map(|i| format!("  derive d{i} = on ? d{} : d{}\n", i - 1, i - 1))
        .collect();
    let source = format!(
        "component App\n  view\n    Run(on=true)\ncomponent Run\n  props\n    on: bool\n  derive d0 = on ? \"-exact-body\" : d0\n{chain}  view\n    text \"x\" font=d39\n"
    );
    let found = contract_lower::lint(&contract_syntax::parse(&source).unwrap());
    assert!(found.is_empty(), "{found:?}");
    // A platform token written unquoted reads as a minus and a name: said so.
    let unknown = |value: &str| {
        contract::compile(&format!(
            "component A\n  view\n    text \"a\" color={value}\n"
        ))
        .unwrap_err()
    };
    let e = unknown("-exact-label");
    assert_eq!(e.id, "type-unknown-name");
    assert!(e.message.contains("write `\"-exact-label\"`"), "{e}");
    // From the minus through the name.
    assert_eq!((e.span.col, e.span.end_col), (20, 32), "{e:?}");
    // Not where a shorter name is in scope: a subtraction missing its spaces.
    let e = contract::compile(
        "component A\n  state exact-a = 1\n  view\n    text \"a\" opacity=-exact-a-exact-b\n",
    )
    .unwrap_err();
    assert!(e.message.contains("subtraction"), "{e}");
}

/// What a React Native port writes first is refused with what to write
/// (the euxy port, 2026-10-17: 19 of 50 compiles refused).
#[test]
fn a_react_native_habit_is_refused_with_what_to_write() {
    for (line, id, needle) in [
        (
            "text \"a\" numberOfLines=2",
            "lower-unknown-attr",
            "`line-clamp`",
        ),
        (
            "text \"a\" ellipsizeMode=\"tail\"",
            "lower-unknown-attr",
            "`text-overflow`",
        ),
        (
            "image \"a.png\" resizeMode=\"cover\"",
            "lower-unknown-attr",
            "`object-fit`",
        ),
        (
            "text \"a\" textTransform=\"uppercase\"",
            "lower-unknown-attr",
            "`text-transform`",
        ),
        (
            "view paddingHorizontal=16",
            "lower-unknown-attr",
            "`padding-left` and `padding-right`",
        ),
        (
            "view marginVertical=8",
            "lower-unknown-attr",
            "`margin-top` and `margin-bottom`",
        ),
        ("view shadowRadius=4", "lower-unknown-attr", "`box-shadow="),
        ("view elevation=4", "lower-attr-tag", "`box-shadow="),
        (
            "text \"a\" selectable=true",
            "lower-unknown-attr",
            "`user-select=\"text\"`",
        ),
        (
            "text \"a\" font-weight=\"bold\"",
            "lower-attr-value",
            "write `font-weight=700`",
        ),
        (
            "text \"a\" font-weight=\"normal\"",
            "lower-attr-value",
            "write `font-weight=400`",
        ),
    ] {
        let src = format!("component A\n  view\n    column\n      {line}\n");
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, id, "{line}: {e}");
        assert!(e.message.contains(needle), "{line}: {e}");
    }
    // A keyframe's unit needs quotes, as an element's does; and a keyframe is one line.
    let src = "keyframes W\n  from rotate=-2deg\n  to rotate=2deg\ncomponent A\n  view\n    text \"a\" animation=\"W 1s\"\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "syntax-unquoted-length", "{e}");
    assert!(e.message.contains("`rotate=\"-2deg\"`"), "{e}");
    let src = "keyframes W\n  from\n    opacity=0\ncomponent A\n  view\n    text \"a\" animation=\"W 1s\"\n";
    let e = contract::compile(src).unwrap_err();
    assert!(e.message.contains("a keyframe is one line"), "{e}");
}

#[test]
fn a_literal_value_is_checked_against_its_row_at_compile_time() {
    for (src, id, needle) in [
        (
            "component A\n  view\n    column width=true\n      text \"a\"\n",
            "lower-attr-value",
            "not a bool",
        ),
        (
            "component A\n  view\n    column align-items=\"middle\"\n      text \"a\"\n",
            "lower-attr-value",
            "`align-items`",
        ),
        (
            "component A\n  view\n    column background-color=\"blurple\"\n      text \"a\"\n",
            "lower-attr-value",
            "a named colour",
        ),
        (
            "component A\n  view\n    column padding=\"auto\"\n      text \"a\"\n",
            "lower-attr-value",
            "auto",
        ),
        (
            "component A\n  state on = true\n  view\n    column width=on\n      text \"a\"\n",
            "lower-attr-type",
            "bool",
        ),
        (
            "component A\n  view\n    column disabled=1\n      text \"a\"\n",
            "lower-attr-type",
            "a bool",
        ),
    ] {
        let e = contract::compile(src).unwrap_err();
        assert_eq!(e.id, id, "{src}: {e}");
        assert!(e.message.contains(needle), "{src}: {e}");
    }
    // What passes: numbers, percentages, `auto`, hex colours, enum names,
    // and a computed number or string.
    let src = "component A\n  state w = 10\n  state c = \"#fff\"\n  view\n    column width=w height=\"50%\" max-width=\"auto\" background-color=c align-items=\"center\" flex=1\n      text \"a\" font-size=14 color=\"#00000080\"\n";
    contract::compile(src).unwrap();
}

#[test]
fn a_hyphenated_unknown_name_says_why() {
    let src =
        "component A\n  state a = 1\n  state b = 2\n  derive c = a-b\n  view\n    text `${c}`\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-unknown-name");
    assert!(e.message.contains("`a - b`"), "{e}");
    let src =
        "component A\n  state a = 1\n  state b = 2\n  derive c = a - b\n  view\n    text `${c}`\n";
    contract::compile(src).unwrap();
}

#[test]
fn bake_refuses_a_scroll_that_only_a_row_could_have_bounded_and_did_not() {
    // The compiler's rule lets this through (the parent is a `row`, whose
    // stretch could bound it); at the first frame the row is content-sized,
    // so the scroll is exactly as tall as its children.
    let src = "component A\n  view\n    row testId=\"root\"\n      scroll testId=\"rows\"\n        column\n          text \"one\"\n          text \"two\"\n          text \"three\"\n";
    let plan = contract::compile(src).unwrap();
    let e = contract::bake(plan, NoData).unwrap_err();
    match e {
        BakeError::Lint { id, message, .. } => {
            assert_eq!(id, "bake-scroll-unbounded");
            assert!(message.contains("testId=\"rows\""), "{message}");
        }
        other => panic!("{other:?}"),
    }
    // Bounded by the row: fine.
    let src = "component A\n  view\n    row height=40 testId=\"root\"\n      scroll testId=\"rows\"\n        column\n          text \"one\"\n          text \"two\"\n          text \"three\"\n";
    let plan = contract::compile(src).unwrap();
    contract::bake(plan, NoData).unwrap();
}

#[test]
fn a_horizontal_scroll_can_grow_vertically_with_its_contents() {
    let src = "component A\n  view\n    column width=200\n      scroll width=200 overflow-x=\"scroll\" overflow-y=\"hidden\" scroll-snap-type=\"x mandatory\"\n        row width=400\n          box width=200 height=80 scroll-snap-align=\"start\"\n          box width=200 height=80\n";
    contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
}

#[test]
fn bake_refuses_a_pressable_with_zero_area() {
    let src = "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    column\n      button press=go width=0 height=0 testId=\"go\"\n";
    let plan = contract::compile(src).unwrap();
    let e = contract::bake(plan, NoData).unwrap_err();
    match e {
        BakeError::Lint { id, message, .. } => {
            assert_eq!(id, "bake-zero-size");
            assert!(message.contains("testId=\"go\""), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// Ledger2 Rough 4: an absolutely positioned button is sized by its insets
/// (a modal's backdrop, `inset=0`), so neither the compiler's check nor
/// the bake's refuses it; a relatively positioned one with `inset` still has
/// no size, and one with only `top` and `bottom` keeps its zero width.
#[test]
fn an_inset_sized_button_has_area() {
    for attrs in [
        "position=\"absolute\" inset=0",
        "position=\"absolute\" top=0 bottom=0 left=0 right=0",
        "position=(open ? \"absolute\" : \"sticky\") inset=0",
    ] {
        let src = format!("component A\n  state open = true\n  action close\n    open = false\n  view\n    column position=\"relative\" width=200 height=200\n      button press=close {attrs} background-color=\"rgba(0,0,0,0.4)\" testId=\"backdrop\"\n");
        let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{attrs}: {e}"));
        contract::check(&plan).unwrap_or_else(|e| panic!("{attrs}: {e:?}"));
    }
    let e = contract::compile("component A\n  action close\n    let x = 1\n  view\n    button press=close position=\"relative\" inset=0\n")
        .unwrap_err();
    assert!(format!("{e}").contains("zero area"), "{e}");
}

/// The web's JS target bakes nothing; `contract::check` refuses there what
/// every native bake would, before the page's module answers (files diary
/// F13). Hidden controls and descendants have no pressable layout until shown.
#[test]
fn check_refuses_without_data_what_no_answer_could_change() {
    let hidden = "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    column\n      button \"Down\" press=go display=\"none\" aria-keyshortcuts=\"Shift+ArrowDown\" testId=\"down\"\n";
    contract::check(&contract::compile(hidden).unwrap()).unwrap();
    let nested = hidden
        .replace("    column\n", "    column display=\"none\"\n")
        .replace(" display=\"none\" aria-keyshortcuts", " aria-keyshortcuts");
    contract::check(&contract::compile(&nested).unwrap()).unwrap();
    contract::bake(contract::compile(&nested).unwrap(), NoData).unwrap();
    // A label the module answers is empty until it does: not this check's
    // to judge, nor a `scroll` that only its answered rows reach the bound
    // of (unanswered, it is exactly as tall as its header).
    let answered = "component A\n  resource title = title() as shape string\n  resource rows = rows() as shape string[]\n  state n = 0\n  action go\n    n = 1\n  view\n    column\n      button press=go padding=0 border-width=0 testId=\"title\"\n        text title\n      row max-height=300\n        scroll\n          column\n            text \"Header\"\n            for r in rows key=r\n              text r\n";
    contract::check(&contract::compile(answered).unwrap()).unwrap();
}

/// Review C2: `check` excuses only a source that answers in the page. Any
/// other refusal at boot is the bake's too, and fails the web build (here a
/// plan for another kernel schema, refused before a frame).
#[test]
fn check_refuses_a_boot_the_bake_refuses() {
    let mut plan = contract::compile("component A\n  view\n    text \"a\"\n").unwrap();
    contract::check(&plan).unwrap();
    plan.kernel_schema_digest ^= 1;
    match contract::check(&plan).unwrap_err() {
        BakeError::Runner(exact_runner::RunnerError::KernelSchemaMismatch { .. }) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn conditional_style_literals_are_refused_at_the_offending_branch() {
    for (property, value, bad) in [
        ("top", r#"(on ? "0pxx" : "0%")"#, "0pxx"),
        ("top", r#"(on ? "0%" : "0pxx")"#, "0pxx"),
        (
            "align-items",
            r#"(on ? "center" : (on ? "flex-end" : "middle"))"#,
            "middle",
        ),
        (
            "padding",
            r#"(match maybe { case some(n) => n, case none => "auto" })"#,
            "auto",
        ),
        (
            "top",
            r#"(match maybe { case some(n) => "0pxx", case none => n })"#,
            "0pxx",
        ),
    ] {
        let source = format!("component App\n  state on = false\n  state n = \"0%\"\n  state maybe = some(\"10%\")\n  view\n    column {property}={value}\n      text \"branch\"\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{source}: {error}");
        assert!(error.message.contains(bad), "{error}");
        let line = source.lines().nth(5).unwrap();
        assert_eq!(
            (error.span.line, error.span.col),
            (6, (line.find(&format!("\"{bad}\"")).unwrap() + 1) as u32),
            "{error}"
        );
    }
}

#[test]
fn conditional_style_checks_preserve_computation_and_match_bindings() {
    let source = r#"component App
  state on = false
  state n = true
  state maybe = some(20)
  action toggle
    on = !on
  view
    column testId="branch" top=(on ? -10 : (match maybe { case some(n) => n, case none => 0 })) align-items=(on ? "center" : "flex-end")
      text "branch"
"#;
    let plan = contract::compile(source).unwrap();
    let mut runner = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let top = |runner: &exact_runner::Runner<NoData>| {
        let kernel = runner.kernel();
        let key = kernel.find_by_test_id("branch")[0];
        kernel.node_by_key(key).unwrap().style.top
    };
    assert_eq!(top(&runner), exact_kernel::Dimension::Points(20.0));
    runner.act("toggle", vec![]).unwrap();
    assert_eq!(top(&runner), exact_kernel::Dimension::Points(-10.0));
    assert!(!runner.is_poisoned());
}

#[test]
fn a_platform_colour_is_a_plan_literal_never_data() {
    // LLP 1095 D3: state may choose between the plan's literals; a string
    // that came from an action's argument names no platform colour.
    let source = r##"component App
  state on = false
  state named = "#000000"
  action toggle
    on = !on
  action name(v: string)
    named = v
  view
    column
      text "a" testId="chosen" color=(on ? "-exact-platform-color(ios lintTestOnColor, #010203)" : "-exact-platform-color(ios lintTestOffColor, #040506)")
      text "b" testId="named" color=named
"##;
    let mut runner = exact_runner::Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let color = |runner: &exact_runner::Runner<NoData>, id: &str| {
        let kernel = runner.kernel();
        let key = kernel.find_by_test_id(id)[0];
        kernel.node_by_key(key).unwrap().style.text_color
    };
    let platform = |c: exact_kernel::style::ColorValue| {
        let exact_kernel::style::ColorValue::Platform(id) = c else {
            return None;
        };
        exact_kernel::style::roles::platform(id).and_then(|p| p.ios.clone())
    };
    assert_eq!(
        platform(color(&runner, "chosen")).as_deref(),
        Some("lintTestOffColor")
    );
    runner.act("toggle", vec![]).unwrap();
    assert_eq!(
        platform(color(&runner, "chosen")).as_deref(),
        Some("lintTestOnColor")
    );
    runner
        .act(
            "name",
            vec![Value::str(
                "-exact-platform-color(ios lintTestDataColor, #070809)",
            )],
        )
        .unwrap();
    let named = color(&runner, "named");
    assert_eq!(
        platform(named),
        None,
        "data names no platform colour: {named:?}"
    );
    assert!(!runner.is_poisoned());
}

#[test]
fn conditional_pixel_lengths_compile_and_update() {
    for expression in [
        r#"(on ? "0px" : "20px")"#,
        r#"(match maybe { case some(n) => "0px", case none => "20px" })"#,
    ] {
        let source = format!(
            r#"component App
  state on = false
  state maybe = none
  action toggle
    on = !on
    maybe = some(1)
  view
    column testId="branch" top={expression}
      text "branch"
"#
        );
        let mut runner = exact_runner::Runner::boot(
            contract::compile(&source).unwrap(),
            NoData,
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        for expected in [20.0, 0.0] {
            let key = runner.kernel().find_by_test_id("branch")[0];
            assert_eq!(
                runner.kernel().node_by_key(key).unwrap().style.top,
                exact_kernel::Dimension::Points(expected)
            );
            runner.act("toggle", vec![]).unwrap();
        }
        assert!(!runner.is_poisoned());
    }
}

#[test]
fn a_bound_string_on_a_number_row_is_refused_as_its_literal_is() {
    let app = |node: &str| {
        format!("component A\n  state size = 16\n  state label = \"2\"\n  action grow\n    size = 20\n  view\n    column\n      {node}\n")
    };
    // No native host reads text on these rows; a browser would apply it.
    for (node, name) in [
        ("text \"a\" opacity=`${size / 20}`", "opacity"),
        ("column flex-grow=label", "flex-grow"),
        ("text \"a\" z-index=label", "z-index"),
        ("text \"a\" font-weight=`${size}0`", "font-weight"),
        ("view column-count=label", "column-count"),
        ("view column-rule-width=`${size}px`", "column-rule-width"),
    ] {
        let e = contract::compile(&app(node)).unwrap_err();
        assert_eq!(e.id, "lower-attr-type", "{node}: {e}");
        assert!(
            e.message.contains(&format!("`{name}` takes a number")),
            "{node}: {e}"
        );
    }
    // A pixel row reads `<n>px` where it binds, as the browser does.
    let mut runner = exact_runner::Runner::boot(
        contract::compile(&app(
            "text \"a\" testId=\"t\" font-size=`${size}px` letter-spacing=`${size / 16}px`",
        ))
        .unwrap(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for expected in [16.0, 20.0] {
        let key = runner.kernel().find_by_test_id("t")[0];
        let style = &runner.kernel().node_by_key(key).unwrap().style;
        assert_eq!(
            (style.font_size, style.letter_spacing),
            (expected, expected / 16.0)
        );
        runner.act("grow", vec![]).unwrap();
    }
    assert!(!runner.is_poisoned());
}

#[test]
fn number_only_style_choices_reject_computed_string_arms() {
    let app = |node: &str| {
        format!(
            "component A\n  state on = true\n  state label = \"bad\"\n  state maybe = some(0.5)\n  view\n    {node}\n"
        )
    };
    for node in [
        "view opacity=(on ? 0.5 : label)",
        "view opacity=(on ? 0.5 : (on ? label : 1))",
        "view opacity=(match maybe { case some(x) => x, case none => (on ? label : 1) })",
        "view opacity=(match maybe { case some(x) => x, case none => label })",
        "view flex-grow=(on ? 1 : label)",
        "view opacity=`${label}`",
        "view opacity=label.toString()",
    ] {
        let e = contract::compile(&app(node)).unwrap_err();
        assert_eq!(e.id, "lower-attr-type", "{node}: {e}");
        assert!(e.message.contains("string"), "{node}: {e}");
    }
    contract::compile(&app("view opacity=(on ? 0.5 : 0.75)")).unwrap();
    contract::compile(&app("view font-size=(on ? 12 : \"14px\")")).unwrap();
}

// The type checker refuses these forms itself (`type-branches`), before
// lowering; this pins that refusal. The lowerer's own walk is tested by
// `mixed_types_inside_computed_arms_and_match_subjects_are_found`.
#[test]
fn number_only_style_choices_reject_mixed_nested_expressions() {
    let app = |node: &str| {
        format!("component A\n  state on = true\n  state label = \"0.75\"\n  view\n    {node}\n")
    };
    for node in [
        "view opacity=`${on ? label : 1}`",
        "view opacity=(on ? label : 1).toString()",
        "view opacity=(on ? 0.5 : `${on ? label : 1}`)",
        "view opacity=(match (on ? some(label) : some(1)) { case some(x) => x, case none => 0 })",
    ] {
        let e = contract::compile(&app(node)).unwrap_err();
        assert_eq!(e.id, "type-branches", "{node}: {e}");
    }
}

#[test]
fn number_only_style_choices_accept_valid_css_wide_keywords() {
    let app = |node: &str| format!("component A\n  state on = true\n  view\n    {node}\n");
    for node in [
        "view opacity=(on ? 0.5 : \"unset\")",
        "text \"w\" font-weight=(on ? 700 : \"inherit\")",
        "view opacity=(on ? 0.5 : (on ? \"unset\" : 1))",
        "view opacity=(on ? \"unset\" : \"unset\")",
    ] {
        contract::compile(&app(node)).unwrap_or_else(|e| panic!("{node}: {e}"));
    }
}

/// The `style=` hint's suggestion is read back as a node's attributes and
/// compiled: what it suggests must compile (the euxy port, review round 2).
#[test]
fn the_style_hint_suggests_only_what_compiles() {
    let hint = |style: &str| {
        let src = format!("component A\n  view\n    text \"a\" style={style:?}\n");
        let e = contract::compile(&src).expect_err("`style=` is refused");
        let message = e.to_string();
        assert!(message.contains("has no attribute `style`"), "{message}");
        message
    };
    let compiles = |node: &str| {
        let src = format!("style Title\n  font-weight=600\n\ncomponent A\n  view\n    {node}\n");
        contract::compile(&src).unwrap_or_else(|e| panic!("{node}: {e}"));
    };
    let suggested = |message: &str, lead: &str| -> String {
        let at = message
            .find(lead)
            .unwrap_or_else(|| panic!("{lead:?} in {message}"))
            + lead.len();
        message[at..].split('`').next().unwrap().to_string()
    };
    for (style, attrs) in [
        // Split at `;` outside quotes and parentheses (a string attribute
        // keeps CSS's quotes, as a quoted family name does).
        (
            "font-family: \"Helvetica Neue\", Arial; color: red",
            "font-family=\"\\\"Helvetica Neue\\\", Arial\" color=\"red\"",
        ),
        (
            "aria-label: \"Save; later\"; color: red",
            "aria-label=\"\\\"Save; later\\\"\" color=\"red\"",
        ),
        (
            "aria-label: f(a; b); width: calc(100% - 16px)",
            "aria-label=\"f(a; b)\" width=\"calc(100% - 16px)\"",
        ),
        // `!important` is dropped from a property written once.
        (
            "color: red !important; padding: 8px ! important",
            "color=\"red\" padding=\"8px\"",
        ),
        (
            "font-size: 14px; font-weight: 600; line-height: 1.4",
            "font-size=\"14px\" font-weight=600 line-height=1.4",
        ),
        // String shorthands keep quotes even when their CSS value is a number.
        ("border: 0", "border=\"0\""),
        ("border-top: 0", "border-top=\"0\""),
        ("border-right: 0", "border-right=\"0\""),
        ("border-bottom: 0", "border-bottom=\"0\""),
        ("border-left: 0", "border-left=\"0\""),
        ("columns: 2", "columns=\"2\""),
        ("columns: 1", "columns=\"1\""),
        ("columns: 65535", "columns=\"65535\""),
        ("columns: 02", "columns=\"02\""),
        ("columns: 12px", "columns=\"12px\""),
        ("column-rule: 0", "column-rule=\"0\""),
        (
            "column-count: 2; column-rule-width: 0; padding: 8",
            "column-count=2 column-rule-width=0 padding=8",
        ),
        // A custom property is not an attribute; the others are.
        ("--gap: 8px; padding: 8px", "padding=\"8px\""),
        // A plain number is unquoted, as the lexer wants it.
        (
            "-exact-press-scale: 0.97; opacity: 0.5; flex: 1; color: red",
            "-exact-press-scale=0.97 opacity=0.5 flex=1 color=\"red\"",
        ),
    ] {
        let message = hint(style);
        assert_eq!(
            suggested(&message, "write the declarations as attributes, `"),
            attrs,
            "{style}: {message}"
        );
        compiles(&format!("text \"a\" {attrs}"));
    }
    let message = hint("Title");
    assert_eq!(suggested(&message, "write `"), "class=Title");
    compiles("text \"a\" class=Title");
    // No rewrite for properties that may overlap, which CSS resolves by
    // importance and order and attributes do not: one written twice, a
    // shorthand beside a longhand of its own, by prefix or by family.
    for style in [
        "padding: 8px !important; padding: 0",
        "padding: 8px; padding: 0",
        "padding-left: 16px !important; padding: 0",
        "flex-direction: column; flex: 1 !important",
        "border-top: 1px solid red; border-top-left-radius: 8px",
        "border-radius: 8px; border-top-left-radius: 0",
        "font: 14px Inter; line-height: 1.4",
        "top: 4px; inset: 0",
        "row-gap: 4px; gap: 8px",
    ] {
        let message = hint(style);
        assert!(!message.contains("write "), "{style}: {message}");
        assert!(
            message.contains("a look nodes share is a `style Name` block"),
            "{style}: {message}"
        );
    }
    // Nothing to suggest: a custom property alone, a name with a quote or
    // brace, a reserved word.
    for style in [
        "--gap: 8px",
        "a\"b: 1",
        "a{b}: 1",
        "none",
        "if",
        "true",
        "8px",
    ] {
        let message = hint(style);
        assert!(!message.contains("write "), "{style}: {message}");
        assert!(
            message.contains("a look nodes share is a `style Name` block"),
            "{style}: {message}"
        );
    }
}

#[test]
fn the_style_hint_keeps_importance_for_properties_with_shared_rows() {
    for (a, b) in [
        ("border-color: red", "border-bottom-color: blue"),
        ("border-width: 2px", "border-top: 1px solid red"),
        ("border-top: 1px solid red", "border-color: blue"),
        ("animation: pulse 1s", "animation-duration: 2s"),
        ("animation-duration: 1s", "animation-duration: 2s"),
    ] {
        for (first, last) in [(a, b), (b, a)] {
            let style = format!("{first} !important; {last}");
            let src = format!("component A\n  view\n    text \"a\" style={style:?}\n");
            let e = contract::compile(&src).unwrap_err();
            assert_eq!(e.id, "lower-unknown-attr");
            assert!(!e.message.contains("write "), "{style}: {e}");
            assert!(e
                .message
                .contains("a look nodes share is a `style Name` block"));
        }
    }
    // Distinct animation longhands compose into parts of one row without competing.
    let app = |attrs: &str| {
        format!("keyframes pulse\n  to opacity=0\ncomponent A\n  view\n    text \"a\" {attrs}\n")
    };
    let longhands: Vec<_> = exact_motion::animation::LONGHANDS
        .into_iter()
        .zip([
            "pulse", "1s", "linear", "0s", "infinite", "reverse", "both", "paused",
        ])
        .collect();
    for (i, a) in longhands.iter().enumerate() {
        for b in &longhands[i + 1..] {
            for ((first, x), (last, y)) in [(a, b), (b, a)] {
                for importance in ["", " !important"] {
                    let style = format!("{first}: {x}{importance}; {last}: {y}");
                    let e = contract::compile(&app(&format!("style={style:?}"))).unwrap_err();
                    let attrs = format!("{first}=\"{x}\" {last}=\"{y}\"");
                    assert!(
                        e.message
                            .contains(&format!("write the declarations as attributes, `{attrs}`")),
                        "{style}: {e}"
                    );
                    contract::compile(&app(&attrs)).unwrap_or_else(|e| panic!("{attrs}: {e}"));
                }
            }
        }
    }
}

#[test]
fn the_style_hint_does_not_turn_invalid_column_counts_into_widths() {
    let app = |attrs: &str| format!("component A\n  view\n    text \"a\" {attrs}\n");
    for value in ["2.0", "2.5", "2.", "0.5", "0", "65536"] {
        let style = format!("columns: {value}");
        let e = contract::compile(&app(&format!("style={style:?}"))).unwrap_err();
        let attrs = format!("columns={value}");
        assert!(
            e.message
                .contains(&format!("write the declarations as attributes, `{attrs}`")),
            "{style}: {e}"
        );
        contract::compile(&app(&attrs)).expect_err("invalid counts must remain refused");
    }
}

#[test]
fn refusals_name_what_the_author_wrote_and_suggest_one_repair() {
    let app = |state: &str, node: &str| {
        format!("shape Todo\n  title: string\ncomponent A\n  state draft = \"\"\n{state}  view\n    column\n      {node}\n")
    };
    for (src, id, message) in [
        (app("", "text drat"), "type-unknown-name", "unknown name `drat`; did you mean `draft`?"),
        (app("", "text Draft"), "type-unknown-name", "unknown name `Draft`; did you mean `draft`?"),
        (app("  state drab = \"\"\n", "text drat"), "type-unknown-name", "unknown name `drat`"),
        (app("  state t = some(\"a\")\n", "buton"), "lower-unknown-tag", "unknown tag `buton`; did you mean `button`?"),
        (app("", "div"), "lower-unknown-tag", "unknown tag `div`; a flex container is `column` or `row`, and a plain box `view`"),
        // An SVG element's own attribute on a box is refused by name, not
        // kept as a prop no host reads; CSS `order` is a box's (feed F19).
        (app("", "view mode=\"multiply\""), "lower-attr-tag", "`mode` is an SVG element's attribute; it does nothing on `view`"),
        (app("", "view mask=\"url(#m)\""), "lower-attr-tag", "`mask` masks SVG elements so far; a box takes `mask-image` (a gradient)"),
        // HTML's and CSS's own spellings (feed F1): `alt` is an image's,
        // and `inherit` takes what the parent computed, which only an
        // inherited row has.
        (app("", "view alt=\"x\""), "lower-attr-tag", "`alt` belongs to `image`, not `view`; another element's accessible name is `aria-label`"),
        (app("", "view background-color=\"inherit\""), "lower-attr-value", "`background-color=\"inherit\"`: `background-color` does not inherit, and exact2 inherits only the rows CSS inherits; write the value"),
        (app("", "text \"a\" color=\"bleu\""), "lower-attr-value", "`color=\"bleu\"` is not a valid `color`: a color is a CSS colour: hex, `rgb()`, `hsl()`, `hwb()`, a named colour, `transparent`, or one in its own space: `color(display-p3 1 0 0)`, `oklch()`, `oklab()`, `lab()`, `lch()` (LLP 1100) — `light-dark(a, b)` of two, a role (`\"-exact-secondary-label\"`, `\"CanvasText\"`: LLP 1095, LLP 1081), or `-exact-platform-color(ios <name>Color, …, <fallback>)` written whole as a string literal"),
        (app("", "text \"a\" color=`-exact-platform-color(ios ${draft}Color, #000)`"), "lower-platform-color-literal", "`color`: write `-exact-platform-color(…)` whole, as a string literal (a branch of `?:` or `match` may be one); it is never built from a template, a concatenation or data, so the platform colours a plan names are fixed when it compiles (LLP 1095 D3)"),
        // `14px` on a pixel row is CSS's (LLP 1102 §3.10); a negative font size is not.
        (app("", "text \"a\" font-size=\"-2px\""), "lower-attr-value", "`font-size=\"-2px\"` is not a valid `font-size`: expected a nonnegative length"),
        (app("", "text \"a\" width=10px"), "syntax-unquoted-length", "`width=10px` needs quotes: a value with a unit is a string, `width=\"10px\"` (a bare number is pixels)"),
        (app("", "text \"a\" className=\"x\""), "lower-unknown-attr", "`text` has no attribute `className`; `class` names a `style` declared in this file, as in `class=Card`"),
        // The euxy port's `style=`: the word is `class=`, or the declarations are attributes.
        (app("", "text \"a\" style=\"title\""), "lower-unknown-attr", "`text` has no attribute `style`; write `class=title`: a look nodes share is a `style Name` block in the file, named with `class=Name`, and a node's own look is its attributes"),
        (app("", "text \"a\" style=\"padding: 8px; color: red;\""), "lower-unknown-attr", "`text` has no attribute `style`; write the declarations as attributes, `padding=\"8px\" color=\"red\"`: a node's look is its attributes, and a look nodes share is a `style Name` block in the file, named with `class=Name`"),
        (
            app("  state n = 0\n", "text n"),
            "lower-attr-type",
            "`text` takes a string; this expression is `number`: interpolate it in a template, `${…}`, or write `x.toString()`",
        ),
        (
            app("", "if draft = \"x\"\n        text \"a\""),
            "syntax-expected-newline",
            "expected end of line, found `=`; `==` compares, and only a statement assigns",
        ),
        // The euxy port's `after(0, …)`: 0 would be due in the commit that armed it.
        (
            app("  task t mount\n    after(0, tick)\n  action tick\n    draft = \"x\"\n", "text draft"),
            "lower-timer-interval",
            "`after` needs a whole number of milliseconds, at least 1; given 0: write `after(1, …)`, the soonest, which runs a commit after the one that armed it, as the web's `setTimeout(f, 0)` runs after the current task",
        ),
        (
            app("", "button press={add}"),
            "syntax-expected-expression",
            "expected an expression, found `{`; Contract has no `{…}`: a handler names an action (`press=add`, `press=add(item)`) and a value is written directly (`width=10`)",
        ),
        // The app farm's guesses (round 1): a tag, a count, a placeholder.
        (app("", "heading \"T\""), "lower-unknown-tag", "unknown tag `heading`; a heading is `h1` (to `h6` for the level)"),
        (app("", "grid"), "lower-unknown-tag", "unknown tag `grid`; a grid is `view display=\"grid\"` with `grid-template-columns`, or a `column` of `row`s"),
        (app("  state xs = [1]\n", "text count(xs).toString()"), "type-refused-idiom", "write `x.length`, of text or of a list"),
        (app("  state xs = [1]\n", "text xs.size.toString()"), "type-not-a-record", "`number[]` has no fields: write `x.length`, of text or of a list"),
        (
            app("  resource todos = todosOf() as shape Todo[] else []\n", "text \"a\""),
            "syntax-placeholder-call",
            "a resource's `else` is a call: `else empty()` is its shape's zero (`[]` for a list, `\"\"` for text), with `field=constant` overrides for a record, or `else source(values…)`",
        ),
        (
            app("  resource todo = todoOf() as shape Todo else Todo(title=\"…\")\n", "text todo.title"),
            "type-placeholder-shape",
            "`todo`'s placeholder calls the shape `Todo`: write `else empty(field=…)`, the shape's zero with `field=constant` overrides",
        ),
    ] {
        let e = contract::compile(&src).unwrap_err();
        assert_eq!((e.id.as_str(), e.message.as_str()), (id, message), "{src}");
    }
}

/// A prop written with a default (the euxy port's) is told props have none,
/// and the `option` form the refusal names compiles.
#[test]
fn a_prop_default_is_refused_naming_the_option_form() {
    let app = |props: &str, read: &str, pass: &str| {
        format!("component A\n  view\n    Toggle({pass})\ncomponent Toggle\n  props\n    {props}\n  view\n    button \"x\" disabled={read}\n")
    };
    let e =
        contract::compile(&app("disabled: bool = false", "disabled", "disabled=true")).unwrap_err();
    assert_eq!(
        (e.id.as_str(), e.message.as_str()),
        ("syntax-expected-newline", "a prop has no default: pass `disabled=<value>` at every use, or declare `disabled: option<bool>`, pass `disabled=none` where it is unset, and read it as `match disabled { case some(v) => v, case none => <default> }`"),
    );
    contract::compile(&app(
        "disabled: option<bool>",
        "(match disabled { case some(v) => v, case none => false })",
        "disabled=none",
    ))
    .unwrap();
    // The other form it names, `disabled=<value>` with a value, compiles too.
    contract::compile(&app("disabled: bool", "disabled", "disabled=true")).unwrap();
}

/// `style=`'s declarations come back as attributes that compile: a quote, a
/// backslash or another control character in a value escaped as a string
/// literal takes it, and a run of whitespace (a newline) one space, as CSS
/// reads it.
#[test]
fn style_declarations_are_suggested_as_attributes_that_compile() {
    let app = |attrs: &str| format!("component A\n  view\n    text \"a\" {attrs}\n");
    for (written, suggested) in [
        (
            r#"style="font-family: \"Times New Roman\", serif; color: red""#,
            r#"font-family="\"Times New Roman\", serif" color="red""#,
        ),
        (
            r#"style="padding: 8px\n 16px;\tmargin:\u{d}\n4px""#,
            r#"padding="8px 16px" margin="4px""#,
        ),
        (r#"style="aria-label: a\u{1}b""#, r#"aria-label="a\u{1}b""#),
    ] {
        let e = contract::compile(&app(written)).unwrap_err();
        let edit = e
            .message
            .split('`')
            .nth(5)
            .unwrap_or_else(|| panic!("{}", e.message));
        assert_eq!(edit, suggested, "{written}");
        contract::compile(&app(edit)).unwrap_or_else(|e| panic!("{edit}: {e}"));
    }
}

#[test]
fn the_element_lint_finds_nothing_in_any_app() {
    let apps = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
    let mut checked = 0;
    for entry in std::fs::read_dir(apps).unwrap() {
        let path = entry.unwrap().path().join("app.contract");
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut file = contract_syntax::parse(&src).unwrap();
        // The app's design is the driver's to read (LLP 1121 D2), as `contract build` does.
        let design = path.with_file_name("app.tamber");
        file.design =
            std::fs::read_to_string(&design)
                .ok()
                .map(|text| contract_syntax::DesignSource {
                    path: design.display().to_string(),
                    home: "app.tamber".into(),
                    text,
                    source_id: 1,
                    bindable: false,
                    package: None,
                    root: None,
                });
        let found = contract_lower::lint(&file);
        assert!(found.is_empty(), "{}: {found:?}", path.display());
        checked += 1;
    }
    assert!(checked >= 16, "{checked} apps");
}

/// A file of its own, so `use`, fonts and the manifest resolve as for an app.
fn compile_all(name: &str, source: &str) -> Vec<(String, u32, String)> {
    let dir = std::env::temp_dir().join(format!("exact-plural-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, source).unwrap();
    let Err(errors) = contract::compile_path_all(&path, false) else {
        panic!("{name} compiled");
    };
    std::fs::remove_dir_all(&dir).unwrap();
    errors
        .into_iter()
        .map(|e| (e.id, e.span.line, e.message))
        .collect()
}

const TODOS: &str = r#"shape Todo
  id: string
  title: string
component App
  state draft = ""
  state count = 0
  resource todos = items(3) as shape Todo[]
  action add
    count = count + 1
  view
    main padding=16 gap=8 className="x"
      input value=drat
      buton press=add
        text "Add" color="bleu"
      for t in todos key=t.id
        Row(todo=t, onPik=add)
component Row
  props
    todo: Todo
    onPick: action
  view
    button press=onPick
      text todo.titel
"#;

#[test]
fn one_run_reports_every_independent_mistake_call_sites_first() {
    let found = compile_all("mistakes", TODOS);
    let ids: Vec<(&str, u32)> = found
        .iter()
        .map(|(id, line, _)| (id.as_str(), *line))
        .collect();
    assert_eq!(
        ids,
        [
            ("type-unknown-prop", 16),
            ("type-unknown-field", 23),
            ("type-unknown-name", 12),
            ("lower-unknown-attr", 11),
            ("lower-unknown-tag", 13),
            ("lower-attr-value", 14),
        ],
        "{found:#?}"
    );
    for (id, _, message) in &found {
        assert!(
            id == "lower-attr-value"
                || message.contains("did you mean")
                || message.contains("`class`"),
            "{message}"
        );
    }
    // The singular entry points report the first of them.
    assert_eq!(
        contract::compile(TODOS).unwrap_err().id,
        "type-unknown-prop"
    );
    // Syntax: every line the lexer refuses, and every declaration that does not parse.
    let broken = TODOS
        .replace("  id: string", "  id string")
        .replace("    count = count + 1", "    count = = 1")
        .replace("text \"Add\"", "text \"Add");
    let found = compile_all("syntax", &broken);
    let ids: Vec<(&str, u32)> = found
        .iter()
        .map(|(id, line, _)| (id.as_str(), *line))
        .collect();
    assert_eq!(
        ids,
        [
            ("syntax-expected", 2),
            ("syntax-expected-expression", 9),
            ("syntax-unterminated-string", 14),
        ],
        "{found:#?}"
    );
    // At most twenty.
    let many = format!(
        "component App\n  view\n    column\n{}",
        (0..30)
            .map(|i| format!("      text missing{i}\n"))
            .collect::<String>()
    );
    assert_eq!(compile_all("cap", &many).len(), contract::MAX_DIAGNOSTICS);
}

/// A refused member is skipped and the passes after the syntax check the
/// rest; what they say about a refused name is left out, since it may only
/// be that the name is missing.
#[test]
fn a_refused_member_does_not_hide_the_other_passes_refusals() {
    let source = "component App\n  state n = 0\n  action pick\n    match none\n      case some(r) => n = r\n      case none => n = 0\n  action bump\n    n = n + \"x\"\n  view\n    button \"go\" press=pick color=missingName\n";
    let found = compile_all("member", source);
    let ids: Vec<(&str, u32)> = found
        .iter()
        .map(|(id, line, _)| (id.as_str(), *line))
        .collect();
    assert_eq!(
        ids,
        [
            ("syntax-case-arrow", 5),
            ("type-operand", 8),
            ("type-unknown-name", 10)
        ],
        "{found:#?}"
    );
    assert!(
        found[0].2.contains("end the line at `case some(r)`"),
        "{}",
        found[0].2
    );
    // A refused member's name is still its own: a call of it is no method's free form.
    let named = "component App\n  state n = \"\"\n  action trim(s: string)\n    n =\n  view\n    button \"go\" press=trim(\"x\")\n";
    let ids: Vec<String> = compile_all("named", named)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, ["syntax-expected-expression"]);
    // A state only a refused action wrote cannot be inferred now: that is not said.
    let written = "component App\n  state selected = none\n  action pick\n    selected = some(1) +\n  view\n    button \"go\" press=pick color=missingName\n";
    let ids: Vec<String> = compile_all("written", written)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, ["syntax-expected-expression", "type-unknown-name"]);
    // A refused first component would make another the app: nothing else is checked.
    let first = "component App\n  props\n    x number\n  view\n    Card()\ncomponent Card\n  view\n    text \"a\" color=missingName\n";
    let ids: Vec<String> = compile_all("first", first)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, ["syntax-expected"]);
}

#[test]
fn an_unclosed_bracket_or_a_stray_line_does_not_hide_the_declarations_after_it() {
    let ids = |name: &str, source: &str| -> Vec<(String, u32)> {
        compile_all(name, source)
            .into_iter()
            .map(|(id, line, _)| (id, line))
            .collect()
    };
    let unclosed = "component App\n  state n = 0\n  view\n    main\n      Row(label=f(n)\ncomponent Row\n  props\n    label: string\n  derive d = = 1\n  view\n    text label\nshape S\n  a number\n";
    assert_eq!(
        ids("unclosed", unclosed),
        [
            ("syntax-expected".to_owned(), 5),
            ("syntax-expected-expression".to_owned(), 9),
            ("syntax-expected".to_owned(), 13),
        ]
    );
    let stray = "else\ncomponent App\n  view\n    text \"a\" =\n";
    assert_eq!(
        ids("stray", stray),
        [
            ("syntax-expected-declaration".to_owned(), 1),
            ("syntax-expected-expression".to_owned(), 4),
        ]
    );
}
