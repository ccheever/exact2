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

fn write(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let at = dir.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
}

fn package(name: &str, exports: &str) -> String {
    format!(r#"{{"name":"{name}","version":"1.0.0","private":true,"exports":{exports}}}"#)
}

#[test]
fn the_gallery_never_writes_over_the_apps_sources_nor_where_another_install_is_reached() {
    let dir = fixture("destination");
    let app = dir.join("app.contract");
    // The app's own directory, a directory among its sources, a file this didn't write.
    for (out, why) in [
        (dir.clone(), "the app's own directory"),
        (dir.join("src"), "never among its sources"),
    ] {
        let e = contract::gallery::generate(&app, &out).unwrap_err();
        assert!(e.contains(why), "{e}");
    }
    let foreign = dir.join("target/mine");
    write(
        &foreign,
        &[("app.contract", "component Mine\n  view\n    text \"m\"\n")],
    );
    let e = contract::gallery::generate(&app, &foreign).unwrap_err();
    assert!(e.contains("not a gallery this wrote"), "{e}");
    assert_eq!(
        std::fs::read_to_string(foreign.join("app.contract")).unwrap(),
        "component Mine\n  view\n    text \"m\"\n"
    );
    assert_eq!(std::fs::read_to_string(&app).unwrap(), APP);
    // A cache no node_modules of the app's reaches.
    let away = std::env::temp_dir().join(format!("exact2-gallery-away-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&away);
    let e = contract::gallery::generate(&app, &away).unwrap_err();
    assert!(e.contains("can't reach the app's packages"), "{e}");
    let _ = std::fs::remove_dir_all(&away);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_component_is_the_declaration_the_apps_use_reaches_spelled_as_the_app_spells_it() {
    let dir = std::env::temp_dir().join(format!("exact2-gallery-names-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let card = "component Card\n  props\n    tone: enum<\"plain\", \"quiet\">\n  parts\n    face: paint\n  view\n    text tone part=\"face\" opacity=(tone == \"quiet\" ? 0.5 : 1)\n";
    write(
        &dir,
        &[
            // A re-export's rename (`Fancy`), and a component an exports subpath reaches (`Pill`).
            ("node_modules/@acme/lib/package.json", &package("@acme/lib", r#"{".":"./index.contract","./pill":"./pill.contract"}"#)),
            ("node_modules/@acme/lib/lib.tamber", "tamber lib\n\ntheme\n  lib.pad = 4px\n"),
            ("node_modules/@acme/lib/card.contract", card),
            ("node_modules/@acme/lib/pill.contract", "component Pill\n  parts\n    face: paint\n  view\n    text \"p\" part=\"face\"\n"),
            ("node_modules/@acme/lib/index.contract", "use Card as Fancy from \"./card.contract\"\nuse Pill from \"./pill.contract\"\n"),
            // A second package with a `Card` and a `Gallery` of its own.
            ("node_modules/@other/kit/package.json", &package("@other/kit", r#"{".":"./index.contract"}"#)),
            ("node_modules/@other/kit/kit.tamber", "tamber kit\n\ntheme\n  kit.pad = 2px\n"),
            ("node_modules/@other/kit/index.contract", "component Card\n  parts\n    face: paint\n  view\n    text \"k\" part=\"face\"\n\ncomponent Gallery\n  parts\n    face: paint\n  view\n    text \"g\" part=\"face\"\n"),
            ("app.contract", "use Fancy from \"@acme/lib\"\nuse Pill from \"@acme/lib/pill\"\nuse Card as KitCard, Gallery from \"@other/kit\"\n\ncomponent App\n  view\n    column\n      Fancy(tone=\"plain\")\n      Pill()\n      KitCard()\n      Gallery()\n"),
            ("app.tamber", "tamber app\n\ntheme\n  lib.pad = 6px\n"),
        ],
    );
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let names: Vec<(&str, &str, bool)> = g
        .cells
        .iter()
        .map(|c| {
            (
                c.component.as_str(),
                c.identity.as_str(),
                c.refused.is_none(),
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![
            ("Fancy", "@acme/lib/card.contract#Card", true),
            ("Fancy", "@acme/lib/card.contract#Card", true),
            ("Pill", "@acme/lib/pill.contract#Pill", true),
            ("Card", "@other/kit/index.contract#Card", true),
            ("Gallery_2", "@other/kit/index.contract#Gallery", true),
        ]
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    for line in [
        "use Fancy from \"@acme/lib\"",
        "use Pill from \"@acme/lib/pill\"",
        "use Card from \"@other/kit\"",
        "use Gallery as Gallery_2 from \"@other/kit\"",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
    // A manifest of the app's defaults when it has no app.json.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("app.json")).unwrap()).unwrap();
    assert!(
        manifest["app"]["id"]
            .as_str()
            .unwrap()
            .ends_with(".gallery"),
        "{manifest}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shape_the_loader_renamed_is_brought_by_its_own_name_and_a_refusal_at_a_librarys_span_is_that_cells(
) {
    let dir = fixture("renamed");
    let lib = dir.join("node_modules/@acme/lib");
    // A component whose placeholder the library refuses at its own span (a string as an alignment).
    let index = format!("{LIB}\ncomponent Align\n  props\n    align: string\n  parts\n    face: paint\n  view\n    text \"a\" part=\"face\" text-align=align\n");
    std::fs::write(lib.join("index.contract"), index).unwrap();
    // The app declares its own `Info`, so the loader renames the package's.
    std::fs::write(
        dir.join("app.contract"),
        "use Card, Needs, Align from \"@acme/lib\"\n\nshape Info\n  mine: bool\n\ncomponent App\n  provide\n    theme = \"t\"\n  view\n    column\n      Needs()\n",
    )
    .unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let refused: Vec<(&str, bool)> = g
        .cells
        .iter()
        .map(|c| (c.component.as_str(), c.refused.is_some()))
        .collect();
    assert_eq!(
        refused,
        vec![
            ("Card", false),
            ("Card", false),
            ("Card", false),
            ("Needs", true),
            ("Align", true)
        ]
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(
        text.contains("use Info from \"@acme/lib\"")
            && text.contains("info=Info(title=\"title\", n=0)"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
