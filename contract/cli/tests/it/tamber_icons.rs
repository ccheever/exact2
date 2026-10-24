//! @ref LLP 1121 D10 (stage 6 F) — icon tokens at build: each icon's SVG read
//! as one shape under its viewBox, a source per target the app ships, a
//! literal `symbol:icon.…` written as the source form hosts read, the
//! plan's `icons` table for a computed source, and `symbol:sf/…` naming the
//! role whose Apple name it is or the icon whose `sf` it is.

use std::path::PathBuf;

/// The SVG every host but the web draws (`contract_lower`'s `icons::TEMPLATE`).
const TEMPLATE: &str = "filled `<path>`s in one opaque colour under the `<svg>`'s viewBox, all by one `fill-rule` (nonzero or evenodd), with no stroke, transform, opacity or other element";

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
    // It names both forms that build: an SVG, or (the euxy port, Apple-only with none) a bare SF Symbol.
    assert_eq!(
        e.message,
        format!("icon.share has no source for web, which draws an icon only from an `svg` source: add `svg \"icons/share.svg\"`, with `assets/icons/share.svg` drawn as {TEMPLATE} (a Material Symbols SVG is). With no SVG for it, declare no icon and write `symbol:sf/square.and.arrow.up.circle` where it is used: Apple draws it, and the web draws it blank, which the build says in one warning"),
    );
    let path = app(
        "missing-linux",
        "\"ios\": {}, \"linux\": {}, \"web\": {}",
        "  icon.share = icon(sf \"square.and.arrow.up.circle\")\n",
        &view("symbol:icon.share"),
    );
    let Err(all) = contract::compile_path_all(&path, false) else {
        panic!("compiled")
    };
    assert!(
        all.iter().any(|e| e.message.contains("linux and the web draw it blank, with a warning at each use. Not shipping linux? `bun exact.mjs update --without linux` removes it")),
        "{all:?}"
    );
    // An SF Symbol that is a role's Apple name is that role, drawn everywhere: no "blank" advice.
    let path = app(
        "missing-role",
        "\"ios\": {}, \"web\": {}",
        "  icon.play = icon(sf \"play.fill\")\n",
        &view("symbol:icon.play"),
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert!(
        e.message.ends_with("declare no icon and write `symbol:play-fill`, the role Apple draws as `play.fill`, which every host draws")
            && !e.message.contains("blank"),
        "{e:?}"
    );
    let advised = app(
        "advised-role",
        "\"ios\": {}, \"web\": {}",
        "",
        &view("symbol:play-fill"),
    );
    contract::compile_path(&advised).unwrap_or_else(|e| panic!("{e:?}"));
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
    // Apple draws a template's path too (stage 9 A): an SVG alone is a
    // source there; a file of more than one colour needs an SF Symbol.
    let svg_only = "  icon.svg = icon(svg \"icons/reply.svg\")\n";
    let path = app("apple", "\"ios\": {}", svg_only, &view("symbol:icon.svg"));
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        plan.strings.iter().any(|s| s.starts_with("symbol:icon||")),
        "an svg-only icon, no sf"
    );
    let two = "  icon.two = icon(svg \"icons/two.svg\")\n";
    let path = app("apple-two", "\"ios\": {}", two, &view("symbol:icon.two"));
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-icon-svg", "{e:?}");
    assert!(
        e.message.contains("ios") && e.message.contains("an SF Symbol"),
        "{e:?}"
    );
    // A watch draws only the SF Symbol yet: a one-colour SVG alone is no source there, said so.
    let path = app(
        "watch",
        "\"watchos\": {}",
        svg_only,
        &view("symbol:icon.svg"),
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-icon-missing", "{e:?}");
    assert!(
        e.message
            .contains("a watch draws an icon only as an SF Symbol"),
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
/// line each, and a later warning still prints. The group is outside the
/// cap, the advice its lines point at too, after a full cap of warnings.
#[test]
fn later_platform_symbols_are_a_line_each_outside_the_cap() {
    let uses: String = (0..contract_lower::MAX_WARNINGS + 5)
        .map(|i| format!("      image \"symbol:sf/flag.{i}\"\n"))
        .collect();
    let view = format!("component App\n  view\n    column\n{uses}      image \"symbol:sf/pencil\"\n      image \"symbol:gearshape\"\n");
    let aliases: String = (0..contract_lower::MAX_WARNINGS)
        .map(|_| "      image \"symbol:gearshape\"\n")
        .collect();
    let late = view.replace("    column\n", &format!("    column\n{aliases}"));
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
    let path = app("repeats-late", "\"ios\": {}, \"web\": {}", "", &late);
    let (plan, warnings) = contract::compile_path_all_warned(&path, false);
    plan.unwrap_or_else(|e| panic!("{e:?}"));
    let advice = warnings
        .iter()
        .filter(|w| w.id == "lower-symbol-platform")
        .find(|w| w.message.contains("in app.tamber's `icons`"));
    let line = 4 + contract_lower::MAX_WARNINGS as u32;
    assert_eq!(advice.map(|w| w.span.line), Some(line), "{warnings:?}");
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

/// Stage 9 A2: a grouped row led by an svg-only icon is a standard cell
/// whose symbol is "" and whose icon is its token form, which Apple draws
/// as the cell's template image; an `sf` icon's row names its symbol and
/// carries no icon.
#[test]
fn a_grouped_row_carries_an_svg_only_icon() {
    struct NoData;
    impl exact_runner::DataSource for NoData {
        fn query(
            &mut self,
            source: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(source.into()))
        }
    }
    let path = app(
        "grouped",
        "\"ios\": {}",
        "  icon.svg = icon(svg \"icons/reply.svg\")\n  icon.sf = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n",
        "component App\n  action go\n  view\n    column testId=\"root\" width=402 height=874\n      list appearance=\"auto\" testId=\"list\" flex=1\n        section\n          button press=go\n            image \"symbol:icon.svg\"\n            text \"Reply\"\n          button press=go\n            image \"symbol:icon.sf\"\n            text \"Answer\"\n",
    );
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, exact_kernel::Offer::definite(402.0, 874.0))
        .unwrap();
    let k = r.kernel();
    let list = k.node_by_key(k.find_by_test_id("list")[0]).unwrap().id;
    let rows = &k.grouped_list(list).unwrap().sections[0].rows;
    assert!(!rows[0].custom, "{:?}", rows[0]);
    assert_eq!(rows[0].symbol.as_deref(), Some(""));
    assert_eq!(
        rows[0].icon.as_deref(),
        Some("icon||0 -960 960 960|M760-200v-160H200Z|icons/reply.svg")
    );
    assert_eq!(rows[0].title.as_deref(), Some("Reply"));
    assert_eq!(rows[1].symbol.as_deref(), Some("bubble.left"));
    assert_eq!(rows[1].icon, None);
}

/// A design icon naming another builds in an app with params, an icon
/// param's default naming the alias too (beside `tamber_cli`'s test of an
/// app without params, theme2 22:12).
#[test]
fn an_icon_alias_builds_beside_params() {
    let design =
        "  icon.house = icon(sf \"house\", svg \"icons/reply.svg\")\n  icon.home = icon.house\n";
    let view = "component App\n  params\n    glyph: icon = \"icon.home\"\n    gap: length = \"4px\"  range=\"0..40\"\n  view\n    column  gap=\"param.gap\"\n      image \"symbol:icon.home\"\n      image \"symbol:param.glyph\"\n";
    let path = app("alias-params", "\"ios\": {}, \"web\": {}", design, view);
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let form = "symbol:icon|house|0 -960 960 960|M760-200v-160H200Z|icons/reply.svg";
    assert!(plan.strings.iter().any(|s| s == form), "{:?}", plan.strings);
}

/// A build's warnings come in rank order (`contract::warning_rank`): what
/// changes the native app first, symbols only the web draws blank last, and
/// as text those are one line, which `--json` lists each of. The euxy port's
/// header warning was one of 43, behind 21 of them and 13 aliases.
#[test]
fn warnings_that_change_the_native_app_come_first_and_web_only_symbols_are_one_line() {
    let view = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n      image \"symbol:sf/flag.one\"\n      image \"symbol:gearshape\"\n      image \"symbol:sf/flag.two\"\n      button press=go\n        image \"symbol:add\"\n";
    let path = app("ranked", "\"ios\": {}, \"web\": {}", "", view);
    let (plan, warnings) = contract::compile_path_all_warned(&path, false);
    plan.unwrap_or_else(|e| panic!("{e:?}"));
    let ids: Vec<&str> = warnings.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "lower-button-unlabelled",
            "lower-symbol-alias",
            "lower-symbol-platform",
            "lower-symbol-platform"
        ],
        "{warnings:#?}"
    );
    assert!(warnings[2..].iter().all(contract::web_only));
    let run = |json: bool| {
        let mut args = vec!["build", path.to_str().unwrap()];
        if json {
            args.push("--json");
        }
        std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(&args)
            .output()
            .unwrap()
    };
    let json: serde_json::Value = serde_json::from_slice(&run(true).stdout).unwrap();
    let listed: Vec<&str> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ids);
    let text = String::from_utf8(run(false).stderr).unwrap();
    let said: Vec<&str> = text.lines().filter(|l| l.contains(" warning [")).collect();
    assert_eq!(said.len(), 3, "{text}");
    assert!(said[0].contains("[lower-button-unlabelled]"), "{text}");
    assert!(
        said[2].ends_with("warning [lower-symbol-platform] 2 SF Symbol uses draw blank on the web, here first: an icon token or a role draws on every host (`contract build --json` says each, and how)"),
        "{text}"
    );
    // With Linux shipped too, a blank symbol is a native host's, said each.
    let path = app(
        "ranked-linux",
        "\"ios\": {}, \"web\": {}, \"linux\": {}",
        "",
        view,
    );
    let (_, warnings) = contract::compile_path_all_warned(&path, false);
    assert!(!warnings.iter().any(contract::web_only), "{warnings:#?}");
}

