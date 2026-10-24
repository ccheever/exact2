//! LLP 1121 stage 13: a package design's icon tokens (*tamber/1.16*). A
//! package's `icons` block names its icons under its namespace; one may name
//! its own package's icons and no other; the app overrides one by full name,
//! whole; D7 keeps one only when it is read. An SVG path is relative to the
//! `assets/` of the root whose file writes it, so a package's reads the
//! package's, and must be a one-colour template in v1 (theme2's Q2 (a)).
//! describe's and explain's icon rows name a package icon's package and
//! file, and explain gives `package_value` under an app override.
#![cfg(unix)]

use super::tamber_package_params::package;
use super::tamber_serve::{app, Serve};
use serde_json::json;
use std::path::{Path, PathBuf};

/// One shape, its path `d`: a template every host draws.
fn shape(d: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{d}"/></svg>"#)
}

const TWO: &str =
    r#"<svg viewBox="0 0 24 24"><path fill="red" d="M0 0h1"/><path fill="blue" d="M2 2h1"/></svg>"#;

/// The package's star, and an app file of the same name drawn otherwise.
const PACKAGE_STAR: &str = "M1 1h22v22H1Z";
const APP_STAR: &str = "M9 9h1v1H9Z";

/// The app at `tag`, its design `design`, using `@acme/reading`'s `Card`,
/// whose view draws `symbol:<symbol>`; the package's design declares
/// `icons` (its lines) and its `assets/star.svg` is [`PACKAGE_STAR`], the
/// app's own `assets/star.svg` [`APP_STAR`]. It ships iOS and the web.
fn reading(tag: &str, design: &str, symbol: &str, icons: &str) -> PathBuf {
    let dir = app(tag, design);
    package(
        &dir,
        "reading",
        &format!("component Card\n  view\n    column\n      image \"symbol:{symbol}\"\n"),
        &format!("tamber reading\n\ntheme\n  reading.ink = #112233\n\nicons\n{icons}"),
    );
    let assets = dir.join("node_modules/@acme/reading/assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("star.svg"), shape(PACKAGE_STAR)).unwrap();
    std::fs::write(assets.join("two.svg"), TWO).unwrap();
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("assets/star.svg"), shape(APP_STAR)).unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "dev.test.pkgicons", "name": "Icons"}, "host": {"ios": {}, "web": {}}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    dir
}

const APP: &str = "tamber t\n\ntheme\n  ink = #111111\n";
const STAR: &str = "  reading.star = icon(sf \"star\", svg \"star.svg\")\n";

fn build(dir: &Path) -> exact_plan::Plan {
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"))
}

fn refused(dir: &Path) -> contract::CompileError {
    contract::compile_path(&dir.join("app.contract")).expect_err("refused")
}

/// The source form a host reads for an icon of `sf` drawn as `path`.
fn form(sf: &str, path: &str, svg: &str) -> String {
    format!("symbol:icon|{sf}|0 0 24 24|{path}|{svg}")
}

