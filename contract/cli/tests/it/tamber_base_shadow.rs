//! @ref LLP 1121 D11 (stage 12 B3) — a shadow's lengths over base names the
//! design leaves unset: kept as B2's expressions are (a bare one, a token
//! aliasing one or over some, inlined, or a length's function over one),
//! which each host folds in its column (`contract/cli/tests/base_expression.rs`
//! holds the five columns at run time). The compiler checks every column
//! and names the one a shadow fails in. B2's refusals hold inside a shadow.
//! A literal shadow row stays px: a word in one that names a length or no
//! token says so (QUEUE's two items).

use exact_kernel::style::scale;
use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact2-base-shadow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

/// A view whose one column's `box-shadow` names `card.shadow`.
const VIEW: &str =
    "component App\n  view\n    column box-shadow=\"card.shadow\"\n      text \"a\"\n";

fn compiled(name: &str, theme: &str, view: &str) -> Result<exact_plan::Plan, (String, String)> {
    let design = format!("tamber t\n\ntheme\n{theme}");
    contract::compile_path(&app(
        name,
        &[("app.tamber", &design), ("app.contract", view)],
    ))
    .map_err(|e| (e.id.to_string(), e.message))
}

fn value(plan: &exact_plan::Plan, name: &str) -> String {
    let t = plan
        .theme
        .iter()
        .find(|t| plan.str(t.name) == name)
        .unwrap_or_else(|| panic!("no `{name}`"));
    plan.str(t.value).to_string()
}

/// A shadow's text in `column`, as that column's host folds it.
fn folded(text: &str, column: exact_kernel::style::metrics::Column) -> Option<String> {
    scale::fold_lengths(text, |n| scale::of(n, column).map(scale::css))
}

/// An expression's px text in `column`.
fn px(expr: &str, column: exact_kernel::style::metrics::Column) -> String {
    format!("{}px", scale::evaluate(expr, column).unwrap())
}

#[test]
fn a_shadow_over_base_names_is_kept_and_each_column_folds_it() {
    use exact_kernel::style::metrics::Column;
    // Each the shadow's offset-y as written, and its kept text.
    for (name, theme, kept) in [
        (
            "bare",
            "  card.shadow: box-shadow = 0 space.md 4px black\n",
            "space.md",
        ),
        (
            "token",
            "  lift: length = calc(space.md * 2)\n  card.shadow: box-shadow = 0 lift 4px black\n",
            "calc(space.md * 2)",
        ),
        (
            "alias",
            "  lift: length = space.md\n  card.shadow: box-shadow = 0 lift 4px black\n",
            "space.md",
        ),
        (
            "calc",
            "  card.shadow: box-shadow = 0 calc(space.md * 2) 4px black\n",
            "calc(space.md * 2)",
        ),
        (
            "calc-alias",
            "  lift: length = space.md\n  card.shadow: box-shadow = 0 calc(lift * 2) 4px black\n",
            "calc(space.md * 2)",
        ),
        (
            "calc-lone",
            "  card.shadow: box-shadow = 0 calc(space.md + 2px - 2px) 4px black\n",
            "space.md",
        ),
        (
            "min",
            "  card.shadow: box-shadow = 0 min(space.lg, 20px, 30px) 4px black\n",
            "min(space.lg, 20px)",
        ),
    ] {
        let plan = compiled(name, theme, VIEW).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let text = value(&plan, "card.shadow");
        let colour = text
            .strip_prefix(&format!("0px {kept} 4px "))
            .unwrap_or_else(|| panic!("{name}: {text}"));
        // Each column's px, the colour kept.
        let expr = format!("calc({kept})");
        for column in [Column::Neutral, Column::Mac, Column::Ios] {
            assert_eq!(
                folded(&text, column),
                Some(format!("0px {} 4px {colour}", px(&expr, column))),
                "{name} {column:?}"
            );
        }
    }
    // The neutral look's and the Mac's differ for space.md: the plan holds
    // no column's px.
    assert_ne!(
        px("calc(space.md)", Column::Neutral),
        px("calc(space.md)", Column::Mac)
    );
    // A list, `inset` and a text shadow keep theirs too.
    let theme = "  card.shadow: box-shadow = inset 0 1px space.sm #000, 0 space.md calc(space.lg / 2) 0 #000\n  glow: text-shadow = 0 space.xs space.sm #ff0000\n";
    let view = "component App\n  view\n    column box-shadow=\"card.shadow\"\n      text \"a\" text-shadow=\"glow\"\n";
    let plan = compiled("list", theme, view).unwrap_or_else(|e| panic!("{e:?}"));
    let text = value(&plan, "card.shadow");
    assert!(
        text.starts_with("inset 0px 1px space.sm ")
            && text.contains(", 0px space.md calc(space.lg * 0.5) 0px "),
        "{text}"
    );
    assert!(
        value(&plan, "glow").starts_with("0px space.xs space.sm "),
        "{}",
        value(&plan, "glow")
    );
}