/// `contract tamber check` and `eval` read a design's icon SVGs under its
/// app's `assets/`, as its build does, whatever directory they are run from
/// and however the design is named: a design with no params is admitted by
/// compiling a copy, which sees the app's own assets.
#[test]
fn the_tamber_verbs_read_icon_svgs_under_the_apps_assets() {
    let path = app(
        "verbs",
        "\"ios\": {}, \"web\": {}",
        "  icon.reply = icon(sf \"bubble.left\", svg \"icons/reply.svg\")\n",
        &view("symbol:icon.reply"),
    );
    let dir = path.parent().unwrap();
    for (cwd, design) in [
        (std::env::temp_dir(), dir.join("app.tamber")),
        (dir.to_path_buf(), PathBuf::from("app.tamber")),
        (dir.to_path_buf(), PathBuf::from(".")),
    ] {
        for verb in ["check", "eval"] {
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
                .args(["tamber", verb])
                .arg(&design)
                .current_dir(&cwd)
                .output()
                .unwrap();
            let said = String::from_utf8_lossy(&out.stderr);
            assert!(
                out.status.success(),
                "{verb} {} from {}: {said}",
                design.display(),
                cwd.display()
            );
            assert!(!said.contains("tamber-icon-svg"), "{said}");
        }
    }
    // A design app.json names in another directory: the app's assets, not
    // the design's directory's.
    std::fs::create_dir_all(dir.join("design")).unwrap();
    std::fs::rename(dir.join("app.tamber"), dir.join("design/theme.tamber")).unwrap();
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    std::fs::write(
        dir.join("app.json"),
        manifest.replacen('{', "{\"tamber\": \"design/theme.tamber\", ", 1),
    )
    .unwrap();
    for input in [
        dir.to_path_buf(),
        dir.join("app.contract"),
        dir.join("design/theme.tamber"),
    ] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(["tamber", "check"])
            .arg(&input)
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}: {}",
            input.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    std::fs::rename(dir.join("design/theme.tamber"), dir.join("app.tamber")).unwrap();
    std::fs::write(dir.join("app.json"), manifest).unwrap();
    // The SVG is the app's still, its bytes as they were (copied, never
    // moved or linked).
    assert_eq!(
        std::fs::read_to_string(dir.join("assets/icons/reply.svg")).unwrap(),
        REPLY
    );
    // A read-only SVG two icons share is read once into the copy.
    let svg = dir.join("assets/icons/reply.svg");
    let mut perms = std::fs::metadata(&svg).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&svg, perms.clone()).unwrap();
    let design = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        format!("{design}  icon.again = icon(sf \"star\", svg \"icons/reply.svg\")\n"),
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "check"])
        .arg(dir.join("app.tamber"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&svg, perms).unwrap();
    // An SVG the app lacks is still refused.
    std::fs::remove_file(dir.join("assets/icons/reply.svg")).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "eval"])
        .arg(dir.join("app.tamber"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("tamber-icon-svg"));
    let _ = std::fs::remove_dir_all(dir);
}

