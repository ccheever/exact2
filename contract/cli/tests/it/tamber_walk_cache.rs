//! The design server's walk and params across design edits (the design tool's third hands-on pass: a hand edit's
//! `changed` event took ~700 ms on the clone, three parses of every Contract source; theme2's ruling of the 26th).
//! A design-only change keeps the walk's Contract half and folds the rest again from the designs; the app's params are
//! keyed by what their load reads of the designs. (a) a design edit that moves what the graph reads (an icon's SVG, a
//! package's design) is a cold walk's graph; (b) each of what the params read (a native component's param, a motion
//! token, the design parsing, a package design) loads them again, a value they don't read keeps them; (c) after each
//! edit of a sequence, the walk's graph and the kept params are a cold walk's and load's, byte for byte, which the
//! server itself checks under `EXACT_TAMBER_WALK_VERIFY` (a mismatch panics it, and its next answer is none).
#![cfg(unix)]

use super::tamber_package_params::package;
use super::tamber_serve::{app, Serve};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The app's design: a colour (`ink`), a motion token a view reads (`motion.fast`), a native component's param
/// (`Chip.tint`), and an icon token by SVG (`mark`).
fn design(ink: &str, fast: &str, tint: &str, svg: &str) -> String {
    format!(
        "tamber t\n\ntheme\n  ink = {ink}\n  motion.fast: duration = {fast}\n\nicons\n  icon.mark = icon(sf \"star\", svg \"{svg}\")\n\ncomponent Chip\n  params\n    tint: color = {tint}\n"
    )
}

/// A package's design: a token a native param of its own reads, and an icon by SVG.
fn package_design(pin: &str, svg: &str) -> String {
    format!(
        "tamber reading\n\ntheme\n  reading.pin = {pin}\n\nicons\n  reading.star = icon(sf \"star\", svg \"{svg}\")\n\ncomponent Map\n  params\n    pin: color = reading.pin\n"
    )
}

const CARD: &str = "component Card\n  view\n    column\n      text \"card\"\n";

fn setup(tag: &str) -> PathBuf {
    let dir = app(tag, &design("#111111", "200ms", "ink", "a.svg"));
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><path d=\"M0 0h1v1z\"/></svg>";
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    for name in ["a.svg", "b.svg"] {
        std::fs::write(dir.join("assets").join(name), svg).unwrap();
    }
    package(
        &dir,
        "reading",
        CARD,
        &package_design("#112233", "star.svg"),
    );
    let assets = dir.join("node_modules/@acme/reading/assets");
    std::fs::create_dir_all(&assets).unwrap();
    for name in ["star.svg", "moon.svg"] {
        std::fs::write(assets.join(name), svg).unwrap();
    }
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column transition=\"opacity motion.fast\"\n      Card()\n",
    )
    .unwrap();
    dir
}

/// The log's lines since `from`.
fn since(log: &Path, from: usize) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .skip(from)
        .map(str::to_string)
        .collect()
}

/// After a design file was removed: a `hello {paths}` answered, and its walk lines.
fn gone(s: &mut Serve, log: &Path) -> Vec<String> {
    let from = since(log, 0).len();
    let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
    assert!(hello.get("ok").is_some(), "{hello}");
    since(log, from)
        .into_iter()
        .filter(|l| l.starts_with("walk "))
        .collect()
}

/// An edit, then the requests a hand edit's watcher and a tool make: `hello {paths}` and `describe` (which reads the
/// params). Both answer (the server did not panic on a check); the log's lines for the edit are returned.
fn edit_and_ask(s: &mut Serve, log: &Path, path: &Path, text: &str) -> Vec<String> {
    let from = since(log, 0).len();
    std::fs::write(path, text).unwrap();
    let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
    assert!(
        hello.get("ok").is_some(),
        "hello answered after writing {}: {hello}",
        path.display()
    );
    let (described, _) = s.ask(json!({"method": "describe"}));
    assert_ne!(
        described,
        Value::Null,
        "describe answered after writing {}",
        path.display()
    );
    since(log, from)
}

fn start(dir: &Path) -> (Serve, PathBuf) {
    let log = dir.join("walk.log");
    let s = Serve::start(
        dir,
        &[
            ("EXACT_TAMBER_WALK_LOG", log.to_str().unwrap()),
            ("EXACT_TAMBER_WALK_VERIFY", "1"),
        ],
    );
    (s, log)
}

