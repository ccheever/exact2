//! `contract gallery` (LLP 1139 D5, §9 Q2): the design tool's variant
//! gallery of an app's library components, generated into a build cache
//! and checked by the compiler. Each cell is a component's variants' first
//! values or one other value alone; every other prop a placeholder by type;
//! a cell the compiler refuses says so in place; the app's design is the
//! gallery's; nothing of the app's own build or describe changes.
#![cfg(unix)]

use std::path::{Path, PathBuf};

const LIB: &str = r#"shape Info
  title: string
  n: number

style Plain
  opacity=1

style Quiet
  opacity=0.8

component Card
  props
    tone: enum<"plain", "quiet">
    compact: bool
    label: string
    count: number
    maybe: option<string>
    items: string[]
    press: action
    info: Info
  slot
  parts
    surface: paint
  view
    column part="surface" class=(tone == "quiet" ? Quiet : Plain) opacity=(compact ? 0.5 : 1) press=press
      text label
      children

component Needs
  inject
    theme: string
  parts
    face: paint
  view
    text theme part="face"
"#;

const APP: &str = r#"use Card, Needs, Info from "@acme/lib"

component App
  provide
    theme = "t"
  action go
  view
    column
      Card(tone="plain", compact=false, label="a", count=1, maybe=none, items=[], press=go, info=Info(title="x", n=1))
        text "fill"
      Needs()
"#;

/// An app using a private library package's `Card` (an enum and a bool
/// variant, a prop of every other kind, a slot) and `Needs` (an inject the
/// gallery can't provide).
fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-gallery-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("package.json"), r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#).unwrap();
    std::fs::write(
        lib.join("lib.tamber"),
        "tamber lib\n\ntheme\n  lib.pad = 4px\n",
    )
    .unwrap();
    std::fs::write(lib.join("index.contract"), LIB).unwrap();
    std::fs::write(dir.join("app.contract"), APP).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber app\n\ntheme\n  lib.pad = 6px\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "com.acme.fx", "name": "Fx"}}"#,
    )
    .unwrap();
    dir
}

fn plan(app: &Path) -> Vec<u8> {
    let (plan, _) =
        contract::compile_path_all(&app.join("app.contract"), false).expect("the app compiles");
    plan.encode()
}

#[test]
fn the_gallery_places_each_cell_refuses_one_the_compiler_wont_take_and_leaves_the_app_alone() {
    let dir = fixture("cells");
    let before = (
        plan(&dir),
        contract::tamber_params::params_of(&dir.join("app.contract")).files,
    );
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    type Shown = (String, Option<(String, String)>, bool);
    let shown: Vec<Shown> = g
        .cells
        .iter()
        .map(|c| (c.component.clone(), c.varied.clone(), c.refused.is_some()))
        .collect();
    let v = |p: &str, o: &str| Some((p.to_string(), o.to_string()));
    assert_eq!(
        shown,
        vec![
            ("Card".into(), None, false),
            ("Card".into(), v("tone", "quiet"), false),
            ("Card".into(), v("compact", "true"), false),
            ("Needs".into(), None, true),
        ]
    );
    // The refusal is the compiler's own words, in the cell.
    let why = g.cells[3].refused.as_deref().unwrap();
    assert!(why.contains("injects `theme`"), "{why}");
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(text.contains(&format!("can't place: {why}")), "{text}");
    // A placeholder of each kind, an inert action, the variant values as written, the slot's box.
    assert!(text.contains(r#"Card(tone="quiet", compact=false, label="label", count=0, maybe=none, items=[], press=noop, info=Info(title="title", n=0))"#), "{text}");
    assert!(
        text.contains("  action noop\n") && text.contains(r#"box testId="gallery-Card-2-slot""#),
        "{text}"
    );
    // The gallery's design is the app's own; the app's plan and describe's files are as they were.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("app.json")).unwrap()).unwrap();
    assert_eq!(manifest["tamber"], "../../../app.tamber");
    assert_eq!(manifest["app"]["id"], "com.acme.fx.gallery");
    let after = (
        plan(&dir),
        contract::tamber_params::params_of(&dir.join("app.contract")).files,
    );
    assert_eq!(before, after);
    assert!(
        !after.1.iter().any(|f| f.contains("exact-gallery")),
        "{:?}",
        after.1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same package with `Card` and its shape `Info` in a file the entry
/// re-exports only `Card` from, and an app that names `Card` but never
/// builds an `Info`: the gallery can't name the shape, so `Card`'s cells
/// say so, and the gallery still compiles.
const CARD: &str = r#"shape Info
  title: string
  n: number

style Plain
  opacity=1

style Quiet
  opacity=0.8

component Card
  props
    tone: enum<"plain", "quiet">
    compact: bool
    label: string
    count: number
    maybe: option<string>
    items: string[]
    press: action
    info: Info
  slot
  parts
    surface: paint
  view
    column part="surface" class=(tone == "quiet" ? Quiet : Plain) opacity=(compact ? 0.5 : 1) press=press
      text label
      children

"#;

const INDEX: &str = r#"use Card from "./card.contract"

component Needs
  inject
    theme: string
  parts
    face: paint
  view
    text theme part="face"
"#;

const APP_UNBUILT: &str = r#"use Card, Needs from "@acme/lib"

component App
  provide
    theme = "t"
  action go
  view
    column
      Needs()
"#;

#[test]
fn a_shape_the_package_does_not_export_refuses_the_cells_that_build_it_not_the_gallery() {
    let dir = fixture("unexported");
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::write(lib.join("card.contract"), CARD).unwrap();
    std::fs::write(lib.join("index.contract"), INDEX).unwrap();
    std::fs::write(dir.join("app.contract"), APP_UNBUILT).unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let refused: Vec<(&str, &str)> = g
        .cells
        .iter()
        .map(|c| (c.component.as_str(), c.refused.as_deref().unwrap_or("")))
        .collect();
    assert_eq!(refused.len(), 4);
    for (component, why) in &refused[..3] {
        assert_eq!(*component, "Card");
        assert!(
            why.starts_with("its shape Info can't be named here"),
            "{why}"
        );
    }
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(!text.contains("use Info"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