/// Each base a shadow reads is in the token's `bases`, an inlined token's
/// too; a base name the design sets is its token, folded to px.
#[test]
fn a_shadows_bases_and_a_base_the_design_sets() {
    let design = "tamber t\n\ntheme\n  lift: length = calc(space.lg - space.xs)\n  card.shadow: box-shadow = 0 space.md 4px #000, 0 lift space.sm #000\n";
    let folding = exact_tamber::check_design(
        exact_tamber::parse(design).unwrap(),
        &contract_lower::tamber_vocab(),
    )
    .unwrap();
    let token = folding
        .tokens
        .iter()
        .find(|t| t.name == "card.shadow")
        .unwrap();
    assert_eq!(
        token.bases,
        ["space.md", "space.lg", "space.xs", "space.sm"]
    );
    // The design's own space.md is its token: px, as before.
    let plan = compiled(
        "set",
        "  space.md = 10px\n  card.shadow: box-shadow = 0 space.md 4px black\n",
        VIEW,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        value(&plan, "card.shadow").starts_with("0px 10px 4px "),
        "{}",
        value(&plan, "card.shadow")
    );
}

/// Round 1 (Astra 1): a length's function in a shadow folds by the length's
/// own code whatever it names, so setting a base never turns the same text
/// into a refusal, and a function over a px length token folds to px (main
/// read either as a colour, and refused it).
#[test]
fn a_shadows_function_folds_as_a_lengths_whatever_it_names() {
    const SHADOW: &str = "  card.shadow: box-shadow = 0 calc(space.md * 2) 4px black
";
    let unset = compiled("fn-unset", SHADOW, VIEW).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(value(&unset, "card.shadow").starts_with("0px calc(space.md * 2) 4px "));
    let set = compiled(
        "fn-set",
        &format!(
            "  space.md = 10px
{SHADOW}"
        ),
        VIEW,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        value(&set, "card.shadow").starts_with("0px 20px 4px "),
        "{}",
        value(&set, "card.shadow")
    );
    for (name, theme, want) in [
        (
            "fn-token",
            "  gap = 3px
  card.shadow: box-shadow = 0 calc(gap * 2) 4px black
",
            "0px 6px 4px ",
        ),
        (
            "fn-px",
            "  card.shadow: box-shadow = 0 max(1px, 2px) calc(2px * 3) black
",
            "0px 2px 6px ",
        ),
        (
            "fn-mixed",
            "  gap = 3px
  card.shadow: box-shadow = 0 min(space.lg, gap * 4) 4px black
",
            "0px min(space.lg, 12px) 4px ",
        ),
    ] {
        let plan = compiled(name, theme, VIEW).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(
            value(&plan, "card.shadow").starts_with(want),
            "{name}: {}",
            value(&plan, "card.shadow")
        );
    }
}

