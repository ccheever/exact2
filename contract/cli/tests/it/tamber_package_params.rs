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

const CARD: &str = "component Card\n  params\n    inset: length = \"reading.gap\"\n  view\n    column padding=\"param.inset\" testId=\"card\"\n      text \"c\"\n";

/// A package `@acme/<ns>` in the app at `dir`: its entry `index.contract`
/// (`contract`) and its design `<ns>.tamber`.
fn package(dir: &std::path::Path, ns: &str, contract: &str, design: &str) {
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
fn reading(tag: &str) -> std::path::PathBuf {
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
    let (full, _) = s.ask(op("@acme/maps/maps.tamber#Map.pin"));
    assert!(full.get("ok").is_some(), "{full}");
    let _ = std::fs::remove_dir_all(&dir);
}
