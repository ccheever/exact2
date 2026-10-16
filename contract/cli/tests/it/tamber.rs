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

#[test]
fn a_changed_inherited_colour_is_set_again_at_a_boundary_below() {
    let path = app(
        "inherited-change",
        &[
            ("app.tamber", "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = light-dark(#111111, #eeeeee)\n    when dark-style=dim: light-dark(#111111, #cccccc)\n  soft = light-dark(#555555, #999999)\n    when dark-style=dim: light-dark(#555555, #777777)\n"),
            ("app.contract", "component App\n  state quiet = false\n  action flip\n    quiet = not quiet\n  view\n    column color=(quiet ? \"soft\" : \"ink\")\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column -exact-mode=\"dark-style=dim\"\n        text \"b\" testId=\"inner\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    assert_eq!(
        row(&r, "inner", StyleId::TextColor),
        parsed("light-dark(#111111, #cccccc)")
    );
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_eq!(
        row(&r, "inner", StyleId::TextColor),
        parsed("light-dark(#555555, #777777)")
    );
}

const TWO_AXES: &str = "tamber t\n\naxes\n  dark-style = black | dim\n  density = cozy | compact\n\ntheme\n  ink = #111111\n    when dark-style=dim: #222222\n    when density=compact: #333333\n    when dark-style=dim density=compact: #444444\n";

fn press(r: &mut Runner<NoData>, id: &str) {
    let view = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id
    };
    r.dispatch(view, exact_runner::Event::Press).unwrap();
}

#[test]
fn an_inner_pin_takes_the_axis_its_outer_pin_moves() {
    let path = app(
        "nested-pins",
        &[
            ("app.tamber", TWO_AXES),
            ("app.contract", "component App\n  state dim = false\n  action flip\n    dim = not dim\n  view\n    column -exact-mode=(dim ? \"dark-style=dim\" : \"dark-style=black\")\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column testId=\"inner\" -exact-mode=\"density=compact\"\n        text \"a\" testId=\"text\" color=\"ink\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    fn mode(r: &Runner<NoData>) -> String {
        let key = r.kernel().find_by_test_id("inner")[0];
        format!(
            "{:?}",
            r.kernel().node_by_key(key).unwrap().computed(StyleId::Mode)
        )
    }
    assert_eq!(row(&r, "text", StyleId::TextColor), parsed("#333333"));
    assert_eq!(mode(&r), "Number(2.0)", "black, compact: tuple 1");
    press(&mut r, "flip");
    assert_eq!(row(&r, "text", StyleId::TextColor), parsed("#444444"));
    assert_eq!(mode(&r), "Number(4.0)", "dim, compact: tuple 3");
}

#[test]
fn an_unset_colour_passes_the_token_around_it_to_a_boundary() {
    let path = app(
        "unset-colour",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  state loud = false\n  action flip\n    loud = not loud\n  view\n    column color=\"ink\"\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column color=(loud ? \"#ff0000\" : none)\n        column -exact-mode=\"dark-style=dim\"\n          text \"a\" testId=\"text\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    assert_eq!(
        row(&r, "text", StyleId::TextColor),
        parsed("light-dark(#111111, #cccccc)")
    );
    press(&mut r, "flip");
    assert_eq!(
        row(&r, "text", StyleId::TextColor),
        parsed("#ff0000"),
        "a literal above the boundary"
    );
    press(&mut r, "flip");
    assert_eq!(
        row(&r, "text", StyleId::TextColor),
        parsed("light-dark(#111111, #cccccc)")
    );
}

#[test]
fn a_token_edit_skips_retired_rows_refuses_roles_and_stays_its_plans() {
    let path = app(
        "edit-retired",
        &[
            ("app.tamber", "tamber t\n\ntheme\n  ink = #111111\n  Canvas = #fafafa\n"),
            ("app.contract", "component App\n  state shown = true\n  action hide\n    shown = false\n  view\n    column\n      button press=hide testId=\"hide\"\n        text \"hide\"\n      text \"kept\" testId=\"kept\" color=\"ink\"\n      if shown\n        text \"gone\" color=\"ink\"\n"),
        ],
    );
    let plan = contract::compile_path(&path).unwrap();
    let mut r = booted(plan.clone());
    press(&mut r, "hide");
    let (_, set) = r.set_token("ink", "#ff0000", &[]).unwrap();
    assert_eq!(set, 1, "the retired row is not set");
    assert_eq!(row(&r, "kept", StyleId::TextColor), parsed("#ff0000"));
    assert!(
        r.set_token("Canvas", "#000000", &[]).is_err(),
        "a role's edit is refused"
    );
    assert!(r.set_token("nonesuch", "#000000", &[]).is_err());
    // Another runner of the same app starts from the plan's own value.
    let other = booted(plan);
    assert_eq!(row(&other, "kept", StyleId::TextColor), parsed("#111111"));
}

#[test]
fn a_boundarys_own_colour_going_restores_its_implied_row() {
    let path = app(
        "own-colour",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  state loud = false\n  action flip\n    loud = not loud\n  view\n    column color=\"ink\"\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column -exact-mode=\"dark-style=dim\" color=(loud ? \"#ff0000\" : none)\n        text \"a\" testId=\"text\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let dim = parsed("light-dark(#111111, #cccccc)");
    assert_eq!(row(&r, "text", StyleId::TextColor), dim);
    press(&mut r, "flip");
    assert_eq!(row(&r, "text", StyleId::TextColor), parsed("#ff0000"));
    press(&mut r, "flip");
    assert_eq!(row(&r, "text", StyleId::TextColor), dim);
}

#[test]
fn set_mode_sets_the_windows_slice_in_its_actions_commit() {
    let path = app(
        "set-mode",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  action dim\n    setMode(\"dark-style\", \"dim\")\n  action black\n    setMode(\"dark-style\", \"black\")\n  view\n    column testId=\"page\" background-color=\"page\" color=\"ink\"\n      button press=dim testId=\"dim\"\n        text \"dim\"\n      button press=black testId=\"black\"\n        text \"black\"\n      column -exact-mode=\"dark-style=black\" testId=\"pinned\" background-color=\"page\"\n        text \"p\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let (black, dim) = (
        parsed("light-dark(#ffffff, #000000)"),
        parsed("light-dark(#ffffff, #161e27)"),
    );
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), black);
    press(&mut r, "dim");
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), dim);
    assert_eq!(
        row(&r, "pinned", StyleId::BackgroundColor),
        black,
        "a pin keeps its own"
    );
    assert_eq!(r.modes(), [("dark-style".to_string(), "dim".to_string())]);
    press(&mut r, "black");
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), black);
    // The agent's `prefer mode`.
    assert!(r.set_mode("dark-style", "dim").unwrap().is_some());
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), dim);
    assert!(
        r.set_mode("dark-style", "dim").unwrap().is_none(),
        "the same mode commits nothing"
    );
    assert!(r.set_mode("dark-style", "grey").is_err());
    // A literal the design lacks is refused where it is written.
    std::fs::write(&path, "component App\n  action go\n    setMode(\"density\", \"compact\")\n  view\n    button press=go\n      text \"go\"\n").unwrap();
    assert_eq!(contract::compile_path(&path).unwrap_err().id, "tamber-mode");
}

