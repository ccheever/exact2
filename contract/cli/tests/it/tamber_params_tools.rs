//! @ref LLP 1121 D15 (stage 7 A, review round 2): what the tools say of a
//! param: by its full name, at its declaration, and folded with the design
//! wherever the design is read.

use std::path::{Path, PathBuf};
use std::process::Command;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (file, text) in files {
        let at = dir.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    dir.join("app.contract")
}

fn tamber(args: &[&str], app: &Path) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("tamber")
        .args(args)
        .arg(app)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_eval_and_ir_name_a_param_in_full_at_its_declaration() {
    let src = r##"component App
  params
    quiet: color = "#ffffff"
    fill: color = "accent.fill"  control="select"  options="param.quiet|accent.fill"
    body: type = "16px/1.4 system-ui"
  view
    column background-color="param.fill" border-color="param.quiet"
      text "x" font="param.body"
"##;
    let design = "tamber t\n\ntheme\n  accent.fill = #0a7aff\n";
    let path = app("check", &[("app.contract", src), ("app.tamber", design)]);
    let (ok, out, err) = tamber(&["check"], &path);
    assert!(ok, "{out}{err}");
    assert!(
        !err.contains("param.f") && !err.contains("536870912"),
        "{err}"
    );
    assert!(
        err.contains("app.contract:5:"),
        "a param's warning is at its default: {err}"
    );
    assert!(out.contains("1 token, 3 params"), "{out}");
    let (ok, out, err) = tamber(&["eval"], &path);
    assert!(ok, "{err}");
    assert!(!out.contains("param."), "{out}");
    let (ok, out, err) = tamber(&["ir"], &path);
    assert!(ok, "{err}");
    let ir: serde_json::Value = serde_json::from_str(&out).unwrap();
    let fill = ir["components"][0]["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["param"] == "fill")
        .unwrap()
        .clone();
    assert_eq!(
        fill["options"],
        serde_json::json!(["app.contract#App.quiet", "accent.fill"])
    );
    // A bad kind's refusal is the param's, at its declaration.
    let bad = "component App\n  params\n    a: length = \"#fff\"\n  view\n    column padding=\"param.a\"\n      text \"x\"\n";
    let path = app("bad", &[("app.contract", bad), ("app.tamber", design)]);
    let (ok, _, err) = tamber(&["check"], &path);
    assert!(!ok);
    assert!(
        !err.contains("param.f") && !err.contains("536870912"),
        "{err}"
    );
}

#[test]
fn a_dial_only_a_param_reads_is_read_by_check() {
    let src = r##"component App
  params
    radius: length = "calc(12px * roundness)"
  view
    column border-radius="param.radius"
      text "x"
"##;
    let design = "tamber t\n\ndials\n  roundness: number = 1 in 0..2\n";
    let path = app("dial", &[("app.contract", src), ("app.tamber", design)]);
    let (ok, out, err) = tamber(&["check"], &path);
    assert!(ok, "{out}{err}");
    let (ok, _, err) = tamber(&["ir"], &path);
    assert!(ok, "{err}");
}

#[test]
fn a_lint_after_an_earlier_refusal_names_a_param_in_full() {
    let src = r##"component App
  params
    gap: length = "8px"
  view
    column background-color="param.gap"
      text missing
"##;
    let path = app("lint", &[("app.contract", src)]);
    let all = contract::compile_path_all(&path, false).err().unwrap();
    assert!(
        all.iter().all(|e| !e.message.contains("param.f")),
        "{all:?}"
    );
}

#[test]
fn a_component_without_params_makes_a_short_name_ambiguous() {
    let a = "component Card\n  params\n    radius: length = \"4px\"\n  view\n    column border-radius=\"param.radius\"\n      text \"a\"\n";
    let b = "component Card\n  view\n    column\n      text \"b\"\n";
    let root = "use Card from \"./a.contract\"\nuse Card as Other from \"./b.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n      Other()\n";
    let path = app(
        "plain",
        &[("app.contract", root), ("a.contract", a), ("b.contract", b)],
    );
    let params = contract::tamber_params::params_of(&path);
    let e = contract_lower::params::resolve(&params.tokens, &params.identities, "Card.radius")
        .unwrap_err();
    assert_eq!(e.id, "tamber-param-ambiguous");
    let block = contract::tamber_params::components(&params, None, &contract_lower::tamber_vocab());
    assert!(block[0]["params"][0]["short"].is_null(), "{block}");
}

#[test]
fn a_package_file_is_its_package_s_however_it_is_reached() {
    let card = "component Card\n  params\n    radius: length = \"4px\"\n  view\n    column border-radius=\"param.radius\"\n      text \"a\"\n";
    let manifest = r#"{"name": "pkg", "version": "1.0.0", "exports": {".": "./index.contract"}}"#;
    let orders = [
        "use Card as A from \"./node_modules/pkg/index.contract\"\nuse Card as B from \"pkg\"\n",
        "use Card as B from \"pkg\"\nuse Card as A from \"./node_modules/pkg/index.contract\"\n",
    ];
    for (i, uses) in orders.iter().enumerate() {
        let root = format!("{uses}\ncomponent App\n  view\n    column\n      A()\n      B()\n");
        let path = app(
            &format!("route{i}"),
            &[
                ("app.contract", &root),
                ("node_modules/pkg/package.json", manifest),
                ("node_modules/pkg/index.contract", card),
            ],
        );
        let params = contract::tamber_params::params_of(&path);
        let ids: Vec<&str> = params.tokens.iter().map(|p| p.identity.as_str()).collect();
        assert_eq!(ids, ["pkg/index.contract#Card"], "order {i}");
    }
}

#[test]
fn an_icon_param_naming_another_reads_its_svg_as_a_source() {
    let src = r##"component App
  params
    base: icon = "icon(svg \"mark.svg\")"
    glyph: icon = "param.base"
  view
    column
      image "symbol:param.glyph"
"##;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0h24v24H0z"/></svg>"#;
    let path = app(
        "svg",
        &[("app.contract", src), ("assets/icons/mark.svg", svg)],
    );
    let graph = contract::source_graph(&path);
    let all = format!("{graph:?}");
    assert!(all.contains("mark.svg"), "{all}");
}

#[test]
fn an_images_rows_take_params_and_only_its_symbol_source_names_one() {
    let design = "tamber t\n\ntheme\n  accent.fill = #0a7aff\n";
    let styled = r##"component App
  params
    fill: color = "#ffffff"
  view
    image "https://example.com/photo.png" background-color="param.fill"
"##;
    let path = app(
        "img-row",
        &[("app.contract", styled), ("app.tamber", design)],
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let branches = r##"component App
  params
    fill: color = "#ffffff"
  state show = false
  view
    column background-color="param.fill"
      image (show ? "symbol:star" : "https://example.com/param.fill")
      text "x" font-family=`"${"param.fill"}", system-ui`
"##;
    let path = app(
        "img-branch",
        &[("app.contract", branches), ("app.tamber", design)],
    );
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let strings = format!("{:?}", plan.strings);
    assert!(
        strings.contains("https://example.com/param.fill"),
        "{strings}"
    );
}

#[cfg(unix)]
#[test]
fn a_linked_package_is_its_package_s_and_its_diagnostics_its_file_s() {
    let card = "component Card\n  params\n    body: type = \"16px/1.4 system-ui\"\n  view\n    column\n      text \"a\" font=\"param.body\"\n";
    let manifest = r#"{"name": "pkg", "version": "1.0.0", "exports": {".": "./index.contract"}}"#;
    let orders = [
        "use Card as A from \"./node_modules/pkg/index.contract\"\nuse Card as B from \"pkg\"\n",
        "use Card as B from \"pkg\"\nuse Card as A from \"./node_modules/pkg/index.contract\"\n",
    ];
    for (i, uses) in orders.iter().enumerate() {
        let root = format!("{uses}\ncomponent App\n  view\n    column\n      A()\n      B()\n");
        let path = app(
            &format!("linked{i}"),
            &[
                ("app.contract", &root),
                ("vendor/pkg/package.json", manifest),
                ("vendor/pkg/index.contract", card),
            ],
        );
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::os::unix::fs::symlink(dir.join("vendor/pkg"), dir.join("node_modules/pkg")).unwrap();
        let params = contract::tamber_params::params_of(&path);
        let ids: Vec<&str> = params.tokens.iter().map(|p| p.identity.as_str()).collect();
        assert_eq!(ids, ["pkg/index.contract#Card"], "order {i}");
        if i == 0 {
            let (_, _, err) = tamber(&["check"], &path);
            assert!(err.contains("vendor/pkg/index.contract:3:"), "{err}");
        }
    }
}

#[test]
fn a_design_named_apart_is_admitted_alone() {
    let src = r##"component App
  params
    fill: color = "#ffffff"
  view
    column background-color="param.fill"
      text "x"
"##;
    let bad = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  Canvas = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #161e27)\n";
    let path = app("apart", &[("app.contract", src), ("bad.tamber", bad)]);
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "check"])
        .arg(path.parent().unwrap().join("bad.tamber"))
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && err.contains("tamber-role-axis"),
        "{err}"
    );
}

