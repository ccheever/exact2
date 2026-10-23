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
    std::fs::write(at.join("index.contract"), "component Card\n  params\n    inset: length = \"reading.gap\"\n    edge: length = \"param.inset\"\n  view\n    column padding=\"param.inset\" gap=\"param.edge\"\n      text \"c\"\n").unwrap();
    std::fs::write(
        at.join("reading.tamber"),
        "tamber reading\n\ntheme\n  reading.gap: length = space.md\n  reading.pin = #333333\n  reading.chain = reading.pin\n  reading.ink = AccentColor\n  reading.quiet = -exact-secondary-label\n\ncomponent Map\n  params\n    pin: color = reading.pin\n    chain: color = reading.chain\n    ink: color = reading.ink\n    mix: color = color-mix(in srgb, reading.pin 50%, white)\n    quiet: color = reading.quiet\n    edge: color = param.pin\n    clear: color = transparent\n    lift: number = -1\n    glyph: icon = icon(sf \"heart\")\n",
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

/// describe's `reaches` of param `name`.
fn reaches(s: &mut Serve, name: &str) -> Value {
    let (described, _) = s.ask(json!({"method": "describe"}));
    described["ok"]["ir"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["params"].as_array().unwrap().clone())
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("{name}: {described}"))["reaches"]
        .clone()
}