#[test]
fn set_mode_takes_one_axis_and_value_and_the_last_of_an_action_wins() {
    let path = app(
        "set-mode-many",
        &[
            ("app.tamber", TWO_AXES),
            ("app.contract", "component App\n  state sneaky: string = \"dim,density:compact\"\n  action both\n    setMode(\"dark-style\", \"dim\")\n    copyText(\"x\")\n    setMode(\"density\", \"compact\")\n  action sneak\n    setMode(\"dark-style\", sneaky)\n  view\n    column\n      button press=both testId=\"both\"\n        text \"both\"\n      button press=sneak testId=\"sneak\"\n        text \"sneak\"\n      text \"a\" testId=\"text\" color=\"ink\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    // A value holding pins is no value: refused, nothing moved (the window
    // is at black and cozy, which the pins would move).
    press(&mut r, "sneak");
    assert_eq!(
        r.modes(),
        [
            ("dark-style".to_string(), "black".to_string()),
            ("density".to_string(), "cozy".to_string())
        ]
    );
    press(&mut r, "both");
    assert_eq!(row(&r, "text", StyleId::TextColor), parsed("#444444"));
    // A literal axis the design lacks is refused whatever the value.
    std::fs::write(&path, "component App\n  state v: string = \"dim\"\n  action go\n    setMode(\"contrast\", v)\n  view\n    button press=go\n      text \"go\"\n").unwrap();
    assert_eq!(contract::compile_path(&path).unwrap_err().id, "tamber-mode");
    // Several at once are checked whole: one bad pin moves none.
    std::fs::write(
        &path,
        "component App\n  view\n    text \"a\" testId=\"text\" color=\"ink\"\n",
    )
    .unwrap();
    let mut r = booted(contract::compile_path(&path).unwrap());
    assert!(r
        .set_modes(&[("dark-style", "dim"), ("density", "loose")])
        .is_err());
    assert_eq!(row(&r, "text", StyleId::TextColor), parsed("#111111"));
}

/// LLP 1121 stage 2's gate line: a `setMode` that changes only colour
/// tokens on a 1,000-row list moves no geometry.
#[test]
fn a_colour_only_set_mode_on_a_thousand_rows_lays_nothing_out() {
    let rows: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    let view = format!("component App\n  state rows: number[] = [{}]\n  view\n    column\n      for i in rows key=i\n        row testId=`row-${{i}}` background-color=\"page\"\n          text `${{i}}` color=\"ink\" testId=`text-${{i}}`\n", rows.join(", "));
    let path = app(
        "set-mode-1000",
        &[("app.tamber", MODES), ("app.contract", &view)],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let started = std::time::Instant::now();
    let receipt = r.set_mode("dark-style", "dim").unwrap().unwrap();
    let took = started.elapsed();
    assert!(!receipt.layout_invalidated, "a colour moves no geometry");
    assert_eq!(
        row(&r, "row-999", StyleId::BackgroundColor),
        parsed("light-dark(#ffffff, #161e27)")
    );
    eprintln!("setMode over 1000 rows {took:?}");
}

#[test]
fn a_mode_is_written_axis_equals_value_and_a_colon_is_refused() {
    let path = app(
        "pin-colon",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  view\n    column -exact-mode=\"dark-style:dim\"\n      text \"a\"\n"),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-mode", "{e:?}");
    assert!(e.message.contains("`dark-style=dim`"), "{}", e.message);
    // A computed one is the runner's: the row is unset, the tuple around stands.
    std::fs::write(
        &path,
        "component App\n  state pin: string = \"dark-style:dim\"\n  view\n    column -exact-mode=pin\n      text \"a\" testId=\"text\" color=\"ink\"\n",
    )
    .unwrap();
    let r = booted(contract::compile_path(&path).unwrap());
    assert_eq!(
        row(&r, "text", StyleId::TextColor),
        parsed("light-dark(#111111, #eeeeee)")
    );
    let journal: Vec<&str> = r.journal().collect();
    assert!(
        journal.iter().any(|l| l.contains("invalid mode value")),
        "{journal:?}"
    );
}

/// LLP 1121 D7 — `layout <node>` shows the token beside each row's value,
/// from the runner's index: what it named, its text, the tuple it was set
/// in, and whether a boundary set it.
#[test]
fn layout_of_a_node_names_the_token_each_row_read() {
    let path = app(
        "layout",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  view\n    column testId=\"page\" background-color=\"page\" color=\"ink\"\n      column testId=\"card\" -exact-mode=\"dark-style=dim\"\n        text \"b\"\n"),
        ],
    );
    let r = booted(contract::compile_path(&path).unwrap());
    let node = |id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        let view = r.kernel().node_by_key(key).unwrap().id;
        exact_runner::agent::handle(&r, &format!("{{\"op\":\"node\",\"id\":{view}}}"))
    };
    let page = node("page");
    assert!(
        page.contains("\"token\":{\"names\":[\"page\"],\"text\":\"page\",\"modes\":\"dark-style=black\",\"implied\":false}"),
        "{page}"
    );
    let card = node("card");
    assert!(
        card.contains("\"token\":{\"names\":[\"ink\"],\"text\":\"ink\",\"modes\":\"dark-style=dim\",\"implied\":true}"),
        "{card}"
    );
}

/// The names `layout` shows are the entries the row's parse read, each by its
/// token's name: never another whose bit or text it shares, and an override
/// read by its role's spelling under the name the design gave it.
#[test]
fn layout_names_exactly_the_tokens_a_row_read() {
    let path = app(
        "layout-names",
        &[
            ("app.tamber", "tamber t\n\ntheme\n  brand = #c8102e\n  brand.label = #00aa00\n  secondary-label = light-dark(#111111, #eeeeee)\n"),
            ("app.contract", "component App\n  view\n    column\n      text \"a\" testId=\"label\" color=\"brand.label\"\n      text \"b\" testId=\"role\" color=\"-exact-secondary-label\"\n"),
        ],
    );
    let r = booted(contract::compile_path(&path).unwrap());
    let node = |id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        let view = r.kernel().node_by_key(key).unwrap().id;
        exact_runner::agent::handle(&r, &format!("{{\"op\":\"node\",\"id\":{view}}}"))
    };
    let label = node("label");
    assert!(label.contains("\"names\":[\"brand.label\"]"), "{label}");
    let role = node("role");
    assert!(role.contains("\"names\":[\"secondary-label\"]"), "{role}");
}

