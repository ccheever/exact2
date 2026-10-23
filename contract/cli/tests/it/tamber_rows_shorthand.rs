//! The write check judges a written shorthand or `flex` row as lowering
//! does (theme2's ruling on the shorthand plan): expanded by the build's own
//! `shorthands::component` or `values::flex_component`, each longhand judged
//! in row order, so the first refusal said is the build's first. A computed
//! one is the build's.
#![cfg(unix)]

use super::tamber_package_params::reading;
use super::tamber_rows_check::same_words;
use super::tamber_serve::Serve;
use serde_json::json;

const DESIGN: &str = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n  mute = #777777\n";

fn kind(dir: &std::path::Path, contract: &str) {
    same_words(
        dir,
        DESIGN,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &DESIGN.replace("gap = 8px", "gap = #444444"),
    );
}

/// A `flex` row before a `padding` row naming the same token: the build
/// refuses the `flex` row first (its basis), and so does the check.
#[test]
fn a_flex_row_the_build_refuses_first_is_refused_first() {
    kind(
        &reading("rows-short-flex"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column flex=\"1 1 gap\" padding=\"gap\"\n      Card()\n",
    );
}

/// A box shorthand, `columns`, a border's width and a choice of shorthand
/// literals, each refused as the build refuses its longhand.
#[test]
fn a_shorthands_longhand_is_refused_as_the_build_refuses_it() {
    for (name, row) in [
        ("rows-short-margin", "margin=\"gap 4px\""),
        ("rows-short-choice", "flex=(on ? \"1 1 gap\" : \"1 1 4px\")"),
    ] {
        kind(
            &reading(name),
            &format!("use Card from \"@acme/reading\"\ncomponent App\n  state on = true\n  view\n    column {row}\n      Card()\n"),
        );
    }
    kind(
        &reading("rows-short-columns"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    view columns=\"gap 2\"\n      Card()\n",
    );
    same_words(
        &reading("rows-short-border"),
        DESIGN,
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column border=\"1px solid ink\"\n      Card()\n",
        json!({"op": "set-token", "name": "ink", "value": "4px"}),
        &DESIGN.replace("ink = #111111", "ink = 4px"),
    );
}

/// A style's `flex` row, judged where an element applies it.
#[test]
fn a_styles_flex_row_is_judged_where_it_is_applied() {
    kind(
        &reading("rows-short-style"),
        "use Card from \"@acme/reading\"\nstyle Grow\n  flex=\"1 1 gap\"\ncomponent App\n  view\n    column class=Grow\n      Card()\n",
    );
}

/// A choice of shorthand literals is judged whole, as the build expands and
/// judges it: a write breaking both arms in other words is refused in the
/// build's first (Astra's round 1), as `contract build` reports it.
#[test]
fn a_choice_of_shorthands_is_refused_with_the_builds_first_words() {
    let dir = reading("rows-short-both");
    let design = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n  bad = 8px\n";
    let contract = "use Card from \"@acme/reading\"\ncomponent App\n  state on = true\n  view\n    column flex=(on ? \"1 1 gap\" : \"1 1 bad\")\n      Card()\n";
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = s
        .ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&dir.join("app.tamber"))}, "ops": [
            {"op": "set-token", "name": "gap", "value": "#444444"},
            {"op": "set-token", "name": "bad", "value": "2"},
        ]}))
        .0;
    drop(s);
    let d = &w["error"]["diagnostics"][0];
    assert!(d.is_object(), "refused: {w}");
    std::fs::write(
        dir.join("app.tamber"),
        design
            .replace("gap = 8px", "gap = #444444")
            .replace("bad = 8px", "bad = 2"),
    )
    .unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(
        (
            d["message"].as_str().unwrap(),
            d["line"].as_u64().unwrap(),
            d["col"].as_u64().unwrap()
        ),
        (
            built.message.as_str(),
            built.span.line as u64,
            built.span.col as u64
        ),
        "{w}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A choice with a `none` arm, which lowering expands as a clear (Grok's
/// round 3), and a token an expansion writes itself (`flex="none"`'s basis
/// `auto`, Astra's round 3): refused in the build's first words.
#[test]
fn a_none_arm_and_an_expansions_own_text_are_judged() {
    kind(
        &reading("rows-short-none"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state on = true\n  view\n    column flex=(on ? \"1 1 gap\" : none) padding=\"gap\"\n      Card()\n",
    );
    let design = "tamber t\n\ntheme\n  ink = #111111\n  auto = 8px\n";
    same_words(
        &reading("rows-short-auto"),
        design,
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column flex=\"none\" width=\"auto\"\n      Card()\n",
        json!({"op": "set-token", "name": "auto", "value": "2"}),
        &design.replace("auto = 8px", "auto = 2"),
    );
}
