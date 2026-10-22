//! The write check judges a written shorthand or `flex` row as lowering
//! does (theme2's ruling on the shorthand plan): expanded by the build's own
//! `shorthands::component` or `values::flex_component`, each longhand judged
//! in row order, so the first refusal said is the build's first. A computed
//! one is the build's.
#![cfg(unix)]

use super::tamber_package_params::reading;
use super::tamber_rows_check::{same_words, transaction};
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
        ("rows-short-choice", "padding=(on ? \"gap 4px\" : \"2px\")"),
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

/// A computed `flex` is the build's: not refused here.
#[test]
fn a_computed_shorthand_is_left_to_the_build() {
    let dir = reading("rows-short-computed");
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  state basis = \"gap\"\n  view\n    column margin=`${basis} 4px`\n      Card()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
    );
    let refused: Vec<&serde_json::Value> = w["error"]["diagnostics"]
        .as_array()
        .map(|d| d.iter().filter(|d| d["file"] == "app.contract").collect())
        .unwrap_or_default();
    assert!(refused.is_empty(), "{w}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}