/// LLP 1121 stage 4, commit 1 (theme2's condition): what publishing a
/// kernel's wide-colour mark after every changed commit costs on the normal
/// write path. Run in release, by hand: `cargo test --release -p contract
/// --test it wide_mark_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn wide_mark_cost() {
    use exact_kernel::style::wide::{Holder, WideMark};
    // `WIDE_MARK_HEX=1` runs the same app in 8-bit colours, so no mark is
    // ever taken: the baseline.
    let hex = std::env::var("WIDE_MARK_HEX").is_ok_and(|v| v == "1");
    let p3 = if hex {
        MODES.to_string()
    } else {
        MODES
            .replace("#ffffff", "color(display-p3 1 1 1)")
            .replace("#000000", "color(display-p3 0 0 0)")
            .replace("#161e27", "color(display-p3 0.09 0.12 0.15)")
            .replace("#111111", "color(display-p3 0.07 0.07 0.07)")
            .replace("#eeeeee", "color(display-p3 0.93 0.93 0.93)")
            .replace("#cccccc", "color(display-p3 0.8 0.8 0.8)")
    };
    let border = if hex {
        "`#${i % 10}0${i % 7}0${i % 9}0`"
    } else {
        "`color(display-p3 ${i / 1000} 0.5 0.5)`"
    };
    let window = if hex {
        "`#${j % 10}0${j % 7}0${j % 9}0`"
    } else {
        "`color(display-p3 0.5 ${j / 1000} 0.5)`"
    };
    let rows: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    // Each row also carries its own wide colour: 1,000 distinct styles. A
    // second list of 200 windowed rows shifts by 10 a commit, as a scroll
    // mounts and retires rows.
    let view = format!("component App\n  state dim = false\n  state rows: number[] = [{}]\n  state n = 0\n  action flip\n    dim = not dim\n  action bump\n    n = n + 1\n  state start = 0\n  action shift\n    start = start + 10\n  view\n    column testId=\"root\" -exact-mode=(dim ? \"dark-style=dim\" : \"dark-style=black\")\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      button press=bump testId=\"bump\"\n        text `${{n}}`\n      button press=shift testId=\"shift\"\n        text \"shift\"\n      for j in rows.slice(start, start + 200) key=j\n        text `w${{j}}` border-color={window} border-width=1\n      for i in rows key=i\n        row testId=`row-${{i}}` background-color=\"page\" border-color={border} border-width=1\n          text `${{i}}` color=\"ink\" testId=`text-${{i}}`\n", rows.join(", "));
    let path = app(
        "wide-mark-cost",
        &[("app.tamber", &p3), ("app.contract", &view)],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let id = |r: &Runner<NoData>, t: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let (flip, bump, shift) = (id(&r, "flip"), id(&r, "bump"), id(&r, "shift"));
    let time = |f: &mut dyn FnMut()| {
        let started = std::time::Instant::now();
        for _ in 0..50 {
            f();
        }
        started.elapsed() / 50
    };
    let switch = time(&mut || {
        r.dispatch(flip, exact_runner::Event::Press).unwrap();
    });
    let small = time(&mut || {
        r.dispatch(bump, exact_runner::Event::Press).unwrap();
    });
    let scroll = time(&mut || {
        r.dispatch(shift, exact_runner::Event::Press).unwrap();
    });
    let k = r.kernel_mut();
    let mark = time(&mut || {
        std::hint::black_box(k.wide_mark());
    });
    // The union rebuilt from every style's cached slots, as after styles go.
    let rebuild = time(&mut || {
        k.trim();
    });
    let holder = Holder::new();
    let publish = time(&mut || holder.publish(WideMark::default()));
    eprintln!(
        "wide colours {}; Dim switch {switch:?}; one-text commit {small:?}; window shift {scroll:?}; mark (cached) {mark:?}; trim incl. rebuilt mark {rebuild:?}; publish {publish:?}",
        exact_kernel::style::wide::used()
    );
}

