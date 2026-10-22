//! LLP 1121 stage 13: a package design's icon tokens (*tamber/1.16*). A
//! package's `icons` block names its icons under its namespace; one may name
//! its own package's icons and no other; the app overrides one by full name,
//! whole; D7 keeps one only when it is read. An SVG path is relative to the
//! `assets/` of the design file it is written in, so a package's reads the
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