#[test]
fn a_colour_edit_folds_the_designs_and_keeps_the_params() {
    let dir = setup("walk-colour");
    let (mut s, log) = start(&dir);
    let (first, _) = s.ask(json!({"method": "hello", "paths": true}));
    assert!(first.get("ok").is_some(), "{first}");
    s.ask(json!({"method": "describe"}));
    let lines = edit_and_ask(
        &mut s,
        &log,
        &dir.join("app.tamber"),
        &design("#121212", "200ms", "ink", "a.svg"),
    );
    assert!(lines.iter().any(|l| l == "walk designs"), "{lines:?}");
    assert!(!lines.iter().any(|l| l == "walk full"), "{lines:?}");
    assert!(!lines.iter().any(|l| l == "params loaded"), "{lines:?}");
    // A Contract source's edit walks again whole.
    let lines = edit_and_ask(
        &mut s,
        &log,
        &dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    row transition=\"opacity motion.fast\"\n      Card()\n",
    );
    assert!(lines.iter().any(|l| l == "walk full"), "{lines:?}");
}

#[test]
fn each_input_the_params_read_loads_them_again() {
    let dir = setup("walk-params");
    let (mut s, log) = start(&dir);
    s.ask(json!({"method": "hello", "paths": true}));
    s.ask(json!({"method": "describe"}));
    let app_design = dir.join("app.tamber");
    let package = dir.join("node_modules/@acme/reading/reading.tamber");
    let cases: [(&str, &Path, String); 5] = [
        (
            "a native component's param",
            &app_design,
            design("#111111", "200ms", "#00ff00", "a.svg"),
        ),
        (
            "a motion token's value",
            &app_design,
            design("#111111", "250ms", "#00ff00", "a.svg"),
        ),
        (
            "the design not parsing",
            &app_design,
            "tamber t\n\ntheme\n  ink = \n".to_string(),
        ),
        (
            "the design parsing again",
            &app_design,
            design("#111111", "250ms", "#00ff00", "a.svg"),
        ),
        (
            "a package token its native param reads",
            &package,
            package_design("#445566", "star.svg"),
        ),
    ];
    for (what, path, text) in cases {
        let lines = edit_and_ask(&mut s, &log, path, &text);
        assert!(
            lines.iter().any(|l| l == "params loaded"),
            "{what}: {lines:?}"
        );
    }
    // And a colour no param or motion token reads keeps them.
    let lines = edit_and_ask(
        &mut s,
        &log,
        &app_design,
        &design("#131313", "250ms", "#00ff00", "a.svg"),
    );
    assert!(
        !lines.iter().any(|l| l == "params loaded"),
        "a colour: {lines:?}"
    );
}

/// (a) and (c): edits that move what the graph reads (the app's icon's SVG, a package's icon's SVG, a package's design
/// gone and back), each checked against a cold walk by the server. What the walk reads after each is seen by an edit of
/// the SVG: the one now named walks again (a source of the build), the one no longer named is nothing to it.
#[test]
fn a_design_edit_that_moves_the_graph_is_a_cold_walks() {
    let dir = setup("walk-graph");
    let (mut s, log) = start(&dir);
    s.ask(json!({"method": "hello", "paths": true}));
    let app_design = dir.join("app.tamber");
    let package = dir.join("node_modules/@acme/reading/reading.tamber");
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 2 2\"><path d=\"M0 0h2v2z\"/></svg>";
    let walked = |s: &mut Serve, path: PathBuf| -> Vec<String> {
        let from = since(&log, 0).len();
        std::fs::write(&path, svg).unwrap();
        let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
        assert!(hello.get("ok").is_some(), "{hello}");
        since(&log, from)
            .into_iter()
            .filter(|l| l.starts_with("walk "))
            .collect()
    };
    // The app's icon moved to b.svg: b.svg is read, a.svg no longer.
    let lines = edit_and_ask(
        &mut s,
        &log,
        &app_design,
        &design("#111111", "200ms", "ink", "b.svg"),
    );
    assert!(lines.iter().any(|l| l == "walk designs"), "{lines:?}");
    assert_eq!(
        walked(&mut s, dir.join("assets/b.svg")),
        ["walk full"],
        "the SVG now named"
    );
    assert_eq!(
        walked(&mut s, dir.join("assets/a.svg")),
        Vec::<String>::new(),
        "the SVG no longer named"
    );
    // A package's icon moved to moon.svg: the same.
    let assets = dir.join("node_modules/@acme/reading/assets");
    let lines = edit_and_ask(
        &mut s,
        &log,
        &package,
        &package_design("#112233", "moon.svg"),
    );
    assert!(lines.iter().any(|l| l == "walk designs"), "{lines:?}");
    assert_eq!(
        walked(&mut s, assets.join("moon.svg")),
        ["walk full"],
        "the package's SVG now named"
    );
    assert_eq!(
        walked(&mut s, assets.join("star.svg")),
        Vec::<String>::new(),
        "the package's SVG no longer named"
    );
    // The package's design gone, and back: each folded from the designs, each a cold walk's.
    std::fs::remove_file(&package).unwrap();
    assert_eq!(
        gone(&mut s, &log),
        ["walk designs"],
        "a design file gone is folded from the designs"
    );
    let lines = edit_and_ask(
        &mut s,
        &log,
        &package,
        &package_design("#112233", "star.svg"),
    );
    assert!(lines.iter().any(|l| l == "walk designs"), "{lines:?}");
}