/// LLP 1121 stage 4, commit 3: a plan's structure leaves the design's values
/// out, so a value-only edit has the same structure; a role override's
/// value, a token's slices and a view changed do not.
#[test]
fn a_value_only_edit_keeps_the_plans_structure() {
    let view = "component App\n  view\n    column background-color=\"page\"\n      text \"a\" color=\"ink\"\n";
    let structure = |name: &str, design: &str, view: &str| {
        let path = app(name, &[("app.tamber", design), ("app.contract", view)]);
        contract::compile_path(&path).unwrap().structure()
    };
    let design = format!("{MODES}  Canvas = #fafafa\n");
    let base = structure("structure-base", &design, view);
    // Values, varying or not, in any tuple.
    let values = design
        .replace(
            "light-dark(#111111, #eeeeee)",
            "light-dark(#111111, #dddddd)",
        )
        .replace(
            "light-dark(#ffffff, #161e27)",
            "light-dark(#ffffff, #1a2430)",
        );
    assert_eq!(structure("structure-values", &values, view), base);
    // A value that stops sharing its string with another token's, and one
    // that comes to.
    let shared = format!("{design}  edge = #fafafa\n");
    let view_edge = view.replace("column ", "column border-color=\"edge\" ");
    let base_edge = structure("structure-shared", &shared, &view_edge);
    let apart = shared.replace("edge = #fafafa", "edge = #abcdef");
    assert_eq!(structure("structure-apart", &apart, &view_edge), base_edge);
    let together = shared.replace("edge = #fafafa", "edge = #111111");
    assert_eq!(
        structure("structure-together", &together, &view_edge),
        base_edge
    );
    // Code that pushes a value's text keeps that string: the literal and the
    // token changed together is no value-only edit.
    let literal = |text: &str| {
        format!("component App\n  state lit = \"{text}\"\n  view\n    column background-color=\"page\"\n      text lit color=\"ink\"\n")
    };
    let with = |ink: &str| {
        design
            .replace(
                "ink = light-dark(#111111, #eeeeee)",
                &format!("ink = {ink}"),
            )
            .replace(
                "    when dark-style=dim: light-dark(#111111, #cccccc)\n",
                "",
            )
    };
    let shared_code = structure("structure-code-a", &with("#111111"), &literal("#111111ff"));
    assert_ne!(
        structure("structure-code-b", &with("#222222"), &literal("#222222ff")),
        shared_code
    );
    assert_eq!(
        structure("structure-code-c", &with("#222222"), &literal("#111111ff")),
        shared_code
    );
    // A role override's value: the hosts take role tables at boot.
    let role = design.replace("Canvas = #fafafa", "Canvas = #f0f0f0");
    assert_ne!(structure("structure-role", &role, view), base);
    // A token that stops varying by an axis (its slices go), and a view
    // changed. (A token no view names is in no plan.)
    let flat = design.replace(
        "    when dark-style=dim: light-dark(#111111, #cccccc)\n",
        "",
    );
    assert_ne!(flat, design);
    assert_ne!(structure("structure-flat", &flat, view), base);
    let other = view.replace("\"a\"", "\"b\"");
    assert_ne!(structure("structure-view", &design, &other), base);
}

