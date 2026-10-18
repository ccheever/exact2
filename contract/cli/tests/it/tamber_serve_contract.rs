//! @ref LLP 1121 stage 8 D — the design server owns the app's `.contract`
//! files: `set-state` and `unset-state` write a `style`'s state part
//! through `contract fmt`, compiled whole first; the inverse gives the
//! bytes back, an unformatted file's included; `propose` says `rebuild`.
#![cfg(unix)]

use super::tamber_serve::{app, digest, Serve};
use serde_json::{json, Value};

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

/// Review round 3: a package linked file by file (a vendored directory
/// with its `package.json`) is read-only; a negative in parentheses set
/// whole; a `(` in a row's comment no line's open one; an imported source
/// stays the server's while the root does not parse.
#[test]
fn set_state_round_three_edges() {
    let dir = app("state-edges3", DESIGN);
    let file = dir.join("app.contract");
    let vendor = dir.join("vendor/p");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(vendor.join("package.json"), "{\"name\": \"p\"}").unwrap();
    std::fs::write(
        vendor.join("index.contract"),
        "style Pkg\n  opacity=1\n\ncomponent Badge\n  view\n    text \"b\" class=Pkg\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules/p")).unwrap();
    std::os::unix::fs::symlink(
        vendor.join("index.contract"),
        dir.join("node_modules/p/index.contract"),
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("ui")).unwrap();
    std::fs::write(dir.join("ui/post.contract"), "style Post\n  background-color=\"#ffffff\"\n  :hover\n    opacity=-(0.2)\n  :active\n    opacity=0.5 // (temporary\n  :focus-visible\n    opacity=(0.5)\n\ncomponent PostButton\n  view\n    column class=Post\n      text \"p\"\n").unwrap();
    let root = "use PostButton from \"./ui/post.contract\"\nuse Badge from \"./node_modules/p/index.contract\"\ncomponent App\n  view\n    column\n      PostButton()\n      Badge()\n";
    std::fs::write(&file, root).unwrap();
    let post = dir.join("ui/post.contract");
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello", "client": "test"}));
    let files = hello["ok"]["files"].as_object().expect("files");
    assert!(files.contains_key("ui/post.contract"), "{hello}");
    assert!(
        !files.keys().any(|f| f.contains("index.contract")),
        "{hello}"
    );
    let write = |s: &mut Serve, op: serde_json::Value| {
        s.ask(json!({"method": "transaction", "client": "one", "files": {"ui/post.contract": digest(&post)}, "ops": [op]})).0
    };
    let neg = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "hover", "row": "opacity", "value": 0.4}),
    );
    assert!(neg["ok"]["inverse"].is_string(), "{neg}");
    let comment = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "active", "row": "opacity", "value": 0.6}),
    );
    assert!(comment["ok"]["inverse"].is_string(), "{comment}");
    let paren = write(
        &mut s,
        json!({"op": "set-state", "style": "Post", "state": "focus-visible", "row": "opacity", "value": 0.7}),
    );
    assert!(paren["ok"]["inverse"].is_string(), "{paren}");
    let text = std::fs::read_to_string(&post).unwrap();
    assert!(
        text.contains("opacity=0.4\n")
            && text.contains("opacity=0.6 // (temporary")
            && text.contains("opacity=0.7\n"),
        "{text}"
    );
    // The root stops parsing: its import is still the server's.
    std::fs::write(&file, format!("{root}  view (\n")).unwrap();
    let (broken, _) = s.ask(json!({"method": "hello", "client": "test"}));
    assert!(
        broken["ok"]["files"]["ui/post.contract"].is_string(),
        "{broken}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// history and revert read a `.contract` file's state rows: an edit each,
/// `code` for its other lines; a revert sets a row back, finds one already
/// back, refuses one moved on, and leaves the code.
#[test]
fn history_reads_state_rows_and_revert_plans_them() {
    let dir = app("state-history", DESIGN);
    let before = "style Post\n  opacity=1\n  :hover\n    opacity=0.9\n  :active\n    opacity=0.5\n\ncomponent App\n  view\n    column class=Post\n      text \"a\"\n";
    let after = "style Post\n  opacity=1\n  :hover\n    opacity=0.8\n  :active\n    opacity=0.4\n    background-color=\"#000000\"\n\ncomponent App\n  view\n    column class=Post\n      text \"b\"\n";
    // Now: hover moved on, active already back, the colour as the commit made it.
    let now = "style Post\n  opacity=1\n  :hover\n    opacity=0.7\n  :active\n    opacity=0.5\n    background-color=\"#000000\"\n\ncomponent App\n  view\n    column class=Post\n      text \"b\"\n";
    std::fs::write(dir.join("app.contract"), now).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let pair = json!([{"file": "app.contract", "before": before, "after": after}]);
    let (compared, _) = s.ask(json!({"method": "compare", "pairs": pair}));
    let file = &compared["ok"]["files"][0];
    assert_eq!(file["code"], true, "{compared}");
    let edits: Vec<(String, String, Value)> = file["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["state"].as_str().unwrap().into(),
                e["row"].as_str().unwrap().into(),
                e["after"].clone(),
            )
        })
        .collect();
    assert_eq!(
        edits,
        [
            ("hover".into(), "opacity".into(), json!("0.8")),
            ("active".into(), "opacity".into(), json!("0.4")),
            (
                "active".into(),
                "background-color".into(),
                json!("\"#000000\"")
            ),
        ]
    );
    let (planned, _) = s.ask(json!({"method": "revert-plan", "pairs": pair}));
    let p = &planned["ok"];
    assert_eq!(
        p["ops"],
        json!([{"op": "unset-state", "style": "Post", "state": "active", "row": "background-color", "file": "app.contract"}]),
        "{planned}"
    );
    assert_eq!(p["already"][0]["edit"]["row"], "opacity", "{planned}");
    assert_eq!(p["conflicts"][0]["edit"]["state"], "hover", "{planned}");
    assert_eq!(p["conflicts"][0]["now"], "0.7", "{planned}");
    assert_eq!(p["code"], json!(["app.contract"]));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 1 of the history follow-up: a literal's kind counts
