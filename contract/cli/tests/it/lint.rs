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
            "component A\n  view\n    column background-color=\"reddish\"\n      text \"a\"\n",
            "lower-attr-value",
            "#rrggbb",
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
fn a_button_keeps_its_cross_host_flex_column() {
    let app = |display: &str| {
        format!("component A\n  view\n    button display=\"{display}\"\n      text \"Go\"\n")
    };
    for display in ["block", "inline", "inline-block", "inline-flex"] {
        let error = contract::compile(&app(display)).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{display}: {error}");
        assert!(
            error.message.contains("a `button` is a flex column"),
            "{error}"
        );
        assert!(error.message.contains("display=\"flex\""), "{error}");
    }
    contract::compile(&app("flex")).unwrap();
    contract::compile(&app("none")).unwrap();

    let source = "style Block\n  display=\"block\"\ncomponent A\n  view\n    button class=Block display=\"flex\"\n      text \"Go\"\n";
    contract::compile(source).unwrap();
    let error = contract::compile(&source.replace(" display=\"flex\"", "")).unwrap_err();
    assert_eq!(error.id, "lower-attr-value", "{error}");
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
        (app("", "text \"a\" color=\"bleu\""), "lower-attr-value", "`color=\"bleu\"` is not a valid `color`: a color is `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, `rgba(r, g, b, a)`, a CSS color name (`gray`), or `transparent`"),
        (app("", "text \"a\" font-size=\"14px\""), "lower-attr-value", "`font-size=\"14px\"` is not a valid `font-size`: expected number; write `font-size=14` (a number is pixels)"),
        (app("", "text \"a\" width=10px"), "syntax-unquoted-length", "`width=10px` needs quotes: a value with a unit is a string, `width=\"10px\"` (a bare number is pixels)"),
        (app("", "text \"a\" className=\"x\""), "lower-unknown-attr", "`text` has no attribute `className`; `class` names a `style` declared in this file, as in `class=Card`"),
        (
            app("  state n = 0\n", "text n"),
            "lower-attr-type",
            "`text` takes a string; this expression is `number`: interpolate it in a template, `${…}`, or write `toString(…)`",
        ),
        (
            app("", "when draft = \"x\"\n        text \"a\""),
            "syntax-expected-newline",
            "expected end of line, found `=`; `==` compares, and only a statement assigns",
        ),
        (
            app("", "button press={add}"),
            "syntax-expected-expression",
            "expected an expression, found `{`; Contract has no `{…}`: a handler names an action (`press=add`, `press=add(item)`) and a value is written directly (`width=10`)",
        ),
    ] {
        let e = contract::compile(&src).unwrap_err();
        assert_eq!((e.id.as_str(), e.message.as_str()), (id, message), "{src}");
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
        let file = contract_syntax::parse(&src).unwrap();
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
  resource todos = items(3) as shape list<Todo>
  action add
    count = count + 1
  view
    main padding=16 gap=8 className="x"
      input value=drat
      buton press=add
        text "Add" color="bleu"
      each t in todos key=t.id
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
