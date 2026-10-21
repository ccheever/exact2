//! @ref LLP 1122 D6, T5 — `set-row`, the design server's checked edit of
//! one Contract row (LLP 1121 stage 9 D): at the attribute's place, named
//! by the map's `at` and the file's digest, or on a `style`; only the value
//! replaced, through `contract fmt`, compiled whole first; the inverse
//! gives the bytes back; a stale digest and a failing candidate write
//! nothing.
#![cfg(unix)]

use super::tamber_serve::{app, digest, Serve};
use serde_json::{json, Value};

const DESIGN: &str = "tamber t\n\ntheme\n  accent.fill = #0a7aff\n  accent.pressed = #0050c8\n";

const VIEW: &str = "component App\n  state busy = false\n  view\n    column background-color=\"accent.fill\" border-color=\"#111111\" width=100 press=go testId=\"card\"\n      :active\n        background-color=\"accent.pressed\"\n      text \"Post\"\n  action go\n    busy = !busy\n";

fn transaction(s: &mut Serve, file: &std::path::Path, op: Value) -> Value {
    s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(file)}, "ops": [op]})).0
}

#[test]
fn set_row_writes_one_line_where_the_author_wrote_it_and_undo_gives_the_bytes_back() {
    let dir = app("set-row", DESIGN);
    let file = dir.join("app.contract");
    std::fs::write(&file, VIEW).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let sha = digest(&file);
    let op = json!({"op": "set-row", "file": "app.contract", "at": {"line": 4, "col": 12}, "sha256": sha, "row": "background-color", "value": "#ffffff"});
    let (proposed, _) = s.ask(json!({"method": "propose", "ops": [op]}));
    assert_eq!(proposed["ok"]["class"], "rebuild", "{proposed}");
    let done = transaction(&mut s, &file, op);
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let written = std::fs::read_to_string(&file).unwrap();
    let changed: Vec<_> = VIEW
        .lines()
        .zip(written.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 1, "one line: {written}");
    assert!(
        changed[0].1.contains("background-color=\"#ffffff\""),
        "{written}"
    );
    // A node's own state part, at its row's place.
    let op = json!({"op": "set-row", "file": "app.contract", "at": {"line": 6, "col": 9}, "sha256": digest(&file), "row": "background-color", "state": "active", "value": "#000000"});
    let state = transaction(&mut s, &file, op);
    assert!(state["ok"]["inverse"].is_string(), "{state}");
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("background-color=\"#000000\""));
    s.ask(json!({"method": "undo", "client": "one", "inverse": state["ok"]["inverse"]}));
    s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), VIEW);
    // An unformatted file: fmt's other changes ride along, and undo gives
    // its bytes back, not fmt's.
    let unformatted = VIEW.replace("    column", "\n    column").replace(
        "background-color=\"accent.pressed\"",
        "background-color = \"accent.pressed\"",
    );
    std::fs::write(&file, &unformatted).unwrap();
    let op = json!({"op": "set-row", "file": "app.contract", "at": {"line": 5, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "#ffffff"});
    let done = transaction(&mut s, &file, op);
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        unformatted,
        "the unformatted bytes back"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_digest_and_a_failing_candidate_write_nothing() {
    let dir = app("set-row-refused", DESIGN);
    let file = dir.join("app.contract");
    std::fs::write(&file, VIEW).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let at = json!({"line": 4, "col": 12});
    let stale = transaction(
        &mut s,
        &file,
        json!({"op": "set-row", "file": "app.contract", "at": at, "sha256": "0".repeat(64), "row": "background-color", "value": "#ffffff"}),
    );
    assert!(
        stale["error"].to_string().contains("changed since"),
        "{stale}"
    );
    let failing = transaction(
        &mut s,
        &file,
        json!({"op": "set-row", "file": "app.contract", "at": at, "sha256": digest(&file), "row": "background-color", "value": "accent.nope"}),
    );
    assert!(failing["error"]["diagnostics"].is_array(), "{failing}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), VIEW);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_row_refuses_what_is_not_a_paint_row_it_can_splice() {
    let dir = app("set-row-edges", DESIGN);
    let file = dir.join("app.contract");
    let view = VIEW.replace(
        "border-color=\"#111111\"",
        "border-color=(busy ? \"#111111\" : \"#222222\")",
    );
    std::fs::write(&file, &view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let sha = digest(&file);
    let try_ = |s: &mut Serve, at: Value, row: &str| {
        transaction(s, &file, json!({"op": "set-row", "file": "app.contract", "at": at, "sha256": sha, "row": row, "value": "#ffffff"}))["error"].to_string()
    };
    // Another attribute than the one named, a place with none, a layout row, an expression.
    assert!(try_(&mut s, json!({"line": 4, "col": 12}), "color").contains("not `color`"));
    assert!(
        try_(&mut s, json!({"line": 4, "col": 13}), "background-color").contains("no attribute")
    );
    let width = view.lines().nth(3).unwrap().find("width").unwrap() as u64 + 1;
    assert!(try_(&mut s, json!({"line": 4, "col": width}), "width").contains("layout row"));
    let border = view.lines().nth(3).unwrap().find("border-color").unwrap() as u64 + 1;
    assert!(try_(&mut s, json!({"line": 4, "col": border}), "border-color").contains("expression"));
    // A longhand of a shorthand: refused, naming the shorthand and its text.
    let padded = view.replace("width=100", "padding=\"4px 8px\"");
    std::fs::write(&file, &padded).unwrap();
    let padding = padded.lines().nth(3).unwrap().find("padding").unwrap() as u64 + 1;
    let sha = digest(&file);
    let longhand = transaction(
        &mut s,
        &file,
        json!({"op": "set-row", "file": "app.contract", "at": {"line": 4, "col": padding}, "sha256": sha, "row": "padding-top", "value": "2px"}),
    );
    // `padding` is a layout row: refused as one, no shorthand rung offered.
    assert!(
        longhand["error"].to_string().contains("layout row"),
        "{longhand}"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), padded);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_class_row_is_edited_in_its_styles_own_file() {
    let dir = app("set-row-style", DESIGN);
    let file = dir.join("app.contract");
    std::fs::write(&file, "use Card from \"./card.contract\"\n\nstyle Raised\n  background-color=\"#eeeeee\"\n\ncomponent App\n  view\n    column class=Raised\n      Card()\n").unwrap();
    let card = dir.join("card.contract");
    let card_text = "style Raised\n  background-color=\"#dddddd\"\n\ncomponent Card\n  view\n    row class=Raised\n      text \"c\"\n";
    std::fs::write(&card, card_text).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let op = json!({"op": "set-row", "style": "Raised", "file": "card.contract", "row": "background-color", "value": "#cccccc"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"card.contract": digest(&card)}, "ops": [op]}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    assert!(std::fs::read_to_string(&card).unwrap().contains("#cccccc"));
    assert!(
        std::fs::read_to_string(&file).unwrap().contains("#eeeeee"),
        "the other file's style kept"
    );
    // Two files declare it: the file is named, else refused.
    let unnamed =
        json!({"op": "set-row", "style": "Raised", "row": "background-color", "value": "#bbbbbb"});
    let (refused, _) = s.ask(json!({"method": "propose", "ops": [unnamed]}));
    assert!(
        refused["error"].to_string().contains("name the `file`"),
        "{refused}"
    );
    // A row the style doesn't write is refused, not added.
    let absent = json!({"op": "set-row", "style": "Raised", "file": "card.contract", "row": "color", "value": "#bbbbbb"});
    let (refused, _) = s.ask(json!({"method": "propose", "ops": [absent]}));
    assert!(
        refused["error"].to_string().contains("writes no `color`"),
        "{refused}"
    );
    s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert_eq!(std::fs::read_to_string(&card).unwrap(), card_text);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 1: one place once; a name in scope is the author's; a
/// neighbour's open parentheses are its own; a shorthand's text as
/// written; a place past u32 refused; a place and a style not both; a
/// refusal says its place.
#[test]
fn set_row_round_one_edges() {
    let dir = app("set-row-r1", DESIGN);
    let file = dir.join("app.contract");
    let view = "component App\n  state alpha = 0.5\n  state busy = false\n  view\n    column opacity=0.5 border-color=\"accent.fill\" background-color=(busy\n      ? \"#111111\" : \"#222222\") press=go\n      box opacity=alpha\n  action go\n    busy = !busy\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let sha = digest(&file);
    let row = |line: u64, col: u64, row: &str, value: Value| json!({"op": "set-row", "file": "app.contract", "at": {"line": line, "col": col}, "sha256": sha, "row": row, "value": value});
    let ask = |s: &mut Serve, ops: Value| s.ask(json!({"method": "propose", "ops": ops})).0;
    let opacity = view.lines().nth(4).unwrap().find("opacity").unwrap() as u64 + 1;
    // Two writes to one place.
    let twice = ask(
        &mut s,
        json!([
            row(5, opacity, "opacity", json!(0.25)),
            row(5, opacity, "opacity", json!(0.7))
        ]),
    );
    assert!(
        twice["error"].to_string().contains("two set-rows"),
        "{twice}"
    );
    // A neighbour's parentheses run to the next line: opacity is still one value.
    let one = ask(&mut s, json!([row(5, opacity, "opacity", json!(0.25))]));
    assert_eq!(one["ok"]["class"], "rebuild", "{one}");
    // A name in scope: state's, the author's.
    let alpha = view.lines().nth(6).unwrap().find("opacity").unwrap() as u64 + 1;
    let scoped = ask(&mut s, json!([row(7, alpha, "opacity", json!(1))]));
    assert!(
        scoped["error"].to_string().contains("expression"),
        "{scoped}"
    );
    assert_eq!(
        scoped["error"]["diagnostics"][0]["line"], 7,
        "the refusal's place"
    );
    // A longhand of a shorthand written as a token's name: its text whole.
    let border = view.lines().nth(4).unwrap().find("border-color").unwrap() as u64 + 1;
    let longhand = ask(
        &mut s,
        json!([row(5, border, "border-top-color", json!("#000000"))]),
    );
    assert!(
        longhand["error"]
            .to_string()
            .contains("border-color=\\\"accent.fill\\\""),
        "{longhand}"
    );
    // A place past u32, and a place with a style.
    let far = ask(
        &mut s,
        json!([row(4294967300, opacity, "opacity", json!(1))]),
    );
    assert!(far["error"].to_string().contains("`at`"), "{far}");
    let both = ask(
        &mut s,
        json!([{"op": "set-row", "style": "X", "at": {"line": 5, "col": 1}, "row": "opacity", "value": 1}]),
    );
    assert!(both["error"].to_string().contains("not both"), "{both}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), view);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 1, theme2 17:18: a style's own row joins history and revert by
/// (style, row): a value edit is undone with `set-row`; a row the commit
/// added or removed is the author's to put back. A template row stays code.
#[test]
fn a_styles_own_row_joins_history_and_revert() {
    let dir = app("row-history", DESIGN);
    let before = "style Post\n  opacity=1\n  background-color=\"#ffffff\"\n\ncomponent App\n  view\n    column class=Post\n      text \"a\" color=\"#111111\"\n";
    let after = "style Post\n  opacity=0.5\n  color=\"#222222\"\n\ncomponent App\n  view\n    column class=Post\n      text \"a\" color=\"#333333\"\n";
    std::fs::write(dir.join("app.contract"), after).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let pair = json!([{"file": "app.contract", "before": before, "after": after}]);
    let (compared, _) = s.ask(json!({"method": "compare", "pairs": pair}));
    let file = &compared["ok"]["files"][0];
    let edits: Vec<(String, String)> = file["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["what"].as_str().unwrap().into(),
                e["row"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        edits,
        [
            ("row".to_string(), "opacity".to_string()),
            ("row".into(), "background-color".into()),
            ("row".into(), "color".into())
        ],
        "{compared}"
    );
    assert_eq!(file["code"], true, "the template row is code: {compared}");
    let (planned, _) = s.ask(json!({"method": "revert-plan", "pairs": pair}));
    let p = &planned["ok"];
    assert_eq!(
        p["ops"],
        json!([{"op": "set-row", "style": "Post", "row": "opacity", "value": 1.0, "file": "app.contract"}]),
        "{planned}"
    );
    let whys: Vec<&str> = p["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["why"].as_str().unwrap())
        .collect();
    assert!(
        whys.iter().any(|w| w.contains("removed")) && whys.iter().any(|w| w.contains("added")),
        "{planned}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 2: history keeps a style's row as `set-row` writes it (a
/// paint row alone on its line), the rest code: a layout row's edit, a
/// reorder of overlapping rows; a held proposal's no-op target guarded by
/// `requires`; a style's longhand refused naming its shorthand; a sign
/// whose number is below it refused as over more lines.
#[test]
fn set_row_round_two_edges() {
    let dir = app("set-row-r2", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    // History: a layout row's edit and a reorder are code, no op.
    let before = "style Post\n  font-size=14\n  border-radius=4\n  border-top-left-radius=12\n\ncomponent App\n  view\n    column class=Post\n";
    let after = "style Post\n  font-size=16\n  border-top-left-radius=12\n  border-radius=4\n\ncomponent App\n  view\n    column class=Post\n";
    std::fs::write(dir.join("app.contract"), after).unwrap();
    let pair = json!([{"file": "app.contract", "before": before, "after": after}]);
    let (compared, _) = s.ask(json!({"method": "compare", "pairs": pair}));
    let file = &compared["ok"]["files"][0];
    assert_eq!(file["code"], true, "{compared}");
    assert!(
        file["edits"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["row"] != "font-size"),
        "{compared}"
    );
    let (planned, _) = s.ask(json!({"method": "revert-plan", "pairs": pair}));
    assert!(
        planned["ok"]["ops"].as_array().is_none_or(|o| o.is_empty()),
        "{planned}"
    );
    // A style's longhand: the shorthand and its text; a layout shorthand: layout.
    let view = "style Post\n  border-color=\"accent.fill\"\n  padding=\"4px 8px\"\n\ncomponent App\n  view\n    column class=Post opacity=-(\n      0.3)\n";
    let file = dir.join("app.contract");
    std::fs::write(&file, view).unwrap();
    let ask = |s: &mut Serve, ops: Value| s.ask(json!({"method": "propose", "ops": ops})).0;
    let longhand = ask(
        &mut s,
        json!([{"op": "set-row", "style": "Post", "row": "border-top-color", "value": "#000000"}]),
    );
    assert!(
        longhand["error"]
            .to_string()
            .contains("shorthand `border-color=\\\"accent.fill\\\"`"),
        "{longhand}"
    );
    let pad = ask(
        &mut s,
        json!([{"op": "set-row", "style": "Post", "row": "padding-top", "value": "2px"}]),
    );
    assert!(pad["error"].to_string().contains("layout row"), "{pad}");
    let opacity = view.lines().nth(6).unwrap().find("opacity").unwrap() as u64 + 1;
    let sign = ask(
        &mut s,
        json!([{"op": "set-row", "file": "app.contract", "at": {"line": 7, "col": opacity}, "sha256": digest(&file), "row": "opacity", "value": 1}]),
    );
    assert!(
        sign["error"].to_string().contains("more than one line"),
        "{sign}"
    );
    // A target a style op left as it was: its digest in `requires` guards it.
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "requires": {"app.contract": digest(&file)}, "ops": [{"op": "set-row", "style": "Post", "row": "border-color", "value": "accent.fill"}]}));
    assert!(done["ok"].is_object(), "{done}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 3: a neighbour's continuation line is the neighbour's
/// (code); own border rows' order doesn't block a revert; an own row
/// moved across a layout shorthand is code; a positional argument after a
/// value is no continuation of it.
#[test]
fn set_row_round_three_edges() {
    let dir = app("set-row-r3", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let history = |s: &mut Serve, before: &str, after: &str| {
        std::fs::write(dir.join("app.contract"), after).unwrap();
        let pair = json!([{"file": "app.contract", "before": before, "after": after}]);
        (
            s.ask(json!({"method": "compare", "pairs": pair})).0,
            s.ask(json!({"method": "revert-plan", "pairs": pair})).0,
        )
    };
    let tail = "\ncomponent App\n  view\n    column class=Post\n";
    // A continuation line: color shares it with opacity's value: code.
    let (c, _) = history(
        &mut s,
        &format!("style Post\n  opacity=(\n    0.5) color=\"#ffffff\"\n{tail}"),
        &format!("style Post\n  opacity=(\n    0.4) color=\"#eeeeee\"\n{tail}"),
    );
    assert_eq!(c["ok"]["files"][0]["code"], true, "{c}");
    // Own border rows swapped beside an opacity edit: code, and the opacity reverts.
    let (c, p) = history(&mut s, &format!("style Post\n  opacity=1\n  border-color=\"#111111\"\n  border-top-color=\"#222222\"\n{tail}"), &format!("style Post\n  opacity=0.5\n  border-top-color=\"#222222\"\n  border-color=\"#111111\"\n{tail}"));
    assert_eq!(c["ok"]["files"][0]["code"], true, "{c}");
    assert_eq!(p["ok"]["ops"][0]["row"], "opacity", "{p}");
    assert!(p["ok"]["conflicts"].as_array().unwrap().is_empty(), "{p}");
    // An own row moved across a layout shorthand: code.
    let (c, _) = history(
        &mut s,
        &format!("style Post\n  border-color=\"#111111\"\n  border=\"1px solid #222222\"\n{tail}"),
        &format!("style Post\n  border=\"1px solid #222222\"\n  border-color=\"#111111\"\n{tail}"),
    );
    assert_eq!(c["ok"]["files"][0]["code"], true, "{c}");
    // A style that writes `border` then `border-color`: the colour that
    // paints is `border-color`'s, so its longhand names `border-color`.
    let view = "style Post\n  border=\"1px solid #111111\"\n  border-color=\"#222222\"\n\ncomponent App\n  view\n    column class=Post\n";
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let (last, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-row", "style": "Post", "row": "border-top-color", "value": "#000000"}]}));
    assert!(
        last["error"]
            .to_string()
            .contains("shorthand `border-color="),
        "{last}"
    );
    // A positional argument after a value: the value is still one.
    let view = "component App\n  view\n    text opacity=0.5 (\"hello\")\n";
    let file = dir.join("app.contract");
    std::fs::write(&file, view).unwrap();
    let col = view.lines().nth(2).unwrap().find("opacity").unwrap() as u64 + 1;
    let (one, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-row", "file": "app.contract", "at": {"line": 3, "col": col}, "sha256": digest(&file), "row": "opacity", "value": 0.25}]}));
    assert_eq!(one["ok"]["class"], "rebuild", "{one}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// After round 3's re-check: a style's row written by name is the one
/// set-row edits (a revert names it), even under a later shorthand that
/// writes it too.
#[test]
fn a_named_row_is_edited_under_a_later_shorthand() {
    let dir = app("set-row-named", DESIGN);
    let before = "style Post\n  border-top-color=\"#111111\"\n  border-color=\"#222222\"\n\ncomponent App\n  view\n    column class=Post\n";
    let after = before.replace("#111111", "#333333");
    std::fs::write(dir.join("app.contract"), &after).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let pair = json!([{"file": "app.contract", "before": before, "after": after}]);
    let (planned, _) = s.ask(json!({"method": "revert-plan", "pairs": pair}));
    let ops = planned["ok"]["ops"].clone();
    assert_eq!(ops[0]["row"], "border-top-color", "{planned}");
    let file = dir.join("app.contract");
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": ops}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

const CARD: &str = "component Card\n  view\n    column background-color=\"accent.fill\" testId=\"card\"\n      text \"c\"\n\ncomponent App\n  view\n    column\n      Card()\n";

/// Stage 9 D3, LLP 1122 D6: promote to param is one transaction (a
/// `set-row` reading it and its `add-param`), with one undo; either alone
/// is refused with the hint; detaching the last reader takes the param.
#[test]
fn promote_to_param_and_detach_are_each_one_transaction() {
    let dir = app("promote-param", DESIGN);
    let file = dir.join("app.contract");
    std::fs::write(&file, CARD).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let row = |line: u32, value: &str, sha: String| json!({"op": "set-row", "file": "app.contract", "at": {"line": line, "col": 12}, "sha256": sha, "row": "background-color", "value": value});
    let add = json!({"op": "add-param", "component": "Card", "name": "fill", "kind": "color", "default": "accent.fill", "attrs": {"label": "Fill", "control": "color"}});
    // Alone, an unread param: refused, with the hint.
    let (alone, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [add]}));
    assert!(
        alone["error"]["hint"]
            .as_str()
            .unwrap_or("")
            .contains("set-row"),
        "{alone}"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), CARD);
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [row(3, "param.fill", digest(&file)), add]}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        written.contains("  params\n    fill: color = \"accent.fill\""),
        "{written}"
    );
    assert!(
        written.contains("control = \"color\" label = \"Fill\""),
        "{written}"
    );
    assert!(
        written.contains("background-color=\"param.fill\""),
        "{written}"
    );
    // Removing a read param alone: refused, with the hint.
    let remove = json!({"op": "remove-param", "component": "Card", "name": "fill"});
    let (read, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [remove]}));
    assert!(read["error"]["hint"].is_string(), "{read}");
    // Detach: the row's own value again, and the param with its last reader.
    let (detached, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [row(5, "#ffffff", digest(&file)), remove]}));
    assert!(detached["ok"]["inverse"].is_string(), "{detached}");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after.contains("params") && after.contains("#ffffff"),
        "{after}"
    );
    s.ask(json!({"method": "undo", "client": "one", "inverse": detached["ok"]["inverse"]}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), written);
    s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        CARD,
        "one undo, the bytes back"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Promote to token: a new token and the row naming it, two files, one
/// transaction and one undo; an undo after a hand edit to either file is
/// refused (the commit guard), writing neither.
#[test]
fn promote_to_token_writes_two_files_as_one() {
    let dir = app("promote-token", DESIGN);
    let file = dir.join("app.contract");
    let design = dir.join("app.tamber");
    std::fs::write(&file, CARD).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let ops = json!([
        {"op": "add-token", "name": "card.fill", "kind": "color", "value": "#fafafa"},
        {"op": "set-row", "file": "app.contract", "at": {"line": 3, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "card.fill"},
    ]);
    let (design_before, card_before) =
        (std::fs::read_to_string(&design).unwrap(), CARD.to_string());
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file), "app.tamber": digest(&design)}, "ops": ops}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    assert!(std::fs::read_to_string(&design)
        .unwrap()
        .contains("card.fill"));
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("background-color=\"card.fill\""));
    // A hand edit to the design: the undo is refused, and neither file moves.
    let edited = std::fs::read_to_string(&design)
        .unwrap()
        .replace("#fafafa", "#f0f0f0");
    std::fs::write(&design, &edited).unwrap();
    let card_now = std::fs::read_to_string(&file).unwrap();
    let (refused, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert!(refused["error"].is_object(), "{refused}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), card_now);
    assert_eq!(std::fs::read_to_string(&design).unwrap(), edited);
    // Put back as written: the one undo takes both files back.
    std::fs::write(&design, edited.replace("#f0f0f0", "#fafafa")).unwrap();
    let (undone, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert!(undone["ok"].is_object(), "{undone}");
    assert_eq!(std::fs::read_to_string(&design).unwrap(), design_before);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), card_before);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D3 review round 1: the section goes in the named component, before
/// its own `view` even with a comment there; the last param's removal
/// takes a commented header and its comments; the hint is for an unread
/// param only.
#[test]
fn param_lines_find_their_own_component() {
    let dir = app("param-lines-r1", DESIGN);
    let file = dir.join("app.contract");
    let view = "component App\n  view\n    column\n      Card()\n      Other()\n\ncomponent Logic\n  state n = 0\n\ncomponent Card\n  view // body\n    column background-color=\"accent.fill\"\n      text \"c\"\n\ncomponent Other\n  view\n    column\n      text \"o\"\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let line = view
        .lines()
        .position(|l| l.contains("accent.fill"))
        .unwrap() as u32
        + 1;
    let row = json!({"op": "set-row", "file": "app.contract", "at": {"line": line, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "param.fill"});
    let add = json!({"op": "add-param", "component": "Card", "name": "fill", "kind": "color", "default": "accent.fill"});
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [row, add]}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let written = std::fs::read_to_string(&file).unwrap();
    let card = written.find("component Card").unwrap();
    let params = written.find("  params").unwrap();
    assert!(
        params > card && params < written.find("component Other").unwrap(),
        "{written}"
    );
    // A commented header and a comment inside: the last param takes both.
    let commented = written.replace("  params\n", "  params // controls\n    // colours\n");
    std::fs::write(&file, &commented).unwrap();
    let pline = commented
        .lines()
        .position(|l| l.contains("param.fill"))
        .unwrap() as u32
        + 1;
    let back = json!({"op": "set-row", "file": "app.contract", "at": {"line": pline, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "#ffffff"});
    let remove = json!({"op": "remove-param", "component": "Card", "name": "fill"});
    let (detached, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": [back, remove]}));
    assert!(detached["ok"]["inverse"].is_string(), "{detached}");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after.contains("params") && !after.contains("colours"),
        "{after}"
    );
    // A promote that fails for another reason: no unread-param hint.
    std::fs::write(&file, view).unwrap();
    let bad = json!({"op": "add-param", "component": "Card", "name": "fill", "kind": "color", "default": "accent.nope"});
    let row = json!({"op": "set-row", "file": "app.contract", "at": {"line": line, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "param.fill"});
    let (refused, _) = s.ask(json!({"method": "propose", "ops": [row, bad]}));
    assert!(refused["error"]["diagnostics"].is_array(), "{refused}");
    assert!(refused["error"].get("hint").is_none(), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// D3 review round 2: a param the app sets a value for in its design is
/// not detached (LLP 1122 D6), both files left as they are; the last
/// param takes the comments under it too.
#[test]
fn detach_refuses_a_param_the_app_sets() {
    let dir = app("detach-set", DESIGN);
    let file = dir.join("app.contract");
    let design = dir.join("app.tamber");
    let view = "component App\n  view\n    column\n      Card()\n\ncomponent Card\n  params\n    fill: color = \"accent.fill\"\n    // under it\n  view\n    column background-color=\"param.fill\"\n      text \"c\"\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let (set, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&design)}, "ops": [{"op": "set-param", "name": "Card.fill", "value": "#fafafa"}]}));
    assert!(set["ok"]["inverse"].is_string(), "{set}");
    let line = view
        .lines()
        .position(|l| l.contains("param.fill\""))
        .unwrap() as u32
        + 1;
    let detach = json!([
        {"op": "set-row", "file": "app.contract", "at": {"line": line, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "#ffffff"},
        {"op": "remove-param", "component": "Card", "name": "fill"},
    ]);
    let (design_before, card_before) = (
        std::fs::read_to_string(&design).unwrap(),
        std::fs::read_to_string(&file).unwrap(),
    );
    let (refused, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file), "app.tamber": digest(&design)}, "ops": detach}));
    assert!(
        refused["error"].to_string().contains("sets in"),
        "{refused}"
    );
    assert_eq!(std::fs::read_to_string(&design).unwrap(), design_before);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), card_before);
    // Unset in the same transaction: detached, the section with its comment.
    let mut ops = detach.as_array().unwrap().clone();
    ops.insert(0, json!({"op": "unset-param", "name": "Card.fill"}));
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file), "app.tamber": digest(&design)}, "ops": ops}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after.contains("params") && !after.contains("under it"),
        "{after}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// D3 after round 3: the hint is for this transaction's param named whole
/// (not `missing` beside `(its params: fill)`); the last param takes a
/// comment under it past a blank line.
#[test]
fn param_hint_names_its_own_param_whole() {
    let dir = app("param-hint-r3", DESIGN);
    let file = dir.join("app.contract");
    let view = "component App\n  view\n    column\n      Card()\n\ncomponent Card\n  view\n    column background-color=\"accent.fill\" color=\"param.missing\"\n      text \"c\"\n";
    std::fs::write(&file, view).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let line = view
        .lines()
        .position(|l| l.contains("accent.fill"))
        .unwrap() as u32
        + 1;
    let ops = json!([
        {"op": "set-row", "file": "app.contract", "at": {"line": line, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "param.fill"},
        {"op": "add-param", "component": "Card", "name": "fill", "kind": "color", "default": "accent.fill"},
    ]);
    let (refused, _) = s.ask(json!({"method": "propose", "ops": ops}));
    assert!(
        refused["error"]["diagnostics"]
            .to_string()
            .contains("missing"),
        "{refused}"
    );
    assert!(
        refused["error"].get("hint").is_none(),
        "another param's error: {refused}"
    );
    // A comment under the last param, past a blank line, goes with it.
    let with = "component App\n  view\n    column\n      Card()\n\ncomponent Card\n  params\n    fill: color = \"accent.fill\"\n\n    // fill help\n  view\n    column background-color=\"param.fill\"\n      text \"c\"\n";
    std::fs::write(&file, with).unwrap();
    let pline = with
        .lines()
        .position(|l| l.contains("param.fill\""))
        .unwrap() as u32
        + 1;
    let detach = json!([
        {"op": "set-row", "file": "app.contract", "at": {"line": pline, "col": 12}, "sha256": digest(&file), "row": "background-color", "value": "#ffffff"},
        {"op": "remove-param", "component": "Card", "name": "fill"},
    ]);
    let (done, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.contract": digest(&file)}, "ops": detach}));
    assert!(done["ok"]["inverse"].is_string(), "{done}");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after.contains("fill help") && !after.contains("params"),
        "{after}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// LLP 1122's queue item (1): a `:hover` row is edited at its part's place
/// as the compiler's map gives it (its row's `states`), never at its base
/// row's: `set-row` with `state` on the node's own part (a side's
/// `border-color` whole), `set-state` on a style's; undo gives the bytes
/// back.
#[test]
fn a_look_row_is_edited_at_the_part_its_map_gives() {
    let dir = app("set-row-look", DESIGN);
    let file = dir.join("app.contract");
    let view = "style Card\n  :hover\n    opacity=0.9\n\ncomponent App\n  view\n    column class=Card background-color=\"accent.fill\" testId=\"card\"\n      :hover\n        background-color=\"accent.pressed\"\n        border-color=\"#111111\"\n      text \"Post\"\n";
    std::fs::write(&file, view).unwrap();
    let (plan, map) = contract::compile_path_mapped(&file).unwrap();
    let map: Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let part = |row: &str| {
        let bindings = map["nodes"].as_array().unwrap().iter();
        bindings
            .flat_map(|n| n["bindings"].as_array().unwrap())
            .find(|b| b["row"] == row && b.get("states").is_some())
            .map(|b| b["states"][0].clone())
            .unwrap_or_else(|| panic!("{row}: {map}"))
    };
    let (bg, side, faded) = (
        part("background_color"),
        part("border_color_top"),
        part("opacity"),
    );
    assert_eq!(
        [&bg, &side, &faded].map(|p| (p["state"].clone(), p["at"]["line"].clone())),
        [
            (json!("hover"), json!(9)),
            (json!("hover"), json!(10)),
            (json!("hover"), json!(3))
        ]
    );
    assert_eq!(faded["origin"], "class:Card");
    let mut s = Serve::start(&dir, &[]);
    let set = |s: &mut Serve, p: &Value, row: &str, value: &str| {
        let at = json!({"line": p["at"]["line"], "col": p["at"]["col"]});
        let op = json!({"op": "set-row", "file": "app.contract", "at": at, "sha256": digest(&file), "row": row, "state": "hover", "value": value});
        transaction(s, &file, op)
    };
    let one = set(&mut s, &bg, "background-color", "#000000");
    let two = set(&mut s, &side, "border-color", "#222222");
    let op = json!({"op": "set-state", "style": "Card", "state": "hover", "row": "opacity", "value": 0.8});
    let three = transaction(&mut s, &file, op);
    let written = std::fs::read_to_string(&file).unwrap();
    let changed: Vec<usize> = (view.lines().zip(written.lines()).enumerate())
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i + 1)
        .collect();
    // The three parts' lines, and the base row's (line 7) untouched.
    assert_eq!(changed, [3, 9, 10], "{written}");
    assert!(
        written.contains("background-color=\"#000000\"")
            && written.contains("border-color=\"#222222\"")
            && written.contains("opacity=0.8"),
        "{written}"
    );
    for done in [three, two, one] {
        assert!(done["ok"]["inverse"].is_string(), "{done}");
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), view);
    let _ = std::fs::remove_dir_all(&dir);
}

/// LLP 1122's queue item (2): a folded row's base and each of its parts are
/// edited apart, at the places the compiler's map gives (the base's
/// attribute; each part's row in `states`): editing the base leaves each
/// part's line as it was, and editing a part leaves the base's and the
/// other part's, byte for byte (review2's 04:24); undo gives the bytes back.
#[test]
fn a_folded_rows_base_and_parts_are_edited_apart() {
    let dir = app("set-row-folds", DESIGN);
    let file = dir.join("app.contract");
    let view = "component App\n  state flag = true\n  view\n    box background-color=\"accent.fill\" disabled=flag aria-checked=flag role=\"checkbox\" testId=\"card\"\n      :disabled\n        background-color=\"#999999\"\n      :checked\n        background-color=\"#00ff00\"\n      text \"Post\"\n";
    std::fs::write(&file, view).unwrap();
    let (plan, map) = contract::compile_path_mapped(&file).unwrap();
    let map: Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let bg = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|n| n["bindings"].as_array().unwrap())
        .find(|b| b["row"] == "background_color" && b.get("states").is_some())
        .cloned()
        .unwrap_or_else(|| panic!("{map}"));
    assert_eq!(bg["at"]["line"], 4, "{bg}");
    let part = |state: &str| {
        bg["states"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["state"] == state)
            .cloned()
            .unwrap_or_else(|| panic!("{state}: {bg}"))
    };
    let (disabled, checked) = (part("disabled"), part("checked"));
    assert_eq!(
        (
            disabled["at"]["line"].clone(),
            checked["at"]["line"].clone()
        ),
        (json!(6), json!(8))
    );
    let mut s = Serve::start(&dir, &[]);
    let edit = |s: &mut Serve, at: &Value, state: Option<&str>, value: &str| {
        let mut op = json!({"op": "set-row", "file": "app.contract", "at": {"line": at["line"], "col": at["col"]}, "sha256": digest(&file), "row": "background-color", "value": value});
        if let Some(state) = state {
            op["state"] = json!(state);
        }
        let done = transaction(s, &file, op);
        assert!(done["ok"]["inverse"].is_string(), "{done}");
        let written = std::fs::read_to_string(&file).unwrap();
        let changed: Vec<usize> = (view.lines().zip(written.lines()).enumerate())
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i + 1)
            .collect();
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            view,
            "undo gives the bytes back"
        );
        (changed, written)
    };
    let (base, written) = edit(&mut s, &bg["at"], None, "#ffffff");
    assert_eq!(base, [4], "the base's line alone: {written}");
    let (off, written) = edit(&mut s, &disabled["at"], Some("disabled"), "#888888");
    assert_eq!(off, [6], "the `:disabled` part's line alone: {written}");
    let (on, written) = edit(&mut s, &checked["at"], Some("checked"), "#00aa00");
    assert_eq!(on, [8], "the `:checked` part's line alone: {written}");
    let _ = std::fs::remove_dir_all(&dir);
}
