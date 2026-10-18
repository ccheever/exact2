//! @ref LLP 1121 stage 8 D — the design server owns the app's `.contract`
//! files: `set-state` and `unset-state` write a `style`'s state part
//! through `contract fmt`, compiled whole first; the inverse gives the
//! bytes back, an unformatted file's included; `propose` says `rebuild`.
#![cfg(unix)]

use super::tamber_serve::{app, digest, Serve};
use serde_json::json;

const DESIGN: &str = "tamber t\n\ntheme\n  accent.fill = #0a7aff\n  accent.pressed = #0050c8\n";

/// Unformatted on purpose (`=` with spaces, an extra blank line), which
/// the inverse must give back exactly.
const VIEW: &str = "style Post\n  background-color = \"accent.fill\"\n\n\ncomponent App\n  state busy = false\n  view\n    column class=Post press=go disabled=busy\n      text \"Post\"\n  action go\n    busy = !busy\n";

#[test]
fn set_state_writes_a_state_part_and_its_inverse_gives_the_bytes_back() {
    let dir = app("set-state", DESIGN);
    let file = dir.join("app.contract");
    std::fs::write(&file, VIEW).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello", "client": "test"}));
    assert!(hello["ok"]["files"]["app.contract"].is_string(), "{hello}");
    let op = json!({"op": "set-state", "style": "Post", "state": "active", "row": "background-color", "value": "accent.pressed"});
    let (proposed, _) = s.ask(json!({"method": "propose", "ops": [op]}));
    assert_eq!(proposed["ok"]["class"], "rebuild", "{proposed}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), VIEW);
    let (done, events) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [op]}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        written.contains("  :active\n    background-color=\"accent.pressed\"\n"),
        "{written}"
    );
    assert_eq!(events[0]["client"], "one");
    // Again: the row's value replaced, the block kept.
    let op2 = json!({"op": "set-state", "style": "Post", "state": ":active", "row": "background-color", "value": "#000000"});
    let (again, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [op2]}));
    assert!(again["ok"]["inverse"].is_string(), "{again}");
    assert_eq!(
        std::fs::read_to_string(&file)
            .unwrap()
            .matches(":active")
            .count(),
        1
    );
    let (_, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": again["ok"]["inverse"]}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), written);
    let (undone, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert!(undone["ok"].is_object(), "{undone}");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        VIEW,
        "the unformatted bytes back"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unset_state_takes_the_row_and_its_emptied_block() {
    let dir = app("unset-state", DESIGN);
    let file = dir.join("app.contract");
    let view = "style Post\n  background-color=\"accent.fill\"\n  :active\n    background-color=\"accent.pressed\"\n\ncomponent App\n  view\n    column class=Post\n      text \"Post\"\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let op =
        json!({"op": "unset-state", "style": "Post", "state": "active", "row": "background-color"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [op]}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(!written.contains(":active"), "{written}");
    let (_, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), view);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_set_state_the_app_cannot_build_writes_nothing() {
    let dir = app("refused-state", DESIGN);
    let file = dir.join("app.contract");
    std::fs::write(&file, VIEW).unwrap();
    let mut s = Serve::start(&dir, &[]);
    // A host state's row that would lay out: the build refuses it.
    let op = json!({"op": "set-state", "style": "Post", "state": "active", "row": "width", "value": "10px"});
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [op]}));
    assert_eq!(
        refused["error"]["diagnostics"][0]["id"], "lower-state-row",
        "{refused}"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), VIEW);
    // A style no file declares.
    let (missing, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-state", "style": "Nope", "state": "active", "row": "opacity", "value": 0.5}]}));
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no file of the app declares `style Nope`"),
        "{missing}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 1: a negative literal replaced whole; a row with a comment
/// after it unset with its block (the comment on the header too); a new
/// block at a four-space style's own indent; a row's name one name.
#[test]
fn set_state_holds_its_edges() {
    let dir = app("state-edges", DESIGN);
    let file = dir.join("app.contract");
    let view = "style Post\n    background-color=\"#ffffff\"\n    :hover // a hover\n        opacity=-0.2 // dimmer\n\ncomponent App\n  view\n    column class=Post\n      text \"Post\"\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let write = |s: &mut Serve, op: serde_json::Value| {
        s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [op]})).0
    };
    let set = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "hover", "row": "opacity", "value": 0.5}),
    );
    assert!(set["ok"]["inverse"].is_string(), "{set}");
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("opacity=0.5 // dimmer"),
        "{}",
        std::fs::read_to_string(&file).unwrap()
    );
    let unset = write(
        &mut s,
        json!({"op": "unset-state", "style": "Post", "state": "hover", "row": "opacity"}),
    );
    assert!(unset["ok"]["inverse"].is_string(), "{unset}");
    assert!(!std::fs::read_to_string(&file).unwrap().contains(":hover"));
    let block = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "active", "row": "opacity", "value": 0.9}),
    );
    assert!(block["ok"]["inverse"].is_string(), "{block}");
    let bad = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "active", "row": "opacity=0.5\n  width", "value": "300px"}),
    );
    assert!(bad["error"].is_object(), "{bad}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 2: a row in parentheses unset whole; a value over two lines
/// refused (the server answers on); a package linked into `node_modules`
/// and reached by a relative path is not the app's to write.
#[test]
fn set_state_refuses_what_it_cannot_splice_and_a_linked_package() {
    let dir = app("state-splice", DESIGN);
    let file = dir.join("app.contract");
    let vendor = dir.join("vendor/pkg");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(vendor.join("index.contract"), "style Pkg\n  opacity=1\n  :active\n    opacity=0.5\n\ncomponent Badge\n  view\n    text \"b\" class=Pkg\n").unwrap();
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    std::os::unix::fs::symlink(&vendor, dir.join("node_modules/pkg")).unwrap();
    let view = "use Badge from \"./node_modules/pkg/index.contract\"\nstyle Post\n  background-color=\"#ffffff\"\n  :active\n    opacity=(0.5)\n    background-color=(\n      \"#000000\"\n    )\n  :hover\n    opacity=-(0.2)\n  :focus-visible\n    opacity=-(\n      0.3)\n\ncomponent App\n  view\n    column class=Post\n      Badge()\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello", "client": "test"}));
    let files = hello["ok"]["files"].as_object().expect("files");
    assert!(files.contains_key("app.contract"), "{hello}");
    assert!(
        !files.keys().any(|f| f.contains("pkg")),
        "a linked package is read-only: {hello}"
    );
    let write = |s: &mut Serve, op: serde_json::Value| {
        s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [op]})).0
    };
    let pkg = write(
        &mut s,
        json!({"op": "set-state", "style": "Pkg", "state": "active", "row": "opacity", "value": 0.2}),
    );
    assert!(pkg["error"].is_object(), "{pkg}");
    let two = write(
        &mut s,
        json!({"op": "unset-state", "style": "Post", "state": "active", "row": "background-color"}),
    );
    assert!(
        two["error"]["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("more than one line")),
        "{two}"
    );
    let node = write(
        &mut s,
        json!({"op": "set-state", "node": "card", "state": "active", "row": "opacity", "value": 0.5}),
    );
    assert!(
        node["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("set-row")),
        "{node}"
    );
    let neg = write(
        &mut s,
        json!({"op": "unset-state", "style": "Post", "state": "hover", "row": "opacity"}),
    );
    assert!(neg["ok"]["inverse"].is_string(), "{neg}");
    assert!(
        neg["ok"]["diff"]
            .as_str()
            .is_some_and(|d| d.contains("@@ -") && d.contains("-    opacity=-(0.2)\n")),
        "{neg}"
    );
    let neg2 = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "focus-visible", "row": "opacity", "value": 0.4}),
    );
    assert!(
        neg2["error"]["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("more than one line")),
        "{neg2}"
    );
    let paren = write(
        &mut s,
        json!({"op": "unset-state", "style": "Post", "state": "active", "row": "opacity"}),
    );
    assert!(paren["ok"]["inverse"].is_string(), "{paren}");
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        !written.contains("opacity=(") && !written.contains(":hover"),
        "{written}"
    );
    assert_eq!(
        std::fs::read_to_string(vendor.join("index.contract"))
            .unwrap()
            .matches("0.5")
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}
