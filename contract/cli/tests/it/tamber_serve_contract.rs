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
