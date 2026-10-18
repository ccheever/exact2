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

/// Stage 9 C2: `uses` of an icon lists the images that draw it, directly
/// or through an icon param; a motion token's literal in a component used
/// twice is two sites, each its plan node, the literal kept; one in a
/// component no one uses keeps `site: null`.
#[test]
fn uses_lists_an_icons_images_and_a_motion_tokens_instances() {
    let design = "tamber t\n\ntheme\n  motion.fast = 150ms\n\nicons\n  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n";
    let dir = app("uses-c2", design);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/reply.svg"), SVG).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column\n      image \"symbol:icon.reply\" testId=\"direct\"\n      Card()\n      Card()\n\ncomponent Card\n  params\n    glyph: icon = \"icon.reply\"\n  view\n    column transition=\"opacity motion.fast\"\n      image \"symbol:param.glyph\"\n\ncomponent Unused\n  view\n    column transition=\"opacity motion.fast\"\n      text \"u\"\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (icon, _) = s.ask(json!({"method": "uses", "token": "icon.reply"}));
    let sites = icon["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{icon}"));
    assert_eq!(sites.len(), 3, "the direct image and Card's twice: {icon}");
    assert!(
        sites
            .iter()
            .all(|s| s["row"] == "symbol" && s["site"].is_u64()),
        "{icon}"
    );
    assert_eq!(
        sites
            .iter()
            .filter(|s| s["via"]["token"]
                .as_str()
                .is_some_and(|t| t.ends_with("Card.glyph")))
            .count(),
        2,
        "{icon}"
    );
    let (motion, _) = s.ask(json!({"method": "uses", "token": "motion.fast"}));
    let sites = motion["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{motion}"));
    let card: Vec<_> = sites.iter().filter(|s| s["line"] == 12).collect();
    assert_eq!(card.len(), 2, "Card used twice: {motion}");
    assert!(
        card.iter()
            .all(|s| s["site"].is_u64() && s["literal"]["line"] == 12),
        "{motion}"
    );
    assert_ne!(card[0]["site"], card[1]["site"]);
    let unused: Vec<_> = sites.iter().filter(|s| s["line"] == 17).collect();
    assert_eq!(unused.len(), 1, "{motion}");
    assert!(
        unused[0]["site"].is_null() && unused[0].get("literal").is_none(),
        "{motion}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// C2 review round 1: an icon is read from an image's source only (not a
/// text, an attribute or a comment), a parenthesized source over lines
/// too; an icon param asked about gives its images; motion sites are
/// located at their node, an animation's longhands included; a design
/// the app doesn't build has none.
#[test]
fn uses_reads_sources_not_text() {
    let design = "tamber t\n\ntheme\n  motion.fast = 150ms\n\nicons\n  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n";
    let dir = app("uses-c2-r1", design);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/reply.svg"), SVG).unwrap();
    let view = "component App\n  view\n    column\n      text \"symbol:icon.reply\"\n      image \"photo.png\" aria-label=\"symbol:icon.reply\" // \"symbol:icon.reply\"\n      image (\n        \"symbol:icon.reply\")\n      Card()\n\ncomponent Card\n  params\n    glyph: icon = \"icon.reply\"\n  view\n    column animation-name=\"pulse\" animation-duration=\"motion.fast\"\n      image \"symbol:param.glyph\"\n\nkeyframes pulse\n  from opacity=0\n  to opacity=1\n";
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (icon, _) = s.ask(json!({"method": "uses", "token": "icon.reply"}));
    let sites = icon["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{icon}"));
    let lines: Vec<u64> = sites.iter().map(|s| s["line"].as_u64().unwrap()).collect();
    assert_eq!(
        lines,
        [6, 15],
        "the parenthesized image and Card's, not the text or the attribute: {icon}"
    );
    let (param, _) = s.ask(json!({"method": "uses", "token": "Card.glyph"}));
    let sites = param["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{param}"));
    assert_eq!(sites.len(), 1, "{param}");
    assert_eq!(sites[0]["line"], 15);
    assert!(
        param["ok"]["token"]
            .as_str()
            .unwrap()
            .ends_with("Card.glyph"),
        "{param}"
    );
    let (motion, _) = s.ask(json!({"method": "uses", "token": "motion.fast"}));
    let sites = motion["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{motion}"));
    let instance = sites
        .iter()
        .find(|s| s["site"].is_u64())
        .unwrap_or_else(|| panic!("an animation longhand's instance: {motion}"));
    assert_eq!(instance["line"], 14, "the node's place: {motion}");
    assert_eq!(instance["col"], 5, "{motion}");
    assert_eq!(instance["literal"]["line"], 14);
    assert!(
        instance["literal"]["col"].as_u64().unwrap() > 40,
        "{motion}"
    );
    std::fs::write(dir.join("other.tamber"), design).unwrap();
    let (other, _) =
        s.ask(json!({"method": "uses", "token": "icon.reply", "file": "other.tamber"}));
    assert_eq!(other["ok"]["sites"], json!([]), "{other}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C2 review round 2: a motion literal in a condition's arm or a `+`'s
/// side, or on a style two nodes take, has its instances (with their call
/// chains); an image naming it in a `match` arm is a site.
#[test]
fn uses_follows_expressions_styles_and_icon_aliases() {
    let design = "tamber t\n\ntheme\n  motion.fast = 150ms\n\nicons\n  icon.base = icon(sf \"house\", svg \"icons/reply.svg\")\n";
    let dir = app("uses-c2-r2", design);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/reply.svg"), SVG).unwrap();
    let view = "style Fade\n  transition=\"opacity motion.fast\"\n\ncomponent App\n  state on = true\n  state pick = some(\"x\")\n  view\n    column\n      Card()\n      Card()\n      box class=Fade\n      box class=Fade\n      image (match pick { case some(p) => \"symbol:icon.base\", case none => \"symbol:star\" })\n\ncomponent Card\n  state on = true\n  params\n    glyph: icon = \"icon.base\"\n  view\n    column transition=(on ? \"opacity motion.fast\" : \"none\") animation-duration=(\"\" + \"motion.fast\")\n      image \"symbol:param.glyph\"\n";
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (motion, _) = s.ask(json!({"method": "uses", "token": "motion.fast"}));
    let sites = motion["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{motion}"));
    let placed: Vec<_> = sites.iter().filter(|s| s["site"].is_u64()).collect();
    // Card's two literals, twice each, and the style's on two boxes.
    assert_eq!(placed.len(), 6, "{motion}");
    assert!(
        placed
            .iter()
            .all(|s| s["literal"].is_object() && s["chain"].is_array()),
        "{motion}"
    );
    assert!(
        sites.iter().all(|s| s["site"].is_u64()),
        "every literal reached: {motion}"
    );
    let (icon, _) = s.ask(json!({"method": "uses", "token": "icon.base"}));
    let sites = icon["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{icon}"));
    assert_eq!(sites.len(), 3, "the match image and Card's twice: {icon}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C2 after round 3: a button's text child, which shares its span, is no
/// motion site; an icon param asked about reads its own image directly
/// (no `via` back to itself).
#[test]
fn uses_keeps_to_the_nodes_that_read() {
    let design = "tamber t\n\ntheme\n  motion.fast = 150ms\n\nicons\n  icon.base = icon(sf \"house\", svg \"icons/reply.svg\")\n";
    let dir = app("uses-c2-r3", design);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/reply.svg"), SVG).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n      button \"Go\" press=go appearance=\"none\" transition=\"opacity motion.fast\"\n      Card()\n\ncomponent Card\n  params\n    glyph: icon = \"icon.base\"\n  view\n    image \"symbol:param.glyph\"\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (motion, _) = s.ask(json!({"method": "uses", "token": "motion.fast"}));
    let sites = motion["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{motion}"));
    assert_eq!(sites.len(), 1, "the button, not its text child: {motion}");
    let (param, _) = s.ask(json!({"method": "uses", "token": "Card.glyph"}));
    let sites = param["ok"]["sites"]
        .as_array()
        .unwrap_or_else(|| panic!("{param}"));
    assert_eq!(sites.len(), 1, "{param}");
    assert!(sites[0].get("via").is_none(), "{param}");
    let _ = std::fs::remove_dir_all(&dir);
}
