//! A Markdown node's faces in the theme index (LLP 1045.000 D2, LLP 1121
//! D7): read again when a pin above them, the mode or a token moves; kept
//! as they were when a read again finds a colour the first read took is
//! none, and never for one the first read refused; and gone from the index
//! when they read no token, are cleared, or their node is unmounted.

use crate::{DataError, DataSource, Event, Runner};
use exact_kernel::{Kernel, PropId, ViewId};
use exact_plan::Value;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A design with a token whose slice the `dark-style` axis picks, and one
/// that does not vary.
const MODES: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = light-dark(#111111, #eeeeee)\n    when dark-style=dim: light-dark(#111111, #cccccc)\n  brand = #c8102e\n";

/// `contract`'s plan beside the design.
fn plan(name: &str, contract: &str) -> exact_plan::Plan {
    let dir = std::env::temp_dir().join(format!("exact2-faces-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), MODES).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    plan
}

fn boot(plan: exact_plan::Plan) -> Runner<NoData> {
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view(r: &Runner<NoData>, test: &str) -> Option<ViewId> {
    let k = r.kernel();
    let key = *k.find_by_test_id(test).first()?;
    Some(k.node_by_key(key).unwrap().id)
}

fn press(r: &mut Runner<NoData>, test: &str) {
    let id = view(r, test).unwrap();
    r.dispatch(id, Event::Press).unwrap();
}

/// The editor's faces, or `None` when its prop is unset.
fn faces(r: &Runner<NoData>) -> Option<serde_json::Value> {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("editor")[0]).unwrap();
    let text = node.props.str(PropId::AtomFaces)?;
    Some(serde_json::from_str(text).unwrap())
}

fn paint(r: &Runner<NoData>, key: &str) -> String {
    faces(r).unwrap()[0][key].as_str().unwrap().to_string()
}

/// The views whose faces the index keeps.
fn indexed(r: &Runner<NoData>) -> Vec<ViewId> {
    r.ids.theme.faces().map(|(view, _)| view).collect()
}

const FACE: &str = "AtomFace(source=\"@a\", label=\"\", icon=\"\", title=\"\", direction=\"\"";

/// A pin above the node that moves reads its faces again in the slice the
/// pin makes, though nothing they read changed (D7, as a row's text).
#[test]
fn a_pin_above_reads_a_faces_colour_again() {
    let mut r = boot(plan(
        "pin",
        &format!("atom mention prefix=\"@\"\ncomponent App\n  state dim = false\n  derive faces = [{FACE}, color=\"ink\", backgroundColor=\"\")]\n  action flip\n    dim = not dim\n  view\n    column\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column -exact-mode=(dim ? \"dark-style=dim\" : \"dark-style=black\")\n        textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"editor\"\n"),
    ));
    assert_eq!(paint(&r, "color"), "light-dark(#111111ff, #eeeeeeff)");
    press(&mut r, "flip");
    assert_eq!(paint(&r, "color"), "light-dark(#111111ff, #ccccccff)");
    press(&mut r, "flip");
    assert_eq!(paint(&r, "color"), "light-dark(#111111ff, #eeeeeeff)");
}

/// An app whose editor's faces paint `color` and `backgroundColor`, with a
/// button that switches the app's mode to the token's dim slice.
fn painted(name: &str, color: &str, background: &str, head: &str) -> Runner<NoData> {
    boot(plan(
        name,
        &format!("{head}atom mention prefix=\"@\"\ncomponent App\n  derive faces = [{FACE}, color=\"{color}\", backgroundColor=\"{background}\")]\n  action dim\n    setMode(\"dark-style\", \"dim\")\n  view\n    column\n      button press=dim testId=\"dim\"\n        text \"dim\"\n      textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"editor\"\n"),
    ))
}

/// The journal's face lines.
fn said(r: &Runner<NoData>) -> Vec<String> {
    let lines = r
        .journal()
        .filter(|l| l.contains("invalid atomFaces colour"));
    lines.map(str::to_string).collect()
}

/// A token edited to a value that is no colour leaves the faces that read
/// it as they were, as a row its parse refuses keeps its value, and the
/// journal names the face's colour, on that edit and on each read again
/// while it is none (a mode switch); a colour again sets them, in the
/// tuple the node is in now.
#[test]
fn a_token_that_is_no_colour_leaves_the_faces_as_they_were() {
    let mut r = painted("no-colour", "ink", "brand", "");
    let before = faces(&r);
    let (_, set) = r.set_token("brand", "12px", &[]).unwrap();
    assert_eq!(set, 0);
    assert_eq!(faces(&r), before, "the faces stay as they were");
    press(&mut r, "dim");
    assert_eq!(
        faces(&r),
        before,
        "and when the mode moves while it is none"
    );
    let editor = view(&r, "editor").unwrap();
    let kept =
        format!("view {editor}: invalid atomFaces colour \"brand\"; its faces stay as they were");
    let lines = said(&r);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|l| l.ends_with(&kept)), "{lines:?}");
    let (_, set) = r.set_token("brand", "#00ff00", &[]).unwrap();
    assert_eq!(set, 1);
    assert_eq!(paint(&r, "backgroundColor"), "#00ff00ff");
    assert_eq!(
        paint(&r, "color"),
        "light-dark(#111111ff, #ccccccff)",
        "in the mode's slice"
    );
}

/// A colour the faces' first read refused, said once there, keeps no edit
/// from them: beside a token, a mode switch and a token's edit set the
/// token's colour and leave the refused one empty, with no other line.
/// So for a colour in a profile the app declares (LLP 1100 D3): the first
/// read refuses it, and every read again does, as the first.
#[test]
fn a_refused_colour_beside_a_token_blocks_no_edit() {
    let press_icc =
        "color-profile --press src=\"assets/press.icc\" rendering-intent=\"perceptual\"\n";
    for (name, refused, head) in [
        ("beside", "notacolour", ""),
        ("declared", "color(--press 0.1 0.8 0.2)", press_icc),
    ] {
        let mut r = painted(name, "ink", refused, head);
        let editor = view(&r, "editor").unwrap();
        let first = [format!(
            "view {editor}: invalid atomFaces colour {refused:?}; its kind's paint stands"
        )];
        let ends = |lines: Vec<String>| lines.len() == 1 && lines[0].ends_with(&first[0]);
        assert_eq!(paint(&r, "backgroundColor"), "", "{name}");
        assert!(ends(said(&r)), "{name}: {:?}", said(&r));
        press(&mut r, "dim");
        assert_eq!(
            paint(&r, "color"),
            "light-dark(#111111ff, #ccccccff)",
            "{name}"
        );
        assert_eq!(paint(&r, "backgroundColor"), "", "{name}");
        let (_, set) = r.set_token("ink", "#0000ff", &[]).unwrap();
        assert_eq!(set, 1, "{name}");
        assert_eq!(paint(&r, "color"), "#0000ffff", "{name}");
        assert_eq!(paint(&r, "backgroundColor"), "", "{name}");
        assert!(ends(said(&r)), "{name}: said once: {:?}", said(&r));
    }
}

/// `test`'s faces' `key` colour.
fn paint_of(r: &Runner<NoData>, test: &str, key: &str) -> String {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id(test)[0]).unwrap();
    let faces: serde_json::Value =
        serde_json::from_str(node.props.str(PropId::AtomFaces).unwrap()).unwrap();
    faces[0][key].as_str().unwrap().to_string()
}

