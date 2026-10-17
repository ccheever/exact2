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
        out.js.contains(",\"padding-top\",\"px\",()=>$tl("),
        "{}",
        out.js
    );
}

/// Review round 1: two varying numbers each get a sentinel no other is a
/// part of; an SVG stroke's length, which css.rs writes unitless, is its
/// property too; a font weight's number says it is inline, so a change
/// builds.
#[test]
fn sentinels_stay_apart_and_unitless_lengths_and_weights_are_handled() {
    let design = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  opacity.a: number = 0.5\n    when density=compact: 0.25\n  opacity.b: number = 0.75\n    when density=compact: 0.6\n  stroke.width = 2px\n  weight.strong = 600\n";
    let plan = plan(
        "apart",
        design,
        "component App\n  view\n    column opacity=\"opacity.a\"\n      column opacity=\"opacity.b\"\n        svg viewBox=\"0 0 24 24\" width=24 height=24\n          path d=\"M0 0L24 24\" stroke=\"#000\" stroke-width=\"stroke.width\"\n      text \"a\" font-weight=\"weight.strong\"\n",
    );
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let css = &out.css;
    assert!(
        css.contains("opacity:var(--x-opacity-a)") && css.contains("opacity:var(--x-opacity-b)"),
        "{css}"
    );
    assert!(!css.contains("70000") && !css.contains("0.0007"), "{css}");
    // A development build: the stroke's length is a property.
    let dev = crate::emit::emit(&plan, false, true).unwrap();
    assert!(
        dev.css.contains("stroke-width:var(--x-stroke-width)"),
        "{}",
        dev.css
    );
    assert!(!dev.css.contains("70000"), "{}", dev.css);
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)
    };
    let weight = design["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "weight.strong")
        .unwrap();
    assert_eq!(weight["inline"], true);
}

/// Review round 2: a bound row's map holds only the kinds the row takes;
/// an SVG attribute reads no property, so its token is inline in a
/// development build and refused when it varies.
#[test]
fn a_rows_map_takes_its_kinds_and_an_svg_attribute_writes_inline() {
    let kinds = plan(
        "kinds",
        DESIGN,
        "component App\n  state w = \"weight.strong\"\n  view\n    column width=w opacity=(w == \"x\" ? \"opacity.faded\" : 1)\n      text \"a\"\n",
    );
    let out = crate::emit::emit(&kinds, false, false).unwrap();
    let width = out.js.split(",\"width\",").nth(1).unwrap_or_default();
    let map = |name: &str| {
        out.js
            .split(&format!("const {name}="))
            .nth(1)
            .and_then(|m| m.split(';').next())
            .unwrap_or_default()
            .to_string()
    };
    assert!(width.starts_with("\"px\",()=>$tl("), "{}", out.js);
    assert!(
        !map("$tl").contains("weight.strong") && map("$tl").contains("space.row"),
        "{}",
        out.js
    );
    assert!(
        map("$tn").contains("weight.strong") && !map("$tn").contains("space.row"),
        "{}",
        out.js
    );
    let tile = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  tile = 8px\n  tile.varying = 8px\n    when density=compact: 6px\n";
    let view = |token: &str| {
        format!("component App\n  view\n    svg viewBox=\"0 0 24 24\" width=24 height=24\n      pattern id=\"p\" width=\"{token}\" height=\"{token}\"\n        path d=\"M0 0L4 4\"\n")
    };
    let constant = plan("tile", tile, &view("tile"));
    let dev = crate::emit::emit(&constant, false, true).unwrap();
    assert!(
        !dev.css.contains("var(--x-tile)") && !dev.js.contains("var(--x-tile)"),
        "{}",
        dev.js
    );
    let varying = plan("tile-varying", tile, &view("tile.varying"));
    let refused = crate::emit::emit(&varying, false, false)
        .err()
        .unwrap_or_default();
    assert!(
        refused.contains("tile.varying") && refused.contains("SVG"),
        "{refused}"
    );
}

/// Review round 3: a varying token's name computed into an SVG attribute
/// stands (no tuple's rule reaches an attribute); a bound opacity's layer
/// test reads a constant token's value, and a varying one may be below 1.
#[test]
fn an_attribute_map_holds_constants_and_opacity_reads_a_tokens_value() {
    let tile = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  tile = 8px\n  tile.varying = 8px\n    when density=compact: 6px\n  dim: number = 1\n  faded: number = 0.5\n    when density=compact: 0.25\n";
    let plan = plan(
        "attribute",
        tile,
        "component App\n  state size = \"tile.varying\"\n  state o = \"dim\"\n  view\n    column opacity=o\n      svg viewBox=\"0 0 24 24\" width=24 height=24\n        pattern id=\"p\" width=size height=size\n          path d=\"M0 0L4 4\"\n",
    );
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let map = |name: &str| {
        out.js
            .split(&format!("const {name}="))
            .nth(1)
            .and_then(|m| m.split(';').next())
            .unwrap_or_default()
            .to_string()
    };
    assert!(
        map("$tla").contains("\"tile\":\"8px\"") && !map("$tla").contains("tile.varying"),
        "{}",
        out.js
    );
    assert!(
        out.js.contains("{\"dim\":1,\"faded\":0}[v.trim()]??1)<1"),
        "{}",
        out.js
    );
}

/// Review round 3: a token's name only compared in a font weight's value
/// is no use of it; the build goes on.
#[test]
fn a_compared_name_is_no_inline_use() {
    let design = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  weight.strong: number = 600\n    when density=compact: 700\n";
    let plan = plan(
        "compared",
        design,
        "component App\n  state w = \"x\"\n  view\n    text \"a\" font-weight=(w == \"weight.strong\" ? 700 : 400)\n",
    );
    assert!(crate::emit::emit(&plan, false, false).is_ok());
}
