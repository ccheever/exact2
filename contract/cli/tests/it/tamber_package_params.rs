//! LLP 1121 stage 12 B1: a package component's params (D15). A package's
//! Contract component and a component its design declares (`component …
//! params`, no longer `tamber-package-block`) list their params in
//! `describe` under the package's identity (`@acme/reading/index.contract#
//! Card`, `@acme/reading/reading.tamber#Map`). The app sets one by full or
//! short name in its own design, never the package's files; the build keeps
//! it; `uses` lists a Contract component's sites in the package's file.
#![cfg(unix)]

use super::tamber_serve::{app, digest, Serve};
use serde_json::{json, Value};

pub(crate) const CARD: &str = "component Card\n  params\n    inset: length = \"reading.gap\"\n  view\n    column padding=\"param.inset\" testId=\"card\"\n      text \"c\"\n";

/// A package `@acme/<ns>` in the app at `dir`: its entry `index.contract`
/// (`contract`) and its design `<ns>.tamber`.
pub(crate) fn package(dir: &std::path::Path, ns: &str, contract: &str, design: &str) {
    let at = dir.join(format!("node_modules/@acme/{ns}"));
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(
        at.join("package.json"),
        format!(
            r#"{{"name":"@acme/{ns}","version":"1.0.0","exports":{{".":"./index.contract"}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(at.join("index.contract"), contract).unwrap();
    std::fs::write(at.join(format!("{ns}.tamber")), design).unwrap();
}

/// The app at `tag`: `@acme/reading`'s `Card` used once, its design
/// declaring `component Map` with a param defaulting to a package token.
pub(crate) fn reading(tag: &str) -> std::path::PathBuf {
    let dir = app(tag, "tamber t\n\ntheme\n  ink = #111111\n");
    package(
        &dir,
        "reading",
        CARD,
        "tamber reading\n\ntheme\n  reading.gap: length = space.md\n  reading.pin = #112233\n\ncomponent Map\n  params\n    pin: color = reading.pin\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    dir
}

fn params(described: &Value) -> Vec<Value> {
    described["ok"]["ir"]["components"]
        .as_array()
        .unwrap_or_else(|| panic!("{described}"))
        .iter()
        .flat_map(|c| c["params"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn describe_lists_a_packages_params_under_its_identity() {
    let dir = reading("pkg-params-describe");
    let mut s = Serve::start(&dir, &[]);
    let (described, _) = s.ask(json!({"method": "describe"}));
    let ids: Vec<&str> = described["ok"]["ir"]["components"]
        .as_array()
        .unwrap_or_else(|| panic!("{described}"))
        .iter()
        .map(|c| c["identity"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "@acme/reading/index.contract#Card",
            "@acme/reading/reading.tamber#Map"
        ],
        "{described}"
    );
    let all = params(&described);
    let names: Vec<&str> = all.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "@acme/reading/index.contract#Card.inset",
            "@acme/reading/reading.tamber#Map.pin"
        ]
    );
    assert_eq!(all[0]["short"], "Card.inset");
    assert_eq!(all[1]["short"], "Map.pin");
    // Their defaults name the package's tokens, folded.
    assert_eq!(all[1]["default"], "reading.pin", "{}", all[1]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_and_unset_param_write_the_apps_line_alone() {
    let dir = reading("pkg-params-set");
    let file = dir.join("app.tamber");
    let package_files: Vec<(std::path::PathBuf, String)> = ["index.contract", "reading.tamber"]
        .iter()
        .map(|f| {
            let p = dir.join("node_modules/@acme/reading").join(f);
            let text = std::fs::read_to_string(&p).unwrap();
            (p, text)
        })
        .collect();
    let mut s = Serve::start(&dir, &[]);
    for (name, value, line) in [
        (
            "@acme/reading/index.contract#Card.inset",
            "20px",
            "@acme/reading/index.contract#Card.inset = 20px",
        ),
        ("Map.pin", "#abcdef", "Map.pin = #abcdef"),
    ] {
        let op = json!({"op": "set-param", "name": name, "value": value});
        let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [op]}));
        assert_eq!(done["ok"]["written"], json!(["app.tamber"]), "{done}");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(&format!("\n  {line}\n")), "{text}");
    }
    for (p, text) in &package_files {
        assert_eq!(
            &std::fs::read_to_string(p).unwrap(),
            text,
            "{} untouched",
            p.display()
        );
    }
    // The build keeps the app's value (D7): the plan reads it.
    let (resolved, _) = s.ask(json!({"method": "resolve", "token": "Card.inset"}));
    assert_eq!(resolved["ok"]["tuples"][0]["value"], "20px", "{resolved}");
    let (resolved, _) =
        s.ask(json!({"method": "resolve", "token": "@acme/reading/reading.tamber#Map.pin"}));
    assert_eq!(
        resolved["ok"]["tuples"][0]["value"], "#abcdefff",
        "{resolved}"
    );
    let plan = format!(
        "{:?}",
        contract::compile_path(&dir.join("app.contract")).unwrap()
    );
    assert!(plan.contains("20px"), "the plan keeps the set value");
    assert!(
        plan.contains("abcdef"),
        "and the design component's (a root, D15)"
    );
    // `uses`: the Contract component's sites, in the package's file.
    let (uses, _) = s.ask(json!({"method": "uses", "token": "Card.inset"}));
    let sites = uses["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{uses}"));
    assert!(!sites.is_empty(), "{uses}");
    assert!(
        sites.iter().all(|x| x["file"]
            .as_str()
            .unwrap()
            .ends_with("node_modules/@acme/reading/index.contract")),
        "{uses}"
    );
    // unset-param by short name removes the app's line.
    for name in ["Card.inset", "@acme/reading/reading.tamber#Map.pin"] {
        let op = json!({"op": "unset-param", "name": name});
        let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [op]}));
        assert!(done.get("ok").is_some(), "{done}");
    }
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("params"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A package's design declares its components' params; their values are
/// the app's to set: a `params` line in a package's design is refused.
#[test]
fn a_package_design_setting_a_param_is_refused() {
    let dir = reading("pkg-params-value");
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    std::fs::write(&design, format!("{text}\nparams\n  Map.pin = #000000\n")).unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).expect_err("refused");
    assert_eq!(e.id, "tamber-package-block", "{e}");
    assert!(e.message.contains("the app's to set"), "{e}");
    // A param's own refusal is said in the package's design, at its line.
    std::fs::write(
        &design,
        text.replace("pin: color = reading.pin", "pin: color = param.nothing"),
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).expect_err("refused");
    assert_eq!(e.id, "tamber-param-unknown", "{e}");
    let shown = format!("{e}");
    assert!(shown.contains("reading.tamber:9:"), "{shown}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two packages declaring a component of one name: each its own identity,
/// no token collision; a short name is ambiguous, a full one is not.
#[test]
fn two_packages_components_of_one_name_stay_apart() {
    let dir = reading("pkg-params-two");
    package(
        &dir,
        "maps",
        "component Pin\n  view\n    text \"p\"\n",
        "tamber maps\n\ntheme\n  maps.pin = #223344\n\ncomponent Map\n  params\n    pin: color = maps.pin\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nuse Pin from \"@acme/maps\"\ncomponent App\n  view\n    column\n      Card()\n      Pin()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let mut s = Serve::start(&dir, &[]);
    let (described, _) = s.ask(json!({"method": "describe"}));
    let names: Vec<String> = params(&described)
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect();
    assert!(
        names.contains(&"@acme/reading/reading.tamber#Map.pin".to_string()),
        "{names:?}"
    );
    assert!(
        names.contains(&"@acme/maps/maps.tamber#Map.pin".to_string()),
        "{names:?}"
    );
    let file = dir.join("app.tamber");
    let op = |name: &str| json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [{"op": "set-param", "name": name, "value": "#ffffff"}]});
    let (short, _) = s.ask(op("Map.pin"));
    assert_eq!(short["error"]["id"], "tamber-param-ambiguous", "{short}");
    // describe advertises no short name that `set-param` would refuse
    // (round 1, Astra 1 and Grok 1); `Card.inset` stays short.
    for p in params(&described) {
        let name = p["name"].as_str().unwrap();
        if name.ends_with("#Map.pin") {
            assert!(p["short"].is_null(), "{p}");
        } else {
            assert!(!p["short"].is_null(), "{p}");
        }
    }
    let (full, _) = s.ask(op("@acme/maps/maps.tamber#Map.pin"));
    assert!(full.get("ok").is_some(), "{full}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1 (Astra 1): an app's own design component and a package's of one
/// name: neither is advertised by its short name.
#[test]
fn an_apps_component_and_a_packages_of_one_name_have_no_short_name() {
    let dir = reading("pkg-params-app-map");
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n\ncomponent Map\n  params\n    pin: color = ink\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (described, _) = s.ask(json!({"method": "describe"}));
    let maps: Vec<Value> = params(&described)
        .into_iter()
        .filter(|p| p["name"].as_str().unwrap().ends_with("#Map.pin"))
        .collect();
    assert_eq!(maps.len(), 2, "{described}");
    assert!(maps.iter().all(|p| p["short"].is_null()), "{maps:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1 (Astra 2): a param a package's design comes to declare after the
/// server read the app is a root of retention at once: overriding it over
/// an axis is a rebuild, as a fresh server says, never `live`.
#[test]
fn a_package_param_added_after_priming_is_retained() {
    let dir = reading("pkg-params-prime");
    let file = dir.join("app.tamber");
    std::fs::write(
        &file,
        "tamber t\n\naxes\n  mode = a | b\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-token", "name": "ink", "value": "#222222"});
    let (primed, _) = s.ask(json!({"method": "propose", "ops": [op]}));
    assert!(primed.get("ok").is_some(), "{primed}");
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    std::fs::write(&design, format!("{text}    size: length = 4px\n")).unwrap();
    let vary = json!([
        {"op": "set-param", "name": "Map.size", "value": "6px"},
        {"op": "set-param", "name": "Map.size", "when": [{"axis": "mode", "value": "b"}], "value": "8px"}
    ]);
    let (after, _) = s.ask(json!({"method": "propose", "ops": vary}));
    // One server holds the design at a time.
    drop(s);
    let mut fresh = Serve::start(&dir, &[]);
    let (expected, _) = fresh.ask(json!({"method": "propose", "ops": vary}));
    assert!(expected.get("ok").is_some(), "{expected}");
    assert_eq!(
        after["ok"]["class"], expected["ok"]["class"],
        "{after} {expected}"
    );
    assert_eq!(expected["ok"]["class"], "rebuild", "{expected}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1 (Grok 2): a package param's default names its package's own
/// tokens, roles, base names and its component's params only, and no dial:
/// another package's token or an app token is refused at the default, in
/// the package's file. A value the app sets names the app's tokens freely.
#[test]
fn a_package_params_default_names_its_packages_own() {
    let dir = reading("pkg-params-refs");
    package(
        &dir,
        "maps",
        "component Pin\n  view\n    text \"p\"\n",
        "tamber maps\n\ntheme\n  maps.pin = #223344\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nuse Pin from \"@acme/maps\"\ncomponent App\n  view\n    column\n      Card()\n      Pin()\n",
    )
    .unwrap();
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    for (default, said) in [("maps.pin", "maps.pin"), ("ink", "ink")] {
        std::fs::write(
            &design,
            text.replace(
                "pin: color = reading.pin",
                &format!("pin: color = {default}"),
            ),
        )
        .unwrap();
        let e = contract::compile_path(&dir.join("app.contract")).expect_err("refused");
        assert_eq!(e.id, "tamber-unknown-token", "{default}: {e}");
        let shown = format!("{e}");
        assert!(
            shown.contains("reading.tamber:9:") && shown.contains(said),
            "{default}: {shown}"
        );
    }
    // Its own token, and a sibling param, are its own.
    std::fs::write(
        &design,
        text.replace(
            "    pin: color = reading.pin\n",
            "    pin: color = reading.pin\n    edge: color = param.pin\n",
        ),
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    // The app sets the param to its own token: the app's value.
    std::fs::write(&design, &text).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n\nparams\n  Map.pin = ink\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2 (Astra 1): what a package param's default reads moves with the
/// package's design, whole: the server's cached retention follows it, as a
/// fresh server's does.
#[test]
fn a_package_tokens_edit_moves_what_its_params_keep() {
    let dir = reading("pkg-params-chain");
    let file = dir.join("app.tamber");
    std::fs::write(
        &file,
        "tamber t\n\naxes\n  mode = a | b\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let chain = |to: &str| {
        format!("tamber reading\n\ntheme\n  reading.gap: length = space.md\n  reading.old = #111111\n  reading.new = #222222\n  reading.pin = {to}\n\ncomponent Map\n  params\n    pin: color = reading.pin\n")
    };
    std::fs::write(&design, chain("reading.old")).unwrap();
    let vary = |name: &str| {
        json!({"method": "propose", "ops": [
            {"op": "set-token", "name": name, "value": "#333333"},
            {"op": "set-token", "name": name, "when": [{"axis": "mode", "value": "b"}], "value": "#444444"}
        ]})
    };
    let mut s = Serve::start(&dir, &[]);
    // Primed on the chain through `reading.old`, which the param keeps.
    let (before, _) = s.ask(vary("reading.old"));
    assert_eq!(before["ok"]["class"], "rebuild", "{before}");
    std::fs::write(&design, chain("reading.new")).unwrap();
    let primed: Vec<Value> = ["reading.old", "reading.new"]
        .iter()
        .map(|n| s.ask(vary(n)).0)
        .collect();
    drop(s);
    let mut fresh = Serve::start(&dir, &[]);
    for (i, n) in ["reading.old", "reading.new"].iter().enumerate() {
        let (expected, _) = fresh.ask(vary(n));
        assert!(expected.get("ok").is_some(), "{expected}");
        assert_eq!(
            primed[i]["ok"]["class"], expected["ok"]["class"],
            "{n}: {} {expected}",
            primed[i]
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2 (Astra 2, Grok 1 and 2): a default written as a whole alias is
/// judged as written: the app's profile colour (folded as `transparent`,
/// naming none) and the app's icon are refused at the default in the
/// package's file; a literal icon is the package's own.
#[test]
fn a_package_params_written_alias_is_judged_as_written() {
    let dir = reading("pkg-params-written");
    std::fs::write(
        dir.join("app.contract"),
        "color-profile --brand src=\"assets/brand.icc\"\n\nuse Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = color(--brand 0.1 0.2 0.3 0.4)\n\nicons\n  icon.app = icon(sf \"star\")\n",
    )
    .unwrap();
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    for (param, said) in [
        ("pin: color = ink", "ink"),
        ("glyph: icon = icon.app", "icon.app"),
    ] {
        std::fs::write(&design, text.replace("pin: color = reading.pin", param)).unwrap();
        let e = contract::compile_path(&dir.join("app.contract")).expect_err("refused");
        assert_eq!(e.id, "tamber-unknown-token", "{param}: {e}");
        let shown = format!("{e}");
        assert!(
            shown.contains("reading.tamber:9:") && shown.contains(said),
            "{param}: {shown}"
        );
    }
    std::fs::write(
        &design,
        text.replace(
            "pin: color = reading.pin",
            "glyph: icon = icon(sf \"heart\")",
        ),
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2 (Grok 3): the app's `params` line sets a package's param only by
/// a name only it answers; an ambiguous short name sets none, so the
/// default's own refusal is still said beside `tamber-param-ambiguous`.
#[test]
fn an_ambiguous_short_name_sets_no_package_param() {
    let dir = reading("pkg-params-ambiguous");
    package(
        &dir,
        "maps",
        "component Pin\n  view\n    text \"p\"\n",
        "tamber maps\n\ntheme\n  maps.pin = #223344\n\ncomponent Map\n  params\n    pin: color = maps.pin\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\nuse Pin from \"@acme/maps\"\ncomponent App\n  view\n    column\n      Card()\n      Pin()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n\nparams\n  Map.pin = ink\n",
    )
    .unwrap();
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    std::fs::write(
        &design,
        text.replace("pin: color = reading.pin", "pin: color = ink"),
    )
    .unwrap();
    let Err(errors) = contract::compile_path_all(&dir.join("app.contract"), false) else {
        panic!("refused")
    };
    let ids: Vec<String> = errors.iter().map(|e| e.id.to_string()).collect();
    assert!(ids.iter().any(|i| i == "tamber-unknown-token"), "{ids:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 3 (Astra): a package design's icon param reads its SVG as the
/// build does: one there is a source (a watcher rebuilds on it), one not
/// there yet is looked for, so making it rebuilds; with no app design file
/// too. Stage 13 (theme2's Q1): a default written in the package's design
/// reads the PACKAGE's `assets/`, never the app's same-named file.
#[test]
fn a_package_icon_params_svg_is_a_source() {
    let dir = reading("pkg-params-svg");
    let _ = std::fs::remove_file(dir.join("app.tamber"));
    let package = dir.join("node_modules/@acme/reading");
    let design = package.join("reading.tamber");
    let text = std::fs::read_to_string(&design).unwrap();
    std::fs::write(&design, format!("{text}    glyph: icon = icon(svg \"pin.svg\")\n    other: icon = icon(svg \"later.svg\")\n")).unwrap();
    let svg = |d: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><path d=\"{d}\"/></svg>"
        )
    };
    for (at, d) in [
        (package.join("assets"), "M0 0h1v1H0Z"),
        (dir.join("assets"), "M0 0h9"),
    ] {
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join("pin.svg"), svg(d)).unwrap();
    }
    let graph = contract::source_graph(&dir.join("app.contract"));
    let pin = |s: &&contract::Source| s.path.ends_with("assets/pin.svg");
    let pins: Vec<_> = graph.sources.iter().filter(pin).collect();
    assert!(
        pins.len() == 1
            && pins[0]
                .path
                .ends_with("node_modules/@acme/reading/assets/pin.svg"),
        "{:?}",
        graph.sources
    );
    assert!(
        matches!(&pins[0].origin, contract::Origin::Package { name, .. } if name == "@acme/reading"),
        "{:?}",
        pins[0]
    );
    assert!(
        graph
            .consulted
            .iter()
            .any(|p| p.ends_with("node_modules/@acme/reading/assets/later.svg")),
        "{:?}",
        graph.consulted
    );
    // The build draws the package's file, never the app's of that name.
    std::fs::write(package.join("assets/later.svg"), svg("M0 0h1")).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let glyph = plan
        .icons
        .iter()
        .find(|i| plan.str(i.name).ends_with(".Map.glyph"))
        .unwrap_or_else(|| panic!("{:?}", plan.icons));
    assert_eq!(icon_path(&plan, glyph), "M0 0h1v1H0Z");
    // The app's `params` line is written in the app's design: the app's file.
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n\nparams\n  Map.glyph = icon(svg \"pin.svg\")\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let glyph = plan
        .icons
        .iter()
        .find(|i| plan.str(i.name).ends_with(".Map.glyph"))
        .unwrap();
    assert_eq!(icon_path(&plan, glyph), "M0 0h9");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 3 (Grok 1): the app's design whole keys what the server keeps of
/// retention. An app override of a package's token moves what a param's
/// default reaches without moving a token's name: priming on the old chain
/// and moving the override, an override of the old token varying over an
/// axis is classed as a fresh server classes it. And once the app sets a
/// package's param to a token of its own, that token is kept.
#[test]
fn the_apps_design_whole_keys_what_its_params_keep() {
    let dir = reading("pkg-params-value-root");
    let file = dir.join("app.tamber");
    let design = dir.join("node_modules/@acme/reading/reading.tamber");
    std::fs::write(&design, "tamber reading\n\ntheme\n  reading.gap: length = space.md\n  reading.old = #111111\n  reading.new = #222222\n  reading.pin = reading.old\n\ncomponent Map\n  params\n    pin: color = reading.pin\n").unwrap();
    std::fs::write(&file, "tamber t\n\naxes\n  mode = a | b\n\ntheme\n  ink2 = #222222\n  reading.pin = reading.old\n").unwrap();
    let vary = |name: &str| {
        json!({"method": "propose", "ops": [
            {"op": "set-token", "name": name, "value": "#333333"},
            {"op": "set-token", "name": name, "when": [{"axis": "mode", "value": "b"}], "value": "#444444"}
        ]})
    };
    let mut s = Serve::start(&dir, &[]);
    let (before, _) = s.ask(vary("reading.old"));
    assert_eq!(before["ok"]["class"], "rebuild", "{before}");
    let (moved, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [{"op": "set-token", "name": "reading.pin", "value": "reading.new"}]}));
    assert!(moved.get("ok").is_some(), "{moved}");
    // The override's move alone: the chain now reaches `reading.new`, not
    // `reading.old` (a fresh server's classes, held below too).
    let (old, _) = s.ask(vary("reading.old"));
    let (new, _) = s.ask(vary("reading.new"));
    assert_eq!(
        (&old["ok"]["class"], &new["ok"]["class"]),
        (&json!("live"), &json!("rebuild")),
        "{old} {new}"
    );
    let (set, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [{"op": "set-param", "name": "Map.pin", "value": "ink2"}]}));
    assert!(set.get("ok").is_some(), "{set}");
    let ink2 = json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink2", "when": [{"axis": "mode", "value": "b"}], "value": "#444444"}]});
    let primed = [
        s.ask(vary("reading.old")).0,
        s.ask(vary("reading.new")).0,
        s.ask(ink2.clone()).0,
    ];
    drop(s);
    let mut fresh = Serve::start(&dir, &[]);
    let expected = [
        fresh.ask(vary("reading.old")).0,
        fresh.ask(vary("reading.new")).0,
        fresh.ask(ink2).0,
    ];
    for (p, e) in primed.iter().zip(&expected) {
        assert!(e.get("ok").is_some(), "{e}");
        assert_eq!(p["ok"]["class"], e["ok"]["class"], "{p} {e}");
    }
    assert_eq!(expected[0]["ok"]["class"], "live", "{}", expected[0]);
    // `Map.pin` now reads `ink2`: `reading.new` is no longer kept either.
    assert_eq!(expected[1]["ok"]["class"], "live", "{}", expected[1]);
    assert_eq!(expected[2]["ok"]["class"], "rebuild", "{}", expected[2]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// theme2's sign-off of B1: the server's cached retention is keyed by what
/// can move it, parsed. A value edit that names nothing (a hex, a length)
/// reuses it, where a whole compile of a large app costs seconds; an alias
/// moved, or a param set, builds it again. Counted by the server's own
/// hook (`EXACT_TAMBER_RETENTION_LOG`, a line a build), not timed.
#[test]
fn a_value_edit_naming_nothing_keeps_the_cached_retention() {
    let dir = reading("pkg-params-cache-hit");
    let file = dir.join("app.tamber");
    let log = dir.join("retention.log");
    let builds = || std::fs::read_to_string(&log).map_or(0, |t| t.lines().count());
    std::fs::write(&file, "tamber t\n\ntheme\n  ink = #111111\n  ink2 = #222222\n  gap = 8px\n  reading.pin = reading.pin.base\n  reading.pin.base = #333333\n").unwrap();
    let mut s = Serve::start(
        &dir,
        &[("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())],
    );
    let propose = |s: &mut Serve| {
        let (r, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
        assert!(r.get("ok").is_some(), "{r}");
    };
    propose(&mut s);
    assert_eq!(builds(), 1);
    // Values that name nothing: a hex, a length, the colour an alias's target holds.
    for (from, to) in [
        ("#222222", "#232323"),
        ("8px", "10px"),
        ("#333333", "#343434"),
    ] {
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, text.replacen(from, to, 1)).unwrap();
        propose(&mut s);
        assert_eq!(builds(), 1, "{from} → {to} is a value edit: kept");
    }
    // An alias moved: what a token names is retention's.
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        text.replacen("reading.pin = reading.pin.base", "reading.pin = ink2", 1),
    )
    .unwrap();
    propose(&mut s);
    assert_eq!(builds(), 2, "an alias moved");
    // A param set to a token, and a colour turned into a length.
    let (set, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [{"op": "set-param", "name": "Map.pin", "value": "ink"}]}));
    assert!(set.get("ok").is_some(), "{set}");
    propose(&mut s);
    assert_eq!(builds(), 3, "a param set");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replacen("ink2 = #232323", "ink2 = 12px", 1)).unwrap();
    s.ask(
        json!({"method": "propose", "ops": [{"op": "set-token", "name": "gap", "value": "11px"}]}),
    );
    assert_eq!(builds(), 4, "a colour turned into a length");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The retention key's re-check (Astra): names compared as the checker
/// resolves them. A param set to `pick`, which names a role override in
/// another case (`CANVAS`, the design's `Canvas = old`), moved to
/// `CANVASTEXT` (`CanvasText = fresh`): retention follows, as a fresh
/// server's does, and the server builds it again.
#[test]
fn a_role_alias_moved_by_its_case_is_followed() {
    let dir = reading("pkg-params-role-case");
    let file = dir.join("app.tamber");
    let log = dir.join("retention.log");
    let builds = || std::fs::read_to_string(&log).map_or(0, |t| t.lines().count());
    let design = |to: &str| {
        format!("tamber t\n\naxes\n  mode = a | b\n\ntheme\n  old = #111111\n  fresh = #222222\n  Canvas = old\n  CanvasText = fresh\n  pick = {to}\n\nparams\n  Map.pin = pick\n")
    };
    std::fs::write(&file, design("CANVAS")).unwrap();
    // (A token under a role can't vary by an axis, the role would: a value edit, its whole answer compared.)
    let vary = |name: &str| json!({"method": "propose", "ops": [{"op": "set-token", "name": name, "value": "#444444"}]});
    let mut s = Serve::start(
        &dir,
        &[("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())],
    );
    let (before, _) = s.ask(vary("old"));
    assert!(before.get("ok").is_some(), "{before}");
    assert_eq!(builds(), 1);
    std::fs::write(&file, design("CANVASTEXT")).unwrap();
    let primed = [s.ask(vary("old")).0, s.ask(vary("fresh")).0];
    assert_eq!(builds(), 2, "the alias moved by case alone");
    drop(s);
    let mut fresh = Serve::start(&dir, &[]);
    for (p, name) in primed.iter().zip(["old", "fresh"]) {
        let (e, _) = fresh.ask(vary(name));
        assert!(e.get("ok").is_some(), "{e}");
        let (mut p, mut e) = (p["ok"].clone(), e["ok"].clone());
        for v in [&mut p, &mut e] {
            v.as_object_mut().unwrap().remove("files");
        }
        assert_eq!(p, e, "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The re-check (Astra): each key row says its section, so a name moved
/// from `icons` to `theme` builds again; and a token's `pair=` naming
/// another token keys too.
#[test]
fn a_name_moved_between_sections_or_a_pair_moved_builds_again() {
    let dir = reading("pkg-params-sections");
    let file = dir.join("app.tamber");
    let log = dir.join("retention.log");
    let builds = || std::fs::read_to_string(&log).map_or(0, |t| t.lines().count());
    std::fs::write(&file, "tamber t\n\ntheme\n  ink = #111111\n  paper = #ffffff\n  ground = #eeeeee\n  text = #000000  pair=paper\n\nicons\n  icon.brand = icon(sf \"star\")\n").unwrap();
    let mut s = Serve::start(
        &dir,
        &[("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())],
    );
    let propose = |s: &mut Serve| {
        s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]})).0
    };
    assert!(propose(&mut s).get("ok").is_some());
    assert_eq!(builds(), 1);
    std::fs::write(&file, "tamber t\n\ntheme\n  ink = #111111\n  paper = #ffffff\n  ground = #eeeeee\n  text = #000000  pair=ground\n\nicons\n  icon.brand = icon(sf \"star\")\n").unwrap();
    propose(&mut s);
    assert_eq!(builds(), 2, "a pair moved");
    std::fs::write(&file, "tamber t\n\ntheme\n  ink = #111111\n  paper = #ffffff\n  ground = #eeeeee\n  text = #000000  pair=ground\n  icon.brand = icon(sf \"star\")\n").unwrap();
    propose(&mut s);
    assert_eq!(builds(), 3, "icon.brand moved from icons to theme");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The key's second re-check (Astra): a reference is found folded but keyed
/// as written, so two tokens differing only in case (`Foo`, `foo`) stay two:
/// a param's chain moved from one to the other builds again, and a `when`
/// on the one no longer read is as a fresh server classes it.
#[test]
fn two_tokens_of_one_name_in_two_cases_stay_two() {
    let dir = reading("pkg-params-two-cases");
    let file = dir.join("app.tamber");
    let log = dir.join("retention.log");
    let builds = || std::fs::read_to_string(&log).map_or(0, |t| t.lines().count());
    let design = |to: &str| {
        format!("tamber t\n\naxes\n  mode = a | b\n\ntheme\n  Foo = #111111\n  foo = #222222\n  pick = {to}\n\nparams\n  Map.pin = pick\n")
    };
    std::fs::write(&file, design("Foo")).unwrap();
    let vary = |name: &str| {
        json!({"method": "propose", "ops": [
            {"op": "set-token", "name": name, "when": [{"axis": "mode", "value": "b"}], "value": "#444444"}
        ]})
    };
    let mut s = Serve::start(
        &dir,
        &[("EXACT_TAMBER_RETENTION_LOG", log.to_str().unwrap())],
    );
    let (before, _) = s.ask(vary("Foo"));
    assert!(before.get("ok").is_some(), "{before}");
    assert_eq!(builds(), 1);
    std::fs::write(&file, design("foo")).unwrap();
    let primed = [s.ask(vary("Foo")).0, s.ask(vary("foo")).0];
    assert_eq!(builds(), 2, "the chain moved from Foo to foo");
    drop(s);
    let mut fresh = Serve::start(&dir, &[]);
    for (p, name) in primed.iter().zip(["Foo", "foo"]) {
        let (e, _) = fresh.ask(vary(name));
        assert_eq!(p["ok"]["class"], e["ok"]["class"], "{name}: {p} vs {e}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// LLP 1139 D8: a library component's param is named in full in the app's
/// design, so `set-param` by its short name writes the new line by the full
/// one, which the build takes (a short one is `tamber-param-short-library`);
/// `propose` answers the same diff the write makes.
#[test]
fn a_short_name_writes_a_library_params_line_in_full() {
    let dir = app(
        "pkg-params-library-short",
        "tamber t\n\ntheme\n  ink = #111111\n",
    );
    let at = dir.join("node_modules/@acme/lib");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(
        at.join("package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(
        at.join("index.contract"),
        CARD.replace("reading.gap", "lib.gap"),
    )
    .unwrap();
    std::fs::write(
        at.join("lib.tamber"),
        "tamber lib\n\ntheme\n  lib.gap: length = space.md\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/lib\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-param", "name": "Card.inset", "value": "20px"});
    let files = json!({"app.tamber": digest(&file)});
    let (proposed, _) = s.ask(json!({"method": "propose", "files": files, "ops": [op]}));
    let (done, _) =
        s.ask(json!({"method": "transaction", "client": "one", "files": files, "ops": [op]}));
    assert_eq!(done["ok"]["written"], json!(["app.tamber"]), "{done}");
    assert_eq!(proposed["ok"]["diff"], done["ok"]["diff"], "{proposed}");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("\n  @acme/lib/index.contract#Card.inset = 20px\n"),
        "{text}"
    );
    let plan = contract::compile_path(&dir.join("app.contract"))
        .unwrap_or_else(|e| panic!("the design the server wrote builds: {e:?}"));
    assert!(
        format!("{plan:?}").contains("20px"),
        "the plan keeps the set value"
    );
    // A later write by the short name finds that line, and so does unset.
    let op = json!({"op": "set-param", "name": "Card.inset", "value": "24px"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [op]}));
    assert!(done.get("ok").is_some(), "{done}");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("#Card.inset = 24px\n") && text.matches("Card.inset").count() == 1,
        "{text}"
    );
    let op = json!({"op": "unset-param", "name": "Card.inset"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [op]}));
    assert!(done.get("ok").is_some(), "{done}");
    assert!(!std::fs::read_to_string(&file).unwrap().contains("params"));
    drop(s);
    // A line already spelled short (as the server wrote it before): a set
    // respells it in full, its comment and `when` line kept, and builds.
    let broken = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  ink = #111111\n\nparams\n  Card.inset = 16px  // roomy\n    when density=compact: 8px\n";
    std::fs::write(&file, broken).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-param", "name": "Card.inset", "value": "18px"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [op]}));
    assert!(done.get("ok").is_some(), "{done}");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.ends_with("\nparams\n  @acme/lib/index.contract#Card.inset = 18px  // roomy\n    when density=compact: 8px\n"),
        "{text}"
    );
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    // Its undo gives the short line back, byte for byte.
    let (undone, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert!(undone.get("ok").is_some(), "{undone}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
    // Unsetting only its `when` line keeps the param's line, respelled.
    let op = json!({"op": "unset-param", "name": "Card.inset", "when": [{"axis": "density", "value": "compact"}]});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [op]}));
    assert!(done.get("ok").is_some(), "{done}");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.ends_with("\nparams\n  @acme/lib/index.contract#Card.inset = 16px  // roomy\n"),
        "{text}"
    );
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    let (undone, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert!(undone.get("ok").is_some(), "{undone}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
    // Unset and set again in one transaction: the new line is in full.
    let ops = json!([{"op": "unset-param", "name": "Card.inset"}, {"op": "set-param", "name": "Card.inset", "value": "20px"}]);
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": ops}));
    assert!(done.get("ok").is_some(), "{done}");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.ends_with("\nparams\n  @acme/lib/index.contract#Card.inset = 20px\n"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A kept icon's path, read through `exact_runner::icon_of` (stage 14 C2:
/// the one reader of an `icons` row, which strips an even-odd icon's marker).
fn icon_path<'a>(plan: &'a exact_plan::Plan, row: &exact_plan::IconsRow) -> &'a str {
    exact_runner::icon_of(plan, row).map_or("", |icon| icon.path)
}
