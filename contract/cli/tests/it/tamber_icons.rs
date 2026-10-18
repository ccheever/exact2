//! @ref LLP 1121 D10 (stage 6 F) — icon tokens at build: each icon's SVG read
//! as one shape under its viewBox, a source per target the app ships, a
//! literal `symbol:icon.…` written as the source form hosts read, the
//! plan's `icons` table for a computed source, and `symbol:sf/…` naming the
//! role whose Apple name it is or the icon whose `sf` it is.

use std::path::PathBuf;

const REPLY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="M760-200v-160H200Z"/></svg>"#;
const TWO: &str =
    r#"<svg viewBox="0 0 24 24"><path fill="red" d="M0 0h1"/><path fill="blue" d="M2 2h1"/></svg>"#;

fn app(name: &str, targets: &str, design: &str, view: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-icons-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets/icons")).unwrap();
    std::fs::write(dir.join("assets/icons/reply.svg"), REPLY).unwrap();
    std::fs::write(dir.join("assets/icons/two.svg"), TWO).unwrap();
    std::fs::write(dir.join("app.json"), format!("{{\"app\": {{\"id\": \"dev.test.icons\", \"name\": \"Icons\"}}, \"host\": {{{targets}}}}}")).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  ink = #000000\n\nicons\n{design}"),
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    dir.join("app.contract")
}

fn view(source: &str) -> String {
    format!("component App\n  view\n    image \"{source}\"\n")
}

#[test]
fn a_literal_icon_is_written_as_its_source_form() {
    let path = app(
        "literal",
        "\"ios\": {}, \"web\": {}",
        "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n  icon.unused = icon(sf \"star\", svg \"icons/reply.svg\")\n",
        &view("symbol:icon.reply"),
    );
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let form = "symbol:icon|bubble.left|0 -960 960 960|M760-200v-160H200Z|icons/reply.svg";
    assert!(plan.strings.iter().any(|s| s == form), "{:?}", plan.strings);
    // The table keeps what a literal named, not the rest.
    assert_eq!(plan.icons.len(), 1);
}

#[test]
fn a_shipped_target_without_a_source_is_refused() {
    let path = app(
        "missing",
        "\"ios\": {}, \"web\": {}",
        "  icon.share = icon(sf \"square.and.arrow.up.circle\")\n",
        &view("symbol:icon.share"),
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-icon-missing", "{e:?}");
    assert!(
        e.message.contains("icon.share has no source for web"),
        "{e:?}"
    );
    // Several colours: the web alone draws it as the file.
    let design = "  icon.two = icon(svg \"icons/two.svg\")\n";
    let path = app(
        "colours",
        "\"linux\": {}, \"web\": {}",
        design,
        &view("symbol:icon.two"),
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-icon-svg", "{e:?}");
    assert!(
        e.message.contains("linux") && e.message.contains("more than one colour"),
        "{e:?}"
    );
    // Apple draws the SF Symbol: an SVG alone is no source there.
    let svg_only = "  icon.svg = icon(svg \"icons/reply.svg\")\n";
    let path = app("apple", "\"ios\": {}", svg_only, &view("symbol:icon.svg"));
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-icon-missing", "{e:?}");
    assert!(
        e.message.contains("icon.svg has no source for ios"),
        "{e:?}"
    );
    let path = app("web-only", "\"web\": {}", design, &view("symbol:icon.two"));
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        plan.strings
            .iter()
            .any(|s| s == "symbol:icon||0 0 24 24||icons/two.svg"),
        "{:?}",
        plan.strings
    );
}

#[test]
fn an_sf_name_draws_its_role_or_its_icon_and_says_so() {
    let design = "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n";
    let view = "component App\n  view\n    column\n      image \"symbol:sf/chevron.backward\"\n      image \"symbol:sf/bubble.left\"\n      image \"symbol:sf/flag.checkered\"\n";
    let path = app("sf", "\"ios\": {}, \"web\": {}", design, view);
    let (plan, warnings) = contract::compile_path_all_warned(&path, false);
    let (plan, _) = plan.unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        plan.strings.iter().any(|s| s == "symbol:back"),
        "{:?}",
        plan.strings
    );
    assert!(
        plan.strings
            .iter()
            .any(|s| s.starts_with("symbol:icon|bubble.left|")),
        "{:?}",
        plan.strings
    );
    let said: Vec<(&str, &str)> = warnings
        .iter()
        .map(|w| (w.id.as_str(), w.message.as_str()))
        .collect();
    assert!(
        said.contains(&(
            "lower-symbol-alias",
            "symbol `sf/chevron.backward` is the role `back`: write `symbol:back`"
        )),
        "{said:?}"
    );
    assert!(
        said.contains(&(
            "lower-symbol-alias",
            "symbol `sf/bubble.left` is the icon `icon.reply`: write `symbol:icon.reply`"
        )),
        "{said:?}"
    );
    assert!(
        said.iter().any(|(id, m)| *id == "lower-symbol-platform"
            && m.contains("sf/flag.checkered")
            && m.contains("web")),
        "{said:?}"
    );
}

