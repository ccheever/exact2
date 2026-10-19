//! @ref LLP 1121 stage 11 B2 — the base scale on the JS target: a static row
//! is the build's column's length, the sheet declares each named base name
//! once as `--exact-<name>` from the kernel's table, a bound row's value
//! reads it, and a design's own definition stays its token.

use exact_kernel::style::{metrics, scale};

/// A base name's CSS in this build's column, from the kernel's table.
fn kernel(name: &str) -> String {
    scale::css(scale::of(name, metrics::column()).unwrap())
}

#[test]
fn a_static_row_is_the_columns_length_and_the_sheet_declares_each_named_base_once() {
    let plan = contract::compile("component App\n  view\n    column padding=\"space.md\" border-radius=\"radius.lg\" gap=\"space.sm\"\n      text \"a\" border-radius=\"radius.full\"\n").unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    let css = &out.css;
    assert!(
        css.contains(&format!("padding-top:{}", kernel("space.md"))),
        "{css}"
    );
    assert!(
        css.contains(&format!("border-top-left-radius:{}", kernel("radius.lg"))),
        "{css}"
    );
    assert!(css.contains("border-top-left-radius:9999px"), "{css}");
    // Declared once, the names the plan names alone.
    let root = format!(":root{{--exact-space-sm:{};--exact-space-md:{};--exact-radius-lg:{};--exact-radius-full:{};}}", kernel("space.sm"), kernel("space.md"), kernel("radius.lg"), kernel("radius.full"));
    assert_eq!(css.matches(&root).count(), 1, "{css}");
    assert!(
        !css.contains("--exact-space-xs") && !css.contains("--exact-radius-sm"),
        "{css}"
    );
}

#[test]
fn a_bound_rows_value_naming_a_base_reads_its_property() {
    let plan = contract::compile("component App\n  state wide = false\n  view\n    column padding=(wide ? \"space.lg\" : \"space.md\")\n      text \"a\"\n").unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    assert!(
        out.js.contains("\"space.lg\":\"var(--exact-space-lg)\""),
        "{}",
        out.js
    );
    assert!(
        out.js.contains("\"space.md\":\"var(--exact-space-md)\""),
        "{}",
        out.js
    );
    assert!(
        out.css
            .contains(&format!("--exact-space-lg:{};", kernel("space.lg"))),
        "{}",
        out.css
    );
}

#[test]
fn no_base_named_no_declarations_and_a_designs_own_is_its_token() {
    let plan =
        contract::compile("component App\n  view\n    column padding=\"12px\"\n      text \"a\"\n")
            .unwrap();
    let out = crate::emit::emit(&plan, false, false).unwrap();
    assert!(!out.css.contains("--exact-space"), "{}", out.css);
    // A design's `space.md` is its own token (the token path writes it).
    let dir = std::env::temp_dir().join(format!("exact-web-js-base-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  space.md = 40px\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column padding=\"space.md\" gap=\"space.sm\"\n      text \"a\"\n").unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let out = crate::emit::emit(&plan, false, false).unwrap();
    assert!(out.css.contains("padding-top:40px"), "{}", out.css);
    assert!(!out.css.contains("--exact-space-md"), "{}", out.css);
    assert!(
        out.css
            .contains(&format!("--exact-space-sm:{};", kernel("space.sm"))),
        "{}",
        out.css
    );
}