/// Each tuple's `(level, name)` on `side` (`light` or `dark`).
fn side(resolved: &Value, side: &str) -> Vec<(String, String)> {
    resolved["tuples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["level"][side].as_str().unwrap().to_string(),
                t["name"][side].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Each tuple's `(level, name)`, the same on both sides where the scheme
/// decides nothing.
fn steps(resolved: &Value) -> Vec<(String, String)> {
    let light = side(resolved, "light");
    assert_eq!(
        light,
        side(resolved, "dark"),
        "one level per tuple: {resolved}"
    );
    light
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
    // Through a chain of the package's tokens: the last line wins; its
    // levels' token is the first.
    let chain = ask(&mut s, "resolve", &format!("{MAP}.chain"));
    assert_eq!(
        steps(&chain),
        [("package".to_string(), "reading.pin".to_string())]
    );
    assert_eq!(chain["levels"]["token"]["name"], "reading.chain", "{chain}");
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
    assert_eq!(reaches(&mut s, &pin)["app_set"], true);
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
    assert_eq!(mix.get("levels"), Some(&Value::Null), "{mix}");
    assert_eq!(steps(&mix), [("default".to_string(), format!("{MAP}.mix"))]);
    let e = ask(&mut s, "explain", &format!("{MAP}.mix"));
    assert_eq!(e.get("levels"), Some(&Value::Null), "{e}");
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
    assert_eq!(
        inset["levels"]["base"],
        Value::Null,
        "one tuple reaches no base"
    );
    drop(s);
    // A `when` sends one tuple to another base: each tuple's own, no one `base`.
    std::fs::write(dir.join("app.tamber"), "tamber t\n\naxes\n  mode = a | b\n\ntheme\n  ink = #111111\n  reading.gap = space.md\n    when mode=b: space.lg\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let inset = ask(&mut s, "resolve", &format!("{CARD}.inset"));
    assert_eq!(
        steps(&inset),
        [
            ("base".to_string(), "space.md".to_string()),
            ("base".to_string(), "space.lg".to_string())
        ]
    );
    assert_eq!(
        inset["levels"]["base"],
        Value::Null,
        "the tuples differ: {inset}"
    );
    let e = ask(&mut s, "explain", &format!("{CARD}.inset"));
    assert_eq!(e["levels"]["base"], Value::Null);
    drop(s);
    // Both tuples reach the package's `space.md`: that is `levels.base`.
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\naxes\n  mode = a | b\n\ntheme\n  ink = #111111\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let inset = ask(&mut s, "resolve", &format!("{CARD}.inset"));
    assert_eq!(steps(&inset).len(), 2);
    assert_eq!(inset["levels"]["base"], "space.md", "{inset}");
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

/// Round 1 (Astra 1 = Grok 1): a role's override, in either spelling and any
/// case, as the checker keys it, named by the row spelling the fold
/// publishes; an app-set base name is the app's token (Grok 3).
#[test]
fn a_roles_override_in_either_spelling_and_an_app_set_base_name() {
    let one = |s: &mut Serve, p: &str| steps(&ask(s, "resolve", &format!("{MAP}.{p}")));
    let step = |l: &str, n: &str| vec![(l.to_string(), n.to_string())];
    let dir = reading("levels-roles", "tamber t\n\ntheme\n  ink = #111111\n");
    let mut s = Serve::start(&dir, &[]);
    assert_eq!(one(&mut s, "quiet"), step("role", "-exact-secondary-label"));
    assert_eq!(one(&mut s, "ink"), step("role", "AccentColor"));
    drop(s);
    for (app, quiet) in [
        (
            "  Secondary-Label = #123456\n  accentcolor = #654321\n",
            "-exact-secondary-label",
        ),
        (
            "  -exact-Secondary-Label = #123456\n  ACCENTCOLOR = #654321\n",
            "-exact-secondary-label",
        ),
    ] {
        let design = format!("tamber t\n\ntheme\n  ink = #111111\n{app}  space.md = 10px\n\nparams\n  Map.pin = secondary-label\n");
        std::fs::write(dir.join("app.tamber"), design).unwrap();
        let mut s = Serve::start(&dir, &[]);
        assert_eq!(one(&mut s, "quiet"), step("app-token", quiet), "{app}");
        // The app's value in the bare spelling reaches the override too.
        assert_eq!(one(&mut s, "pin"), step("app-token", quiet), "{app}");
        let pin = ask(&mut s, "resolve", &format!("{MAP}.pin"));
        assert_eq!(pin["tuples"][0]["value"], "#123456ff", "{pin}");
        let ink = ask(&mut s, "resolve", &format!("{MAP}.ink"));
        assert_eq!(steps(&ink), step("app-token", "AccentColor"), "{app}");
        assert_eq!(
            ink["tuples"][0]["value"], "#654321ff",
            "the fold agrees: {ink}"
        );
        // `space.md = 10px`: the app's token, not the base.
        let inset = ask(&mut s, "resolve", &format!("{CARD}.inset"));
        assert_eq!(steps(&inset), step("app-token", "space.md"));
        assert_eq!(inset["levels"]["base"], Value::Null, "{inset}");
        drop(s);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1 (Astra 3, 4, 5 = Grok 2): an icon param's tuples and levels; a
/// default naming another param, in Contract and in a `.tamber`, reaches
/// that param's token; a literal default (`transparent`) or an expression
/// has `levels: null`.
#[test]
fn icon_params_param_aliases_and_literals() {
    let dir = reading(
        "levels-icons",
        "tamber t\n\ntheme\n  ink = #111111\n\nicons\n  icon.app = icon(sf \"star\")\n",
    );
    let mut s = Serve::start(&dir, &[]);
    // A literal package icon param: its own default, no levels.
    let glyph = ask(&mut s, "resolve", &format!("{MAP}.glyph"));
    assert_eq!(
        steps(&glyph),
        [("default".to_string(), format!("{MAP}.glyph"))]
    );
    assert_eq!(glyph.get("levels"), Some(&Value::Null), "{glyph}");
    let e = ask(&mut s, "explain", &format!("{MAP}.glyph"));
    assert_eq!(e.get("levels"), Some(&Value::Null), "{e}");
    // Aliases: a `.tamber` param's `param.pin`, a Contract param's `param.inset`.
    let edge = ask(&mut s, "resolve", &format!("{MAP}.edge"));
    assert_eq!(
        steps(&edge),
        [("package".to_string(), "reading.pin".to_string())]
    );
    assert_eq!(edge["levels"]["token"]["name"], "reading.pin", "{edge}");
    let edge = ask(&mut s, "explain", &format!("{CARD}.edge"));
    assert_eq!(edge["levels"]["token"]["name"], "reading.gap", "{edge}");
    assert_eq!(edge["levels"]["base"], "space.md");
    assert_eq!(
        reaches(&mut s, &format!("{CARD}.edge"))["token"],
        "reading.gap"
    );
    // A literal colour keyword and a negative number: no reference, `levels: null`.
    let clear = ask(&mut s, "resolve", &format!("{MAP}.clear"));
    assert_eq!(
        steps(&clear),
        [("default".to_string(), format!("{MAP}.clear"))]
    );
    assert_eq!(clear.get("levels"), Some(&Value::Null), "{clear}");
    let lift = ask(&mut s, "explain", &format!("{MAP}.lift"));
    assert_eq!(lift.get("levels"), Some(&Value::Null), "{lift}");
    drop(s);
    // An app component's icon param naming the app's icon (a package's,
    // stage 13: `package_icons.rs`).
    std::fs::write(dir.join("app.contract"), "use Card from \"@acme/reading\"\ncomponent Badge\n  params\n    mark: icon = \"icon.app\"\n  view\n    image \"symbol:param.mark\"\ncomponent App\n  view\n    column\n      Card()\n      Badge()\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let mark = ask(&mut s, "resolve", "app.contract#Badge.mark");
    assert_eq!(
        steps(&mark),
        [("app-token".to_string(), "icon.app".to_string())]
    );
    assert_eq!(
        mark["levels"]["token"],
        json!({"name": "icon.app", "file": "app.tamber", "line": 7, "value": "icon(sf \"star\")"}),
        "{mark}"
    );
    let e = ask(&mut s, "explain", "app.contract#Badge.mark");
    assert_eq!(e["levels"], mark["levels"], "{e}");
    drop(s);
    // The app sets the package's icon param: its value wins, through its
    // icon token, or as written.
    let app =
        "tamber t\n\ntheme\n  ink = #111111\n\nicons\n  icon.app = icon(sf \"star\")\n\nparams\n";
    std::fs::write(
        dir.join("app.tamber"),
        format!("{app}  Map.glyph = icon.app\n"),
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let glyph = ask(&mut s, "resolve", &format!("{MAP}.glyph"));
    assert_eq!(
        steps(&glyph),
        [("app-token".to_string(), "icon.app".to_string())]
    );
    assert_eq!(glyph["tuples"][0]["sf"], "star", "{glyph}");
    drop(s);
    std::fs::write(
        dir.join("app.tamber"),
        format!("{app}  Map.glyph = icon(sf \"bolt\")\n"),
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let glyph = ask(&mut s, "resolve", &format!("{MAP}.glyph"));
    assert_eq!(
        steps(&glyph),
        [("app-param".to_string(), format!("{MAP}.glyph"))]
    );
    assert_eq!(glyph["tuples"][0]["sf"], "bolt", "{glyph}");
    assert_eq!(
        glyph.get("levels"),
        Some(&Value::Null),
        "a literal default: {glyph}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each tuple's `(modes, level, name, value)`.
fn rows(resolved: &Value) -> Vec<(Value, String, String, Value)> {
    resolved["tuples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            // No axis here is under `scheme=dark`: both sides alike.
            let s = |k: &str| {
                assert_eq!(t[k]["light"], t[k]["dark"], "{k}: {t}");
                t[k]["light"].as_str().unwrap().to_string()
            };
            (
                t["modes"].clone(),
                s("level"),
                s("name"),
                t["value"].clone(),
            )
        })
        .collect()
}

/// Round 2 (Astra 1 = Grok 1): `levels.token` keeps the walk's cap. Nine
/// params, each naming the next and the last the package's token, are
/// eight aliases: the walk stops on the last param and `levels` names no
/// token. Eight reach the token at hop eight, and both say it.
#[test]
fn levels_token_keeps_the_walks_eight_hops() {
    let dir = reading("levels-hops", "tamber t\n\ntheme\n  ink = #111111\n");
    let pkg = dir.join("node_modules/@acme/reading/reading.tamber");
    let mut text = std::fs::read_to_string(&pkg).unwrap();
    for (component, n) in [("Nine", 9), ("Eight", 8)] {
        text.push_str(&format!("\ncomponent {component}\n  params\n"));
        for i in 0..n {
            let next = match i + 1 < n {
                true => format!("param.p{}", i + 1),
                false => "reading.pin".to_string(),
            };
            text.push_str(&format!("    p{i}: color = {next}\n"));
        }
    }
    std::fs::write(&pkg, text).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let nine = "@acme/reading/reading.tamber#Nine";
    let r = ask(&mut s, "resolve", &format!("{nine}.p0"));
    assert_eq!(steps(&r), [("default".to_string(), format!("{nine}.p8"))]);
    assert_eq!(r["levels"]["token"], Value::Null, "{r}");
    assert_eq!(reaches(&mut s, &format!("{nine}.p0"))["token"], Value::Null);
    let eight = "@acme/reading/reading.tamber#Eight";
    let r = ask(&mut s, "resolve", &format!("{eight}.p0"));
    assert_eq!(
        steps(&r),
        [("package".to_string(), "reading.pin".to_string())]
    );
    assert_eq!(r["levels"]["token"]["name"], "reading.pin", "{r}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2 (Grok 2), and the per-side correction (theme2's 19:30 ruling,
/// option (a)): a `when` naming an axis `under scheme=dark` makes a
/// colour's light/dark pair, its light side the token's own line. Each side
/// is walked on its own: the light side stops on the line, the dark side
/// follows its `when`.
#[test]
fn a_light_dark_pair_is_walked_per_side() {
    let dir = reading("levels-pair", "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = #111111\n  reading.chain = #ffffff\n    when dark-style=dim: reading.pin\n");
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &format!("{MAP}.chain"));
    let chain = ("app-token".to_string(), "reading.chain".to_string());
    let pin = ("package".to_string(), "reading.pin".to_string());
    assert_eq!(side(&r, "light"), [chain.clone(), chain.clone()], "{r}");
    assert_eq!(side(&r, "dark"), [chain, pin], "{r}");
    assert_eq!(
        (&r["tuples"][1]["light"], &r["tuples"][1]["dark"]),
        (&json!("#ffffffff"), &json!("#333333ff")),
        "the pair: {r}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 2 (Astra 2): two axes, general and specific `when` lines that
/// overlap, on a package token and on the app's `params` line with its
/// own: every tuple's modes, level, name and value, so a wrong tuple order,
/// the first holding `when` in place of the most specific, or a param's
/// `when` lines ignored, each fails.
#[test]
fn two_axes_overlapping_whens_on_a_package_token_and_an_app_param() {
    let dir = reading(
        "levels-axes",
        "tamber t\n\naxes\n  mode = a | b\n  size = s | l\n\ntheme\n  ink = #111111\n",
    );
    let pkg = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&pkg).unwrap().replace(
        "  reading.pin = #333333\n",
        "  reading.pin = #333333\n  reading.tone = #101010\n    when mode=b: #202020\n    when size=l: AccentColor\n    when mode=b size=l: reading.deep\n  reading.deep = #303030\n",
    ) + "    tone: color = reading.tone\n";
    std::fs::write(&pkg, text).unwrap();
    let tone = format!("{MAP}.tone");
    // Each tuple's (mode, size, level, name, value), a role's value its own.
    type Row<'a> = (&'a str, &'a str, &'a str, &'a str, Option<&'a str>);
    let expect = |r: &Value, want: [Row; 4]| {
        let got = rows(r);
        for (g, (m, z, l, n, v)) in got.iter().zip(want) {
            assert_eq!(g.0, json!({"mode": m, "size": z}), "the tuples' order: {r}");
            assert_eq!((g.1.as_str(), g.2.as_str()), (l, n), "{m} {z}: {r}");
            if let Some(v) = v {
                assert_eq!(g.3, json!(v), "{m} {z}: {r}");
            }
        }
        assert_eq!(got.len(), 4);
    };
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &tone);
    expect(
        &r,
        [
            ("a", "s", "package", "reading.tone", Some("#101010ff")),
            ("a", "l", "role", "AccentColor", None),
            ("b", "s", "package", "reading.tone", Some("#202020ff")),
            ("b", "l", "package", "reading.deep", Some("#303030ff")),
        ],
    );
    drop(s);
    // The app's value, with `when` lines of its own over both axes.
    std::fs::write(dir.join("app.tamber"), "tamber t\n\naxes\n  mode = a | b\n  size = s | l\n\ntheme\n  ink = #111111\n\nparams\n  Map.tone = #505050\n    when size=l: reading.tone\n    when mode=a size=l: ink\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &tone);
    expect(
        &r,
        [
            ("a", "s", "app-param", &tone, Some("#505050ff")),
            ("a", "l", "app-token", "ink", Some("#111111ff")),
            ("b", "s", "app-param", &tone, Some("#505050ff")),
            ("b", "l", "package", "reading.deep", Some("#303030ff")),
        ],
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The per-side correction's counterexample (dtool2's T1 reviewers, 18:55):
/// an app's `params` line whose `when` lines are all on an axis `under
/// scheme=dark`. Light follows the line's own reference to the package's
/// colour; dark is the app's literal in every tuple. One level per tuple
/// named the dark side's for Light.
#[test]
fn a_params_line_with_dark_only_whens_wins_dark_and_follows_its_reference_light() {
    let dir = reading("levels-sides", "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = #111111\n\nparams\n  Map.pin = reading.chain\n    when dark-style=black: #000000\n    when dark-style=dim: #111111\n");
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &format!("{MAP}.pin"));
    let app = ("app-param".to_string(), format!("{MAP}.pin"));
    let package = ("package".to_string(), "reading.pin".to_string());
    assert_eq!(side(&r, "light"), [package.clone(), package], "{r}");
    assert_eq!(side(&r, "dark"), [app.clone(), app], "{r}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each tuple's `(modes, light, dark)`: each side's level, name and value.
fn per_side(r: &Value) -> Vec<(Value, [String; 3], [String; 3])> {
    let side = |t: &Value, k: &str| -> [String; 3] {
        let at = |v: &Value| v.as_str().unwrap_or("").to_string();
        [at(&t["level"][k]), at(&t["name"][k]), at(&t[k])]
    };
    r["tuples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["modes"].clone(), side(t, "light"), side(t, "dark")))
        .collect()
}

/// Round 1 of the per-side follow-up (Astra 1, Grok 1): `when` lines that
/// mix an axis `under scheme=dark` with a normal one, and a normal-only
/// `when` that outranks one under dark. Light takes the most specific
/// holding `when` naming no axis under dark, else the line's own value;
/// dark takes the most specific holding `when` of all. On an app's
/// `params` line and on a package token, values asserted beside levels.
#[test]
fn mixed_axis_whens_are_walked_per_side() {
    let dir = reading("levels-mixed", "tamber t\n\naxes\n  mode = a | b\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = light-dark(#111111, #222222)\n\nparams\n  Map.pin = reading.chain\n    when mode=b: ink\n    when mode=b dark-style=dim: #000000\n");
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &format!("{MAP}.pin"));
    let pin = format!("{MAP}.pin");
    let row = |level: &str, name: &str, value: &str| {
        [level.to_string(), name.to_string(), value.to_string()]
    };
    let package = row("package", "reading.pin", "#333333ff");
    assert_eq!(
        per_side(&r),
        [
            (
                json!({"mode": "a", "dark-style": "black"}),
                package.clone(),
                package.clone()
            ),
            (
                json!({"mode": "a", "dark-style": "dim"}),
                package.clone(),
                package.clone()
            ),
            (
                json!({"mode": "b", "dark-style": "black"}),
                row("app-token", "ink", "#111111ff"),
                row("app-token", "ink", "#222222ff")
            ),
            (
                json!({"mode": "b", "dark-style": "dim"}),
                row("app-token", "ink", "#111111ff"),
                row("app-param", &pin, "#000000ff")
            ),
        ],
        "{r}"
    );
    drop(s);
    // A package token: a two-axis normal `when` outranks one under dark.
    std::fs::write(dir.join("app.tamber"), "tamber t\n\naxes\n  mode = a | b\n  size = s | l\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = #111111\n").unwrap();
    let pkg = dir.join("node_modules/@acme/reading/reading.tamber");
    let text = std::fs::read_to_string(&pkg).unwrap().replace(
        "  reading.pin = #333333\n",
        "  reading.pin = #333333\n  reading.tone = #101010\n    when mode=b size=l: reading.deep\n    when dark-style=dim: #111111\n  reading.deep = #303030\n",
    ) + "    tone: color = reading.tone\n";
    std::fs::write(&pkg, text).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let r = ask(&mut s, "resolve", &format!("{MAP}.tone"));
    let tone = |v: &str| row("package", "reading.tone", v);
    let deep = row("package", "reading.deep", "#303030ff");
    let got = per_side(&r);
    let at = |mode: &str, size: &str, dark: &str| {
        got.iter()
            .find(|(m, ..)| *m == json!({"mode": mode, "size": size, "dark-style": dark}))
            .map(|(_, l, d)| (l.clone(), d.clone()))
            .unwrap_or_else(|| panic!("{mode} {size} {dark}: {r}"))
    };
    assert_eq!(
        at("a", "s", "dim"),
        (tone("#101010ff"), tone("#111111ff")),
        "{r}"
    );
    assert_eq!(
        at("a", "s", "black"),
        (tone("#101010ff"), tone("#101010ff")),
        "{r}"
    );
    assert_eq!(
        at("b", "l", "dim"),
        (deep.clone(), deep.clone()),
        "the two-axis when outranks: {r}"
    );
    assert_eq!(at("b", "l", "black"), (deep.clone(), deep), "{r}");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}
