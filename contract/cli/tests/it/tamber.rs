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
    assert_eq!(e.span.line, 5, "{e:?}");
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
