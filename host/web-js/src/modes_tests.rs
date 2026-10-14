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
            .contains("color:rgba(18,52,86,1);color-scheme:light;display:block;--x-i0:-1;"),
        "{}",
        out.css
    );
    assert!(
        out.css
            .contains("color:var(--x-text-primary);position:relative;--x-i0:0;"),
        "{}",
        out.css
    );
    assert!(js.contains("P(") && js.contains(",\"data-x-pin\","), "{js}");
    assert!(js.contains("[\"var(--x-text-primary)\"]"), "{js}");
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
        !out.css.contains("data-x-modes") && !out.css.contains("--x-i"),
        "{}",
        out.css
    );
}
