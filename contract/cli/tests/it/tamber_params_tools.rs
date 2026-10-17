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