/// (`0.5` to `"0.5"`, `true` to `false`); a style declared twice is
/// refused; a node's state header is code; a file the commit created is
/// refused, not stripped; a border row put back last is refused.
#[test]
fn history_holds_its_edges() {
    let dir = app("state-history2", DESIGN);
    let base = "style Post\n  opacity=1\n  :active\n    opacity=0.5\n    border-left-color=\"#00ff00\"\n\ncomponent App\n  view\n    column class=Post\n      :hover\n        opacity=0.9\n      text \"a\"\n";
    std::fs::write(dir.join("app.contract"), base).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let compare = |s: &mut Serve, before: Value, after: Value| {
        s.ask(json!({"method": "compare", "pairs": [{"file": "app.contract", "before": before, "after": after}]})).0["ok"]["files"][0].clone()
    };
    let kind = compare(
        &mut s,
        json!(base),
        json!(base.replace("opacity=0.5", "opacity=\"0.5\"")),
    );
    assert_eq!(kind["edits"][0]["after"], "\"0.5\"", "{kind}");
    let twice = compare(
        &mut s,
        json!(base),
        json!(format!("{base}\nstyle Post\n  opacity=1\n")),
    );
    assert!(
        twice["refused"].as_str().unwrap().contains("twice"),
        "{twice}"
    );
    let node = compare(
        &mut s,
        json!(base),
        json!(base.replace("      :hover", "      :active")),
    );
    assert_eq!(node["code"], true, "{node}");
    assert_eq!(node["edits"], json!([]), "{node}");
    let header = compare(
        &mut s,
        json!(base),
        json!(base.replace("  :active\n", "  :active // pressed\n")),
    );
    assert_eq!(header["code"], true, "{header}");
    let revert = |s: &mut Serve, before: Value, after: Value| {
        s.ask(json!({"method": "revert-plan", "pairs": [{"file": "app.contract", "before": before, "after": after}]})).0["ok"].clone()
    };
    let created = revert(&mut s, Value::Null, json!(base));
    assert!(
        created["conflicts"][0]["why"]
            .as_str()
            .unwrap()
            .contains("created"),
        "{created}"
    );
    // The commit removed `border-color` from before `border-left-color`.
    let had = base.replace(
        "    border-left-color",
        "    border-color=\"#ff0000\"\n    border-left-color",
    );
    let back = revert(&mut s, json!(had), json!(base));
    assert!(
        back["conflicts"][0]["why"]
            .as_str()
            .unwrap()
            .contains("paint over"),
        "{back}"
    );
    // Round 2: put back last where it was last, it goes back.
    let last = base.replace(
        "    border-left-color=\"#00ff00\"\n",
        "    border-left-color=\"#00ff00\"\n    border-color=\"#ff0000\"\n",
    );
    let ok = revert(&mut s, json!(last), json!(base));
    assert_eq!(ok["ops"][0]["op"], "set-state", "{ok}");
    // A "(" or "//" in a string is no parenthesis or comment.
    let quoted = base.replace("opacity=0.5", "opacity=0.5\n    box-shadow=\"( // x\"");
    let code = compare(
        &mut s,
        json!(quoted),
        json!(quoted.replace("text \"a\"", "text \"b\"")),
    );
    assert_eq!(code["code"], true, "{code}");
    // A "//" in a string leaves nothing behind.
    let url = compare(
        &mut s,
        json!(base),
        json!(base.replace("opacity=0.5", "opacity=0.5\n    box-shadow=\"https://x\"")),
    );
    assert!(url["code"].is_null() && url["edits"].is_array(), "{url}");
    // Unset would take a comment on the row's line.
    std::fs::write(
        dir.join("app.contract"),
        base.replace("opacity=0.5", "opacity=0.5 // kept"),
    )
    .unwrap();
    let added = revert(
        &mut s,
        json!(base.replace("    opacity=0.5\n", "")),
        json!(base),
    );
    assert!(
        added["conflicts"][0]["why"]
            .as_str()
            .unwrap()
            .contains("comment"),
        "{added}"
    );
    // Round 3: two rows a commit added, unset together, empty a block
    // whose header has a comment now.
    let two = base.replace(
        "  :active\n    opacity=0.5\n    border-left-color=\"#00ff00\"\n",
        "",
    );
    std::fs::write(
        dir.join("app.contract"),
        base.replace("  :active\n", "  :active // keep\n"),
    )
    .unwrap();
    let both = revert(&mut s, json!(two), json!(base));
    assert_eq!(both["ops"], json!([]), "{both}");
    assert_eq!(both["conflicts"].as_array().unwrap().len(), 2, "{both}");
    // Another side's row is no overlap: put back last, it goes back.
    std::fs::write(dir.join("app.contract"), base).unwrap();
    let side = base.replace(
        "    border-left-color",
        "    border-top-color=\"#0000ff\"\n    border-left-color",
    );
    let sides = revert(&mut s, json!(side), json!(base));
    assert_eq!(sides["ops"][0]["row"], "border-top-color", "{sides}");
    // A `border` shorthand is a border row: put back last, it would paint over.
    let short = base.replace(
        "    border-left-color",
        "    border=\"1px solid #ff0000\"\n    border-left-color",
    );
    let shorts = revert(&mut s, json!(short), json!(base));
    assert!(
        shorts["conflicts"][0]["why"]
            .as_str()
            .unwrap()
            .contains("paint over"),
        "{shorts}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
