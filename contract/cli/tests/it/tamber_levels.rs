//! LLP 1121 stage 12 B2-levels (*1.15*): a param's levels, from one walker
//! (`contract::tamber_levels`). `resolve` of a param says each tuple's
//! winning `level` and `name`; `explain` and `resolve` say its `levels`
//! when its default is a bare reference; `describe` says what each param's
//! default `reaches`.
#![cfg(unix)]

use super::tamber_serve::{app, Serve};
use serde_json::{json, Value};

const MAP: &str = "@acme/reading/reading.tamber#Map";
const CARD: &str = "@acme/reading/index.contract#Card";

/// An app using `@acme/reading`: a Contract `Card` whose `inset` defaults
/// to `reading.gap` (a base name), and the design's `Map`, whose params
/// default to a package colour, a chain through it, a role, and an
/// expression; with `design` the app's.
fn reading(tag: &str, design: &str) -> std::path::PathBuf {
    let dir = app(tag, design);
    let at = dir.join("node_modules/@acme/reading");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(
        at.join("package.json"),
        r#"{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    )
    .unwrap();
    std::fs::write(at.join("index.contract"), "component Card\n  params\n    inset: length = \"reading.gap\"\n  view\n    column padding=\"param.inset\"\n      text \"c\"\n").unwrap();
    std::fs::write(
        at.join("reading.tamber"),
        "tamber reading\n\ntheme\n  reading.gap: length = space.md\n  reading.pin = #333333\n  reading.chain = reading.pin\n  reading.ink = AccentColor\n\ncomponent Map\n  params\n    pin: color = reading.pin\n    chain: color = reading.chain\n    ink: color = reading.ink\n    mix: color = color-mix(in srgb, reading.pin 50%, white)\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"@acme/reading\"\ncomponent App\n  view\n    column\n      Card()\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    dir
}

fn ask(s: &mut Serve, method: &str, token: &str) -> Value {
    let (r, _) = s.ask(json!({"method": method, "token": token}));
    assert!(r.get("ok").is_some(), "{method} {token}: {r}");
    r["ok"].clone()
}

/// Each tuple's `[level, name]`.
fn steps(resolved: &Value) -> Vec<(String, String)> {
    resolved["tuples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["level"].as_str().unwrap().to_string(),
                t["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn a_package_default_an_app_token_and_an_app_param_each_win_in_turn() {
    let dir = reading("levels-wins", "tamber t\n\ntheme\n  ink = #111111\n");
    let pin = format!("{MAP}.pin");
    // The package's own value.
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &pin);
    assert_eq!(
        steps(&r),
        [("package".to_string(), "reading.pin".to_string())]
    );
    let levels = &r["levels"];
    assert_eq!(
        levels["token"],
        json!({"name": "reading.pin", "package": "@acme/reading", "file": "@acme/reading/reading.tamber", "line": 5, "value": "#333333"}),
        "{levels}"
    );
    assert_eq!(
        (&levels["app"], &levels["base"], &levels["param"]),
        (&Value::Null, &Value::Null, &Value::Null)
    );
    let e = ask(&mut s, "explain", &pin);
    assert_eq!(e["levels"], *levels, "explain's levels are resolve's");
    // Through a chain of the package's tokens: the last line wins.
    assert_eq!(
        steps(&ask(&mut s, "resolve", &format!("{MAP}.chain"))),
        [("package".to_string(), "reading.pin".to_string())]
    );
    drop(s);
    // The app overrides the token: the app's line wins.
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n  reading.pin = #222222\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &pin);
    assert_eq!(
        steps(&r),
        [("app-token".to_string(), "reading.pin".to_string())]
    );
    assert_eq!(
        r["levels"]["app"],
        json!({"file": "app.tamber", "line": 5, "value": "#222222"}),
        "{r}"
    );
    assert_eq!(
        r["levels"]["token"]["value"], "#333333",
        "the package's own line"
    );
    drop(s);
    // The app sets the param: its value wins, kept beside today's `set`.
    std::fs::write(dir.join("app.tamber"), "tamber t\n\ntheme\n  ink = #111111\n  reading.pin = #222222\n\nparams\n  Map.pin = #abcdef\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &pin);
    assert_eq!(steps(&r), [("app-param".to_string(), pin.clone())]);
    assert_eq!(
        r["levels"]["param"],
        json!({"file": "app.tamber", "line": 8, "value": "#abcdef"}),
        "{r}"
    );
    let e = ask(&mut s, "explain", &pin);
    assert_eq!(e["param"]["set"]["value"], "#abcdef");
    // An app value naming an app token: the token's line wins.
    drop(s);
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #111111\n\nparams\n  Map.pin = ink\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    assert_eq!(
        steps(&ask(&mut s, "resolve", &pin)),
        [("app-token".to_string(), "ink".to_string())]
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_base_a_role_and_an_expression() {
    let dir = reading("levels-ends", "tamber t\n\ntheme\n  ink = #111111\n");
    let mut s = Serve::start(&dir, &[]);
    // A Contract package component's param to a base name, through the package's token.
    let inset = ask(&mut s, "resolve", &format!("{CARD}.inset"));
    assert_eq!(
        steps(&inset),
        [("base".to_string(), "space.md".to_string())]
    );
    assert_eq!(inset["levels"]["base"], "space.md", "{inset}");
    assert_eq!(inset["levels"]["token"]["name"], "reading.gap");
    // A role.
    assert_eq!(
        steps(&ask(&mut s, "resolve", &format!("{MAP}.ink"))),
        [("role".to_string(), "AccentColor".to_string())]
    );
    // An expression: no levels; the param's own default wins.
    let mix = ask(&mut s, "resolve", &format!("{MAP}.mix"));
    assert!(mix.get("levels").is_none(), "{mix}");
    assert_eq!(steps(&mix), [("default".to_string(), format!("{MAP}.mix"))]);
    assert!(ask(&mut s, "explain", &format!("{MAP}.mix"))
        .get("levels")
        .is_none());
    // describe: what each default reaches, compact.
    let (described, _) = s.ask(json!({"method": "describe"}));
    let params: Vec<Value> = described["ok"]["ir"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["params"].as_array().unwrap().clone())
        .collect();
    let row = |n: &str| {
        params
            .iter()
            .find(|p| p["name"] == n)
            .cloned()
            .unwrap_or_else(|| panic!("{n}: {described}"))
    };
    assert_eq!(
        row(&format!("{CARD}.inset"))["reaches"],
        json!({"token": "reading.gap", "base": "space.md", "app_set": false})
    );
    assert_eq!(
        row(&format!("{MAP}.pin"))["reaches"],
        json!({"token": "reading.pin", "base": null, "app_set": false})
    );
    assert!(row(&format!("{MAP}.mix")).get("reaches").is_none());
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn levels_differ_by_tuple_and_a_chain_stops_after_eight_hops() {
    // `reading.gap` overridden: 12px in one tuple, the base space.lg in the other.
    let dir = reading("levels-tuples", "tamber t\n\naxes\n  mode = a | b\n\ntheme\n  ink = #111111\n  reading.gap = 12px\n    when mode=b: space.lg\n");
    let mut s = Serve::start(&dir, &[]);
    let inset = ask(&mut s, "resolve", &format!("{CARD}.inset"));
    assert_eq!(
        steps(&inset),
        [
            ("app-token".to_string(), "reading.gap".to_string()),
            ("base".to_string(), "space.lg".to_string())
        ]
    );
    assert_eq!(inset["tuples"][1]["modes"], json!({"mode": "b"}));
    drop(s);
    // Ten app tokens, each naming the next: the walk stops after eight.
    let chain: String = (0..10)
        .map(|i| {
            if i < 9 {
                format!("  c{i} = c{}\n", i + 1)
            } else {
                format!("  c{i} = #000000\n")
            }
        })
        .collect();
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  ink = #111111\n{chain}\nparams\n  Map.pin = c0\n"),
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    assert_eq!(
        steps(&ask(&mut s, "resolve", &format!("{MAP}.pin"))),
        [("app-token".to_string(), "c7".to_string())]
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}
