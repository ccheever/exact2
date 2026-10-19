//! @ref LLP 1121 stage 11 B — the base scale's nine names: a view reads one
//! with no design defining it (the compiler knows the names, the kernel
//! reads its column's length), the plan keeps nothing for it, an app's own
//! definition wins and must be a length, and a base name is never an app
//! axis's inherited-row refusal.

use exact_kernel::style::{metrics::Column, scale};
use exact_kernel::{Kernel, StyleId};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Runner};
use std::path::PathBuf;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-base-scale-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
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

fn row(r: &Runner<NoData>, id: &str, style: StyleId) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    format!("{:?}", r.kernel().node_by_key(key).unwrap().computed(style))
}

fn theme_names(plan: &Plan) -> Vec<String> {
    plan.theme
        .iter()
        .map(|t| plan.str(t.name).to_string())
        .collect()
}

/// The web's first look's px for a base name (this test process says no column).
fn web(name: &str) -> f64 {
    match scale::of(name, Column::Web).unwrap() {
        scale::ScaleValue::Px(n) => n,
        scale::ScaleValue::Full => scale::RADIUS_FULL,
        scale::ScaleValue::Cells(_) => unreachable!(),
    }
}

const VIEW: &str = "component App\n  view\n    column testId=\"card\" padding=\"space.md\" gap=\"space.sm\" border-radius=\"radius.full\"\n      text \"a\" testId=\"pill\" border-radius=\"radius.lg\" letter-spacing=\"space.xs\"\n";

#[test]
fn a_base_name_reads_the_scale_with_no_design_and_the_plan_keeps_nothing_for_it() {
    let plan = contract::compile_path(&app("bare", &[("app.contract", VIEW)])).unwrap();
    // No entry for any base name: the kernel holds every column.
    let names = theme_names(&plan);
    assert!(!names.iter().any(|n| scale::is_base(n)), "{names:?}");
    // The row keeps the name, which the kernel reads in its process's column.
    assert!(format!("{plan:?}").contains("space.md"));
    let r = booted(plan);
    let md = format!("{:.1}", web("space.md"));
    assert!(
        row(&r, "card", StyleId::PaddingTop).contains(&md),
        "{} vs {md}",
        row(&r, "card", StyleId::PaddingTop)
    );
    assert!(
        row(&r, "card", StyleId::BorderRadiusTopLeft).contains("9999.0"),
        "{}",
        row(&r, "card", StyleId::BorderRadiusTopLeft)
    );
    assert!(
        row(&r, "pill", StyleId::BorderRadiusTopLeft).contains(&format!("{:.1}", web("radius.lg")))
    );
}

#[test]
fn an_apps_own_definition_wins_and_an_unread_one_is_not_kept() {
    let design = "tamber t\n\ntheme\n  space.md = 40px\n  space.lg = 50px\n";
    let plan = contract::compile_path(&app(
        "own",
        &[("app.tamber", design), ("app.contract", VIEW)],
    ))
    .unwrap();
    let names = theme_names(&plan);
    // Read: kept, as any token is; unread (`space.lg`): nothing, as any token.
    assert!(names.contains(&"space.md".to_string()), "{names:?}");
    assert!(!names.contains(&"space.lg".to_string()), "{names:?}");
    // The base names the design leaves unset keep nothing either.
    assert!(
        !names.contains(&"space.sm".to_string()) && !names.contains(&"radius.full".to_string()),
        "{names:?}"
    );
    let r = booted(plan);
    assert!(
        row(&r, "card", StyleId::PaddingTop).contains("40.0"),
        "{}",
        row(&r, "card", StyleId::PaddingTop)
    );
    assert!(
        row(&r, "card", StyleId::RowGap).contains(&format!("{:.1}", web("space.sm"))),
        "{}",
        row(&r, "card", StyleId::RowGap)
    );
}

#[test]
fn an_app_definition_of_a_base_name_is_a_length() {
    for (name, design) in [
        ("colour", "tamber t\n\ntheme\n  space.md = #ff0000\n"),
        ("number", "tamber t\n\ntheme\n  radius.md: number = 4\n"),
        // Round 1 (Astra 2): an icon under a base name. (A dial's name is an
        // identifier, so none can be one.)
        (
            "icon",
            "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  space.md = icon(sf \"star\")\n",
        ),
    ] {
        let e = contract::compile_path(&app(
            name,
            &[("app.tamber", design), ("app.contract", VIEW)],
        ))
        .unwrap_err();
        assert_eq!(e.id, "tamber-base-kind", "{name}: {e:?}");
        assert!(e.message.contains("base length"), "{}", e.message);
    }
    // A length, or one varying by an app axis, is the app's.
    let design = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  space.md = 12px\n    when density=compact: 8px\n";
    contract::compile_path(&app(
        "axis",
        &[("app.tamber", design), ("app.contract", VIEW)],
    ))
    .unwrap();
}

