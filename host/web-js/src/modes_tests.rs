//! @ref LLP 1121 D7 — the app's modes on the JS target: varying tokens as
//! custom properties, one rule per tuple, and the boundaries' attributes.

/// The JS target's output for an app directory holding `design` and `view`.
fn emitted(tag: &str, design: &str, view: &str) -> crate::emit::Output {
    let dir = std::env::temp_dir().join(format!("exact-web-js-modes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

const DESIGN: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  surface.card = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #15202b)\n  text.primary = light-dark(#0f1419, #e7e9ea)\n    when dark-style=dim: light-dark(#0f1419, #f7f9f9)\n  brand = #c8102e\n";

#[test]
fn a_varying_token_is_a_property_declared_by_each_tuples_rule() {
    let out = emitted(
        "rules",
        DESIGN,
        "component App\n  view\n    column background-color=\"surface.card\" color=\"text.primary\"\n      text \"a\" color=\"brand\"\n      text \"b\" text-shadow=\"0 1px 2px surface.card\"\n",
    );
    let css = &out.css;
    assert!(
        css.contains("background-color:var(--x-surface-card)"),
        "{css}"
    );
    assert!(css.contains("color:var(--x-text-primary)"), "{css}");
    assert!(
        css.contains("text-shadow:0px 1px 2px var(--x-surface-card);"),
        "{css}"
    );
    // A token that does not vary stays its value.
    assert!(
        css.contains("color:rgba(200,16,46,1)") || css.contains("#c8102eff"),
        "{css}"
    );
    let black = css
        .find(":root,[data-x-modes=\"dark-style=black\"]{")
        .expect(css);
    let dim = css.find("[data-x-modes=\"dark-style=dim\"]{").expect(css);
    assert!(black < dim, "{css}");
    assert!(
        css[black..dim].contains("--x-surface-card:light-dark(#ffffffff, #000000ff);"),
        "{css}"
    );
    assert!(
        css[dim..].contains("--x-surface-card:light-dark(#ffffffff, #15202bff);"),
        "{css}"
    );
    assert!(!css[black..].contains("--x-brand"), "{css}");
    // No sentinel is left in the page.
    assert!(!css.contains("#e1") && !css.contains("rgba(225,"), "{css}");
    assert!(
        out.js
            .contains("import{modes as $modes,Sx}from\"./modes.js\""),
        "{}",
        out.js
    );
    assert!(
        out.js
            .contains("$modes([[\"dark-style\",[\"black\",\"dim\"]]]"),
        "{}",
        out.js
    );
    // What a page rendered ahead wrote inline that the class names a token in.
    assert!(
        out.js.contains("c1:[\"background-color\",\"color\"]"),
        "{}",
        out.js
    );
}

#[test]
fn a_sentinel_moves_off_a_colour_the_app_writes() {
    let out = emitted(
        "sentinel",
        DESIGN,
        "component App\n  view\n    column background-color=\"#e10000\" color=\"surface.card\"\n      text \"a\" border-color=\"rgba(226, 0, 0, 1)\" border-width=1\n",
    );
    assert!(
        out.css.contains("background-color:rgba(225,0,0,1)"),
        "{}",
        out.css
    );
    assert!(out.css.contains("rgba(226,0,0,1)"), "{}", out.css);
    assert!(
        out.css.contains("color:var(--x-surface-card)"),
        "{}",
        out.css
    );
}

#[test]
fn a_boundary_carries_its_rows_and_pin_and_a_class_its_channels() {
    let out = emitted(
        "boundary",
        DESIGN,
        "component App\n  state pin: string = \"dark-style=dim\"\n  view\n    column color=\"text.primary\"\n      column -exact-mode=\"dark-style=black\" testId=\"black\"\n        text \"a\"\n      column color-scheme=\"light\" color=\"#123456\" testId=\"light\"\n        text \"b\"\n      column -exact-mode=pin testId=\"bound\"\n        text \"c\"\n",
    );
    let js = &out.js;
    assert!(
        js.contains("\"data-x-b\":\"0\",\"data-x-pin\":\"dark-style=black\""),
        "{js}"
    );
    // The Light card sets its own colour, from no token.
    assert!(js.contains("\"data-x-b\":\"1\""), "{js}");
    assert!(
        out.css
            .contains("color:rgba(18,52,86,1);color-scheme:light;display:block;--xi0:-1;"),
        "{}",
        out.css
    );
    assert!(
        out.css
            .contains("color:var(--x-text-primary);position:relative;--xi0:0;"),
        "{}",
        out.css
    );
    assert!(js.contains("P(") && js.contains(",\"data-x-pin\","), "{js}");
    assert!(js.contains("[\"color:var(--x-text-primary)\"]"), "{js}");
}

#[test]
fn a_bound_inherited_colour_says_whether_it_read_a_token() {
    let out = emitted(
        "bound",
        DESIGN,
        "component App\n  state c: string = \"text.primary\"\n  view\n    column color=c fill=c background-color=c\n      text \"a\"\n",
    );
    let js = &out.js;
    assert!(js.contains("Sx(S,"), "{js}");
    assert!(js.contains("\"color\",\"\","), "{js}");
    assert!(js.contains(",0);") && js.contains(",2);"), "{js}");
    // A row no boundary writes again is the plain binding.
    assert!(
        js.contains("S(e") && js.contains("\"background-color\""),
        "{js}"
    );
    // The theme map names a varying token's property, and says it read one.
    assert!(
        js.contains("\"text.primary\":\"var(--x-text-primary)\""),
        "{js}"
    );
    assert!(js.contains("$th=1"), "{js}");
}

#[test]
fn an_accent_naming_token_makes_an_authored_accent_a_boundary() {
    let design = "tamber t\n\naxes\n  ink = tint | plain\n\ntheme\n  accent.mix = AccentColor\n    when ink=plain: #000000\n";
    let out = emitted(
        "accent",
        design,
        "component App\n  view\n    column color=\"accent.mix\"\n      column accent-color=\"#00ff00\" testId=\"tinted\"\n        text \"a\"\n",
    );
    assert!(
        out.css.contains("--x-accent-mix:var(--exact-accent"),
        "{}",
        out.css
    );
    assert!(out.js.contains("\"data-x-b\":\"127\""), "{}", out.js);
}

#[test]
fn a_plan_with_no_modes_loads_none() {
    let plan = contract::compile("theme\n  brand = \"#c8102e\"\n\ncomponent App\n  view\n    column color=\"brand\"\n      text \"a\"\n").unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    assert!(
        !out.js.contains("modes.js") && !out.js.contains("data-x-"),
        "{}",
        out.js
    );
    assert!(
        !out.css.contains("data-x-modes") && !out.css.contains("--xi"),
        "{}",
        out.css
    );
}

#[test]
fn a_bound_colour_on_a_boundary_is_the_runtimes_to_say() {
    let out = emitted(
        "bound-own",
        DESIGN,
        "component App\n  state c: string = \"#123456\"\n  view\n    column color=\"text.primary\"\n      column -exact-mode=\"dark-style=dim\" color=c testId=\"pin\"\n        text \"a\"\n",
    );
    // Its CSS sets no row: while the binding is unset the row is written again.
    assert!(
        out.js
            .contains("\"data-x-b\":\"0\",\"data-x-pin\":\"dark-style=dim\""),
        "{}",
        out.js
    );
    assert!(out.js.contains("Sx(S,"), "{}", out.js);
}

#[test]
fn an_accent_naming_its_own_token_names_no_property_on_a_boundary() {
    let design = "tamber t\n\naxes\n  ink = tint | plain\n\ntheme\n  accent.mix = AccentColor\n    when ink=plain: #000000\n";
    let out = emitted(
        "accent-own",
        design,
        "component App\n  view\n    column\n      column accent-color=\"accent.mix\" testId=\"plain\"\n        text \"a\"\n      column accent-color=\"accent.mix\" -exact-mode=\"ink=plain\" testId=\"pinned\"\n        text \"b\"\n",
    );
    // Elsewhere it reads the token from above, and is no boundary.
    assert!(
        out.css
            .contains("accent-color:var(--x-accent-mix);--exact-accent:var(--x-accent-mix);"),
        "{}",
        out.css
    );
    assert!(
        out.js.contains("\"data-testid\":\"plain\"") && !out.js.contains("\"data-x-b\":\"127\""),
        "{}",
        out.js
    );
    // On a pin, which declares the token, the accent around stands.
    assert_eq!(
        out.css.matches("accent-color:var(--x-accent-mix);").count(),
        2,
        "{}",
        out.css
    );
    assert!(
        out.css
            .matches("--exact-accent:var(--x-accent-mix);")
            .count()
            == 1,
        "{}",
        out.css
    );
}

#[test]
fn each_varying_token_has_its_own_property() {
    let design = "tamber t\n\naxes\n  ink = tint | plain\n\ntheme\n  a-b-2 = #000001\n    when ink=plain: #000002\n  a.b = #000003\n    when ink=plain: #000004\n  a-b = #000005\n    when ink=plain: #000006\n";
    let out = emitted(
        "names",
        design,
        "component App\n  view\n    column color=\"a-b-2\" background-color=\"a.b\" border-color=\"a-b\" border-width=1\n      text \"a\"\n",
    );
    let rule = &out.css[out.css.find(":root,").expect(&out.css)..];
    let props: Vec<&str> = rule[..rule.find('}').unwrap()]
        .split(['{', ';'])
        .filter_map(|d| d.split_once(':').map(|(p, _)| p))
        .filter(|p| p.starts_with("--x-"))
        .collect();
    assert_eq!(props.len(), 3, "{rule}");
    assert!(
        props
            .iter()
            .all(|p| props.iter().filter(|q| *q == p).count() == 1),
        "{rule}"
    );
}

#[test]
fn a_literal_clear_sets_no_row() {
    let out = emitted(
        "clear",
        DESIGN,
        "component App\n  view\n    column color=\"text.primary\"\n      column -exact-mode=\"dark-style=dim\" color=\"unset\" testId=\"pin\"\n        text \"a\"\n",
    );
    assert!(
        out.js
            .contains("\"data-x-b\":\"0\",\"data-x-pin\":\"dark-style=dim\""),
        "{}",
        out.js
    );
    assert_eq!(out.css.matches("--xi0:").count(), 1, "{}", out.css);
}

#[test]
fn a_pinned_accent_naming_its_token_declares_the_accent_where_the_token_is_a_colour() {
    let design = "tamber t\n\naxes\n  ink = tint | plain\n\ntheme\n  accent.mix = AccentColor\n    when ink=plain: #000000\n";
    let out = emitted(
        "accent-rule",
        design,
        "component App\n  view\n    column accent-color=\"accent.mix\" -exact-mode=\"ink=plain\" testId=\"pinned\"\n      text \"b\"\n",
    );
    assert!(
        out.js.contains("\"data-x-accent\":\"--x-accent-mix\""),
        "{}",
        out.js
    );
    assert!(
        out.css.contains(
            "[data-x-modes=\"ink=plain\"][data-x-accent=\"--x-accent-mix\"]{--exact-accent:#000000ff}"
        ),
        "{}",
        out.css
    );
    assert!(
        !out.css
            .contains("[data-x-modes=\"ink=tint\"][data-x-accent"),
        "{}",
        out.css
    );
    assert!(
        out.js
            .contains(",[\"--x-accent-mix\"],{\"--x-accent-mix\":\"accent.mix\"});"),
        "{}",
        out.js
    );
}

/// LLP 1121 stage 4, commit 4: a development build makes every token a
/// property, varying or not, so the live table can set it in place; its
/// design names each one's CSS per tuple. A release build folds a token
/// that does not vary inline, as before.
#[test]
fn a_development_build_makes_every_token_a_property() {
    let view = "component App\n  view\n    column background-color=\"surface.card\"\n      text \"a\" color=\"brand\"\n";
    let dir = std::env::temp_dir().join(format!("exact-web-js-modes-dev-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let release = crate::emit::emit(&plan, false, false).unwrap();
    let dev = crate::emit::emit(&plan, false, true).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!release.css.contains("--x-brand"), "{}", release.css);
    assert!(dev.css.contains("color:var(--x-brand)"), "{}", dev.css);
    let root = dev
        .css
        .find(":root,[data-x-modes=\"dark-style=black\"]{")
        .expect(&dev.css);
    assert!(
        dev.css[root..].contains("--x-brand:#c8102eff;"),
        "{}",
        dev.css
    );
    // The page knows the design it was built with: a batch's base.
    let digest = exact_runner::design::digest(&exact_runner::design::tokens(&plan));
    let structure = exact_runner::design::shape(&plan);
    assert!(
        dev.js.contains(&format!(",\"{digest}\",\"{structure}\");")),
        "{}",
        dev.js
    );
    assert!(!release.js.contains(&digest));
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)
    };
    assert_eq!(design["digest"], digest);
    let brand = design["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "brand")
        .unwrap();
    assert_eq!(brand["prop"], "--x-brand");
    assert_eq!(brand["css"], serde_json::json!(["#c8102eff", "#c8102eff"]));
    assert_eq!(brand["accent"], serde_json::json!([false, false]));
    let card = design["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "surface.card")
        .unwrap();
    assert_eq!(card["css"][1], "light-dark(#ffffffff, #15202bff)");
}

/// LLP 1121 stage 5, commit 2: a development build makes each role
/// override live. A row naming an overridden role reads its sheet property
/// (`--exact-canvas-text`), declared in the root's tuple rule after the
/// theme's sheet; a role the kernel crosses reads it over its value; the
/// tint's override is its `--exact-accent` in the design. A release build
/// crosses the values, as before.
#[test]
fn a_development_build_makes_every_role_override_a_property() {
    let view = "component App\n  view\n    column\n      text \"a\" color=\"CanvasText\"\n      text \"b\" color=\"-exact-label\"\n";
    let dir = std::env::temp_dir().join(format!("exact-web-js-modes-roles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  CanvasText = ink\n  -exact-label = #222222\n  AccentColor = #0a7aff\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let release = crate::emit::emit(&plan, false, false).unwrap();
    let dev = crate::emit::emit(&plan, false, true).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        release.css.contains("color:rgba(17,17,17,1);"),
        "{}",
        release.css
    );
    let root = dev.css.find(":root,[data-x-modes=\"\"]{").expect(&dev.css);
    let theme = dev
        .css
        .find("--exact-canvas-text:#111111ff")
        .expect(&dev.css);
    assert!(
        theme < root,
        "the tuple rule follows the theme's sheet: {}",
        dev.css
    );
    assert!(
        dev.css[root..].contains("--exact-canvas-text:#111111ff;"),
        "{}",
        dev.css
    );
    assert!(
        dev.css.contains("color:var(--exact-canvas-text)"),
        "{}",
        dev.css
    );
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)
    };
    let row = |name: &str| {
        design["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(row("CanvasText")["prop"], "--exact-canvas-text");
    assert_eq!(row("CanvasText")["css"], serde_json::json!(["#111111ff"]));
    assert_eq!(row("AccentColor")["prop"], "--exact-accent");
    assert_eq!(row("AccentColor")["css"], serde_json::json!(["#0a7affff"]));
    assert_eq!(row("-exact-label")["prop"], "--exact-label");
    assert!(dev.css.contains("color:var(--exact-label);"), "{}", dev.css);
    assert!(
        release.css.contains("color:rgba(34,34,34,1);"),
        "{}",
        release.css
    );
}