#[test]
fn a_packages_icon_is_named_under_its_namespace() {
    let dir = reading("pkgicon-named", APP, "reading.star", STAR);
    let plan = build(&dir);
    let star = form("star", PACKAGE_STAR, "star.svg");
    assert!(plan.strings.iter().any(|s| s == star), "{:?}", plan.strings);
    // One named outside its namespace is refused, at its line in its file.
    let dir = reading(
        "pkgicon-misnamed",
        APP,
        "icon.star",
        "  icon.star = icon(sf \"star\")\n",
    );
    let e = refused(&dir);
    assert_eq!(e.id, "tamber-package-name", "{e}");
    assert!(
        e.message
            .contains("a package's icon is named under its package (`reading.…`)"),
        "{e}"
    );
    assert!(format!("{e}").contains("reading.tamber:7:"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_packages_icon_may_name_its_own_and_no_other() {
    let icons = format!("{STAR}  reading.alias = reading.star\n");
    let dir = reading("pkgicon-alias", APP, "reading.alias", &icons);
    let plan = build(&dir);
    // The alias is drawn as the star, its SVG the package's.
    let star = form("star", PACKAGE_STAR, "star.svg");
    assert!(plan.strings.iter().any(|s| s == star), "{:?}", plan.strings);
    // The app's icon naming the package's reads the package's file too:
    // the path is written in the package's design.
    let design = format!("{APP}\nicons\n  icon.mine = reading.star\n");
    let dir = reading("pkgicon-app-alias", &design, "icon.mine", STAR);
    let plan = build(&dir);
    assert!(plan.strings.iter().any(|s| s == star), "{:?}", plan.strings);
    // A package's icon naming the app's is refused, as a token is.
    let design = format!("{APP}\nicons\n  icon.home = icon(sf \"house\", svg \"star.svg\")\n");
    let icons = "  reading.home = icon.home\n";
    let dir = reading("pkgicon-foreign", &design, "reading.home", icons);
    let e = refused(&dir);
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
    assert!(
        e.message.contains(
            "`reading.home` names `icon.home`, an icon that isn't the package `@acme/reading`'s"
        ),
        "{e}"
    );
    assert!(format!("{e}").contains("reading.tamber:7:"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_apps_override_takes_a_package_icon_whole() {
    let design = format!("{APP}\nicons\n  reading.star = icon(sf \"heart\", svg \"star.svg\")\n");
    let dir = reading("pkgicon-override", &design, "reading.star", STAR);
    let plan = build(&dir);
    // The app's line, its SVG the app's own file; nothing of the package's.
    let heart = form("heart", APP_STAR, "star.svg");
    assert!(
        plan.strings.iter().any(|s| s == heart),
        "{:?}",
        plan.strings
    );
    assert!(
        !plan
            .strings
            .iter()
            .any(|s| s.contains(PACKAGE_STAR) || s.contains("|star|")),
        "{:?}",
        plan.strings
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d7_keeps_a_package_icon_only_when_read() {
    let icons = format!("{STAR}  reading.unused = icon(sf \"trash\", svg \"star.svg\")\n");
    let dir = reading("pkgicon-d7", APP, "reading.star", &icons);
    let plan = build(&dir);
    let kept: Vec<&str> = plan.icons.iter().map(|i| plan.str(i.name)).collect();
    assert_eq!(kept, ["reading.star"], "{kept:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_svg_is_read_from_the_packages_assets() {
    let dir = reading("pkgicon-assets", APP, "reading.star", STAR);
    let plan = build(&dir);
    assert!(
        !plan.strings.iter().any(|s| s.contains(APP_STAR)),
        "the app's same-named file is not the package's: {:?}",
        plan.strings
    );
    // With no file in the package, the app's is still not read: refused,
    // naming the package's file.
    std::fs::remove_file(dir.join("node_modules/@acme/reading/assets/star.svg")).unwrap();
    let e = refused(&dir);
    assert_eq!(e.id, "tamber-icon-svg", "{e}");
    assert!(
        e.message
            .starts_with("`reading.star`: the package `@acme/reading`'s `assets/star.svg`: "),
        "{e}"
    );
    assert!(format!("{e}").contains("reading.tamber:7:"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// theme2's Q2 (a): the web, which alone draws an SVG as written, stages
/// the app's `assets/` only, so a package's must be a template.
#[test]
fn a_packages_svg_of_more_than_one_colour_is_refused() {
    let icons = "  reading.two = icon(sf \"star\", svg \"two.svg\")\n";
    let dir = reading("pkgicon-two", APP, "reading.two", icons);
    let e = refused(&dir);
    assert_eq!(e.id, "tamber-icon-svg", "{e}");
    assert!(
        e.message.starts_with("`reading.two`: the package `@acme/reading`'s `assets/two.svg`: a package's pictures are one-colour templates in v1")
            && e.message.contains("more than one colour"),
        "{e}"
    );
    // The app's own such file stays the web's to draw as written.
    std::fs::write(dir.join("assets/two.svg"), TWO).unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"app": {"id": "dev.test.pkgicons", "name": "Icons"}, "host": {"web": {}}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        format!("{APP}\nicons\n  reading.two = icon(svg \"two.svg\")\n"),
    )
    .unwrap();
    build(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_source_graph_lists_a_package_svg() {
    let icons = format!("{STAR}  reading.later = icon(sf \"clock\", svg \"later.svg\")\n");
    let dir = reading("pkgicon-graph", APP, "reading.star", &icons);
    let graph = contract::source_graph(&dir.join("app.contract"));
    let stars: Vec<_> = graph
        .sources
        .iter()
        .filter(|s| s.path.ends_with("assets/star.svg"))
        .collect();
    assert!(
        stars.len() == 1
            && stars[0]
                .path
                .ends_with("node_modules/@acme/reading/assets/star.svg"),
        "{:?}",
        graph.sources
    );
    assert!(
        matches!(&stars[0].origin, contract::Origin::Package { name, .. } if name == "@acme/reading"),
        "{:?}",
        stars[0]
    );
    // One not there yet is looked for, so making it rebuilds.
    assert!(
        graph
            .consulted
            .iter()
            .any(|p| p.ends_with("node_modules/@acme/reading/assets/later.svg")),
        "{:?}",
        graph.consulted
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn describe_and_explain_name_a_package_icons_package_and_file() {
    let dir = reading("pkgicon-describe", APP, "reading.star", STAR);
    let mut s = Serve::start(&dir, &[]);
    let (described, _) = s.ask(json!({"method": "describe"}));
    let rows = described["ok"]["ir"]["icons"]
        .as_array()
        .unwrap_or_else(|| panic!("{described}"));
    let star = rows.iter().find(|r| r["name"] == "reading.star").unwrap();
    assert_eq!(star["package"], "@acme/reading", "{star}");
    assert_eq!(star["file"], "@acme/reading/reading.tamber", "{star}");
    // Drawn from the package's template: the web draws its path.
    assert_eq!(star["draws"], json!({"ios": "sf", "web": "svg"}), "{star}");
    let (explained, _) = s.ask(json!({"method": "explain", "token": "reading.star"}));
    let ok = &explained["ok"];
    assert_eq!(ok["package"], "@acme/reading", "{explained}");
    assert_eq!(ok["file"], "@acme/reading/reading.tamber", "{explained}");
    assert_eq!(
        ok["identity"], "@acme/reading/reading.tamber#reading.star",
        "{explained}"
    );
    assert_eq!(
        ok["value"], "icon(sf \"star\", svg \"star.svg\")",
        "{explained}"
    );
    assert!(ok["package_value"].is_null(), "{explained}");
    // resolve names them as for a package token (*1.16*).
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "reading.star"}));
    let ok = &resolved["ok"];
    assert_eq!(ok["package"], "@acme/reading", "{resolved}");
    assert_eq!(ok["file"], "@acme/reading/reading.tamber", "{resolved}");
    assert_eq!(
        ok["identity"], "@acme/reading/reading.tamber#reading.star",
        "{resolved}"
    );
    assert_eq!(ok["tuples"][0]["sf"], "star", "{resolved}");
    drop(s);
    // The app's override: its row is the app's file, and explain gives the
    // package's own line.
    std::fs::write(
        dir.join("app.tamber"),
        format!("{APP}\nicons\n  reading.star = icon(sf \"heart\")\n"),
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (described, _) = s.ask(json!({"method": "describe"}));
    let rows = described["ok"]["ir"]["icons"].as_array().unwrap();
    let star = rows.iter().find(|r| r["name"] == "reading.star").unwrap();
    assert_eq!(star["file"], "app.tamber", "{star}");
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "reading.star"}));
    assert_eq!(resolved["ok"]["package"], "@acme/reading", "{resolved}");
    assert_eq!(resolved["ok"]["file"], "app.tamber", "{resolved}");
    assert_eq!(resolved["ok"]["tuples"][0]["sf"], "heart", "{resolved}");
    let (explained, _) = s.ask(json!({"method": "explain", "token": "reading.star"}));
    let ok = &explained["ok"];
    assert_eq!(ok["file"], "app.tamber", "{explained}");
    assert_eq!(ok["value"], "icon(sf \"heart\")", "{explained}");
    assert_eq!(
        ok["package_value"],
        json!({"value": "icon(sf \"star\", svg \"star.svg\")", "whens": [], "file": "@acme/reading/reading.tamber"}),
        "{explained}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The levels walker (stage 12 B2-levels) reads a package's icons as its
/// tokens: an app param naming one reaches the package's line.
#[test]
fn a_param_naming_a_package_icon_reaches_its_line() {
    let dir = reading("pkgicon-levels", APP, "reading.star", STAR);
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent Badge\n  params\n    mark: icon = \"reading.star\"\n  view\n    image \"symbol:param.mark\"\ncomponent App\n  view\n    column\n      Card()\n      Badge()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "app.contract#Badge.mark"}));
    assert_eq!(
        resolved["ok"]["levels"]["token"],
        json!({"name": "reading.star", "package": "@acme/reading", "file": "@acme/reading/reading.tamber", "line": 7, "value": "icon(sf \"star\", svg \"star.svg\")"}),
        "{resolved}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A write's check of an icon param's SVG (`param_values`) reads a package
/// default's file in the package's `assets/`: missing there, the app's
/// same-named file doesn't stand in for it.
#[test]
fn a_writes_check_reads_a_package_defaults_svg_in_the_package() {
    let dir = reading("pkgicon-ops", APP, "reading.star", STAR);
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    std::fs::write(
        &design,
        format!("{text}\ncomponent Map\n  params\n    glyph: icon = icon(svg \"pin.svg\")\n"),
    )
    .unwrap();
    std::fs::write(dir.join("assets/pin.svg"), shape(APP_STAR)).unwrap();
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-token", "name": "ink", "value": "#222222"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&file)}, "ops": [op]}));
    let said = &done["error"]["diagnostics"][0];
    assert_eq!(said["id"], "tamber-icon-svg", "{done}");
    assert!(
        said["message"].as_str().is_some_and(
            |m| m.contains("`assets/pin.svg` is no file of the package `@acme/reading`")
        ),
        "{done}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// theme2's Q1 for a package's Contract component (the lead's ruling on
/// round 1's open question): a literal `icon(svg …)` default written in a
/// package's `.contract` reads the package's `assets/`, whether or not the
/// package ships a design, never the app's same-named file; an app's
/// `params` line setting it reads the app's. The source graph lists the
/// package's file, and a write's check reads it there.
#[test]
fn a_package_contracts_icon_param_reads_its_packages_assets() {
    let dir = reading("pkgicon-contract", APP, "reading.star", STAR);
    let package = dir.join("node_modules/@acme/reading");
    std::fs::write(
        package.join("index.contract"),
        "component Card\n  params\n    glyph: icon = \"icon(svg \\\"pin.svg\\\")\"\n  view\n    column\n      image \"symbol:param.glyph\"\n",
    )
    .unwrap();
    std::fs::write(package.join("assets/pin.svg"), shape(PACKAGE_STAR)).unwrap();
    std::fs::write(dir.join("assets/pin.svg"), shape(APP_STAR)).unwrap();
    let drawn = |dir: &Path| {
        let plan = build(dir);
        let glyph = plan
            .icons
            .iter()
            .find(|i| plan.str(i.name).ends_with(".Card.glyph"))
            .unwrap_or_else(|| panic!("{:?}", plan.icons));
        icon_path(&plan, glyph).to_string()
    };
    for design in [true, false] {
        if !design {
            std::fs::remove_file(package.join("reading.tamber")).unwrap();
        }
        assert_eq!(drawn(&dir), PACKAGE_STAR, "with a design: {design}");
        let graph = contract::source_graph(&dir.join("app.contract"));
        let pins: Vec<_> = graph
            .sources
            .iter()
            .filter(|s| s.path.ends_with("assets/pin.svg"))
            .collect();
        assert!(
            pins.len() == 1
                && pins[0]
                    .path
                    .ends_with("node_modules/@acme/reading/assets/pin.svg")
                && matches!(&pins[0].origin, contract::Origin::Package { name, .. } if name == "@acme/reading"),
            "{:?}",
            graph.sources
        );
    }
    // Missing in the package: a write's check says so, the app's file no stand-in.
    std::fs::remove_file(package.join("assets/pin.svg")).unwrap();
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-token", "name": "ink", "value": "#222222"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&file)}, "ops": [op]}));
    let said = &done["error"]["diagnostics"][0];
    assert!(
        said["message"].as_str().is_some_and(
            |m| m.contains("`assets/pin.svg` is no file of the package `@acme/reading`")
        ),
        "{done}"
    );
    drop(s);
    let e = refused(&dir);
    assert_eq!(e.id, "tamber-icon-svg", "{e}");
    assert!(
        e.message
            .contains("the package `@acme/reading`'s `assets/pin.svg`"),
        "{e}"
    );
    // The app's `params` line is the app's: its file.
    std::fs::write(
        &file,
        format!("{APP}\nparams\n  Card.glyph = icon(svg \"pin.svg\")\n"),
    )
    .unwrap();
    assert_eq!(drawn(&dir), APP_STAR);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1, Astra 1: a package component's icon param may name its own
/// package's icon, drawn from the package's file.
#[test]
fn a_package_param_may_name_its_own_packages_icon() {
    let dir = reading("pkgicon-param-own", APP, "reading.star", STAR);
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    std::fs::write(
        &design,
        format!("{text}\ncomponent Map\n  params\n    glyph: icon = reading.star\n"),
    )
    .unwrap();
    let plan = build(&dir);
    let glyph = plan
        .icons
        .iter()
        .find(|i| plan.str(i.name).ends_with(".Map.glyph"))
        .unwrap_or_else(|| panic!("{:?}", plan.icons));
    assert_eq!(icon_path(&plan, glyph), PACKAGE_STAR);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1, Astra 2: a write's plan folds the package's icons beside the
/// app's design, so an app icon naming one doesn't refuse an unrelated
/// write.
#[test]
fn a_write_folds_the_packages_icons() {
    let design = format!("{APP}\nicons\n  icon.mine = reading.star\n");
    let dir = reading("pkgicon-write", &design, "reading.star", STAR);
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-token", "name": "ink", "value": "#222222"});
    let (proposed, _) = s.ask(json!({"method": "propose", "client": "one", "ops": [op]}));
    assert!(proposed.get("ok").is_some(), "{proposed}");
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&file)}, "ops": [op]}));
    assert_eq!(done["ok"]["written"], json!(["app.tamber"]), "{done}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1, Astra 4 = Grok 2: a package's SVG that links out of its
/// `assets/` (to the app's file, or a linked `assets/` into another
/// package) is refused in the build, drawn as none, and no source.
#[test]
fn a_package_svg_linked_out_of_its_package_is_refused() {
    let dir = reading("pkgicon-link-file", APP, "reading.star", STAR);
    let assets = dir.join("node_modules/@acme/reading/assets");
    std::fs::remove_file(assets.join("star.svg")).unwrap();
    std::os::unix::fs::symlink(dir.join("assets/star.svg"), assets.join("star.svg")).unwrap();
    let leaves = |dir: &Path| {
        let e = refused(dir);
        assert_eq!(e.id, "tamber-icon-svg", "{e}");
        assert!(
            e.message
                .contains("the package `@acme/reading`'s `assets/star.svg` leaves the package"),
            "{e}"
        );
        let graph = contract::source_graph(&dir.join("app.contract"));
        assert!(
            !graph.sources.iter().any(|s| s.path.ends_with("star.svg")),
            "{:?}",
            graph.sources
        );
        assert!(
            graph
                .consulted
                .iter()
                .any(|p| p.ends_with("@acme/reading/assets/star.svg")),
            "{:?}",
            graph.consulted
        );
        let mut s = Serve::start(dir, &[]);
        let (described, _) = s.ask(json!({"method": "describe"}));
        let star = described["ok"]["ir"]["icons"]
            .as_array()
            .and_then(|r| r.iter().find(|r| r["name"] == "reading.star").cloned())
            .unwrap_or_else(|| panic!("{described}"));
        assert_eq!(star["draws"], json!({"ios": "sf", "web": null}), "{star}");
    };
    leaves(&dir);
    // A linked `assets/` into another package.
    let other = dir.join("node_modules/@acme/other/assets");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("star.svg"), shape(PACKAGE_STAR)).unwrap();
    std::fs::remove_dir_all(&assets).unwrap();
    std::os::unix::fs::symlink(&other, &assets).unwrap();
    leaves(&dir);
    // A package default's SVG linked out: a write's check says so.
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    std::fs::write(
        &design,
        "tamber reading\n\ntheme\n  reading.ink = #112233\n\ncomponent Map\n  params\n    glyph: icon = icon(svg \"star.svg\")\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("node_modules/@acme/reading/index.contract"),
        "component Card\n  view\n    text \"c\"\n",
    )
    .unwrap();
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-token", "name": "ink", "value": "#222222"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&file)}, "ops": [op]}));
    assert!(
        done["error"]["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("`assets/star.svg` leaves the package `@acme/reading`")),
        "{done}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1, Astra 5: with no app design and no params, a package's design
/// still lists the SVGs its icons read.
#[test]
fn a_package_only_design_lists_its_svgs() {
    let dir = reading("pkgicon-package-only", APP, "reading.star", STAR);
    std::fs::remove_file(dir.join("app.tamber")).unwrap();
    build(&dir);
    let graph = contract::source_graph(&dir.join("app.contract"));
    assert!(
        graph.sources.iter().any(|s| s
            .path
            .ends_with("node_modules/@acme/reading/assets/star.svg")),
        "{:?}",
        graph.sources
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1, Astra 6: the local wire's watcher's `hello {paths: true}` gives
/// each package its SVGs' digest, which an edit of one moves.
#[test]
fn a_package_svg_edit_moves_its_assets_digest() {
    let dir = reading("pkgicon-assets-digest", APP, "reading.star", STAR);
    let assets = |s: &mut Serve| {
        let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
        hello["ok"]["packages"][0]["assets"].clone()
    };
    let mut s = Serve::start(&dir, &[]);
    let before = assets(&mut s);
    assert!(before.is_string(), "{before}");
    assert_eq!(assets(&mut s), before);
    std::fs::write(
        dir.join("node_modules/@acme/reading/assets/star.svg"),
        shape("M2 2h20v20H2Z"),
    )
    .unwrap();
    assert_ne!(assets(&mut s), before);
    // A client's hello carries none.
    let (hello, _) = s.ask(json!({"method": "hello"}));
    assert!(
        hello["ok"]["packages"][0].get("assets").is_none(),
        "{hello}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
    // An app with no packages lists none: no key, never null.
    let dir = app("pkgicon-no-packages", APP);
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
    assert!(hello["ok"].get("packages").is_none(), "{hello}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2, Astra 1: a vendored package's assets are its own, whichever
/// route reached it: inside another package, by that package's relative
/// `use`, and inside the app, by the app's; each beside a same-named SVG
/// of the package around it and of the app, drawn otherwise.
#[test]
fn a_vendored_packages_icon_default_reads_its_own_assets() {
    const SHELF: &str = "M5 5h1v1H5Z";
    let write = |path: &Path, text: &str| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let manifest = |name: &str| {
        format!(r#"{{"name":"{name}","version":"1.0.0","exports":{{".":"./index.contract"}}}}"#)
    };
    let card = "component Card\n  params\n    glyph: icon = \"icon(svg \\\"pin.svg\\\")\"\n  view\n    column\n      image \"symbol:param.glyph\"\n";
    let glyph = |dir: &Path| {
        let plan = build(dir);
        let g = plan
            .icons
            .iter()
            .find(|i| plan.str(i.name).ends_with(".Card.glyph"))
            .unwrap_or_else(|| panic!("{:?}", plan.icons));
        icon_path(&plan, g).to_string()
    };
    for inside_package in [true, false] {
        let dir = app(&format!("pkgicon-vendored-{inside_package}"), APP);
        std::fs::write(
            dir.join("app.json"),
            r#"{"app": {"id": "dev.test.pkgicons", "name": "Icons"}, "host": {"web": {}}}"#,
        )
        .unwrap();
        write(&dir.join("assets/pin.svg"), &shape(APP_STAR));
        let vendor = match inside_package {
            true => {
                let shelf = dir.join("node_modules/@acme/shelf");
                write(&shelf.join("package.json"), &manifest("@acme/shelf"));
                write(&shelf.join("index.contract"), "use Card from \"./vendor/reading/index.contract\"\ncomponent Shelf\n  view\n    column\n      Card()\n");
                write(&shelf.join("assets/pin.svg"), &shape(SHELF));
                write(&dir.join("app.contract"), "use Shelf from \"@acme/shelf\"\ncomponent App\n  view\n    column\n      Shelf()\n");
                shelf.join("vendor/reading")
            }
            false => {
                write(&dir.join("app.contract"), "use Card from \"./vendor/reading/index.contract\"\ncomponent App\n  view\n    column\n      Card()\n");
                dir.join("vendor/reading")
            }
        };
        write(&vendor.join("package.json"), &manifest("@acme/reading"));
        write(&vendor.join("index.contract"), card);
        write(&vendor.join("assets/pin.svg"), &shape(PACKAGE_STAR));
        assert_eq!(
            glyph(&dir),
            PACKAGE_STAR,
            "inside a package: {inside_package}"
        );
        let graph = contract::source_graph(&dir.join("app.contract"));
        let pins: Vec<_> = graph
            .sources
            .iter()
            .filter(|s| s.path.ends_with("assets/pin.svg"))
            .collect();
        assert!(
            pins.len() == 1 && pins[0].path.ends_with("vendor/reading/assets/pin.svg"),
            "{:?}",
            graph.sources
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Round 2, R2-3: a dangling link in a package's `assets/` leaves it (it
/// can't be told inside), not missing.
#[test]
fn a_dangling_link_in_a_packages_assets_leaves_it() {
    let dir = reading("pkgicon-dangling", APP, "reading.star", STAR);
    let assets = dir.join("node_modules/@acme/reading/assets");
    std::fs::remove_file(assets.join("star.svg")).unwrap();
    std::os::unix::fs::symlink(dir.join("nowhere.svg"), assets.join("star.svg")).unwrap();
    let e = refused(&dir);
    assert!(e.message.contains("leaves the package"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2, R2-4: a package with no design whose SVGs the build reads (a
/// Contract component's icon param) is listed in `watch.assets`, its
/// digest moved by an edit.
#[test]
fn a_contract_only_packages_svgs_are_watched() {
    let dir = reading("pkgicon-contract-watch", APP, "reading.star", STAR);
    let package = dir.join("node_modules/@acme/reading");
    std::fs::remove_file(package.join("reading.tamber")).unwrap();
    std::fs::write(
        package.join("index.contract"),
        "component Card\n  params\n    glyph: icon = \"icon(svg \\\"star.svg\\\")\"\n  view\n    column\n      image \"symbol:param.glyph\"\n",
    )
    .unwrap();
    let listed = |s: &mut Serve| {
        let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
        hello["ok"]["watch"]["assets"].clone()
    };
    let mut s = Serve::start(&dir, &[]);
    let before = listed(&mut s);
    assert_eq!(before[0]["package"], "@acme/reading", "{before}");
    assert_eq!(before[0]["file"], "@acme/reading/assets/", "{before}");
    std::fs::write(package.join("assets/star.svg"), shape("M2 2h20v20H2Z")).unwrap();
    let after = listed(&mut s);
    assert_ne!(after[0]["assets"], before[0]["assets"], "{after}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 3, R3-3: a package vendored in the app and reached by a relative
/// `use` owns its SVG in the graph (its origin, its manifest consulted) and
/// on the local wire (`watch.assets`, its directory watched), so an edit of
/// it moves its digest.
#[test]
fn an_app_vendored_packages_svg_is_its_packages() {
    let dir = app("pkgicon-app-vendored", APP);
    let vendor = dir.join("vendor/reading");
    std::fs::create_dir_all(vendor.join("assets")).unwrap();
    std::fs::write(
        vendor.join("package.json"),
        r#"{"name":"@acme/reading","version":"1.2.3","exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(
        vendor.join("index.contract"),
        "component Card\n  params\n    glyph: icon = \"icon(svg \\\"pin.svg\\\")\"\n  view\n    column\n      image \"symbol:param.glyph\"\n",
    )
    .unwrap();
    std::fs::write(vendor.join("assets/pin.svg"), shape(PACKAGE_STAR)).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./vendor/reading/index.contract\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    let graph = contract::source_graph(&dir.join("app.contract"));
    let pin = graph
        .sources
        .iter()
        .find(|s| s.path.ends_with("vendor/reading/assets/pin.svg"))
        .unwrap_or_else(|| panic!("{:?}", graph.sources));
    assert!(
        matches!(&pin.origin, contract::Origin::Package { name, version, .. } if name == "@acme/reading" && version == "1.2.3"),
        "{pin:?}"
    );
    assert!(
        graph
            .consulted
            .iter()
            .any(|p| p.ends_with("vendor/reading/package.json")),
        "{:?}",
        graph.consulted
    );
    let watched = |s: &mut Serve| {
        let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
        hello["ok"]["watch"].clone()
    };
    let mut s = Serve::start(&dir, &[]);
    let before = watched(&mut s);
    assert_eq!(
        before["assets"][0]["file"], "@acme/reading/assets/",
        "{before}"
    );
    assert!(
        before["roots"].as_array().is_some_and(|r| r
            .iter()
            .any(|d| d.as_str().is_some_and(|d| d.ends_with("vendor/reading")))),
        "{before}"
    );
    std::fs::write(vendor.join("assets/pin.svg"), shape("M2 2h20v20H2Z")).unwrap();
    let after = watched(&mut s);
    assert_ne!(
        after["assets"][0]["assets"], before["assets"][0]["assets"],
        "{after}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Narrow re-check, Astra RE-1: an app-vendored package whose SVG isn't
/// there yet still owns it: its manifest an input, its directory watched,
/// listed in `watch.assets`, so making the SVG moves the digest.
#[test]
fn an_app_vendored_packages_missing_svg_is_still_watched() {
    let dir = app("pkgicon-app-vendored-missing", APP);
    let vendor = dir.join("vendor/reading");
    std::fs::create_dir_all(vendor.join("assets")).unwrap();
    std::fs::write(
        vendor.join("package.json"),
        r#"{"name":"@acme/reading","version":"1.2.3","exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(
        vendor.join("index.contract"),
        "component Card\n  params\n    glyph: icon = \"icon(svg \\\"pin.svg\\\")\"\n  view\n    column\n      image \"symbol:param.glyph\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./vendor/reading/index.contract\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    let graph = contract::source_graph(&dir.join("app.contract"));
    for wanted in [
        "vendor/reading/package.json",
        "vendor/reading/assets/pin.svg",
    ] {
        assert!(
            graph.consulted.iter().any(|p| p.ends_with(wanted)),
            "{wanted}: {:?}",
            graph.consulted
        );
    }
    let watched = |s: &mut Serve| {
        let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
        hello["ok"]["watch"].clone()
    };
    let mut s = Serve::start(&dir, &[]);
    let before = watched(&mut s);
    assert_eq!(
        before["assets"][0]["file"], "@acme/reading/assets/",
        "{before}"
    );
    assert!(
        before["roots"].as_array().is_some_and(|r| r
            .iter()
            .any(|d| d.as_str().is_some_and(|d| d.ends_with("vendor/reading")))),
        "{before}"
    );
    std::fs::write(vendor.join("assets/pin.svg"), shape(PACKAGE_STAR)).unwrap();
    let after = watched(&mut s);
    assert_ne!(
        after["assets"][0]["assets"], before["assets"][0]["assets"],
        "{after}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A kept icon's path, read through `exact_runner::icon_of` (stage 14 C2:
/// the one reader of an `icons` row, which strips an even-odd icon's marker).
fn icon_path<'a>(plan: &'a exact_plan::Plan, row: &exact_plan::IconsRow) -> &'a str {
    exact_runner::icon_of(plan, row).map_or("", |icon| icon.path)
}
