use super::*;

/// @ref LLP 1095 D4 — a bound colour naming a theme entry or an
/// overridden role writes the entry's CSS, as the literal path does.
#[test]
fn a_bound_theme_name_writes_its_entry() {
    let plan = contract::compile("theme\n  brand = \"#c8102e\"\n  ink = \"brand\"\n  -exact-secondary-label = \"light-dark(#111, #eee)\"\n  AccentColor = \"#00ff00\"\n\ncomponent App\n  state c: string = \"brand\"\n  view\n    column\n      text \"a\" color=c\n      text \"b\" color=\"light-dark(brand, #000)\"\n").unwrap();
    let values = [
        "brand",
        "ink",
        "-exact-secondary-label",
        "-Exact-Secondary-Label",
        "0 1px 2px brand",
        "BRAND",
        "CanvasText",
        "#123456",
        "AccentColor",
        "light-dark(brand, #000)",
        "light-dark(rgb(0 0 0 / calc((1))), brand)",
        "linear-gradient(brand, #000)",
    ];
    assert_eq!(
        run(&color_map(&plan), &values),
        serde_json::json!([
            "#c8102eff",
            "#c8102eff",
            "light-dark(#111111ff, #eeeeeeff)",
            "light-dark(#111111ff, #eeeeeeff)",
            "0 1px 2px #c8102eff",
            "BRAND",
            run(&SYSTEM_COLOR_MAP, &["CanvasText"])[0].clone(),
            "#123456",
            // The tint's role stays itself, under an authored accent too;
            // no name is read inside a colour function but a synthetic
            // token's, which the view wrote (LLP 1121 D3).
            run(&SYSTEM_COLOR_MAP, &["AccentColor"])[0].clone(),
            "light-dark(#c8102eff, #000000ff)",
            "light-dark(rgb(0 0 0 / calc((1))), brand)",
            "linear-gradient(#c8102eff, #000)"
        ])
    );
}

/// LLP 1121 D3 — a design's dotted name and a synthetic token's
/// expression, bound, read their folded colours as the kernel does.
#[test]
fn a_bound_design_name_and_synthetic_token_write_their_values() {
    let dir = std::env::temp_dir().join(format!("exact-web-js-tamber-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  brand = #c8102e\n  surface.card = #000000\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state c: string = \"brand\"\n  view\n    column\n      text \"a\" color=c\n      text \"b\" color=\"light-dark(brand, surface.card)\"\n").unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let values = [
        "surface.card",
        "0 1px 2px surface.card",
        "light-dark(brand, surface.card)",
        "linear-gradient(light-dark(brand, surface.card), #fff)",
        "1.5px",
        "url(\"a/light-dark(brand, surface.card).png\") brand",
        "url(\"a\\\")light-dark(brand, surface.card).png\")",
        "/* \" */ brand /* \" */",
    ];
    assert_eq!(
        run(&color_map(&plan), &values),
        serde_json::json!([
            "#000000ff",
            "0 1px 2px #000000ff",
            "light-dark(#c8102eff, #000000ff)",
            "linear-gradient(light-dark(#c8102eff, #000000ff), #fff)",
            "1.5px",
            "url(\"a/light-dark(brand, surface.card).png\") #c8102eff",
            "url(\"a\\\")light-dark(brand, surface.card).png\")",
            "/* \" */ #c8102eff /* \" */"
        ])
    );
}

#[test]
fn a_bound_accent_role_leaves_the_inherited_accent_unset() {
    let writes = style_writes(StyleId::AccentColor as u16, false).unwrap();
    let values = ["AccentColor", " accentcolor ", "#5856d6", "auto", "inherit"];
    let maps: Vec<_> = writes
        .iter()
        .map(|write| {
            let map = format!(
                "v=>({})(({})(v))",
                write.map.unwrap(),
                SYSTEM_COLOR_MAP.as_str()
            );
            (write.name.as_str(), run(&map, &values))
        })
        .collect();
    assert_eq!(
        maps,
        vec![
            (
                "accent-color",
                serde_json::json!([
                    "var(--exact-accent, AccentColor)",
                    " var(--exact-accent, AccentColor) ",
                    "#5856d6",
                    "auto",
                    "inherit"
                ])
            ),
            (
                "--exact-accent",
                serde_json::json!([null, null, "#5856d6", "AccentColor", "inherit"])
            )
        ]
    );
}

#[test]
fn mapped_clears_and_invalid_values_keep_declarations_but_none_removes_them() {
    for (id, invalid) in [
        (StyleId::Cursor, "not-a-cursor"),
        (StyleId::ClipPath, "circle(20%)"),
        (StyleId::ColorScheme, "orange"),
        (StyleId::CornerShape, "-apple-continuous"),
    ] {
        let writes = style_writes(id as u16, false).unwrap();
        let map = writes[0].map.unwrap();
        let script = format!("globalThis.exact={{journal:[],now:()=>0}};const m={map};console.log(JSON.stringify({{values:[null,'unset',{invalid:?}].map(m),journal:exact.journal}}))");
        let out = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
            .args(["-e", &script])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let observed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            observed["values"],
            serde_json::json!([null, "unset", "unset"]),
            "{id:?}"
        );
        assert_eq!(
            observed["journal"].as_array().unwrap().len(),
            1,
            "{id:?}: {observed}"
        );
    }
}

