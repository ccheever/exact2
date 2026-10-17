//! @ref LLP 1121 D3, D7 (stage 6 B) — length and number tokens on the JS
//! target: a varying one a property declared per tuple, one that does not
//! vary inline as its literal, every one a property in a development build,
//! and a bound row's value mapped.

fn plan(tag: &str, design: &str, view: &str) -> exact_plan::Plan {
    let dir = std::env::temp_dir().join(format!(
        "exact-web-js-measures-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    plan
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  space.row = 12px\n    when density=compact: 0.5rem\n  radius.card = 6px\n  size.title = -exact-title1\n  line.hair = -exact-hairline\n  opacity.faded: number = 0.5\n    when density=compact: 0.25\n  weight.strong = 600\n";

#[test]
fn a_varying_length_or_number_is_a_property_declared_by_each_tuples_rule() {
    let plan = plan(
        "rules",
        DESIGN,
        "component App\n  view\n    column padding=\"space.row\" opacity=\"opacity.faded\" border-radius=\"radius.card\" border-top-width=\"line.hair\"\n      text \"a\" font-size=\"size.title\" font-weight=\"weight.strong\"\n",
    );
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let css = &out.css;
    assert!(css.contains("padding-top:var(--x-space-row)"), "{css}");
    assert!(css.contains("opacity:var(--x-opacity-faded)"), "{css}");
    // One that does not vary is its literal's CSS: px, a text style's rem,
    // the sheet's hairline, a weight.
    assert!(css.contains("border-top-left-radius:6px"), "{css}");
    assert!(css.contains("font-size:1.6875rem"), "{css}");
    assert!(css.contains("font-weight:600"), "{css}");
    assert!(
        css.contains("border-top-width:var(--exact-hairline, 1px)"),
        "{css}"
    );
    let regular = css
        .find(":root,[data-x-modes=\"density=regular\"]{")
        .expect(css);
    let compact = css.find("[data-x-modes=\"density=compact\"]{").expect(css);
    assert!(
        css[regular..compact].contains("--x-space-row:12px;"),
        "{css}"
    );
    assert!(
        css[regular..compact].contains("--x-opacity-faded:0.5;"),
        "{css}"
    );
    // A `rem` stays one: the browser resolves it.
    assert!(css[compact..].contains("--x-space-row:0.5rem;"), "{css}");
    assert!(css[compact..].contains("--x-opacity-faded:0.25;"), "{css}");
    assert!(!css.contains("--x-radius-card"), "{css}");
    // No sentinel is left in the page.
    assert!(!css.contains("7000.") && !css.contains("0.0001"), "{css}");
}

/// A token that does not vary is byte for byte its literal in a release.
#[test]
fn a_release_writes_a_constant_token_as_its_literal() {
    let token = plan(
        "token",
        DESIGN,
        "component App\n  view\n    column border-radius=\"radius.card\"\n      text \"a\" font-size=\"size.title\"\n",
    );
    let literal = plan(
        "literal",
        DESIGN,
        "component App\n  view\n    column border-radius=\"6px\"\n      text \"a\" font-size=\"-exact-title1\"\n",
    );
    let (a, b) = (
        crate::emit::emit(&token, false, false).unwrap(),
        crate::emit::emit(&literal, false, false).unwrap(),
    );
    assert_eq!(a.css, b.css);
    assert_eq!(a.js, b.js);
}

/// A development build makes every length and number token a property, so
/// a live edit sets it in place, and its design names each one's CSS per
/// tuple; a number a font weight names stays inline (no sentinel fraction
/// is a weight).
#[test]
fn a_development_build_makes_every_length_and_number_a_property() {
    let plan = plan(
        "dev",
        DESIGN,
        "component App\n  view\n    column border-radius=\"radius.card\" padding=\"space.row\"\n      text \"a\" font-weight=\"weight.strong\" font-size=\"size.title\"\n",
    );
    let dev = crate::emit::emit(&plan, false, true).unwrap();
    assert!(
        dev.css
            .contains("border-top-left-radius:var(--x-radius-card)"),
        "{}",
        dev.css
    );
    assert!(
        dev.css.contains("font-size:var(--x-size-title)"),
        "{}",
        dev.css
    );
    assert!(dev.css.contains("font-weight:600"), "{}", dev.css);
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)
    };
    let token = |name: &str| {
        design["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(
        token("radius.card")["css"],
        serde_json::json!(["6px", "6px"])
    );
    assert_eq!(
        token("space.row")["css"],
        serde_json::json!(["12px", "0.5rem"])
    );
    assert_eq!(
        token("size.title")["css"],
        serde_json::json!(["1.6875rem", "1.6875rem"])
    );
    assert!(token("weight.strong").get("prop").is_none());
}

#[test]
fn a_bound_row_maps_a_tokens_name_to_its_css() {
    let plan = plan(
        "bound",
        DESIGN,
        "component App\n  state on = false\n  view\n    column padding=(on ? \"space.row\" : \"radius.card\") opacity=(on ? \"opacity.faded\" : 1)\n      text \"a\"\n",
    );
    let out = crate::emit::emit(&plan, false, false).unwrap();
    assert!(
        out.js.contains("\"space.row\":\"var(--x-space-row)\"")
            && out.js.contains("\"radius.card\":\"6px\""),
        "{}",
        out.js
    );
    assert!(
        out.js
            .contains("\"opacity.faded\":\"var(--x-opacity-faded)\""),
        "{}",
        out.js
    );
    assert!(
        out.js.contains(",\"padding-top\",\"px\",()=>$tm("),
        "{}",
        out.js
    );
}
