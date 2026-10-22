//! LLP 1139 D5, LLP 1121 stage 12 D3 (*1.15*): `describe` lists each
//! exported library component's parts and variant props (from C's IR), with
//! params or without; each of its params' winning level per tuple is
//! `resolve`'s (B2-levels' one walker), for each of the five levels.
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
fn describe_lists_a_library_components_parts_and_variants_and_resolve_each_params_level() {
    let dir = library(
        "describe-library",
        "tamber app\n\ntheme\n  lib.gap = 6px\n\nparams\n  @acme/lib/index.contract#Card.set = 10px\n",
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
    // Each param's winner per tuple is `resolve`'s (B2-levels; theme2's 07:20Z ruling: describe keeps only
    // `reaches`): the app's own value, the app's override of the token, the package's default, a base
    // name, a role, through the library component's params.
    let row = |param: &str| {
        card["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["param"] == param)
            .unwrap_or_else(|| panic!("{param}: {card}"))
            .clone()
    };
    assert!(
        card["params"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p.get("winning").is_none()),
        "{card}"
    );
    for (param, level, name) in [
        (
            "set",
            "app-param",
            Some("@acme/lib/index.contract#Card.set"),
        ),
        ("app", "app-token", Some("lib.gap")),
        ("pkg", "package", Some("lib.pad")),
        ("base", "base", Some("space.md")),
        ("ink", "role", None),
    ] {
        let (r, _) = s.ask(json!({"method": "resolve", "token": row(param)["name"]}));
        let tuples = r["ok"]["tuples"]
            .as_array()
            .unwrap_or_else(|| panic!("{param}: {r}"));
        assert!(!tuples.is_empty());
        // Each side's (B2-levels per side): no axis here is under `scheme=dark`, so both alike.
        for t in tuples {
            for side in ["light", "dark"] {
                assert_eq!(t["level"][side], level, "{param} {side}: {t}");
                if let Some(name) = name {
                    assert_eq!(t["name"][side], name, "{param} {side}: {t}");
                }
            }
        }
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
fn a_local_component_or_a_public_packages_with_parts_is_no_library_one() {
    // A local file's `Card` (with a param, so `components` is listed) and a public package's `Tile`, each
    // naming parts: neither is an exported library component (LLP 1139 D0: a private package with a design).
    let dir = app(
        "describe-no-library",
        "tamber app\n\ntheme\n  other = 2px\n",
    );
    std::fs::write(
        dir.join("card.contract"),
        "component Card\n  params\n    pad: length = \"8px\"\n  parts\n    surface: space\n  view\n    column part=\"surface\" padding=\"param.pad\"\n      text \"c\"\n",
    )
    .unwrap();
    let public = dir.join("node_modules/@acme/public");
    std::fs::create_dir_all(&public).unwrap();
    std::fs::write(
        public.join("package.json"),
        r#"{"name":"@acme/public","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(public.join("index.contract"), "component Tile\n  params\n    gap: length = \"4px\"\n  parts\n    face: space\n  view\n    column part=\"face\" gap=\"param.gap\"\n      text \"t\"\n").unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./card.contract\"\nuse Tile from \"@acme/public\"\n\ncomponent App\n  view\n    column\n      Card()\n      Tile()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let all = components(&mut s);
    let names: Vec<&str> = all.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Card", "Tile"], "{all:?}");
    for c in &all {
        assert!(
            c.get("parts").is_none() && c.get("variants").is_none() && c.get("library").is_none(),
            "{c}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_app_whose_only_library_component_has_no_params_lists_it_by_its_own_name() {
    // No params anywhere: `components` is listed for the library's `Badge` alone, named `Badge` though the app
    // has a `Badge` of its own (the compiler's renamed name never shown).
    let dir = app(
        "describe-badge-only",
        "tamber app\n\ntheme\n  other = 2px\n",
    );
    let lib = dir.join("node_modules/@acme/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("package.json"), r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#).unwrap();
    std::fs::write(
        lib.join("index.contract"),
        "component Badge\n  parts\n    face: paint\n  view\n    text \"b\" part=\"face\"\n",
    )
    .unwrap();
    std::fs::write(
        lib.join("lib.tamber"),
        "tamber lib\n\ntheme\n  lib.ink = #111111\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Badge as LibBadge from \"@acme/lib\"\n\ncomponent Badge\n  view\n    text \"mine\"\n\ncomponent App\n  view\n    column\n      LibBadge()\n      Badge()\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let all = components(&mut s);
    assert_eq!(
        all,
        vec![
            json!({"identity": "@acme/lib/index.contract#Badge", "name": "Badge", "params": [], "library": true, "parts": [{"name": "face", "groups": ["paint"], "slot": false}], "variants": []})
        ],
    );
    // `contract tamber ir` lists it the same way (round 2).
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "ir", dir.join("app.tamber").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ir: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(ir["components"], json!(all));
    let _ = std::fs::remove_dir_all(&dir);
}
