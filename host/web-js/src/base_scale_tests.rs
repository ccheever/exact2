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
fn a_static_row_is_the_columns_length_and_the_sheet_declares_the_scale_once() {
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
    // Declared once, every base name (theme2's all-nine-or-none, round 2).
    let root: String = scale::names()
        .map(|n| format!("{}:{};", crate::base_scale::property(n), kernel(n)))
        .collect();
    assert_eq!(css.matches(&format!(":root{{{root}}}")).count(), 1, "{css}");
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

/// theme2's all-nine-or-none (B2 round 2): the sheet declares every base
/// name when the plan may read any as a length, and none when it can't. A
/// literal row, an alias, a computed name, a string read: the nine, and the
/// map holds each. A text or a label that says `space.md`, a condition over
/// px literals, a number read, a length built with a unit: none, so a release
/// that uses no base name is unchanged by the scale.
#[test]
fn the_scale_is_declared_whole_when_the_plan_may_read_a_base_name() {
    let all: Vec<&str> = scale::names().collect();
    let none: Vec<&str> = Vec::new();
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
    let app = |state: &str, body: &str| format!("component App\n{state}  view\n    column\n{body}");
    // (a) A literal base-name row, and one condition's literal leaf.
    let (names, out) = declared(&app(
        "",
        "      column gap=\"space.lg\"\n        text \"a\"\n",
    ));
    assert_eq!(names, all, "{}", out.css);
    let (names, _) = declared(&app(
        "  state wide = false\n",
        "      column padding=(wide ? \"space.xl\" : \"8px\")\n        text \"a\"\n",
    ));
    assert_eq!(names, all);
    // (d) A computed name, normalised as the page's lookup trims it: the
    // nine, each in the map.
    for body in [
        "      column padding=(\"space.\" + size)\n        text \"a\"\n",
        "      column padding=(\"space.\" + size + \" \")\n        text \"a\"\n",
        "      column padding=(\" \" + gap)\n        text \"a\"\n",
        "      column padding=(gap + \" \")\n        text \"a\"\n",
        "      column padding=gap\n        text \"a\"\n",
        // Trimmed as JavaScript trims (round 2's re-review): U+FEFF, which
        // Rust's `trim` keeps, and an ideographic space.
        "      column padding=(\"\u{feff}\" + gap)\n        text \"a\"\n",
        "      column padding=(gap + \"\u{feff}\")\n        text \"a\"\n",
        "      column padding=(\"\u{3000}\" + gap + \"\u{3000}\")\n        text \"a\"\n",
    ] {
        let (names, out) = declared(&app(
            "  state size = \"md\"\n  state gap = \"12px\"\n",
            body,
        ));
        assert_eq!(names, all, "{body}");
        for n in &all {
            let entry = format!("\"{n}\":\"var({})\"", crate::base_scale::property(n));
            assert!(out.js.contains(&entry), "{body} {n}: {}", out.js);
        }
    }
    // (b) Plain text, a label, a px condition, a number state, a unit-built
    // length, a keyword row bound to a string: none, and no map entry.
    let (names, out) = declared(&app(
        "  state wide = false\n  state w = 12\n  state style = \"italic\"\n",
        "      column padding=(wide ? \"24px\" : \"12px\") aria-label=\"space.md\" gap=w font-style=style\n        text \"space.md\" width=(w.toString() + \"px\")\n        text (wide ? \"radius.lg\" : \"b\")\n",
    ));
    assert_eq!(names, none, "{}", out.css);
    assert!(
        !out.js.contains("var(--exact-space") && !out.js.contains("var(--exact-radius"),
        "{}",
        out.js
    );
    // Round 2, Astra 5: a region's item or bound value typed as built: a
    // number list's or option's is none, a string one's may be any.
    for (body, nine) in [
        ("      for n in [12, 16] key=n\n        column padding=n\n          text \"a\"\n", false),
        ("      for n in [\"space.md\"] key=n\n        column padding=n\n          text \"a\"\n", true),
        ("      match some(w)\n        case some(n)\n          column padding=n\n            text \"a\"\n        case none\n          text \"b\"\n", false),
        ("      match some(gap)\n        case some(n)\n          column padding=n\n            text \"a\"\n        case none\n          text \"b\"\n", true),
        ("      for s in sizes key=s\n        column padding=s\n          text \"a\"\n", false),
        // `none` has no payload: a conditional option is its `Some`'s.
        ("      match (wide ? some(12) : none)\n        case some(n)\n          column padding=n\n            text \"a\"\n        case none\n          text \"b\"\n", false),
        ("      match (wide ? some(gap) : none)\n        case some(n)\n          column padding=n\n            text \"a\"\n        case none\n          text \"b\"\n", true),
    ] {
        let (names, out) = declared(&app("  state w = 12\n  state wide = false\n  state gap = \"12px\"\n  state sizes = [12, 16]\n", body));
        assert_eq!(names, if nine { all.clone() } else { none.clone() }, "{body}: {}", out.css);
    }
}

/// Round 2, Astra 1: where the plan declares the scale, a keyword row's map
/// (which holds no base name, `base_scale::reads_length`) is not a length
/// row's, whichever is bound first.
#[test]
fn a_keyword_rows_map_is_not_a_length_rows() {
    let dir = std::env::temp_dir().join(format!("exact-web-js-base-key-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), "tamber t\n\ntheme\n  tile = 8px\n").unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state scheme = \"light\"\n  state pad = \"space.md\"\n  view\n    column color-scheme=scheme padding=\"tile\"\n      column padding=pad\n        text \"a\"\n").unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
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
        map("$tl").contains("\"space.md\":\"var(--exact-space-md)\""),
        "{}",
        out.js
    );
    assert!(
        map("$tk").contains("\"tile\"") && !map("$tk").contains("space.md"),
        "{}",
        out.js
    );
}

/// Round 2, Astra 3: a bound SVG attribute's map holds each token of one
/// value it may name as its CSS (an attribute reads no property), an alias as
/// its length, so each is written inline: a development build's edit of it
/// builds, never a batch that leaves the attribute behind.
#[test]
fn a_token_a_bound_svg_attribute_may_name_is_written_inline() {
    let dir = std::env::temp_dir().join(format!(
        "exact-web-js-base-svg-inline-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  card.pad: length = space.md\n  tile = 8px\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state size = \"card.pad\"\n  view\n    column padding=\"tile\"\n      svg viewBox=\"0 0 24 24\" width=24 height=24\n        pattern id=\"p\" width=size height=\"4\"\n          path d=\"M0 0L4 4\"\n").unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
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
    assert_eq!(
        row("card.pad")["inline"],
        serde_json::json!(true),
        "{}",
        row("card.pad")
    );
    // Widened (the lead): a plain constant the attribute may name too.
    assert_eq!(
        row("tile")["inline"],
        serde_json::json!(true),
        "{}",
        row("tile")
    );
}

/// (c) A live switch between two base names keeps the structure: the nine
/// are declared either way; only none to nine moves it.
#[test]
fn a_switch_between_base_names_keeps_the_structure() {
    let structure = |design: &str| {
        let dir = std::env::temp_dir().join(format!(
            "exact-web-js-base-structure-{}",
            std::process::id()
        ));
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
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)["structure"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let md = structure("tamber t\n\ntheme\n  card.pad: length = space.md\n");
    assert_eq!(
        md,
        structure("tamber t\n\ntheme\n  card.pad: length = space.lg\n")
    );
    assert_ne!(
        md,
        structure("tamber t\n\ntheme\n  card.pad: length = 12px\n")
    );
}

/// Round 3 (Astra): grid.js's track check admits a base name's property,
/// exactly the kernel's nine, written there by hand: held to the table.
#[test]
fn the_grid_track_check_names_the_scale() {
    let grid = include_str!("../grid.js");
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for name in scale::names() {
        let (head, step) = name.split_once('.').unwrap();
        match groups.iter_mut().find(|(h, _)| *h == head) {
            Some((_, steps)) => steps.push(step),
            None => groups.push((head, vec![step])),
        }
    }
    let alternatives: Vec<String> = groups
        .iter()
        .map(|(h, s)| format!("{h}-(?:{})", s.join("|")))
        .collect();
    let pattern = format!("--exact-(?:{})", alternatives.join("|"));
    assert!(grid.contains(&pattern), "grid.js should admit {pattern}");
}

/// Stage 12 B2: a length token over base names the design leaves unset is a
/// `calc()` over their properties, which the browser folds, so the sheet
/// declares the scale although only the expression names it; a release's
/// static row is the build's column's px, as the kernel folds it; an SVG
/// attribute, which reads no property, that px too.
#[test]
fn an_expression_over_base_names_is_a_calc_over_their_properties() {
    use exact_kernel::style::theme::Kind;
    let expr = "calc(space.md * 2 + 4px)";
    assert_eq!(
        crate::measures::css(Kind::Length, expr),
        "calc(var(--exact-space-md) * 2 + 4px)"
    );
    assert_eq!(
        crate::measures::css(Kind::Length, "min(space.lg, space.sm * 3)"),
        "min(var(--exact-space-lg), var(--exact-space-sm) * 3)"
    );
    let px = format!("{}px", scale::evaluate(expr, metrics::column()).unwrap());
    assert_eq!(crate::base_scale::folded(expr), Some(px.clone()));
    assert_eq!(crate::base_scale::folded("12px"), None);
    let dir = std::env::temp_dir().join(format!("exact-web-js-expr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  card.pad: length = {expr}\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  state size = \"card.pad\"\n  view\n    column padding=\"card.pad\"\n      svg viewBox=\"0 0 24 24\" width=24 height=24\n        pattern id=\"p\" width=size height=\"card.pad\"\n          path d=\"M0 0L4 4\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    for dev in [false, true] {
        let out = crate::emit::emit(&plan, false, dev).unwrap();
        // The nine, declared once.
        assert!(
            out.css
                .contains(&format!("--exact-space-md:{};", kernel("space.md"))),
            "dev {dev}: {}",
            out.css
        );
        assert!(!out.css.contains("space.md *"), "dev {dev}: {}", out.css);
        // The SVG attribute: the build's column's px, bound and static.
        let map = out
            .js
            .split("const $tla=")
            .nth(1)
            .and_then(|m| m.split(';').next())
            .unwrap_or_default();
        assert!(
            map.contains(&format!("\"card.pad\":\"{px}\"")),
            "dev {dev}: {map}"
        );
        assert!(
            out.js
                .contains(&format!("\"height\":\"{}\"", px.trim_end_matches("px"))),
            "dev {dev}: {}",
            out.js
        );
    }
    // A release: the static row is the column's px.
    let release = crate::emit::emit(&plan, false, false).unwrap();
    assert!(
        release.css.contains(&format!("padding-top:{px}")),
        "{}",
        release.css
    );
    // A development build: the token's property is the calc() over the
    // base's, so a live edit of a base moves it (a token no SVG attribute
    // names, which is written inline).
    let dir = std::env::temp_dir().join(format!("exact-web-js-expr-dev-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  card.pad: length = {expr}\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column padding=\"card.pad\"\n      text \"a\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)
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
        serde_json::json!(["calc(var(--exact-space-md) * 2 + 4px)"]),
        "{token}"
    );
}

/// Stage 12 B3: a shadow token whose lengths are over base names is the
/// row's CSS with each such length its property or a `calc()` over them,
/// which the browser folds, so the sheet declares the scale although only
/// the shadow names it; a release's static row is the build's column's
/// px, as the kernel folds it.
#[test]
fn a_shadow_over_base_names_writes_their_properties() {
    use exact_kernel::style::theme::Kind;
    let text = "inset 0px space.md calc(space.sm * 2) #000000ff, 0px 1px 2px #000000ff";
    let css = crate::measures::css(Kind::BoxShadow, text);
    assert!(
        css.starts_with("inset 0px var(--exact-space-md) calc(var(--exact-space-sm) * 2) 0px ")
            && css.contains(", 0px 1px 2px 0px "),
        "{css}"
    );
    assert!(
        crate::measures::css(Kind::TextShadow, "0px space.xs space.sm #ff0000ff")
            .starts_with("0px var(--exact-space-xs) var(--exact-space-sm) "),
    );
    // A colour whose words name a base (a platform colour's asset) is a
    // colour, kept whole, and no length is paired with it (round 3): the
    // row's CSS, as with no base at all.
    for (kind, text) in [
        (
            Kind::BoxShadow,
            "0px 1px 2px -exact-platform-color(ios named:space.md, #000000)",
        ),
        (
            Kind::TextShadow,
            "0px 1px -exact-platform-color(ios named:space.md, #000000)",
        ),
    ] {
        let css = crate::measures::css(kind, text);
        let plain = match kind {
            Kind::BoxShadow => exact_kernel::style::BoxShadows::parse(text).map(|s| s.css()),
            _ => exact_kernel::style::TextShadow::parse(text).map(|s| s.css()),
        };
        assert!(!css.contains("var("), "{kind:?}: {css}");
        assert_eq!(Some(css), plain, "{kind:?}");
    }
    // Over no base name: the row's CSS, as before.
    assert_eq!(
        crate::measures::css(Kind::BoxShadow, "0px 1px 2px #000000ff"),
        exact_kernel::style::BoxShadows::parse("0px 1px 2px #000000ff")
            .unwrap()
            .css()
    );
    let dir = std::env::temp_dir().join(format!("exact-web-js-shadow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  card.shadow: box-shadow = 0 space.md calc(space.sm * 2) #000\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column box-shadow=\"card.shadow\"\n      text \"a\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let column = metrics::column();
    let px = |expr: &str| format!("{}px", scale::evaluate(expr, column).unwrap());
    // A release: the static row is the column's px, the nine declared.
    let release = crate::emit::emit(&plan, false, false).unwrap();
    assert!(
        release.css.contains(&format!(
            "box-shadow:0px {} {} 0px",
            px("calc(space.md)"),
            px("calc(space.sm * 2)")
        )),
        "{}",
        release.css
    );
    assert!(
        release
            .css
            .contains(&format!("--exact-space-md:{};", kernel("space.md"))),
        "{}",
        release.css
    );
    // A development build: the token's property is over the bases'.
    let design = {
        let _every = crate::modes::every(true);
        crate::modes::design(&plan)
    };
    let token = design["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "card.shadow")
        .unwrap()
        .clone();
    let css = token["css"][0].as_str().unwrap_or_default();
    assert!(
        css.starts_with("0px var(--exact-space-md) calc(var(--exact-space-sm) * 2) 0px "),
        "{token}"
    );
}