/// LLP 1121 stage 4, commit 4: a value-only edit reaches a running build as
/// a batch, with no plan: its rows, a synthetic token's included, set in one
/// commit with no layout. A stale base is refused (the build asks for a
/// snapshot), a snapshot applies whole, and another structure is refused.
#[test]
fn a_batch_sets_a_running_builds_rows_without_a_plan() {
    use exact_runner::design::{self, Token};
    use exact_runner::LiveRefusal;
    let view = "component App\n  view\n    column background-color=\"page\" testId=\"page\"\n      text \"a\" color=\"ink\" testId=\"ink\"\n      text \"b\" color=\"color-mix(in srgb, ink 50%, white)\" testId=\"mix\"\n";
    let plan = |name: &str, design: &str| {
        let path = app(name, &[("app.tamber", design), ("app.contract", view)]);
        contract::compile_path(&path).unwrap()
    };
    // With a role override, which a snapshot carries and does not set.
    let modes = format!("{MODES}  Canvas = #fafafa\n");
    let a = plan("live-a", &modes);
    let b = plan("live-b", &modes.replace("#111111", "#ff0000"));
    assert_eq!(design::structure(&b), design::structure(&a));
    let structure = design::shape(&a);
    let batch = design::batch(&structure, &design::tokens(&a), &design::tokens(&b)).unwrap();
    let names: Vec<&str> = batch.tokens.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        ["ink", "color-mix(in srgb, ink 50%, white)"],
        "the synthetic token rides along"
    );
    // The wire round-trips.
    assert_eq!(design::Edit::parse(&batch.json()), Some(batch.clone()));
    let mut r = booted(a.clone());
    // A release build takes none.
    r.admit_live(Some(r#"{"inputs":{"trust":"production"}}"#));
    assert!(matches!(r.apply_live(&batch), Err(LiveRefusal::Refused(_))));
    r.admit_live(None);
    let mix_before = row(&r, "mix", StyleId::TextColor);
    let applied = r.apply_live(&batch).unwrap();
    assert!(
        !applied.receipt.layout_invalidated,
        "a colour moves no geometry"
    );
    assert!(!applied.full);
    assert_eq!(
        row(&r, "ink", StyleId::TextColor),
        parsed("light-dark(#ff0000, #eeeeee)")
    );
    assert_ne!(row(&r, "mix", StyleId::TextColor), mix_before);
    assert_eq!(r.design(), batch.design);
    // The same batch again: its base is no longer this build's.
    assert_eq!(r.apply_live(&batch).unwrap_err(), LiveRefusal::Stale);
    // A snapshot of the first design applies whole, whatever this build ran.
    let back = design::snapshot(&structure, &design::tokens(&a));
    r.apply_live(&back).unwrap();
    assert_eq!(
        row(&r, "ink", StyleId::TextColor),
        parsed("light-dark(#111111, #eeeeee)")
    );
    assert_eq!(row(&r, "mix", StyleId::TextColor), mix_before);
    // Another structure's.
    let other = design::snapshot(
        "0".repeat(64).as_str(),
        &[Token {
            name: "ink".into(),
            value: "#000000".into(),
            slices: vec![],
        }],
    );
    assert_eq!(r.apply_live(&other).unwrap_err(), LiveRefusal::Structure);
}
