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

/// An alias to a base name the design leaves unset (B1 round 1,
/// `card.pad: length = space.md`): the token's CSS is the base's property,
/// which the sheet declares at the build's column although only the alias
/// names it. When the design sets the base name the alias folds, so the page
/// shows the app's value and no base property.
#[test]
fn an_alias_to_an_unset_base_reads_its_property_and_one_the_app_sets_folds() {
    let compile = |design: &str, tag: &str| {
        let dir =
            std::env::temp_dir().join(format!("exact-web-js-alias-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.tamber"), design).unwrap();
        std::fs::write(
            dir.join("app.contract"),
            "component App\n  view\n    column padding=\"card.pad\"\n      text \"a\"\n",
        )
        .unwrap();
        let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        plan
    };
    let unset = compile(
        "tamber t\n\ntheme\n  card.pad: length = space.md\n",
        "unset",
    );
    assert_eq!(
        crate::measures::css(exact_kernel::style::theme::Kind::Length, "space.md"),
        "var(--exact-space-md)"
    );
    for dev in [false, true] {
        let out = crate::emit::emit(&unset, false, dev).unwrap();
        assert!(
            out.css
                .contains(&format!("--exact-space-md:{};", kernel("space.md"))),
            "dev {dev}: {}",
            out.css
        );
        assert!(!out.css.contains("padding-top:space.md"), "{}", out.css);
    }
    // A development build: the alias is a property whose value is the base's.
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&unset)
    };
    let token = design["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "card.pad")
        .unwrap()
        .clone();
    assert_eq!(
        token["css"],
        serde_json::json!(["var(--exact-space-md)"]),
        "{token}"
    );
    let dev = crate::emit::emit(&unset, false, true).unwrap();
    assert!(
        dev.css.contains("--x-card-pad:var(--exact-space-md)")
            || dev.css.contains("padding-top:var(--x-card-pad)"),
        "{}",
        dev.css
    );
    // The app sets `space.md`: the alias folds, its value shows, no base property.
    let own = compile(
        "tamber t\n\ntheme\n  space.md = 40px\n  card.pad: length = space.md\n",
        "own",
    );
    let out = crate::emit::emit(&own, false, false).unwrap();
    assert!(
        out.css.contains("padding-top:40px") && !out.css.contains("--exact-space-md"),
        "{}",
        out.css
    );
}

/// B2 round 1 (Astra 1): an SVG attribute reads no CSS property, so an alias
/// to an unset base name in one is the base's length in this build's column,
/// in a release and a development build, bound or static.
#[test]
fn an_alias_in_an_svg_attribute_is_the_bases_length() {
    let dir = std::env::temp_dir().join(format!("exact-web-js-alias-svg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  card.pad: length = space.md\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state size = \"card.pad\"\n  view\n    svg viewBox=\"0 0 24 24\" width=24 height=24\n      pattern id=\"p\" width=size height=\"card.pad\"\n        path d=\"M0 0L4 4\"\n").unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let length = kernel("space.md");
    for dev in [false, true] {
        let out = crate::emit::emit(&plan, false, dev).unwrap();
        let map = out
            .js
            .split("const $tla=")
            .nth(1)
            .and_then(|m| m.split(';').next())
            .unwrap_or_default();
        assert!(
            map.contains(&format!("\"card.pad\":\"{length}\"")),
            "dev {dev}: {map}"
        );
        assert!(!map.contains("var("), "dev {dev}: {map}");
        // The static attribute: the kernel's length, unitless, as SVG writes it.
        let px = length.trim_end_matches("px");
        assert!(
            out.js.contains(&format!("\"height\":\"{px}\"")),
            "dev {dev}: {}",
            out.js
        );
    }
}

/// theme2's (B) for B2 round 1 (Astra 3): the declared base names are the
/// plan's length reads, never its string pool. A computed name declares all
/// nine (and the map holds each, as the kernel reads any); a text that says
/// `space.md` declares none; a literal length row exactly its own.
#[test]
fn the_declared_bases_are_the_plans_length_reads() {
    let all: Vec<&str> = scale::names().collect();
    let declared = |view: &str| {
        let plan = contract::compile(view).unwrap();
        let out = crate::emit::emit(&plan, false, false).unwrap();
        let names: Vec<&str> = all
            .iter()
            .copied()
            .filter(|n| {
                out.css
                    .contains(&format!("{}:", crate::base_scale::property(n)))
            })
            .collect();
        (names, out)
    };
    let (names, out) = declared("component App\n  state size = \"md\"\n  view\n    column padding=(\"space.\" + size)\n      text \"a\"\n");
    assert_eq!(names, all, "{}", out.css);
    for n in &all {
        assert!(
            out.js.contains(&format!(
                "\"{n}\":\"var({})\"",
                crate::base_scale::property(n)
            )),
            "{n}: {}",
            out.js
        );
    }
    // A string state bound to a length row may be any base an action sets.
    let (names, _) = declared("component App\n  state size = \"12px\"\n  view\n    column padding=size\n      text \"a\"\n");
    assert_eq!(names, all);
    // Plain text, a label, a literal length that is no base, a condition
    // over literals, a number state, a length built with a unit: no base
    // read, nothing declared or mapped (a release unchanged by the scale).
    let (names, out) = declared("component App\n  state wide = false\n  state w = 12\n  view\n    column padding=(wide ? \"24px\" : \"12px\") aria-label=\"space.md\" gap=w\n      text \"space.md\" width=(w.toString() + \"px\")\n      text (wide ? \"radius.lg\" : \"b\")\n");
    assert_eq!(names, Vec::<&str>::new(), "{}", out.css);
    assert!(
        !out.js.contains("var(--exact-space") && !out.js.contains("var(--exact-radius"),
        "{}",
        out.js
    );
    // A literal length row, and one condition's literal leaves, exactly.
    let (names, _) =
        declared("component App\n  view\n    column gap=\"space.lg\"\n      text \"space.md\"\n");
    assert_eq!(names, vec!["space.lg"]);
    let (names, _) = declared("component App\n  state wide = false\n  view\n    column padding=(wide ? \"space.xl\" : \"8px\")\n      text \"a\"\n");
    assert_eq!(names, vec!["space.xl"]);
}