/// Round 2 (Astra 1): a colour function whose words name a base (a
/// platform colour's asset `space.md`) is a colour, kept whole: a shadow
/// whose lengths are px is not refused as unfoldable, box or text.
#[test]
fn a_platform_colour_named_for_a_base_is_kept_whole() {
    const COLOUR: &str = "-exact-platform-color(ios named:space.md, #000000)";
    let theme = format!("  card.shadow: box-shadow = 0 1px 2px {COLOUR}\n  glow: text-shadow = 0 1px 2px {COLOUR}\n");
    let view = "component App\n  view\n    column box-shadow=\"card.shadow\"\n      text \"a\" text-shadow=\"glow\"\n";
    let plan = compiled("platform-colour", &theme, view).unwrap_or_else(|e| panic!("{e:?}"));
    for name in ["card.shadow", "glow"] {
        let text = value(&plan, name);
        assert!(
            text.starts_with("0px 1px 2px -exact-platform-color("),
            "{text}"
        );
        assert_eq!(
            folded(&text, exact_kernel::style::metrics::Column::Ios).as_deref(),
            Some(text.as_str())
        );
    }
}

/// Round 2 (Grok 1): an unkinded value of lengths' functions and a colour
/// reads as a shadow, so its kind is asked for.
#[test]
fn an_unkinded_shadow_of_functions_asks_for_its_kind() {
    for (name, v) in [
        ("unkinded-a", "0 max(1px, 2px) calc(2px * 3) black"),
        ("unkinded-b", "min(1px, 2px) max(3px, 4px) calc(5px) #000"),
    ] {
        let (id, message) = compiled(name, &format!("  card.shadow = {v}\n"), VIEW).unwrap_err();
        assert_eq!(id, "tamber-kind", "{name}: {message}");
        assert!(
            message.contains("reads as a shadow: write its kind"),
            "{name}: {message}"
        );
    }
    // Functions over unset base names count too; over plain numbers they
    // are a number's, never a shadow's (round 3, Grok).
    let (_, message) = compiled(
        "unkinded-base",
        "  card.shadow = calc(space.md * 2) calc(space.sm) black\n",
        VIEW,
    )
    .unwrap_err();
    assert!(message.contains("reads as a shadow"), "{message}");
    let (_, message) = compiled(
        "unkinded-numbers",
        "  n = min(1, 2) max(3, 4) calc(5)\n",
        "component App\n  view\n    column\n      text \"a\"\n",
    )
    .unwrap_err();
    assert!(!message.contains("reads as a shadow"), "{message}");
}

/// B2's refusals hold inside a shadow; a literal blur is still never
/// negative.
#[test]
fn b2s_refusals_hold_inside_a_shadow() {
    for (name, length, id, said) in [
        (
            "times-length",
            "calc(space.md * 2px)",
            "tamber-kind",
            "a length times or over a length",
        ),
        (
            "rem",
            "calc(space.md + 1rem)",
            "tamber-kind",
            "`rem`/`em` where the kernel still takes only `px`",
        ),
        (
            "number-over",
            "calc(2 / space.md)",
            "tamber-kind",
            "a number over a length",
        ),
        (
            "mixed",
            "min(space.md, 2)",
            "tamber-kind",
            "mixes a length and a number",
        ),
    ] {
        let theme = format!("  card.shadow: box-shadow = 0 {length} 4px black\n");
        let (got, message) = compiled(name, &theme, VIEW).unwrap_err();
        assert_eq!((got.as_str(), name), (id, name), "{message}");
        assert!(message.contains(said), "{name}: {message}");
    }
    let (id, message) = compiled(
        "negative-literal",
        "  card.shadow: box-shadow = space.md 0 -2px black\n",
        VIEW,
    )
    .unwrap_err();
    assert_eq!(id, "tamber-kind", "{message}");
    assert!(
        message.contains("a blur radius is never negative"),
        "{message}"
    );
}

