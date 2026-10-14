//! @ref LLP 1121 D2, D3, D7 — the compiler reads the app's design:
//! `app.tamber` beside `app.contract` (or `app.json`'s `"tamber"`), checked
//! and folded by `exact-tamber` as an inline `theme` is; names checked in
//! the rows; an expression over tokens a synthetic token; the plan keeps
//! what the view may name and every role override.

use exact_kernel::{ColorValue, Kernel, RowValue, StyleId};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Runner};
use std::path::{Path, PathBuf};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-tamber-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

fn colour(plan: Plan, id: &str, row: StyleId) -> ColorValue {
    let plan = contract::bake(plan, NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = r.kernel().find_by_test_id(id)[0];
    match r.kernel().node_by_key(key).unwrap().computed(row) {
        RowValue::ColorValue(c) => c,
        RowValue::Color(c) => ColorValue::Fixed(c),
        other => panic!("{id}: {other:?}"),
    }
}

fn parsed(text: &str) -> ColorValue {
    ColorValue::parse_light_dark(text)
        .or_else(|| exact_kernel::Color::parse(text).map(ColorValue::Fixed))
        .unwrap_or_else(|| panic!("{text}"))
}

fn table(plan: &Plan) -> Vec<(String, String, String)> {
    plan.theme
        .iter()
        .map(|t| {
            (
                plan.str(t.name).into(),
                plan.str(t.role).into(),
                plan.str(t.value).into(),
            )
        })
        .collect()
}

const DESIGN: &str = "tamber shelf\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  brand = #c8102e\n  surface.card = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #161e27)\n  unused = #123456\n  Canvas = #fafafa\n";

#[test]
fn app_tamber_beside_the_app_is_its_design_and_the_plan_keeps_what_the_view_names() {
    let path = app(
        "beside",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "component App\n  view\n    column testId=\"card\" background-color=\"surface.card\"\n      text \"a\" testId=\"brand\" color=\"brand\"\n"),
        ],
    );
    let plan = contract::compile_path(&path).unwrap();
    let rows = table(&plan);
    let names: Vec<&str> = rows.iter().map(|(n, ..)| n.as_str()).collect();
    // The view's tokens and the role override; `unused` is not kept.
    assert_eq!(names, ["brand", "surface.card", "Canvas"], "{rows:?}");
    assert_eq!(
        colour(plan.clone(), "brand", StyleId::TextColor),
        parsed("#c8102e")
    );
    // The default app tuple until the plan carries the axes (stage 2, commit 3).
    assert_eq!(
        colour(plan, "card", StyleId::BackgroundColor),
        parsed("light-dark(#ffffff, #000000)")
    );
    // `contract sources` lists it: a capture copies it, a watcher watches it.
    let graph = contract::source_graph(&path);
    assert!(
        graph.sources.iter().any(|s| s.path.ends_with("app.tamber")),
        "{:?}",
        graph.sources.iter().map(|s| &s.path).collect::<Vec<_>>()
    );
}

#[test]
fn app_json_names_the_design_and_an_inline_theme_beside_one_is_refused() {
    let path = app(
        "named",
        &[
            (
                "app.json",
                r#"{"name":"S","app":{"id":"com.example.s","name":"S"},"tamber":"design/shelf.tamber"}"#,
            ),
            (
                "app.contract",
                "component App\n  view\n    text \"a\" testId=\"brand\" color=\"brand\"\n",
            ),
        ],
    );
    std::fs::create_dir_all(path.parent().unwrap().join("design")).unwrap();
    std::fs::write(path.parent().unwrap().join("design/shelf.tamber"), DESIGN).unwrap();
    let plan = contract::compile_path(&path).unwrap();
    assert_eq!(colour(plan, "brand", StyleId::TextColor), parsed("#c8102e"));
    std::fs::write(
        &path,
        "theme\n  ink = \"#000\"\n\ncomponent App\n  view\n    text \"a\" color=\"brand\"\n",
    )
    .unwrap();
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-two-sources", "{e:?}");
}