#[test]
fn grid_refusals_keep_declarations_but_absent_values_remove_them() {
    let cases: Vec<_> = [
        (StyleId::GridTemplateColumns, "40px 1fr", "subgrid [rails]"),
        (StyleId::GridTemplateRows, "40px 1fr", "repeat(10001, 1px)"),
        (StyleId::GridColumn, "1 / span 2", "10001"),
        (StyleId::GridRow, "1 / span 2", "span 10001"),
        (StyleId::GridAutoFlow, "row dense", "inherit"),
        (StyleId::JustifyItems, "center", "left legacy"),
    ]
    .into_iter()
    .map(|(id, valid, refused)| {
        let writes = style_writes(id as u16, false).unwrap();
        (writes[0].map.unwrap(), valid, refused)
    })
    .collect();
    let script = format!(
        r#"import {{ mock }} from 'bun:test';
import {{ strict as assert }} from 'node:assert';
const journal = [];
mock.module('./rt.js', () => ({{ normal: (_, v) => v, say: line => journal.push(line) }}));
const {{ gridValue }} = await import('./grid.js');
for (const [source, valid, refused] of {}) {{
  const map = eval(`(${{source}})`);
  assert.equal(map(null), null);
  assert.equal(map(undefined), undefined);
  assert.equal(map(valid), valid);
  for (const value of [refused, '']) {{
const before = journal.length;
assert.equal(map(value), 'unset', source);
assert.equal(journal.length, before + 1);
assert.ok(journal.at(-1).startsWith('unset '));
assert.ok(journal.at(-1).includes(JSON.stringify(value)));
  }}
}}"#,
        serde_json::to_string(&cases).unwrap()
    );
    let out = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
        .args(["-e", &script])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn balanced_wrap_writes_the_same_property_for_bound_values() {
    let writes = style_writes(StyleId::TextWrapStyle as u16, false).unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].name, "text-wrap-style");
    assert_eq!(writes[0].unit, "");
    assert_eq!(writes[0].map, None);
}

/// LLP 1104 D3: a bound `::placeholder` colour writes css.rs's
/// private property, which index.html's `::placeholder` rule reads.
#[test]
fn a_bound_placeholder_colour_writes_the_private_property() {
    let writes = style_writes(StyleId::PlaceholderColor as u16, false).unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].name, "--exact-placeholder-color");
    let map = writes[0].map.unwrap();
    assert_eq!(
        run(
            &format!("v=>({map})(JSON.parse(v))"),
            &["null", "\"unset\"", "\"#123456\""]
        ),
        serde_json::json!([null, "initial", "#123456"])
    );
}

#[test]
fn a_bound_tint_clear_restores_the_symbol_role() {
    let writes = style_writes(StyleId::TintColor as u16, false).unwrap();
    assert_eq!(writes[0].name, "--exact-tint");
    let map = writes[0].map.unwrap();
    assert_eq!(
        run(
            &format!("v=>({map})(JSON.parse(v))"),
            &["null", "\"unset\"", "\"#123456\""]
        ),
        serde_json::json!([
            null,
            "var(--exact-symbol, var(--exact-accent, AccentColor))",
            "#123456"
        ])
    );
}

/// The role's CSS, as the literal path writes it.
pub(crate) fn role(name: &str) -> String {
    let id = exact_kernel::COLOR_ROLES
        .iter()
        .position(|r| r.name == name)
        .unwrap();
    let mut css = String::new();
    exact_kernel::gradient::color_css(&mut css, exact_kernel::ColorValue::Role(id as u8));
    css
}

/// `SYSTEM_COLOR_MAP` run by Bun over each value.
fn mapped(values: &[&str]) -> Vec<String> {
    serde_json::from_value(run(SYSTEM_COLOR_MAP.as_str(), values)).unwrap()
}

