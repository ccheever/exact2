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