#[test]
fn the_second_re_check_a_shared_derive_a_match_source_and_designs_apart() {
    // A child's derive, shared by a `let`, compared: refused.
    let derive = r##"component App
  params
    tint: color = "#ffffff"
  view
    column
      Swatch(tint="param.tint")
component Swatch
  props
    tint: string
  derive saved = tint
  derive paint = saved == "#ffffff" ? saved : "#000000"
  view
    text "x" color=paint
"##;
    let path = app("let", &[("app.contract", derive)]);
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-param-scope", "{e:?}");
    // An image source's `match` arms, each a symbol's.
    let matched = r##"component App
  params
    glyph: icon = "icon(sf \"heart\")"
  view
    image (match some(true) { case some(x) => "symbol:param.glyph", case none => "symbol:star" })
"##;
    let path = app("match", &[("app.contract", matched)]);
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    // No design file: check builds the app, so its params are checked.
    let control = "component App\n  params\n    fill: color = \"#ffffff\"  control=\"slider\"\n  view\n    text \"x\" color=\"param.fill\"\n";
    let path = app("nodesign", &[("app.contract", control)]);
    let (ok, _, err) = tamber(&["check"], &path);
    assert!(!ok && err.contains("tamber-param-control"), "{err}");
    // A design named apart folds no neighbouring param.
    let read = "component App\n  params\n    fill: color = \"accent.fill\"\n  view\n    text \"x\" color=\"param.fill\"\n";
    let path = app(
        "other",
        &[
            ("app.contract", read),
            ("app.tamber", "tamber t\n\ntheme\n  accent.fill = #ffffff\n"),
            ("other.tamber", "tamber other\n"),
        ],
    );
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "check"])
        .arg(path.parent().unwrap().join("other.tamber"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_third_re_check_lets_in_sources_failed_loads_unnamed_packages_and_conditions() {
    // A child's source behind a shared derive: the param is the symbol's.
    let shared = r##"component App
  params
    glyph: icon = "icon(sf \"heart\")"
  state show = true
  view
    Child(glyph="param.glyph", show=show)
component Child
  props
    glyph: string
    show: bool
  derive flag = show
  derive picture = flag ? `symbol:${glyph}` : (flag ? "symbol:star" : "symbol:heart")
  view
    image picture
"##;
    let path = app("let-source", &[("app.contract", shared)]);
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    // A Contract that does not load: check builds it and says why.
    let spare = "component App\n  params\n    fill: color = \"#ffffff\"\n    spare: color = \"#000000\"\n  view\n    text \"x\" color=\"param.fill\"\n";
    let path = app(
        "failed",
        &[("app.contract", spare), ("app.tamber", "tamber t\n")],
    );
    let (ok, _, err) = tamber(&["check"], &path);
    assert!(!ok && err.contains("tamber-param-unused"), "{err}");
    // A param in an image source's condition is a computation: refused.
    let cond = r##"component App
  params
    fill: color = "#ffffff"
  view
    column background-color="param.fill"
      image ("param.fill" == "#ffffff" ? "symbol:heart" : "symbol:star")
"##;
    let path = app("img-cond", &[("app.contract", cond)]);
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-param-scope", "{e:?}");
}

#[cfg(unix)]
#[test]
fn two_unnamed_packages_outside_the_app_are_two_identities() {
    let root = std::env::temp_dir().join(format!("exact2-tools-unnamed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (lib, px) in [("a", "4px"), ("b", "8px")] {
        let dir = root.join("libs").join(lib);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"exports": {".": "./index.contract"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("index.contract"),
            format!("component Card\n  params\n    radius: length = \"{px}\"\n  view\n    column border-radius=\"param.radius\"\n      text \"c\"\n"),
        )
        .unwrap();
    }
    let app_dir = root.join("app");
    std::fs::create_dir_all(app_dir.join("node_modules")).unwrap();
    std::fs::write(
        app_dir.join("package.json"),
        r#"{"name": "app", "dependencies": {"a": "*", "b": "*"}}"#,
    )
    .unwrap();
    for lib in ["a", "b"] {
        std::os::unix::fs::symlink(
            root.join("libs").join(lib),
            app_dir.join("node_modules").join(lib),
        )
        .unwrap();
    }
    std::fs::write(
        app_dir.join("app.contract"),
        "use Card as A from \"a\"\nuse Card as B from \"b\"\n\ncomponent App\n  view\n    column\n      A()\n      B()\n",
    )
    .unwrap();
    let path = app_dir.join("app.contract");
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let params = contract::tamber_params::params_of(&path);
    let ids: Vec<&str> = params.tokens.iter().map(|p| p.identity.as_str()).collect();
    assert_eq!(
        ids,
        [
            "../libs/a/index.contract#Card",
            "../libs/b/index.contract#Card"
        ]
    );
}
