//! @ref LLP 1121 stage 9 C1 — `explain` answers an icon (its `describe`
//! entry, with which source each shipped target draws) and a param (where
//! it is declared, its default as written, and the design's line that sets
//! the app's value); `describe`'s icon entries say `draws` too.
#![cfg(unix)]

use super::tamber_serve::{app, Serve};
use serde_json::json;

const SVG: &str = r#"<svg viewBox="0 -960 960 960"><path d="M760-200v-160H200Z"/></svg>"#;

/// C1 review round 1: a param's `set` is the line of the design asked
/// about (`file`), not always the app's.
#[test]
fn a_params_set_line_is_the_asked_designs() {
    let dir = app(
        "explain-file",
        "tamber t\n\ntheme\n  accent.fill = #0a7aff\n",
    );
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column\n      Card()\n\ncomponent Card\n  params\n    fill: color = \"accent.fill\"\n  view\n    column background-color=\"param.fill\"\n      text \"c\"\n").unwrap();
    std::fs::write(
        dir.join("other.tamber"),
        "tamber t\n\ntheme\n  accent.fill = #0a7aff\n\nparams\n  Card.fill = #123456\n",
    )
    .unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (app_design, _) = s.ask(json!({"method": "explain", "token": "Card.fill"}));
    assert!(app_design["ok"]["param"]["set"].is_null(), "{app_design}");
    let (other, _) =
        s.ask(json!({"method": "explain", "token": "Card.fill", "file": "other.tamber"}));
    let set = &other["ok"]["param"]["set"];
    assert_eq!(set["file"], "other.tamber", "{other}");
    assert_eq!(set["value"], "#123456", "{other}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explain_answers_an_icon_and_describe_says_what_each_target_draws() {
    let design = "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  icon.both = icon(sf \"bubble.left\", material \"reply\", svg \"icons/reply.svg\")\n  icon.apple = icon(sf \"star\")\n  icon.shape = icon(svg \"icons/reply.svg\")\n";
    let dir = app("explain-icon", design);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/reply.svg"), SVG).unwrap();
    std::fs::write(dir.join("app.json"), "{\"app\": {\"id\": \"dev.test.icons\", \"name\": \"Icons\"}, \"host\": {\"ios\": {}, \"web\": {}, \"linux\": {}}}").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (both, _) = s.ask(json!({"method": "explain", "token": "icon.both"}));
    let ok = &both["ok"];
    assert_eq!(ok["kind"], "icon", "{both}");
    assert_eq!(ok["icon"]["sf"], "bubble.left");
    assert_eq!(ok["icon"]["material"], "reply");
    assert_eq!(
        ok["icon"]["draws"],
        json!({"ios": "sf", "linux": "svg", "web": "svg"})
    );
    assert!(ok["value"].as_str().unwrap().starts_with("icon("), "{both}");
    assert_eq!(ok["whens"], json!([]));
    let draws = |s: &mut Serve, name: &str| {
        s.ask(json!({"method": "explain", "token": name})).0["ok"]["icon"]["draws"].clone()
    };
    assert_eq!(
        draws(&mut s, "icon.apple"),
        json!({"ios": "sf", "linux": null, "web": null})
    );
    assert_eq!(
        draws(&mut s, "icon.shape"),
        json!({"ios": "svg", "linux": "svg", "web": "svg"})
    );
    // describe's entry: the same draws.
    let (described, _) = s.ask(json!({"method": "describe"}));
    let icons = described["ok"]["ir"]["icons"].as_array().unwrap();
    let shape = icons.iter().find(|i| i["name"] == "icon.shape").unwrap();
    assert_eq!(
        shape["draws"],
        json!({"ios": "svg", "linux": "svg", "web": "svg"}),
        "{described}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explain_answers_a_param_its_declaration_and_the_apps_value() {
    let dir = app(
        "explain-param",
        "tamber t\n\ntheme\n  accent.fill = #0a7aff\n",
    );
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column\n      Card()\n\ncomponent Card\n  params\n    fill: color = \"accent.fill\"  control=\"color\"\n  view\n    column background-color=\"param.fill\"\n      text \"c\"\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (bare, _) = s.ask(json!({"method": "explain", "token": "Card.fill"}));
    let p = &bare["ok"]["param"];
    assert_eq!(p["component"], "Card", "{bare}");
    assert_eq!(p["kind"], "color");
    assert_eq!(p["file"], "app.contract");
    assert_eq!(p["line"], 8);
    assert_eq!(p["default"], "\"accent.fill\"");
    assert!(p["set"].is_null(), "{bare}");
    let design = dir.join("app.tamber");
    let (set, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": super::tamber_serve::digest(&design)}, "ops": [{"op": "set-param", "name": "Card.fill", "value": "#fafafa"}]}));
    assert!(set["ok"].is_object(), "{set}");
    let (now, _) = s.ask(json!({"method": "explain", "token": "Card.fill"}));
    let set = &now["ok"]["param"]["set"];
    assert_eq!(set["file"], "app.tamber", "{now}");
    assert_eq!(set["value"], "#fafafa", "{now}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C1 review round 2: an icon naming another is explained as written,
/// that one its chain; a one-colour shape draws everywhere but Apple, a
/// shape of several colours on the web alone, a missing file nowhere; a
/// padded param default keeps its chain.
#[test]
fn explain_reads_icons_as_written_and_svgs_as_the_build_does() {
    let design = "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  icon.home = icon(sf \"house\")\n  icon.alias = icon.home\n  icon.two = icon(svg \"icons/two.svg\")\n  icon.gone = icon(sf \"star\", svg \"icons/gone.svg\")\n";
    let dir = app("explain-r2", design);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/two.svg"), r#"<svg viewBox="0 0 24 24"><path fill="red" d="M0 0h1"/><path fill="blue" d="M2 2h1"/></svg>"#).unwrap();
    std::fs::write(dir.join("app.json"), "{\"app\": {\"id\": \"dev.test.icons\", \"name\": \"Icons\"}, \"host\": {\"ios\": {}, \"web\": {}, \"linux\": {}}}").unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  params\n    base: icon = \"icon(sf \\\"star\\\")\"\n    shown: icon = \" param.base \"\n  view\n    image \"symbol:param.shown\"\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let ask = |s: &mut Serve, name: &str| s.ask(json!({"method": "explain", "token": name})).0;
    let alias = ask(&mut s, "icon.alias");
    assert_eq!(alias["ok"]["value"], "icon.home", "{alias}");
    assert_eq!(alias["ok"]["names"], json!(["icon.home"]), "{alias}");
    assert_eq!(
        ask(&mut s, "icon.two")["ok"]["icon"]["draws"],
        json!({"ios": null, "linux": null, "web": "svg"})
    );
    assert_eq!(
        ask(&mut s, "icon.gone")["ok"]["icon"]["draws"],
        json!({"ios": "sf", "linux": null, "web": null})
    );
    let shown = ask(&mut s, "App.shown");
    let chain = shown["ok"]["names"]
        .as_array()
        .unwrap_or_else(|| panic!("{shown}"));
    assert!(
        chain.len() == 1 && chain[0].as_str().unwrap().ends_with("App.base"),
        "{shown}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// C1 review round 3: in an app with no params, an icon naming another is
/// read as the build reads it, not refused.
#[test]
fn an_icon_alias_reads_in_an_app_without_params() {
    let design = "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  icon.home = icon(sf \"house\")\n  icon.alias = icon.home\n";
    let dir = app("explain-alias-bare", design);
    let mut s = Serve::start(&dir, &[]);
    let (alias, _) = s.ask(json!({"method": "explain", "token": "icon.alias"}));
    assert_eq!(alias["ok"]["value"], "icon.home", "{alias}");
    assert_eq!(alias["ok"]["names"], json!(["icon.home"]), "{alias}");
    assert_eq!(alias["ok"]["icon"]["sf"], "house", "{alias}");
    let _ = std::fs::remove_dir_all(&dir);
}