/// What the faces as last set refused decides a read again: a colour they
/// took that is none now keeps them, wherever they were first read. Here
/// B's first read refuses `brand`, a later edit sets B with it green, and
/// the next edit to no colour keeps B green and says so, as it keeps A.
#[test]
fn a_read_again_weighs_the_faces_as_last_set() {
    let mut r = boot(plan(
        "as-set",
        &format!("atom mention prefix=\"@\"\ncomponent App\n  state shown = false\n  derive faces = [{FACE}, color=\"ink\", backgroundColor=\"brand\")]\n  action show\n    shown = true\n  view\n    column\n      button press=show testId=\"show\"\n        text \"show\"\n      textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"a\"\n      if shown\n        textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"b\"\n"),
    ));
    r.set_token("brand", "12px", &[]).unwrap();
    press(&mut r, "show");
    assert_eq!(
        paint_of(&r, "b", "backgroundColor"),
        "",
        "B's first read refuses it"
    );
    let (_, set) = r.set_token("brand", "#00ff00", &[]).unwrap();
    assert_eq!(set, 2);
    for test in ["a", "b"] {
        assert_eq!(paint_of(&r, test, "backgroundColor"), "#00ff00ff", "{test}");
    }
    let before = said(&r).len();
    let (_, set) = r.set_token("brand", "12px", &[]).unwrap();
    assert_eq!(set, 0);
    for test in ["a", "b"] {
        assert_eq!(
            paint_of(&r, test, "backgroundColor"),
            "#00ff00ff",
            "{test} stays"
        );
    }
    let lines = said(&r);
    assert_eq!(lines.len(), before + 2, "{lines:?}");
    for test in ["a", "b"] {
        let line = format!(
            "view {}: invalid atomFaces colour \"brand\"; its faces stay as they were",
            view(&r, test).unwrap()
        );
        assert!(
            lines[before..].iter().any(|l| l.ends_with(&line)),
            "{test}: {lines:?}"
        );
    }
}