/// Round 1's cases (Astra): the default design deleted and made again is a design come (the walk keeps its path there
/// or not); a private package's design gone and back moves which library files are read, folded and checked cold.
#[test]
fn the_default_design_made_again_and_a_private_packages_design_are_cold_walks() {
    let dir = setup("walk-again");
    let manifest = dir.join("node_modules/@acme/reading/package.json");
    std::fs::write(
        &manifest,
        r#"{"name":"@acme/reading","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    let (mut s, log) = start(&dir);
    s.ask(json!({"method": "hello", "paths": true}));
    let app_design = dir.join("app.tamber");
    std::fs::remove_file(&app_design).unwrap();
    assert_eq!(
        gone(&mut s, &log),
        ["walk designs"],
        "a design file gone is folded from the designs"
    );
    let lines = edit_and_ask(
        &mut s,
        &log,
        &app_design,
        &design("#111111", "200ms", "ink", "a.svg"),
    );
    assert!(
        lines.iter().any(|l| l == "walk designs"),
        "the design made again: {lines:?}"
    );
    let package = dir.join("node_modules/@acme/reading/reading.tamber");
    std::fs::remove_file(&package).unwrap();
    assert_eq!(
        gone(&mut s, &log),
        ["walk designs"],
        "a design file gone is folded from the designs"
    );
    let lines = edit_and_ask(
        &mut s,
        &log,
        &package,
        &package_design("#112233", "star.svg"),
    );
    assert!(
        lines.iter().any(|l| l == "walk designs"),
        "the private package's design back: {lines:?}"
    );
}

/// Round 1's case (Astra): `app.json` naming a package's design file as the app's: a colour edit there is in the
/// params whole (the package's text), so they load again, checked cold.
#[test]
fn an_app_design_that_is_a_packages_design_keys_the_params_by_its_text() {
    let dir = setup("walk-shared");
    std::fs::write(
        dir.join("app.json"),
        r#"{"tamber": "node_modules/@acme/reading/reading.tamber"}"#,
    )
    .unwrap();
    let (mut s, log) = start(&dir);
    s.ask(json!({"method": "hello", "paths": true}));
    s.ask(json!({"method": "describe"}));
    let package = dir.join("node_modules/@acme/reading/reading.tamber");
    let lines = edit_and_ask(
        &mut s,
        &log,
        &package,
        &package_design("#112233", "star.svg").replace(
            "reading.pin = #112233",
            "reading.pin = #112233\n  reading.ink = #abcdef",
        ),
    );
    assert!(lines.iter().any(|l| l == "params loaded"), "{lines:?}");
}

/// Round 2's case (Astra): with no app design, a package's design replaced by a link out of its package with the same
/// bytes fails the load (unreadable) where it loaded: the params load again, never kept, checked cold.
#[test]
fn a_package_design_linked_out_with_the_same_bytes_and_no_app_design_loads_the_params_again() {
    let dir = setup("walk-linked");
    // (A package design naming no SVG: the graph then reads the same files either way, so only the read's failure
    // moves the params' key.)
    let package = dir.join("node_modules/@acme/reading/reading.tamber");
    std::fs::write(
        &package,
        "tamber reading\n\ntheme\n  reading.pin = #112233\n\ncomponent Map\n  params\n    pin: color = reading.pin\n",
    )
    .unwrap();
    let (mut s, log) = start(&dir);
    s.ask(json!({"method": "hello", "paths": true}));
    s.ask(json!({"method": "describe"}));
    // The app's design gone while the server runs (it starts only with one).
    std::fs::remove_file(dir.join("app.tamber")).unwrap();
    assert_eq!(gone(&mut s, &log), ["walk designs"]);
    s.ask(json!({"method": "describe"}));
    let outside = dir.join("outside.tamber");
    std::fs::copy(&package, &outside).unwrap();
    std::fs::remove_file(&package).unwrap();
    std::os::unix::fs::symlink(&outside, &package).unwrap();
    let from = since(&log, 0).len();
    let (hello, _) = s.ask(json!({"method": "hello", "paths": true}));
    assert!(hello.get("ok").is_some(), "{hello}");
    let (described, _) = s.ask(json!({"method": "describe"}));
    assert_ne!(described, Value::Null, "describe answered");
    let lines = since(&log, from);
    assert!(lines.iter().any(|l| l == "params loaded"), "{lines:?}");
}
