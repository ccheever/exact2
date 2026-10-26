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
    // The app's own id: the gallery's plan runs in the app's build (its wasm checks the id).
    assert_eq!(manifest["app"]["id"], "com.acme.fx");
    assert_eq!(manifest["name"], "Fx gallery");
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
        (dir.join("src"), "never among sources"),
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
    // A manifest alone this didn't write, and one linked back to the app's: never written over.
    let lone = dir.join("target/lone");
    write(&lone, &[("app.json", "{}")]);
    let e = contract::gallery::generate(&app, &lone).unwrap_err();
    assert!(e.contains("not a gallery this wrote"), "{e}");
    let linked = dir.join("target/linked");
    std::fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(dir.join("app.json"), linked.join("app.json")).unwrap();
    let e = contract::gallery::generate(&app, &linked).unwrap_err();
    assert!(e.contains("not a gallery this wrote"), "{e}");
    let manifest = r#"{"app": {"id": "com.acme.fx", "name": "Fx"}}"#;
    assert_eq!(
        std::fs::read_to_string(dir.join("app.json")).unwrap(),
        manifest
    );
    // Outside the app, a directory among sources (no node_modules or target in its path); and a cache no
    // node_modules of the app's reaches.
    let away = std::env::temp_dir().join(format!("exact2-gallery-away-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&away);
    let e = contract::gallery::generate(&app, &away.join("docs")).unwrap_err();
    assert!(e.contains("never among sources"), "{e}");
    let e = contract::gallery::generate(&app, &away.join("target/g")).unwrap_err();
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
            .starts_with("com.exact."),
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

#[test]
fn the_design_reads_the_apps_assets_and_a_shape_is_brought_by_the_name_the_package_exports() {
    let dir = fixture("assets");
    let lib = dir.join("node_modules/@acme/lib");
    // `Card` and its `Info` in a file the entry re-exports, `Info` as `PublicInfo`.
    std::fs::write(lib.join("card.contract"), CARD).unwrap();
    std::fs::write(
        lib.join("index.contract"),
        "use Card, Info as PublicInfo from \"./card.contract\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    column\n      text \"a\"\n",
    )
    .unwrap();
    // A design whose icon is an SVG under the app's assets.
    write(
        &dir,
        &[
            ("assets/icons/a.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path d=\"M0 0h24v24H0z\"/></svg>\n"),
            ("app.tamber", "tamber app\n\ntheme\n  lib.pad = 6px\n\nicons\n  icon.a = icon(svg \"icons/a.svg\")\n"),
        ],
    );
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    assert!(g.cells.iter().all(|c| c.refused.is_none()), "{:?}", g.cells);
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(
        text.contains("use PublicInfo from \"@acme/lib\"")
            && text.contains("info=PublicInfo(title=\"title\", n=0)"),
        "{text}"
    );
    assert_eq!(
        std::fs::read_link(out.join("assets")).unwrap(),
        dir.join("assets").canonicalize().unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A library package's own example app inside its tree, linking the
/// package by its name: the app's own `use`s, by the loader's record.
#[test]
fn an_app_inside_its_library_package_is_the_apps_own_and_a_compiler_record_has_a_placeholder() {
    let root = std::env::temp_dir().join(format!("exact2-gallery-holding-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let pkg = root.join("pkg");
    write(
        &pkg,
        &[
            ("package.json", &package("@x/pkg", r#"{".":"./index.contract"}"#)),
            ("pkg.tamber", "tamber pkg\n\ntheme\n  pkg.pad = 4px\n"),
            ("index.contract", "component Note\n  props\n    marks: TextDecorations\n  parts\n    face: paint\n  view\n    text marks.value part=\"face\"\n"),
            ("example/app.contract", "use Note from \"@x/pkg\"\n\ncomponent App\n  view\n    column\n      Note(marks=TextDecorations(value=\"v\", spans=[]))\n"),
        ],
    );
    std::fs::create_dir_all(pkg.join("example/node_modules/@x")).unwrap();
    std::os::unix::fs::symlink(&pkg, pkg.join("example/node_modules/@x/pkg")).unwrap();
    let app = pkg.join("example/app.contract");
    contract::compile_path(&app).unwrap_or_else(|e| panic!("{e:?}"));
    let out = pkg.join("example/node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&app, &out).expect("the gallery");
    assert_eq!(g.cells.len(), 1, "{:?}", g.cells);
    assert_eq!(g.cells[0].refused, None);
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(
        text.contains(r#"Note(marks=TextDecorations(value="value", spans=[]))"#),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn git_init(dir: &Path) {
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn a_checkout_under_a_target_directory_and_a_cache_git_does_not_ignore_are_refused() {
    let base = std::env::temp_dir().join(format!("exact2-gallery-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    // A repository whose checkout sits under a directory named `target`: its app's `src` is still sources.
    let repo = base.join("target/repo");
    std::fs::create_dir_all(&repo).unwrap();
    git_init(&repo);
    write(&repo, &[(".gitignore", "node_modules/\n")]);
    let app = fixture("git-app");
    let at = repo.join("apps/x");
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::rename(&app, &at).unwrap();
    let e =
        contract::gallery::generate(&at.join("app.contract"), &at.join("src/gallery")).unwrap_err();
    assert!(e.contains("never among sources"), "{e}");
    // Refused before anything is made there.
    assert!(!at.join("src").exists());
    // A `target` git doesn't ignore here (only node_modules/ is): refused; the ignored node_modules cache is fine.
    let e =
        contract::gallery::generate(&at.join("app.contract"), &repo.join("target/g")).unwrap_err();
    assert!(
        e.contains("git doesn't ignore what the gallery writes"),
        "{e}"
    );
    contract::gallery::generate(&at.join("app.contract"), &at.join("node_modules/.cache/g"))
        .expect("the gallery");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_refused_cell_keeps_its_packages_design_and_a_chain_of_re_exports_names_the_shape() {
    let dir = fixture("chain");
    let lib = dir.join("node_modules/@acme/lib");
    // `Info` reaches the entry through a barrel, renamed; `Align` fails its placeholder and is the app's only use.
    write(
        &lib,
        &[
            ("card.contract", CARD),
            // Two names for one shape in the barrel; the entry takes the second.
            ("barrel.contract", "use Card, Info as Internal, Info as PublicInfo from \"./card.contract\"\n"),
            ("index.contract", "use Card, PublicInfo from \"./barrel.contract\"\n\ncomponent Align\n  props\n    align: string\n  parts\n    face: paint\n  view\n    text \"a\" part=\"face\" text-align=align\n"),
        ],
    );
    std::fs::write(dir.join("app.contract"), "use Align from \"@acme/lib\"\n\ncomponent App\n  view\n    column\n      Align(align=\"left\")\n").unwrap();
    // The app's design names the package's token: the package's design must come with the gallery.
    std::fs::write(
        dir.join("app.tamber"),
        "tamber app\n\ntheme\n  app.pad = lib.pad\n",
    )
    .unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    // Its only cell refused, the package still brought: the gallery compiles with the app's design.
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let shown: Vec<(&str, bool)> = g
        .cells
        .iter()
        .map(|c| (c.component.as_str(), c.refused.is_some()))
        .collect();
    assert_eq!(shown, vec![("Align", true)]);
    assert!(std::fs::read_to_string(&g.source)
        .unwrap()
        .contains("use Align from \"@acme/lib\""));
    // With `Card` used too, its shape by the name the entry gives it through the barrel.
    std::fs::write(dir.join("app.contract"), "use Align, Card from \"@acme/lib\"\n\ncomponent App\n  view\n    column\n      Align(align=\"left\")\n").unwrap();
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let shown: Vec<(&str, bool)> = g
        .cells
        .iter()
        .map(|c| (c.component.as_str(), c.refused.is_some()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("Align", true),
            ("Card", false),
            ("Card", false),
            ("Card", false)
        ]
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(text.contains("use PublicInfo from \"@acme/lib\""), "{text}");
    // The entry re-exporting by the package's own name and a subpath: followed the same.
    write(
        &lib,
        &[
            ("package.json", &package("@acme/lib", r#"{".":"./index.contract","./card":"./card.contract"}"#)),
            ("index.contract", "use Card, Info as Shown from \"@acme/lib/card\"\n\ncomponent Align\n  props\n    align: string\n  parts\n    face: paint\n  view\n    text \"a\" part=\"face\" text-align=align\n"),
        ],
    );
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    assert!(
        g.cells
            .iter()
            .filter(|c| c.component == "Card")
            .all(|c| c.refused.is_none()),
        "{:?}",
        g.cells
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(text.contains("use Shown from \"@acme/lib\""), "{text}");
    // And a package with no `exports`, its file by path, as the loader reaches it.
    write(
        &lib,
        &[
            ("package.json", r#"{"name":"@acme/lib","version":"1.0.0","private":true}"#),
            ("index.contract", "use Card, Info as ByPath from \"@acme/lib/card.contract\"\n\ncomponent Align\n  props\n    align: string\n  parts\n    face: paint\n  view\n    text \"a\" part=\"face\" text-align=align\n"),
        ],
    );
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    assert!(
        g.cells
            .iter()
            .filter(|c| c.component == "Card")
            .all(|c| c.refused.is_none()),
        "{:?}",
        g.cells
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    assert!(text.contains("use ByPath from \"@acme/lib\""), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_probe_leaves_no_manifest_so_the_next_run_proceeds_and_git_must_ignore_every_file() {
    let dir = fixture("retry");
    let app = dir.join("app.contract");
    // A cache no node_modules reaches: refused, and nothing of the gallery's left there.
    let other = std::env::temp_dir().join(format!("exact2-gallery-other-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&other);
    let out = other.join("target/g");
    let e = contract::gallery::generate(&app, &out).unwrap_err();
    assert!(e.contains("can't reach the app's packages"), "{e}");
    assert!(!out.join("app.json").exists());
    // Reached now (its own node_modules links the app's): the same destination proceeds.
    std::os::unix::fs::symlink(dir.join("node_modules"), other.join("node_modules")).unwrap();
    contract::gallery::generate(&app, &out).expect("the gallery");
    // A foreign manifest alone is never written over.
    let lone = dir.join("target/lone2");
    write(&lone, &[("app.json", r#"{"name": "Photo gallery"}"#)]);
    let e = contract::gallery::generate(&app, &lone).unwrap_err();
    assert!(e.contains("not a gallery this wrote"), "{e}");
    // In a repository ignoring only the source, the manifest would show: refused.
    let repo = other.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git_init(&repo);
    let at = repo.join("x");
    std::fs::rename(&dir, &at).unwrap();
    write(&repo, &[(".gitignore", "x/target/g/app.contract\n")]);
    let e =
        contract::gallery::generate(&at.join("app.contract"), &at.join("target/g")).unwrap_err();
    assert!(
        e.contains("git doesn't ignore what the gallery writes"),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&other);
}

const HANDLED: &str = r#"component Row
  props
    symbol: string
    title: string
    press: action
  view
    button press=press
      image `symbol:${symbol}`
      text title

component Toggle
  props
    title: string
    on: bool
    set: action
  view
    row
      text title
      input type="checkbox" switch checked=on change=set aria-label=title

component Field
  props
    text: string
    edit: action
  view
    input value=text input=edit aria-label="Field"
"#;

const APP_HANDLED: &str = r#"use Row, Toggle, Field from "@acme/lib"

component App
  state on = false
  state typed = ""
  action go
  action flip(v: bool)
    on = v
  action type(v: string)
    typed = v
  view
    column
      Row(symbol="settings", title="t", press=go)
      Toggle(title="t", on=on, set=flip)
      Field(text=typed, edit=type)
"#;

/// A string prop the view names a symbol by gets a role (its own name names
/// none), and an action prop a handler calls with its payload gets an inert
/// action of that signature, one per signature (stage 13 C: @exact/lists'
/// ListRow and SwitchRow).
#[test]
fn a_symbols_prop_gets_a_role_and_a_handlers_action_its_payloads_parameters() {
    let dir = fixture("handled");
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::write(lib.join("index.contract"), HANDLED).unwrap();
    std::fs::write(dir.join("app.contract"), APP_HANDLED).unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let refused: Vec<(&str, &str)> = g
        .cells
        .iter()
        .filter_map(|c| Some((c.component.as_str(), c.refused.as_deref()?)))
        .collect();
    assert_eq!(refused, vec![], "every cell placed");
    let text = std::fs::read_to_string(&g.source).unwrap();
    for line in [
        "  action noop\n",
        "  action noop_bool(checked: bool)\n",
        "  action noop_string(value: string)\n",
        r#"Row(symbol="back", title="title", press=noop)"#,
        r#"Toggle(title="title", on=false, set=noop_bool)"#,
        r#"Field(text="text", edit=noop_string)"#,
    ] {
        assert!(text.contains(line), "{line:?} in {text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

const BOUNDARIES: &str = r#"component Stem
  props
    chevron: string
    gear: string
  view
    row
      image `symbol:${chevron}-left`
      image `symbol:${gear}`

component Shadowed
  props
    symbol: string
  view
    column
      text symbol
      for symbol in ["add"] key = symbol
        image `symbol:${symbol}`

component Twice
  props
    on: bool
    set: action
  view
    input type="checkbox" switch checked=on change=set aria-label="Twice"

component Odd
  props
    pick: action
  view
    button press=pick("x")
      text "Pick"

component Forward
  props
    on: bool
    set: action
  view
    Toggle(title="f", on=on, set=set)

component Five
  props
    pick: action
  view
    button press=pick(1, 2, 3, 4, `e${1}`)
      text "Five"

shape Num
  id: number

shape Word
  id: string

component ShadowList
  props
    items: Num[]
    groups: Word[][]
    pick: action
  view
    column
      for items, i in groups key = i
        for x in items key = x.id
          button press=pick(x.id)
            text "x"
"#;

const APP_BOUNDARIES: &str = r#"use Stem, Shadowed, Toggle, Twice, Odd, Forward, Five, ShadowList from "@acme/lib"

component App
  state on = false
  action flip(v: bool)
    on = v
  action pick(v: string)
  view
    column
      Stem(chevron="chevron", gear="gear")
      Shadowed(symbol="s")
      Toggle(title="t", on=on, set=flip)
      Twice(on=on, set=flip)
      Odd(pick=pick)
      Forward(on=on, set=flip)
      Five(pick=pick)
      ShadowList(items=[], groups=[], pick=pick)
"#;

/// A stem (`symbol:${chevron}-left`, an alias of `back`) keeps the prop's
/// own name, which names no role alone, and a prop that
/// is a role's alias gets that role; a `for` that binds the prop's name
/// hides it; two components of one signature share one inert action; an
/// action the view calls with a literal argument gets an inert action of its
/// type (its arguments named by position, five as well as one, a template
/// a string, a `for` item's field typed from the list it iterates, even one
/// an outer `for` binds over the prop's name); one it passes on to another
/// component stays `noop`, for the
/// compiler to refuse. (One given to handlers that pass different payloads
/// stays `noop` too, which `press` and `navigate` both take.)
#[test]
fn a_stem_a_shadow_a_shared_signature_and_an_action_the_view_calls_otherwise() {
    let dir = fixture("boundaries");
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::write(
        lib.join("index.contract"),
        format!("{HANDLED}\n{BOUNDARIES}"),
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), APP_BOUNDARIES).unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    let refused: Vec<&str> = g
        .cells
        .iter()
        .filter(|c| c.refused.is_some())
        .map(|c| c.component.as_str())
        .collect();
    assert_eq!(
        refused,
        vec!["Forward"],
        "only the cell whose action the view passes on"
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    for line in [
        r#"Stem(chevron="chevron", gear="settings")"#,
        r#"Shadowed(symbol="symbol")"#,
        r#"Twice(on=false, set=noop_bool)"#,
        "action noop_string(a0: string)",
        "(a0: number, a1: number, a2: number, a3: number, a4: string)",
        r#"ShadowList(items=[], groups=[], pick=noop_string)"#,
        r#"Odd(pick=noop_string)"#,
        "binding to `noop`",
    ] {
        assert!(text.contains(line), "{line:?} in {text}");
    }
    assert_eq!(text.matches("  action noop_bool(").count(), 1, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

const PICS: &str = r#"component Pic
  props
    src: string
    label: string
  parts
    frame: paint
  view
    column part="frame"
      image src width=20 height=20
      text label

component Face
  props
    avatar: string
    name: string
  parts
    face: paint
  view
    column part="face"
      Pic(src=avatar, label=name)
      text name

component Banner
  props
    banner: string
    fallback: string
    backup: string
    plain: bool
    icon: string
  parts
    band: paint
  view
    column part="band"
      image `${banner}` width=20 height=20
      image (plain ? fallback : `${backup}`) width=20 height=20
      image `symbol:${icon}` width=20 height=20
"#;

const PICS_APP: &str = r#"use Pic, Face, Banner from "@acme/lib"

component App
  view
    column
      Pic(src="a.png", label="a")
      Face(avatar="b.png", name="b")
      Banner(banner="c.png", fallback="d.png", backup="e.png", plain=false, icon="star")
"#;

/// A string prop shown as an image's source, directly or through a
/// component it is passed to, gets a neutral picture, not its name, which no
/// image loads (design-tool pass 2, #8); a text prop keeps its name.
#[test]
fn a_string_shown_as_an_image_gets_a_neutral_picture_and_text_keeps_its_name() {
    let dir = fixture("images");
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::write(lib.join("index.contract"), PICS).unwrap();
    std::fs::write(dir.join("app.contract"), PICS_APP).unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    assert!(
        g.cells.iter().all(|c| c.refused.is_none()),
        "{:?}",
        g.cells.iter().map(|c| &c.refused).collect::<Vec<_>>()
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    let neutral = r#""data:image/svg+xml,%3Csvg"#;
    let pic = text
        .lines()
        .find(|l| l.contains("Pic("))
        .expect("a Pic cell");
    assert!(pic.contains(&format!("src={neutral}")), "{pic}");
    assert!(pic.contains(r#"label="label""#), "{pic}");
    let face = text
        .lines()
        .find(|l| l.contains("Face("))
        .expect("a Face cell");
    assert!(face.contains(&format!("avatar={neutral}")), "{face}");
    assert!(face.contains(r#"name="name""#), "{face}");
    // Through a template of it alone and either branch of a `?:`; a symbol's name is no image.
    let banner = text
        .lines()
        .find(|l| l.contains("Banner("))
        .expect("a Banner cell");
    assert!(banner.contains(&format!("banner={neutral}")), "{banner}");
    assert!(banner.contains(&format!("fallback={neutral}")), "{banner}");
    assert!(banner.contains(&format!("backup={neutral}")), "{banner}");
    assert!(!banner.contains(&format!("icon={neutral}")), "{banner}");
    // Sized by itself, as a browser sizes an SVG without a size, for native readers.
    assert!(text.contains("width='150' height='150'"), "{banner}");
    let _ = std::fs::remove_dir_all(&dir);
}

const STYLED: &str = r#"component Dot
  props
    size: string
    ink: string
    label: string
  view
    column width=size height=size background-color=ink aria-label=label
      text label

component Ring
  props
    edge: string
    tint: string
  view
    Dot(size=edge, ink=tint, label="ring")

component Glow
  props
    glow: string
  view
    text "g" text-shadow=glow
"#;

const STYLED_APP: &str = r#"use Dot, Ring, Glow from "@acme/lib"

component App
  view
    column
      Dot(size="4px", ink="red", label="a")
      Ring(edge="4px", tint="red")
      Glow(glow="glow")
"#;

/// A string prop written as a style row's value, directly or through a
/// component it is passed to, gets a value of that row's kind, not its name,
/// which no row reads (LLP 1121 stage 15 B: Avatar's `width=size`,
/// VerifiedBadge's `color=ink`): a length row `24px`, a colour row
/// `currentcolor`; a label keeps its name, and so does a shadow (a row that
/// holds a colour but takes no colour alone; its name may be a token's).
#[test]
fn a_string_written_as_a_style_row_gets_a_value_of_its_kind() {
    let dir = fixture("styled");
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::write(lib.join("index.contract"), STYLED).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber app\n\ntheme\n  lib.pad = 6px\n  glow: text-shadow = 0 1px 2px #000000\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), STYLED_APP).unwrap();
    let out = dir.join("node_modules/.cache/exact-gallery");
    let g = contract::gallery::generate(&dir.join("app.contract"), &out).expect("the gallery");
    assert!(
        g.cells.iter().all(|c| c.refused.is_none()),
        "{:?}",
        g.cells.iter().map(|c| &c.refused).collect::<Vec<_>>()
    );
    let text = std::fs::read_to_string(&g.source).unwrap();
    let dot = text
        .lines()
        .find(|l| l.contains("Dot("))
        .expect("a Dot cell");
    assert!(dot.contains(r#"size="24px""#), "{dot}");
    assert!(dot.contains(r#"ink="currentcolor""#), "{dot}");
    assert!(dot.contains(r#"label="label""#), "{dot}");
    let ring = text
        .lines()
        .find(|l| l.contains("Ring("))
        .expect("a Ring cell");
    assert!(ring.contains(r#"edge="24px""#), "{ring}");
    assert!(ring.contains(r#"tint="currentcolor""#), "{ring}");
    let glow = text
        .lines()
        .find(|l| l.contains("Glow("))
        .expect("a Glow cell");
    assert!(glow.contains(r#"glow="glow""#), "{glow}");
    let _ = std::fs::remove_dir_all(&dir);
}
