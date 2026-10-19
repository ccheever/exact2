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

/// theme2's conditions on `--exact-<name>` (2026-10-19): an app's own base
/// name wins on the JS target, with no base property for it, and follows a
/// live edit: in a development build it is the design's property, which a
/// design edit's batch sets again (`modes::design`), so the page moves.
#[test]
fn an_apps_own_base_name_wins_and_follows_a_live_edit() {
    let compile = |design: &str| {
        let dir =
            std::env::temp_dir().join(format!("exact-web-js-base-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.tamber"), design).unwrap();
        std::fs::write(dir.join("app.contract"), "component App\n  view\n    column padding=\"space.md\" gap=\"space.sm\"\n      text \"a\"\n").unwrap();
        let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        plan
    };
    let before = compile("tamber t\n\ntheme\n  space.md = 40px\n");
    // A development build: the row reads the design's property, never a base one.
    let dev = crate::emit::emit(&before, false, true).unwrap();
    assert!(
        dev.css.contains("padding-top:var(--x-space-md)"),
        "{}",
        dev.css
    );
    assert!(!dev.css.contains("--exact-space-md"), "{}", dev.css);
    // The base name the design leaves unset stays the base's.
    assert!(
        dev.css
            .contains(&format!("--exact-space-sm:{};", kernel("space.sm"))),
        "{}",
        dev.css
    );
    let token = |plan: &exact_plan::Plan| {
        let design = {
            let _every = crate::modes::every(true);
            crate::modes::design(plan)
        };
        design["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "space.md")
            .unwrap()
            .clone()
    };
    let (was, now) = (
        token(&before),
        token(&compile("tamber t\n\ntheme\n  space.md = 48px\n")),
    );
    // The edit's batch sets the same property to the new value: the page moves.
    assert_eq!(
        (&was["prop"], &now["prop"]),
        (
            &serde_json::json!("--x-space-md"),
            &serde_json::json!("--x-space-md")
        ),
        "{was} {now}"
    );
    assert_eq!(was["css"], serde_json::json!(["40px"]), "{was}");
    assert_eq!(now["css"], serde_json::json!(["48px"]), "{now}");
    // A release build: the design's own value, inline.
    let release = crate::emit::emit(&before, false, false).unwrap();
    assert!(
        release.css.contains("padding-top:40px") && !release.css.contains("--exact-space-md"),
        "{}",
        release.css
    );
}
