//! The write check judges a style's rows where an element applies it, as the
//! build lowers them there (`class::rows`; theme2's 02:08 ruling): at each
//! element, with its fonts and typing, keyed by the element, refused once
//! for the style row in the build's words and place. A style no element
//! applies, a choice of styles, and a row the element overrides are the
//! build's.
#![cfg(unix)]

use super::tamber_package_params::reading;
use super::tamber_rows_check::{same_words, transaction};
use super::tamber_serve::Serve;
use serde_json::{json, Value};
use std::path::Path;

const DESIGN: &str = "tamber t\n\ntheme\n  ink = #111111\n  gap = 8px\n  mute = #777777\n";

/// The server's refusals of `op` on the app `contract` under `design`, as
/// (line, col, message); `None` when the write is accepted.
fn refusals(
    dir: &Path,
    design: &str,
    contract: &str,
    op: Value,
) -> Option<Vec<(u64, u64, String)>> {
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let mut s = Serve::start(dir, &[]);
    let (first, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    assert!(first.get("ok").is_some(), "{first}");
    let w = transaction(&mut s, dir, op);
    drop(s);
    if w.get("ok").is_some() {
        return None;
    }
    Some(
        w["error"]["diagnostics"]
            .as_array()
            .unwrap_or_else(|| panic!("{w}"))
            .iter()
            .filter(|d| d["file"] == "app.contract")
            .map(|d| {
                (
                    d["line"].as_u64().unwrap(),
                    d["col"].as_u64().unwrap(),
                    d["message"].as_str().unwrap().to_string(),
                )
            })
            .collect(),
    )
}

/// A style's row a write breaks is refused at the style row, in the build's
/// words and place: `contract build` of the written design says the same.
#[test]
fn a_used_styles_row_is_refused_where_the_build_refuses_it() {
    let contract = "use Card from \"@acme/reading\"\nstyle Pad\n  padding=\"gap\"\ncomponent App\n  view\n    column class=Pad\n      Card()\n";
    same_words(
        &reading("rows-style"),
        DESIGN,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &DESIGN.replace("gap = 8px", "gap = #444444"),
    );
}

/// A style row two elements apply and the write breaks at both is one
/// refusal, the build's (theme2's condition 2); applied with each element's
/// own font-family, it is judged at each.
#[test]
fn a_style_row_broken_at_two_elements_is_refused_once() {
    let dir = reading("rows-style-two");
    let contract = "use Card from \"@acme/reading\"\nstyle Pad\n  padding=\"gap\"\ncomponent App\n  view\n    column class=Pad\n      row class=Pad\n        Card()\n";
    let got = refusals(
        &dir,
        DESIGN,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
    )
    .expect("refused");
    std::fs::write(
        dir.join("app.tamber"),
        DESIGN.replace("gap = 8px", "gap = #444444"),
    )
    .unwrap();
    let src = std::fs::read_to_string(dir.join("app.contract")).unwrap();
    let mut built: Vec<(u64, u64, String)> =
        contract::compile_path_source_all(&dir.join("app.contract"), &src, false)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.span.line as u64, e.span.col as u64, e.message))
            .collect();
    built.dedup();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0], built[0], "{got:?} against {built:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Left to the build, which accepts each: a style no element applies, a row
/// the element sets itself (its own replaces the style's), and a choice of
/// styles.
#[test]
fn an_unused_an_overridden_and_a_chosen_style_row_are_not_judged() {
    for (name, contract) in [
        ("rows-style-unused", "use Card from \"@acme/reading\"\nstyle Pad\n  padding=\"gap\"\ncomponent App\n  view\n    column\n      Card()\n"),
        ("rows-style-own", "use Card from \"@acme/reading\"\nstyle Pad\n  padding=\"gap\"\ncomponent App\n  view\n    column class=Pad padding=4\n      Card()\n"),
    ] {
        let dir = reading(name);
        let got = refusals(&dir, DESIGN, contract, json!({"op": "set-token", "name": "gap", "value": "#444444"}));
        assert_eq!(got, None, "{name}");
        contract::compile_path(&dir.join("app.contract")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
    // A choice of styles is the build's: its refusal comes from the build.
    let dir = reading("rows-style-choice");
    let contract = "use Card from \"@acme/reading\"\nstyle Pad\n  padding=\"gap\"\nstyle Flat\n  padding=0\ncomponent App\n  state on = false\n  view\n    column class=(on ? Pad : Flat)\n      Card()\n";
    assert_eq!(
        refusals(
            &dir,
            DESIGN,
            contract,
            json!({"op": "set-token", "name": "gap", "value": "#444444"})
        ),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A style applied under a `for` is the build's there (the row's scope is
/// the region's), and judged at an element outside it: one refusal, the
/// column's, in the build's words.
#[test]
fn a_style_row_under_a_for_is_left_to_the_build_there() {
    let contract = "use Card from \"@acme/reading\"\nstyle Pad\n  padding=\"gap\"\ncomponent App\n  state items = [1]\n  view\n    column class=Pad\n      for i in items key=i\n        row class=Pad\n          Card()\n";
    same_words(
        &reading("rows-style-each"),
        DESIGN,
        contract,
        json!({"op": "set-token", "name": "gap", "value": "#444444"}),
        &DESIGN.replace("gap = 8px", "gap = #444444"),
    );
}