/// `map` run by Bun over each value.
pub(crate) fn run(map: &str, values: &[&str]) -> serde_json::Value {
    let script = format!(
        "console.log(JSON.stringify({}.map({map})))",
        serde_json::to_string(values).unwrap(),
    );
    let out = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
        .args(["-e", &script])
        .output()
        .expect("bun runs the map (the repo pins it)");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

/// LLP 1115 D3: a bound text style is written as css.rs writes a
/// literal one; a number is left for the row's `px`.
#[test]
fn a_bound_text_style_is_its_rem() {
    let out = run(
        TEXT_STYLE_MAP.as_str(),
        &[
            "-exact-title1",
            " -Apple-System-Headline ",
            "1.5rem",
            "-exact-nope",
        ],
    );
    assert_eq!(
        out,
        serde_json::json!(["1.6875rem", "1rem", "1.5rem", "-exact-nope"])
    );
}

#[test]
fn a_computed_transition_is_checked_as_an_authors() {
    // LLP 1081 D5: the new tint becomes the property the browser
    // animates; the old spelling, the host's own name (behind a comment
    // too) and any old name are dropped, as the native parser refuses
    // them; springs are the engine's.
    let values = [
        "-exact-tint-color 1s",
        "tint-color 1s, opacity 1s",
        "/**/--exact-tint 1s",
        "opacity 1s -exact-spring(1, 2, 3), scale 1s",
        "press-scale 1s",
        "animation-trigger 1s, --exact-π 1s",
    ];
    assert_eq!(
        run(TRANSITION_MAP.as_str(), &values),
        serde_json::json!([
            "--exact-tint 1s",
            "opacity 1s",
            "none",
            "scale 1s",
            "none",
            "none"
        ])
    );
}

/// LLP 1034 §8: a bound `color-scheme` writes `light` or `dark`, and a
/// bound `normal` (or an invalid value) retains an unset declaration.
#[test]
fn a_bound_color_scheme_is_light_dark_or_unset() {
    let writes = style_writes(StyleId::ColorScheme as u16, false).unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].name, "color-scheme");
    assert_eq!(
        run(
            writes[0].map.unwrap(),
            &["light", "dark", "normal", "light dark"]
        ),
        serde_json::json!(["light", "dark", "unset", "unset"])
    );
}

#[test]
fn a_bound_role_is_its_css_whole_or_inside_a_composite_value() {
    let (orange, label, fill) = (role("system-orange"), role("secondary-label"), role("fill"));
    assert_eq!(
        mapped(&[
            " -exact-secondary-label ",
            "-Exact-System-Orange",
            "linear-gradient(-exact-system-orange, #fff)",
            "radial-gradient(circle,-exact-fill 0%,-exact-system-orange 100%)",
            "0 1px 2px -exact-system-orange, inset 0 0 4px -apple-system-secondary-label",
            "1px -apple-system-orange",
        ]),
        [
            format!(" {label} "),
            orange.clone(),
            format!("linear-gradient({orange}, #fff)"),
            format!("radial-gradient(circle,{fill} 0%,{orange} 100%)"),
            format!("0 1px 2px {orange}, inset 0 0 4px {label}"),
            format!("1px {orange}"),
        ]
    );
}

#[test]
fn a_role_name_inside_another_token_stays_as_written() {
    let values = [
        "context-fill",
        "var(--exact-label,#000)",
        "url(label.png)",
        "url(\"fill (1).png\"), linear-gradient(red, blue)",
        "image-set(\"background.png\" 1x)",
        "url(#fill)",
        "#label",
        "fill-rule",
        "labels",
        "background2",
        "fill(1)",
        "/* label */ red",
        // An Exact role's bare name is no colour (LLP 1081 D2).
        "system-orange",
        "linear-gradient(secondary-label, fill)",
    ];
    assert_eq!(mapped(&values), values);
}

#[test]
fn a_bound_platform_color_is_admitted_only_as_a_plan_literal() {
    let literal = "-exact-platform-color(ios webJsTestColor, #010203)";
    let gradient =
        "linear-gradient(-exact-platform-color(ios webJsTestColor, #010203), -exact-system-orange)";
    let mut plan = exact_plan::Plan::default();
    plan.strings.extend([literal.into(), gradient.into()]);
    let mut css = String::new();
    exact_kernel::gradient::color_css(
        &mut css,
        exact_kernel::ColorValue::parse_light_dark(literal).unwrap(),
    );
    assert!(!css.contains("platform-color"), "{css}");
    let map = color_map(&plan);
    assert_eq!(
        run(
            &map,
            &[
                literal,
                gradient,
                "-exact-platform-color(ios webJsOtherColor, #010203)",
                "linear-gradient(-exact-platform-color(ios webJsTestColor, #010203), #fff)",
                " -exact-platform-color(ios webJsTestColor, #010203)",
                "-exact-secondary-label"
            ]
        ),
        serde_json::json!([
            css,
            format!("linear-gradient({css}, {})", role("system-orange")),
            "unset",
            "unset",
            "unset",
            role("secondary-label"),
        ])
    );
    // A plan without one writes the plain map.
    assert_eq!(color_map(&exact_plan::Plan::default()), *SYSTEM_COLOR_MAP);
}