#[test]
fn a_design_refusal_names_the_file_line_and_column() {
    let path = app(
        "refused",
        &[
            (
                "app.tamber",
                "tamber shelf\n\ntheme\n  brand = #c8102e\n  ink = brnad\n",
            ),
            (
                "app.contract",
                "component App\n  view\n    text \"a\" color=\"brand\"\n",
            ),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-unknown-token", "{e:?}");
    // The refusal is the design file's, at its line and column.
    assert!(
        e.file.as_deref().is_some_and(|f| f.ends_with("app.tamber")),
        "{e:?}"
    );
    assert_eq!((e.span.line, e.span.col), (5, 9), "{e:?}");
    // Another entry beside it has no design: it is `app.contract`'s.
    let other = Path::new(&path).with_file_name("other.contract");
    std::fs::write(
        &other,
        "component App\n  view\n    text \"a\" color=\"brand\"\n",
    )
    .unwrap();
    assert_eq!(
        contract::compile_path(&other).unwrap_err().id,
        "lower-attr-value"
    );
}

#[test]
fn an_expression_over_tokens_is_a_synthetic_token_folded_by_the_build() {
    let mix = "color-mix(in srgb, brand 50%, #ffffff)";
    let pair = "light-dark(brand, card)";
    let src = format!(
        "theme\n  brand = \"#c8102e\"\n  card = \"#000000\"\n\ncomponent App\n  view\n    column testId=\"ground\" background-image=\"linear-gradient({pair}, #ffffff)\"\n      text \"a\" testId=\"mix\" color=\"{mix}\"\n      text \"b\" testId=\"pair\" color=\"{pair}\"\n"
    );
    let plan = contract::compile(&src).unwrap();
    let rows = table(&plan);
    assert!(rows.iter().any(|(n, ..)| n == mix), "{rows:?}");
    assert!(rows.iter().any(|(n, ..)| n == pair), "{rows:?}");
    // The row holds the folded colour: no device evaluates the mix.
    let folded = rows.iter().find(|(n, ..)| n == mix).unwrap().2.clone();
    assert_eq!(
        colour(plan.clone(), "mix", StyleId::TextColor),
        parsed(&folded)
    );
    assert_eq!(
        colour(plan, "pair", StyleId::TextColor),
        parsed("light-dark(#c8102e, #000000)")
    );
    // One the crate cannot fold is refused at the row.
    let src = "theme\n  brand = \"#c8102e\"\n\ncomponent App\n  view\n    text \"a\" color=\"color-mix(in srgb, brand 50%, CanvasText)\"\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "tamber-residual", "{e:?}");
    assert_eq!(e.span.line, 6, "{e:?}");
}

#[test]
fn a_row_names_a_token_of_its_kind_and_a_computed_colour_keeps_every_token() {
    let theme = "theme\n  brand = \"#c8102e\"\n  ink = \"#111111\"\n\n";
    let e = contract::compile(&format!(
        "{theme}component App\n  view\n    text \"a\" color=\"brnad\"\n"
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-unknown-token", "{e:?}");
    let e = contract::compile(&format!(
        "{theme}component App\n  view\n    column padding=\"brand\"\n"
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    // A string from state may name any token: all are kept (D7).
    let plan = contract::compile(&format!(
        "{theme}component App\n  state c: string = \"brand\"\n  view\n    text \"a\" color=c\n"
    ))
    .unwrap();
    assert_eq!(table(&plan).len(), 2);
    let plan = contract::compile(&format!(
        "{theme}component App\n  view\n    text \"a\" color=\"brand\"\n"
    ))
    .unwrap();
    assert_eq!(table(&plan).len(), 1);
}

#[test]
fn an_app_without_a_design_carries_none() {
    let plan =
        contract::compile("component App\n  view\n    text \"a\" color=\"CanvasText\"\n").unwrap();
    assert!(plan.theme.is_empty());
}

#[test]
fn a_profile_colour_is_named_whole_and_an_expression_over_one_is_refused() {
    let profile = "color-profile --brand src=\"assets/brand.icc\"\n\ntheme\n  ink = \"color(--brand 0.1 0.2 0.3 0.4)\"\n  text = \"ink\"\n";
    let plan = contract::compile(&format!(
        "{profile}\ncomponent App\n  view\n    column\n      text \"a\" color=\"ink\"\n      text \"b\" color=\"text\"\n"
    ))
    .unwrap();
    let rows = table(&plan);
    assert!(
        rows.iter()
            .filter(|(_, _, v)| v == "color(--brand 0.1 0.2 0.3 0.4)")
            .count()
            == 2,
        "{rows:?}"
    );
    // A colour no author writes is no stand-in: a token of the same value
    // keeps its own.
    let plan = contract::compile(&format!(
        "{profile}  faint = \"#00000101\"\n\ncomponent App\n  view\n    text \"a\" color=\"faint\"\n"
    ))
    .unwrap();
    assert!(table(&plan)
        .iter()
        .any(|(n, _, v)| n == "faint" && v == "#00000101"));
    for (extra, row) in [
        (
            "  mixed = \"color-mix(in srgb, text 50%, #fff)\"\n",
            "\"mixed\"",
        ),
        ("", "\"color-mix(in srgb, ink 50%, #fff)\""),
    ] {
        let e = contract::compile(&format!(
            "{profile}{extra}\ncomponent App\n  view\n    text \"a\" color={row}\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, "lower-theme-value", "{row}: {e:?}");
    }
}

#[test]
fn a_literal_a_call_carries_is_a_synthetic_token_and_text_is_not_folded() {
    let theme = "theme\n  brand = \"#c8102e\"\n  card = \"#000000\"\n\n";
    let pair = "light-dark(brand, card)";
    let plan = contract::compile(&format!(
        "{theme}fn echo(c: string): string = c\n\ncomponent App\n  view\n    text \"a\" testId=\"echo\" color=echo(\"{pair}\")\n"
    ))
    .unwrap();
    assert!(table(&plan).iter().any(|(n, ..)| n == pair));
    assert_eq!(
        colour(plan, "echo", StyleId::TextColor),
        parsed("light-dark(#c8102e, #000000)")
    );
    // A label that only reads like a colour is not the build's to fold.
    contract::compile(&format!(
        "{theme}component App\n  state label: string = \"light-dark(brand, CanvasText)\"\n  view\n    text label color=\"brand\"\n"
    ))
    .unwrap();
    // A branch of `?:` naming no token is said to.
    let e = contract::compile(&format!(
        "{theme}component App\n  state on = true\n  view\n    text \"a\" color=(on ? \"brnad\" : \"brand\")\n"
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-unknown-token", "{e:?}");
}

#[test]
fn a_design_tokens_refusal_and_warning_are_at_its_line() {
    let path = app(
        "lines",
        &[
            (
                "app.tamber",
                "tamber shelf\n\ntheme\n  brand = #c8102e\n  top = #000\n",
            ),
            (
                "app.contract",
                "component App\n  view\n    text \"a\" color=\"brand\"\n",
            ),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "lower-theme-name", "{e:?}");
    assert!(
        e.file.as_deref().is_some_and(|f| f.ends_with("app.tamber")),
        "{e:?}"
    );
    assert_eq!((e.span.line, e.span.col), (5, 3), "{e:?}");
    std::fs::write(
        path.with_file_name("app.tamber"),
        "tamber shelf\n\ntheme\n  Canvas = #fbf3e9\n",
    )
    .unwrap();
    let (_, warnings) = contract::compile_path_all_warned(&path, false);
    let w = warnings
        .iter()
        .find(|w| w.id == "lower-colour-one-scheme")
        .unwrap_or_else(|| panic!("{warnings:?}"));
    assert!(
        w.file.as_deref().is_some_and(|f| f.ends_with("app.tamber")),
        "{w:?}"
    );
    assert_eq!(w.span.line, 4, "{w:?}");
}

#[test]
fn a_profile_token_is_still_declared_and_an_alias_may_spell_its_role() {
    let profile = "color-profile --brand src=\"assets/brand.icc\"\n\ntheme\n  -exact-secondary-label = \"color(--brand 0.1 0.2 0.3 0.4)\"\n  ink = \"secondary-label\"\n";
    let plan = contract::compile(&format!(
        "{profile}\ncomponent App\n  view\n    text \"a\" color=\"ink\"\n"
    ))
    .unwrap();
    assert!(
        table(&plan)
            .iter()
            .any(|(n, _, v)| n == "ink" && v == "color(--brand 0.1 0.2 0.3 0.4)"),
        "{:?}",
        table(&plan)
    );
    // Its name is checked as any token's.
    let e = contract::compile("color-profile --brand src=\"assets/brand.icc\"\n\ntheme\n  red = \"color(--brand 0.1 0.2 0.3 0.4)\"\n\ncomponent App\n  view\n    text \"a\"\n").unwrap_err();
    assert_eq!(e.id, "tamber-name-reserved", "{e:?}");
}

#[test]
fn only_what_a_colour_row_is_must_fold_and_an_action_call_is_read() {
    let theme = "theme\n  brand = \"#c8102e\"\n  card = \"#000000\"\n\n";
    // A condition's string is not the row's colour.
    contract::compile(&format!(
        "{theme}component App\n  state label: string = \"x\"\n  view\n    text \"a\" color=(label == \"light-dark(brand, CanvasText)\" ? \"brand\" : \"card\")\n"
    ))
    .unwrap();
    // A colour an action call carries reaches the plan's table.
    let pair = "light-dark(brand, card)";
    let plan = contract::compile(&format!(
        "{theme}component App\n  state c: string = \"brand\"\n  action set(v: string)\n    c = v\n  action choose\n    set(\"{pair}\")\n  view\n    button press=choose color=c\n      text \"a\"\n"
    ))
    .unwrap();
    assert!(
        table(&plan).iter().any(|(n, ..)| n == pair),
        "{:?}",
        table(&plan)
    );
}

#[test]
fn a_profile_role_is_aliased_in_any_spelling_and_may_vary_by_an_axis() {
    let profile = "color-profile --brand src=\"assets/brand.icc\"\n\ntheme\n  Canvas = \"color(--brand 0.1 0.2 0.3 0.4)\"\n  ink = \"canvas\"\n";
    let plan = contract::compile(&format!(
        "{profile}\ncomponent App\n  view\n    text \"a\" color=\"ink\"\n"
    ))
    .unwrap();
    assert!(
        table(&plan)
            .iter()
            .any(|(n, _, v)| n == "ink" && v == "color(--brand 0.1 0.2 0.3 0.4)"),
        "{:?}",
        table(&plan)
    );
    let path = app(
        "profile-axis",
        &[
            ("app.tamber", "tamber shelf\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = color(--brand 0.1 0.2 0.3 0.4)\n    when dark-style=dim: #112233\n  other = #abcdef\n"),
            ("app.contract", "color-profile --brand src=\"assets/brand.icc\"\n\ncomponent App\n  view\n    column\n      text \"a\" color=\"ink\"\n      text \"b\" color=\"other\"\n"),
        ],
    );
    let rows = table(&contract::compile_path(&path).unwrap());
    assert!(
        rows.iter()
            .any(|(n, _, v)| n == "ink" && v == "color(--brand 0.1 0.2 0.3 0.4)"),
        "{rows:?}"
    );
    assert!(rows.iter().any(|(n, ..)| n == "other"), "{rows:?}");
}

#[test]
fn a_when_at_the_default_modes_cannot_replace_a_profile_colour() {
    let path = app(
        "profile-default-when",
        &[
            ("app.tamber", "tamber shelf\n\naxes\n  mode = first | second\n\ntheme\n  ink = color(--brand 0.1 0.2 0.3 0.4)\n    when mode=first: #112233\n"),
            ("app.contract", "color-profile --brand src=\"assets/brand.icc\"\n\ncomponent App\n  view\n    text \"a\" color=\"ink\"\n"),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "lower-theme-value", "{e:?}");
    assert_eq!(e.span.line, 8, "{e:?}");
}

const MODES: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  page = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #161e27)\n  ink = light-dark(#111111, #eeeeee)\n    when dark-style=dim: light-dark(#111111, #cccccc)\n";

#[test]
fn a_pin_selects_its_slice_and_a_boundary_sets_inherited_text_again() {
    let path = app(
        "modes",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  view\n    column testId=\"page\" background-color=\"page\" color=\"ink\"\n      text \"a\" testId=\"outer\"\n      column testId=\"card\" -exact-mode=\"dark-style=dim\" background-color=\"page\"\n        text \"b\" testId=\"inner\"\n"),
        ],
    );
    let plan = contract::compile_path(&path).unwrap();
    assert_eq!(plan.axes.len(), 1);
    let black = parsed("light-dark(#ffffff, #000000)");
    let dim = parsed("light-dark(#ffffff, #161e27)");
    assert_eq!(
        colour(plan.clone(), "page", StyleId::BackgroundColor),
        black
    );
    assert_eq!(colour(plan.clone(), "card", StyleId::BackgroundColor), dim);
    // Text whose colour the page set: the page's slice outside, the pin's in.
    assert_eq!(
        colour(plan.clone(), "outer", StyleId::TextColor),
        parsed("light-dark(#111111, #eeeeee)")
    );
    assert_eq!(
        colour(plan, "inner", StyleId::TextColor),
        parsed("light-dark(#111111, #cccccc)")
    );
}

fn booted(plan: Plan) -> Runner<NoData> {
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn row(r: &Runner<NoData>, id: &str, style: StyleId) -> ColorValue {
    let key = r.kernel().find_by_test_id(id)[0];
    match r.kernel().node_by_key(key).unwrap().computed(style) {
        RowValue::ColorValue(c) => c,
        RowValue::Color(c) => ColorValue::Fixed(c),
        other => panic!("{id}: {other:?}"),
    }
}

#[test]
fn a_changed_pin_sets_its_subtrees_themed_rows_again() {
    let path = app(
        "pin-change",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  state dim = false\n  action flip\n    dim = not dim\n  view\n    column testId=\"page\" background-color=\"page\" color=\"ink\"\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column testId=\"card\" -exact-mode=(dim ? \"dark-style=dim\" : \"dark-style=black\")\n        column testId=\"deep\" background-color=\"page\"\n          text \"b\" testId=\"inner\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let (black, dim) = (
        parsed("light-dark(#ffffff, #000000)"),
        parsed("light-dark(#ffffff, #161e27)"),
    );
    assert_eq!(row(&r, "deep", StyleId::BackgroundColor), black);
    assert_eq!(
        row(&r, "inner", StyleId::TextColor),
        parsed("light-dark(#111111, #eeeeee)")
    );
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    // A node below the pin that reads no state is set again in its slice.
    assert_eq!(row(&r, "deep", StyleId::BackgroundColor), dim);
    assert_eq!(
        row(&r, "inner", StyleId::TextColor),
        parsed("light-dark(#111111, #cccccc)")
    );
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), black);
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_eq!(row(&r, "deep", StyleId::BackgroundColor), black);
}

/// LLP 1121 stage 2's gate (theme1's condition): a one-token edit on a
/// 1,000-row list sets only the rows that read the token, moves no
/// geometry, and costs no more than a mode switch over the same list.
#[test]
fn a_token_edit_sets_only_its_rows_and_paints() {
    let rows: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    let view = format!("component App\n  state dim = false\n  state rows: number[] = [{}]\n  action flip\n    dim = not dim\n  view\n    column testId=\"root\" -exact-mode=(dim ? \"dark-style=dim\" : \"dark-style=black\")\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      for i in rows key=i\n        row testId=`row-${{i}}` background-color=\"page\"\n          text `${{i}}` color=\"ink\" testId=`text-${{i}}`\n", rows.join(", "));
    let path = app(
        "token-edit",
        &[("app.tamber", MODES), ("app.contract", &view)],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let started = std::time::Instant::now();
    let (receipt, set) = r
        .set_token(
            "ink",
            "light-dark(#ff0000, #00ff00)",
            &[
                "light-dark(#ff0000, #00ff00)",
                "light-dark(#ff0000, #0000ff)",
            ],
        )
        .unwrap();
    let edit = started.elapsed();
    assert_eq!(set, 1000, "only the rows that read `ink`");
    assert!(!receipt.layout_invalidated, "a colour moves no geometry");
    assert_eq!(
        row(&r, "text-7", StyleId::TextColor),
        parsed("light-dark(#ff0000, #00ff00)")
    );
    assert_eq!(
        row(&r, "row-7", StyleId::BackgroundColor),
        parsed("light-dark(#ffffff, #000000)")
    );
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    let started = std::time::Instant::now();
    let receipt = r.dispatch(flip, exact_runner::Event::Press).unwrap();
    let switch = started.elapsed();
    assert!(
        !receipt.layout_invalidated,
        "a mode switch moves no geometry"
    );
    assert_eq!(
        row(&r, "text-7", StyleId::TextColor),
        parsed("light-dark(#ff0000, #0000ff)")
    );
    eprintln!("token edit {edit:?} ({set} rows), mode switch {switch:?}");
}

#[test]
fn a_role_override_has_one_value_and_a_pin_names_the_designs_axes() {
    let path = app(
        "role-axis",
        &[
            ("app.tamber", "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  Canvas = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #161e27)\n"),
            ("app.contract", "component App\n  view\n    text \"a\"\n"),
        ],
    );
    assert_eq!(
        contract::compile_path(&path).unwrap_err().id,
        "tamber-role-axis"
    );
    for pin in [
        "dark-style=grey",
        "contrast=high",
        "dark-style",
        "dark-style=dim dark-style=black",
    ] {
        std::fs::write(path.with_file_name("app.tamber"), MODES).unwrap();
        std::fs::write(
            &path,
            format!("component App\n  view\n    column -exact-mode=\"{pin}\"\n      text \"a\"\n"),
        )
        .unwrap();
        assert_eq!(
            contract::compile_path(&path).unwrap_err().id,
            "tamber-mode",
            "{pin}"
        );
    }
}

#[test]
fn a_virtualized_lists_rows_are_in_the_modes_around_it() {
    let path = app(
        "pinned-list",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  state rows: number[] = [1, 2, 3]\n  view\n    column -exact-mode=\"dark-style=dim\" color=\"ink\"\n      list virtualized=true estimated-item-height=40 height=400\n        for i in rows key=i\n          row testId=`row-${i}` background-color=\"page\"\n            text `${i}` testId=`text-${i}`\n"),
        ],
    );
    let r = booted(contract::compile_path(&path).unwrap());
    assert_eq!(
        row(&r, "row-2", StyleId::BackgroundColor),
        parsed("light-dark(#ffffff, #161e27)")
    );
    assert_eq!(
        row(&r, "text-2", StyleId::TextColor),
        parsed("light-dark(#111111, #cccccc)")
    );
}