/// Faces whose colours read no token now, here a colour from state, leave
/// the index, and a token's edit sets nothing; reading one again puts them
/// back.
#[test]
fn faces_that_read_no_token_leave_the_index() {
    let mut r = boot(plan(
        "plain",
        &format!("atom mention prefix=\"@\"\ncomponent App\n  state c = \"ink\"\n  derive faces = [{FACE}, color=c, backgroundColor=\"\")]\n  action plain\n    c = \"#ff0000\"\n  action token\n    c = \"ink\"\n  view\n    column\n      button press=plain testId=\"plain\"\n        text \"plain\"\n      button press=token testId=\"token\"\n        text \"token\"\n      textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"editor\"\n"),
    ));
    let editor = view(&r, "editor").unwrap();
    assert_eq!(indexed(&r), [editor]);
    press(&mut r, "plain");
    assert!(indexed(&r).is_empty());
    let (_, set) = r.set_token("ink", "#0000ff", &[]).unwrap();
    assert_eq!(set, 0);
    assert_eq!(paint(&r, "color"), "#ff0000ff");
    press(&mut r, "token");
    assert_eq!(indexed(&r), [editor]);
    assert_eq!(paint(&r, "color"), "#0000ffff");
}

/// An unmounted node's faces are set by no token's edit, and leave the
/// index with the node's other entries (`Ids::retain`, which a commit runs
/// once destroyed views outnumber live ones).
#[test]
fn an_unmounted_nodes_faces_leave_the_index() {
    let mut r = boot(plan(
        "unmount",
        &format!("atom mention prefix=\"@\"\ncomponent App\n  state shown = true\n  derive faces = [{FACE}, color=\"ink\", backgroundColor=\"\")]\n  action hide\n    shown = not shown\n  view\n    column\n      button press=hide testId=\"hide\"\n        text \"hide\"\n      if shown\n        textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=faces testId=\"editor\"\n"),
    ));
    let editor = view(&r, "editor").unwrap();
    assert_eq!(indexed(&r), [editor]);
    press(&mut r, "hide");
    assert!(view(&r, "editor").is_none());
    let (_, set) = r.set_token("ink", "#0000ff", &[]).unwrap();
    assert_eq!(set, 0, "a retired view is not set");
    let kernel = &r.kernel;
    r.ids.retain(|view| kernel.node(view).is_some());
    assert!(indexed(&r).is_empty());
}

/// Faces the bridge cannot read are cleared, journalled, and leave the
/// index. A compiled plan's records always read; this one's `broken` is
/// made text, as a plan from another binary's compiler might hold.
#[test]
fn cleared_faces_leave_the_index() {
    let mut plan = plan(
        "clear",
        &format!("atom mention prefix=\"@\"\ncomponent App\n  state bad = false\n  state broken: AtomFace[] = []\n  state word = \"text\"\n  derive faces = [{FACE}, color=\"ink\", backgroundColor=\"\")]\n  action spoil\n    bad = true\n  view\n    column\n      button press=spoil testId=\"spoil\"\n        text word\n      textarea value=\"@a \" markup=\"markdown\" atoms=\"mention\" atomFaces=(bad ? broken : faces) testId=\"editor\"\n"),
    );
    let slot = |plan: &exact_plan::Plan, name: &str| {
        plan.slots
            .iter()
            .position(|s| plan.str(s.name) == name)
            .unwrap()
    };
    let (broken, word) = (slot(&plan, "broken"), slot(&plan, "word"));
    plan.slots[broken].init = plan.slots[word].init;
    plan.slots[broken].ty = plan.slots[word].ty;
    let mut r = boot(plan);
    let editor = view(&r, "editor").unwrap();
    assert_eq!(indexed(&r), [editor]);
    press(&mut r, "spoil");
    assert_eq!(faces(&r), None, "cleared");
    assert!(
        r.journal()
            .any(|l| l.contains(&format!("view {editor}: invalid atomFaces value"))),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    assert!(indexed(&r).is_empty());
    let (_, set) = r.set_token("ink", "#0000ff", &[]).unwrap();
    assert_eq!(set, 0);
    assert_eq!(faces(&r), None);
}
