//! LLP 1139 D5, LLP 1121 stage 12 D3 (*1.15*): `describe` lists each
//! exported library component's parts and variant props (from C's IR), with
//! params or without, and each param's winning level per tuple (`winning`,
//! beside `values`), from B2-levels' one walker, as `resolve` says it.
#![cfg(unix)]

use super::tamber_serve::{app, Serve};
use serde_json::{json, Value};

/// An app using `Card` and `Badge` from a private library package
/// `@acme/lib`: `Card` with parts, a slot, a variant prop and a param for
/// each level; `Badge` with parts and no params. `design` is the app's.
fn library(tag: &str, design: &str) -> std::path::PathBuf {
    let dir = app(tag, design);
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(
        lib.join("index.contract"),
        "style Plain\n  opacity=1\n\nstyle Quiet\n  opacity=0.8\n\n\
         component Card\n  props\n    tone: enum<\"plain\", \"quiet\">\n  params\n    \
         set: length = \"lib.pad\"\n    app: length = \"lib.gap\"\n    pkg: length = \"lib.pad\"\n    \
         base: length = \"space.md\"\n    ink: color = \"AccentColor\"\n  slot\n  parts\n    \
         surface: paint shape space\n    body: space\n  view\n    \
         column part=\"surface\" class=(tone == \"quiet\" ? Quiet : Plain) padding=\"param.set\" gap=\"param.app\" color=\"param.ink\"\n      \
         column part=\"body\" padding=\"param.pkg\" margin=\"param.base\"\n        children\n\n\
         component Badge\n  parts\n    face: paint\n  view\n    text \"b\" part=\"face\"\n",
    )
    .unwrap();
    std::fs::write(
        lib.join("lib.tamber"),
        "tamber lib\n\ntheme\n  lib.pad = 4px\n  lib.gap = 2px\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/lib\"\nuse Badge from \"@acme/lib\"\n\ncomponent App\n  view\n    column\n      Card(tone=\"plain\")\n        text \"fill\"\n      Badge()\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    dir
}

fn components(s: &mut Serve) -> Vec<Value> {
    let (r, _) = s.ask(json!({"method": "describe"}));
    assert!(r.get("ok").is_some(), "{r}");
    r["ok"]["ir"]["components"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn describe_lists_a_library_components_parts_variants_and_each_params_winning_level() {
    let dir = library(
        "describe-library",
        "tamber app\n\ntheme\n  lib.gap = 6px\n\nparams\n  Card.set = 10px\n",
    );
    let mut s = Serve::start(&dir, &[]);
    let all = components(&mut s);
    let card = all.iter().find(|c| c["name"] == "Card").expect("Card");
    assert_eq!(card["identity"], "@acme/lib/index.contract#Card");
    assert_eq!(card["library"], true);
    assert_eq!(
        card["parts"],
        json!([
            {"name": "surface", "groups": ["paint", "shape", "space"], "slot": false},
            {"name": "body", "groups": ["space"], "slot": true},
        ])
    );
    assert_eq!(
        card["variants"],
        json!([{"prop": "tone", "options": ["plain", "quiet"]}])
    );
    // Each param's winner, in every tuple: the app's own value, the app's override of the token, the
    // package's default, a base name, a role.
    let winning = |param: &str| -> Vec<(String, String)> {
        let row = card["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["param"] == param)
            .unwrap_or_else(|| panic!("{param}: {card}"));
        let w = row["winning"]
            .as_array()
            .unwrap_or_else(|| panic!("{param}: {row}"));
        assert_eq!(w.len(), row["values"].as_array().unwrap().len(), "{row}");
        w.iter()
            .map(|t| {
                (
                    t["level"].as_str().unwrap().to_string(),
                    t["name"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    for (param, level, name) in [
        ("set", "app-param", "@acme/lib/index.contract#Card.set"),
        ("app", "app-token", "lib.gap"),
        ("pkg", "package", "lib.pad"),
        ("base", "base", "space.md"),
    ] {
        let got = winning(param);
        assert!(
            got.iter().all(|(l, n)| l == level && n == name),
            "{param}: {got:?}"
        );
    }
    let ink = winning("ink");
    assert!(ink.iter().all(|(l, _)| l == "role"), "{ink:?}");
    // As `resolve` says it, tuple by tuple.
    let full = |param: &str| {
        card["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["param"] == param)
            .unwrap()["name"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for param in ["set", "app", "pkg", "base", "ink"] {
        let (r, _) = s.ask(json!({"method": "resolve", "token": full(param)}));
        let resolved: Vec<(String, String)> = r["ok"]["tuples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                (
                    t["level"].as_str().unwrap().to_string(),
                    t["name"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(resolved, winning(param), "{param}");
    }
    // A library component with no params: listed all the same.
    let badge = all.iter().find(|c| c["name"] == "Badge").expect("Badge");
    assert_eq!(
        (
            badge["params"].clone(),
            badge["parts"].clone(),
            badge["variants"].clone(),
            badge["library"].clone()
        ),
        (
            json!([]),
            json!([{"name": "face", "groups": ["paint"], "slot": false}]),
            json!([]),
            json!(true)
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_app_with_no_library_lists_no_parts() {
    let dir = app(
        "describe-no-library",
        "tamber app\n\ntheme\n  other = 2px\n",
    );
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    assert!(components(&mut s).iter().all(|c| c.get("parts").is_none()));
    let _ = std::fs::remove_dir_all(&dir);
}