#[test]
fn a_computed_source_keeps_every_icon_and_the_runner_resolves_it() {
    let design = "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n  icon.flag = icon(sf \"flag\", svg \"icons/reply.svg\")\n";
    let view =
        "component App\n  state name = \"bubble.left\"\n  view\n    image `symbol:sf/${name}`\n";
    let path = app("computed", "\"ios\": {}, \"web\": {}", design, view);
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(plan.icons.len(), 2);
    let source = |text: &str| {
        exact_runner::bridge::icon_source(
            &plan,
            exact_kernel::PropId::ImageSource,
            exact_kernel::PropValue::Str(text.into()),
        )
    };
    let form = "symbol:icon|bubble.left|0 -960 960 960|M760-200v-160H200Z|icons/reply.svg";
    assert_eq!(
        source("symbol:sf/bubble.left"),
        exact_kernel::PropValue::Str(form.into())
    );
    assert_eq!(
        source("symbol:icon.reply"),
        exact_kernel::PropValue::Str(form.into())
    );
    assert_eq!(
        source("symbol:back"),
        exact_kernel::PropValue::Str("symbol:back".into())
    );
}

/// Review round 1: a native button takes an icon token; a template that
/// starts with other text keeps no icon; `deploy.binaries` ships a target;
/// the source graph names each icon's SVG, so a watcher rebuilds on its edit.
#[test]
fn a_button_a_url_template_binaries_and_the_graph() {
    let design = "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n";
    let view = "component App\n  state id = \"1\"\n  view\n    column\n      button appearance=\"auto\" aria-label=\"Reply\"\n        image \"symbol:icon.reply\"\n      image `https://example.com/${id}.png`\n";
    let path = app("button", "\"ios\": {}", design, view);
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(plan.icons.len(), 1, "named by the button's literal only");
    let graph = contract::source_graph(&path);
    assert!(
        graph
            .sources
            .iter()
            .any(|s| s.path.ends_with("assets/icons/reply.svg")),
        "{:?}",
        graph.sources.iter().map(|s| &s.path).collect::<Vec<_>>()
    );
    let dir = path.parent().unwrap();
    // One not there yet is looked for, so making it rebuilds.
    std::fs::rename(dir.join("assets/icons/reply.svg"), dir.join("reply.svg")).unwrap();
    let graph = contract::source_graph(&path);
    assert!(
        graph
            .consulted
            .iter()
            .any(|p| p.ends_with("assets/icons/reply.svg")),
        "{:?}",
        graph.consulted
    );
    std::fs::write(
        dir.join("app.json"),
        "{\"app\": {\"id\": \"dev.test.icons\", \"name\": \"Icons\"}, \"host\": {\"ios\": {}}, \"deploy\": {\"binaries\": {\"linux\": \"manual\"}}}",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  icon.sf = icon(sf \"star\")\n",
    )
    .unwrap();
    std::fs::write(
        &path,
        "component App\n  view\n    image \"symbol:icon.sf\"\n",
    )
    .unwrap();
    let e = contract::compile_path(&path).unwrap_err();
    assert!(e.message.contains("no source for linux"), "{e:?}");
}

/// theme2's ruling: an app's target crate (`web/`, `linux/`, `android/`) is
/// a target it ships.
#[test]
fn a_target_crate_is_a_shipped_target() {
    let path = app(
        "crate",
        "\"ios\": {}",
        "  icon.sf = icon(sf \"star\")\n",
        &view("symbol:icon.sf"),
    );
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir.join("web")).unwrap();
    std::fs::write(dir.join("web/Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    let e = contract::compile_path(&path).unwrap_err();
    assert!(e.message.contains("no source for web"), "{e:?}");
}

/// A template of literals alone (a component's argument, inlined) is the
/// literal it folds to: `symbol:sf/${symbol}` with `symbol="bubble.left"`
/// is the icon, on every host, the JS target's literal rows included.
#[test]
fn a_template_of_literals_is_its_literal() {
    let design = "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n";
    let view = "component App\n  view\n    Row(symbol=\"bubble.left\")\n\ncomponent Row\n  props\n    symbol: string\n  view\n    image `symbol:sf/${symbol}`\n";
    let path = app("template", "\"ios\": {}, \"web\": {}", design, view);
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        plan.strings
            .iter()
            .any(|s| s.starts_with("symbol:icon|bubble.left|")),
        "{:?}",
        plan.strings
    );
}
/// An app `exact new` made ships Linux by its `linux/` crate and
/// `deploy.store.linux`: the warning names both, the role by the same name,
/// the SVG the Linux host draws, and the one step that stops shipping Linux
/// (the euxy port spent 20 minutes finding them).
#[test]
fn the_platform_warning_names_what_ships_and_how_to_stop() {
    let view = "component App\n  view\n    column\n      image \"symbol:sf/pencil\"\n      image \"symbol:sf/flag.checkered\"\n";
    let path = app(
        "ships",
        "\"ios\": {}, \"web\": {}}, \"deploy\": {\"store\": {\"ios\": \"0\", \"linux\": \"0\"}",
        "",
        view,
    );
    let root = path.parent().unwrap();
    std::fs::create_dir_all(root.join("linux")).unwrap();
    std::fs::write(root.join("linux/Cargo.toml"), "").unwrap();
    let (plan, warnings) = contract::compile_path_all_warned(&path, false);
    plan.unwrap_or_else(|e| panic!("{e:?}"));
    let said: Vec<&str> = warnings
        .iter()
        .filter(|w| w.id == "lower-symbol-platform")
        .map(|w| w.message.as_str())
        .collect();
    let [pencil, flag] = said[..] else {
        panic!("{said:?}")
    };
    for part in [
        "linux (the `linux/` crate, app.json `deploy.store.linux`)",
        "web (app.json `host.web`)",
        "The role `pencil` draws on every host (Apple's `pencil.line`): write `symbol:pencil`",
        "filled `<path>`s in one opaque colour",
        "`bun exact.mjs update --without linux`",
    ] {
        assert!(pencil.contains(part), "{part}: {pencil}");
    }
    // The advice is said once a build; a later use is a line.
    assert_eq!(
        flag,
        "symbol `sf/flag.checkered` is blank on linux and web too: declare `icon.flag-checkered` as the first `lower-symbol-platform` warning shows, and write `symbol:icon.flag-checkered`"
    );
}

/// The euxy port's forty SF Symbols printed forty paragraphs and filled the
/// cap, so the warnings after them were not printed: the later uses are a
/// line each, outside the cap, and a later warning still prints.
#[test]
fn later_platform_symbols_are_a_line_each_outside_the_cap() {
    let uses: String = (0..contract_lower::MAX_WARNINGS + 5)
        .map(|i| format!("      image \"symbol:sf/flag.{i}\"\n"))
        .collect();
    let view = format!("component App\n  view\n    column\n{uses}      image \"symbol:sf/pencil\"\n      image \"symbol:gearshape\"\n");
    let path = app("repeats", "\"ios\": {}, \"web\": {}", "", &view);
    let (plan, warnings) = contract::compile_path_all_warned(&path, false);
    plan.unwrap_or_else(|e| panic!("{e:?}"));
    let platform: Vec<&str> = warnings
        .iter()
        .filter(|w| w.id == "lower-symbol-platform")
        .map(|w| w.message.as_str())
        .collect();
    assert_eq!(
        platform.len(),
        contract_lower::MAX_WARNINGS + 6,
        "{platform:?}"
    );
    assert!(
        platform[0].contains("in app.tamber's `icons`"),
        "{}",
        platform[0]
    );
    assert!(
        platform[1..].iter().all(|m| !m.contains("app.tamber")),
        "{platform:?}"
    );
    assert_eq!(
        platform.last().copied(),
        Some("symbol `sf/pencil` is blank on web too: write `symbol:pencil`, the role, which draws on every host")
    );
    assert!(
        warnings.iter().any(|w| w.id == "lower-symbol-alias"),
        "{warnings:?}"
    );
}

/// Review round 1: a nested template of literals folds too, and a template
/// of literals is linted as its literal.
#[test]
fn a_nested_literal_template_folds_and_lints() {
    let design = "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n";
    let view = "component App\n  view\n    column\n      image `${`symbol:icon.reply`}`\n      Row(symbol=\"flag.checkered\")\n\ncomponent Row\n  props\n    symbol: string\n  view\n    image `symbol:sf/${symbol}`\n";
    let path = app("nested", "\"ios\": {}, \"web\": {}", design, view);
    let (plan, warnings) = contract::compile_path_all_warned(&path, false);
    let (plan, _) = plan.unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        plan.strings
            .iter()
            .any(|s| s.starts_with("symbol:icon|bubble.left|")),
        "{:?}",
        plan.strings
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.id == "lower-symbol-platform" && w.message.contains("flag.checkered")),
        "{warnings:?}"
    );
}

/// A refused icon's uses say nothing more: its own error is the one
/// (x2-perf4's stage 6 gate).
#[test]
fn a_refused_icons_uses_are_not_refused_again() {
    let design = "  icon.gone = icon(sf \"star\", svg \"icons/gone.svg\")\n";
    let view = "component App\n  view\n    column\n      image \"symbol:icon.gone\"\n      image \"symbol:icon.gone\"\n";
    let path = app("refused", "\"ios\": {}, \"web\": {}", design, view);
    let Err(errors) = contract::compile_path_all(&path, false) else {
        panic!("the missing SVG is refused");
    };
    let ids: Vec<&str> = errors.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["tamber-icon-svg"], "{errors:?}");
}