/// Stage 14 C2: an SVG whose paths fill even-odd is a template; its plan row
/// and its literal source form carry `evenodd:` before the path (the kernel
/// reads it back as the icon's rule); a nonzero icon's row and form are as
/// before; an SVG that mixes rules on Linux's and the web's targets is
/// refused naming its paths.
#[test]
fn an_even_odd_icon_carries_its_rule_in_the_row_and_the_form() {
    let path = app(
        "evenodd",
        "\"ios\": {}, \"web\": {}, \"linux\": {}",
        "  icon.ring = icon(svg \"icons/ring.svg\")\n  icon.reply = icon(svg \"icons/reply.svg\")\n",
        "component App\n  view\n    column\n      image \"symbol:icon.ring\"\n      image \"symbol:icon.reply\"\n",
    );
    let dir = path.parent().unwrap();
    std::fs::write(
        dir.join("assets/icons/ring.svg"),
        r#"<svg viewBox="0 0 24 24"><g fill-rule="evenodd"><path d="M2 2H22V22H2Z M8 8H16V16H8Z"/><path d="M0 0h1v1H0Z"/></g></svg>"#,
    )
    .unwrap();
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let form =
        "symbol:icon||0 0 24 24|evenodd:M2 2H22V22H2Z M8 8H16V16H8Z;M0 0h1v1H0Z|icons/ring.svg";
    assert!(plan.strings.iter().any(|s| s == form), "{:?}", plan.strings);
    let row = |name: &str| {
        plan.icons
            .iter()
            .find(|i| plan.str(i.name) == name)
            .unwrap()
    };
    let ring = row("icon.ring");
    let icon = exact_runner::icon_of(&plan, ring).unwrap();
    assert_eq!(
        (icon.path, icon.even_odd),
        ("M2 2H22V22H2Z M8 8H16V16H8Z;M0 0h1v1H0Z", true)
    );
    assert!(
        exact_kernel::symbol_path(&form["symbol:".len()..])
            .unwrap()
            .even_odd
    );
    let reply = "symbol:icon||0 -960 960 960|M760-200v-160H200Z|icons/reply.svg";
    assert!(
        plan.strings.iter().any(|s| s == reply),
        "a nonzero icon's form is as before: {:?}",
        plan.strings
    );
    let nonzero = row("icon.reply");
    let icon = exact_runner::icon_of(&plan, nonzero).unwrap();
    assert_eq!((icon.path, icon.even_odd), ("M760-200v-160H200Z", false));
    // Rules mixed across the paths: no template, so Linux has no source.
    std::fs::write(
        dir.join("assets/icons/ring.svg"),
        r#"<svg viewBox="0 0 24 24"><path fill-rule="evenodd" d="M2 2H22V22H2Z M8 8H16V16H8Z"/><path d="M0 0h1v1H0Z"/></svg>"#,
    )
    .unwrap();
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-icon-svg", "{e:?}");
    assert!(
        e.message
            .contains("its paths mix fill rules (evenodd: drawn path 1; nonzero: drawn path 2)"),
        "{}",
        e.message
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Stage 14 C2 round 2 (theme2): an Illustrator-style export (a `<style>`
/// block of class rules) is no template, refused where a template is
/// needed with the fix named; an app that ships only the web draws it as
/// the file, unaffected.
#[test]
fn a_style_block_is_no_template_but_the_web_draws_the_file() {
    const AI: &str = r#"<svg viewBox="0 0 24 24"><defs><style>.cls-1{fill:#000}</style></defs><path class="cls-1" d="M2 2H22V22H2Z"/></svg>"#;
    let web = app(
        "style-web",
        "\"web\": {}",
        "  icon.ai = icon(svg \"icons/ai.svg\")\n",
        &view("symbol:icon.ai"),
    );
    std::fs::write(web.parent().unwrap().join("assets/icons/ai.svg"), AI).unwrap();
    let plan = contract::compile_path(&web).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        plan.strings
            .iter()
            .any(|s| s == "symbol:icon||0 0 24 24||icons/ai.svg"),
        "the web draws the file: {:?}",
        plan.strings
    );
    let linux = app(
        "style-linux",
        "\"web\": {}, \"linux\": {}",
        "  icon.ai = icon(svg \"icons/ai.svg\")\n",
        &view("symbol:icon.ai"),
    );
    std::fs::write(linux.parent().unwrap().join("assets/icons/ai.svg"), AI).unwrap();
    let e = contract::compile_path(&linux).unwrap_err();
    assert_eq!(e.id, "tamber-icon-svg", "{e:?}");
    assert!(
        e.message.contains("it has a `<style>` block")
            && e.message.contains("`fill=\"currentColor\"`")
            && e.message
                .contains("drop the `style` attribute or the `<style>` block"),
        "{}",
        e.message
    );
    for p in [web, linux] {
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