#[test]
fn a_base_name_in_an_inherited_row_is_no_axis_refusal_and_a_wrong_row_is_refused() {
    // `letter-spacing` is inherited; the base varies by column, never by an app axis.
    contract::compile_path(&app("inherited", &[("app.contract", VIEW)])).unwrap();
    let e = contract::compile_path(&app(
        "wrong-row",
        &[(
            "app.contract",
            "component App\n  view\n    column opacity=\"space.md\"\n      text \"a\"\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    // Not a base name: no length token, so the kernel's own refusal.
    assert!(contract::compile_path(&app(
        "not-base",
        &[(
            "app.contract",
            "component App\n  view\n    column padding=\"space.huge\"\n      text \"a\"\n"
        )]
    ))
    .is_err());
}

/// Round 1 (theme2's (a)): an expression over a base name the design leaves
/// unset is refused; over one it sets, it is its token's; a bare one is an alias
/// (`contract/cli/tests/base_alias.rs`).
#[test]
fn an_expression_over_a_base_name_is_refused() {
    let view = "component App\n  view\n    column padding=\"card.pad\"\n      text \"a\"\n";
    for (name, design) in [
        (
            "calc",
            "tamber t\n\ntheme\n  card.pad: length = calc(space.md + 4px)\n",
        ),
        (
            "max",
            "tamber t\n\ntheme\n  card.pad: length = max(space.sm, 4px)\n",
        ),
    ] {
        let e = contract::compile_path(&app(
            name,
            &[("app.tamber", design), ("app.contract", view)],
        ))
        .unwrap_err();
        assert_eq!(e.id, "tamber-base-expression", "{name}: {e:?}");
        assert!(
            e.message.contains("bare") && e.message.contains("QUEUE"),
            "{}",
            e.message
        );
    }
    // Over a base name the design sets, it is the design's token's expression, as any.
    contract::compile_path(&app(
        "own-calc",
        &[
            (
                "app.tamber",
                "tamber t\n\ntheme\n  space.md = 12px\n  card.pad: length = calc(space.md * 2)\n",
            ),
            ("app.contract", view),
        ],
    ))
    .unwrap();
    // Bare, it compiles (an alias).
    contract::compile_path(&app(
        "bare-alias",
        &[
            (
                "app.tamber",
                "tamber t\n\ntheme\n  card.pad: length = space.md\n",
            ),
            ("app.contract", view),
        ],
    ))
    .unwrap();
}

/// Round 2 (Astra 2): outside Exact (the DTCG export) an alias to a base name
/// the design leaves unset writes the neutral column's px, from the kernel's
/// table, never an invented 1px; the base name is the source it keeps.
#[test]
fn an_alias_to_a_base_name_exports_the_neutral_columns_length() {
    use exact_kernel::style::{metrics::Column, scale};
    let design = "tamber t\n\ntheme\n  card.pad: length = space.md\n  pill: length = radius.full\n";
    let vocab = contract_lower::tamber_vocab();
    let folding = exact_tamber::check_design(exact_tamber::parse(design).unwrap(), &vocab).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&exact_tamber::export::dtcg(&folding, &vocab).unwrap()).unwrap();
    let text = json.to_string();
    let neutral = match scale::of("space.md", Column::Neutral).unwrap() {
        scale::ScaleValue::Px(n) => n,
        other => panic!("{other:?}"),
    };
    let light = &json["modifiers"]["mode"]["contexts"]["light"][0];
    let px = |v: f64| serde_json::json!({"value": v, "unit": "px"});
    assert_eq!(light["card"]["pad"]["$value"], px(neutral), "{text}");
    assert_eq!(
        light["card"]["pad"]["$extensions"]["exact"]["lossy"],
        "space.md"
    );
    assert_eq!(light["pill"]["$value"], px(scale::RADIUS_FULL), "{text}");
}