/// The build checks every column: a blur over base names that is negative
/// in one, or a length that doesn't fold in one, is refused, naming the
/// first such column and the expression.
#[test]
fn every_column_is_checked_and_a_refusal_names_its_column() {
    let blur = "calc(space.lg - space.xs * 3.5)";
    let each: Vec<(&str, f32)> = scale::COLUMNS
        .iter()
        .map(|(n, c)| (*n, scale::evaluate(blur, *c).unwrap()))
        .collect();
    // Negative on the Mac only, which is not the table's first column.
    let negative: Vec<&str> = each
        .iter()
        .filter(|(_, v)| *v < 0.0)
        .map(|(n, _)| *n)
        .collect();
    assert_eq!(negative, ["mac"], "{each:?}");
    let (id, message) = compiled(
        "negative-blur",
        &format!("  card.shadow: box-shadow = 0 1px {blur} black\n"),
        VIEW,
    )
    .unwrap_err();
    assert_eq!(id, "tamber-base-expression", "{message}");
    assert!(
        message.contains("on mac")
            && message.contains("calc(space.lg - space.xs * 3.5)")
            && message.contains("a blur radius is never negative"),
        "{message}"
    );
    // A spread may be negative: no column refuses it.
    compiled(
        "negative-spread",
        &format!("  card.shadow: box-shadow = 0 1px 2px {blur} black\n"),
        VIEW,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    // Past what a column folds: the first column, ios.
    let (id, message) = compiled(
        "unfolded",
        "  card.shadow: box-shadow = 0 calc(space.md * 1e38 * 10) 4px black\n",
        VIEW,
    )
    .unwrap_err();
    assert_eq!(id, "tamber-base-expression", "{message}");
    assert!(
        message.contains("don't fold on ios") && message.contains("calc(space.md * 1000"),
        "{message}"
    );
    // A text shadow too.
    let (id, message) = compiled(
        "text-negative-blur",
        &format!("  glow: text-shadow = 0 1px {blur} #f00\n"),
        "component App\n  view\n    text \"a\" text-shadow=\"glow\"\n",
    )
    .unwrap_err();
    assert_eq!(id, "tamber-base-expression", "{message}");
    assert!(message.contains("on mac"), "{message}");
}

/// QUEUE's item: a dotted word in a literal shadow, border or gradient row
/// that names no token is `tamber-unknown-token`, in a colour row's words;
/// a length token or a base name in a literal shadow points to a shadow
/// token. A shadow token's unknown colour is refused by tamber already.
#[test]
fn a_word_in_a_literal_shadow_says_what_it_is() {
    let theme = "  eu.glow = #ff00ff\n  lift: length = 4px\n  lift.edge: length = 2px\n";
    let view = |row: &str| {
        format!("component App\n  view\n    column background-color=\"eu.glow\" {row}\n      text \"a\"\n")
    };
    for (name, row, id, said) in [
        (
            "unknown",
            "box-shadow=\"0px 0px 8px eu.nope\"",
            "tamber-unknown-token",
            "`eu.nope` is no token of this app's design",
        ),
        (
            "unknown-text",
            "text-shadow=\"0px 0px 8px eu.nope\"",
            "tamber-unknown-token",
            "`eu.nope` is no token of this app's design",
        ),
        (
            "length-token",
            "box-shadow=\"0 lift 4px eu.glow\"",
            "tamber-kind",
            "a shadow row's lengths are px: name a shadow token (`x: box-shadow = 0 lift 4px black`) to read `lift`",
        ),
        (
            "base",
            "box-shadow=\"0 space.md 4px eu.glow\"",
            "tamber-kind",
            "name a shadow token (`x: box-shadow = 0 space.md 4px black`) to read `space.md`",
        ),
        (
            "gradient",
            "background-image=\"linear-gradient(eu.nope, #fff)\"",
            "tamber-unknown-token",
            "`eu.nope` is no token of this app's design",
        ),
        (
            "border",
            "border=\"1px solid eu.nope\"",
            "tamber-unknown-token",
            "`eu.nope`",
        ),
        // A base name or a length token in a border or a gradient is no
        // unknown token (round 1, Grok 2): the row's own refusal stands.
        (
            "border-base",
            "border=\"1px solid space.md\"",
            "lower-css-shorthand",
            "`space.md` is not an admitted `border`",
        ),
        (
            "border-length",
            "border=\"1px solid lift.edge\"",
            "lower-css-shorthand",
            "`lift.edge` is not an admitted `border`",
        ),
        // A number, a dimension or a function is no name (round 1, Astra
        // 4): the row's width or colour refusal stands.
        (
            "border-negative",
            "border=\"-1.5px solid black\"",
            "lower-css-shorthand",
            "`-1.5px` is not an admitted `border`",
        ),
        (
            "border-function",
            "border=\"1px solid rgb(0.5 0.25)\"",
            "lower-css-shorthand",
            "`rgb(0.5 0.25)` is not an admitted `border`",
        ),
        (
            "shadow-function",
            "box-shadow=\"0px 0px 8px rgb(0.5 0.25)\"",
            "lower-attr-value",
            "",
        ),
        // Tamber's names may start with `_` (round 2, Astra 2).
        (
            "underscore-shadow",
            "box-shadow=\"0px 0px 8px _eu.nope\"",
            "tamber-unknown-token",
            "`_eu.nope` is no token of this app's design",
        ),
        (
            "underscore-border",
            "border=\"1px solid _eu.nope\"",
            "tamber-unknown-token",
            "`_eu.nope` is no token of this app's design",
        ),
        (
            "gradient-base",
            "background-image=\"linear-gradient(space.md, #fff)\"",
            "lower-attr-value",
            "",
        ),
    ] {
        let (got, message) = compiled(name, theme, &view(row)).unwrap_err();
        assert_eq!((got.as_str(), name), (id, name), "{message}");
        assert!(message.contains(said), "{name}: {message}");
    }
    // The shadow token's own unknown colour: tamber's refusal.
    let (id, message) = compiled(
        "token-unknown",
        "  card.shadow: box-shadow = 0 0 8px eu.nope\n",
        VIEW,
    )
    .unwrap_err();
    assert_eq!(id, "tamber-unknown-token", "{message}");
}

/// `uses` of a length token a shadow inlines lists the shadow's row: the
/// token is still read there (its `names`).
#[test]
fn uses_of_a_length_a_shadow_inlines_lists_its_row() {
    let design = "tamber t\n\ntheme\n  lift: length = calc(space.md * 2)\n  card.shadow: box-shadow = 0 lift 4px #000\n";
    let path = app(
        "uses",
        &[
            ("app.tamber", design),
            ("app.contract", "component App\n  view\n    column padding-top=\"lift\" box-shadow=\"card.shadow\"\n      text \"a\"\n"),
        ],
    );
    let folding = exact_tamber::check_design(
        exact_tamber::parse(design).unwrap(),
        &contract_lower::tamber_vocab(),
    )
    .unwrap();
    let aliases = contract::tamber_uses::aliases(
        "lift",
        folding
            .tokens
            .iter()
            .map(|t| (t.name.as_str(), t.names.as_slice())),
    );
    assert!(aliases.contains("card.shadow"), "{aliases:?}");
    let (plan, map) = contract::compile_path_mapped(&path).unwrap();
    let map: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let sites = contract::tamber_uses::sites(&plan, &map, "lift", &aliases, &[]);
    let rows: Vec<&str> = sites.iter().filter_map(|s| s["row"].as_str()).collect();
    assert!(
        rows.contains(&"padding_top") && rows.contains(&"box_shadow"),
        "{sites:?}"
    );
}

/// Outside Exact (the DTCG export, `tamber gen`'s CSS) a shadow's length
/// over a base name is the neutral column's px, as a length's is, the
/// source kept.
#[test]
fn a_shadow_over_a_base_exports_the_neutral_columns_px() {
    use exact_kernel::style::metrics::Column;
    let design = "tamber t\n\ntheme\n  card.shadow: box-shadow = 0 calc(space.md * 2) 4px #000\n";
    let vocab = contract_lower::tamber_vocab();
    let folding = exact_tamber::check_design(exact_tamber::parse(design).unwrap(), &vocab).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&exact_tamber::export::dtcg(&folding, &vocab).unwrap()).unwrap();
    let want = scale::evaluate("calc(space.md * 2)", Column::Neutral).unwrap();
    let text = json.to_string();
    assert!(
        text.contains(&format!("\"offsetY\":{{\"unit\":\"px\",\"value\":{want}}}"))
            || text.contains(&format!(
                "\"offsetY\":{{\"unit\":\"px\",\"value\":{want}.0}}"
            )),
        "{text}"
    );
    assert!(text.contains("0px calc(space.md * 2) 4px"), "{text}");
}
