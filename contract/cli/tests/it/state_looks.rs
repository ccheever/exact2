//! @ref LLP 1121 D8 (stage 8 B): a node's `:hover` and `:focus-visible`
//! looks reach the kernel resolved, as a static row is, and a token's live
//! edit resolves them again; `:active` is the node's press paint.

use exact_kernel::{Kernel, LOOK_HOVER};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-looks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

fn booted(plan: Plan) -> Runner<NoData> {
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

const DESIGN: &str =
    "tamber t\n\ntheme\n  accent.soft = #eeeeee\n  ink = #111111\n  page = #ffffff\n";

const VIEW: &str = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column testId=\"card\" press=go background-color=\"page\"\n      :active\n        background-color = \"accent.soft\"\n        opacity = 0.9\n      :hover\n        border-color = \"ink\"\n      text \"c\"\n";

fn looks(r: &Runner<NoData>) -> String {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
    format!(
        "{:?}",
        node.looks()
            .iter()
            .map(|(s, p)| (*s, p.background_color, p.opacity, p.border_color_top))
            .collect::<Vec<_>>()
    )
}

#[test]
fn a_nodes_looks_reach_the_kernel_resolved_and_follow_a_live_token() {
    let path = app("looks", &[("app.tamber", DESIGN), ("app.contract", VIEW)]);
    let r = booted(contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}")));
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
    let states: Vec<u8> = node.looks().iter().map(|(s, _)| *s).collect();
    assert_eq!(
        states,
        [LOOK_HOVER],
        "a :hover look; :active is the press paint"
    );
    // `:active` is the node's own press rows (LLP 1121 D8 as amended).
    let pressed = |r: &Runner<NoData>| {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
        (
            format!("{:?}", node.style.press_background_color),
            node.style.press_opacity_to,
        )
    };
    let (fill, to) = pressed(&r);
    assert_eq!(to, 0.9, "the absolute target");
    assert_eq!(
        format!("{:?}", node.style.background_color),
        "Some(Fixed(Color(4294967295)))",
        "the base keeps its own white"
    );
    let before = looks(&r);
    let mut r = r;
    r.set_token("accent.soft", "#ff0000", &[]).unwrap();
    assert_ne!(
        pressed(&r).0,
        fill,
        "the press's row follows its token, as any row"
    );
    assert_eq!(looks(&r), before, "no look read it");
    // The hover's own token moves its look; a token only the base reads sets no look.
    r.set_token("ink", "#222222", &[]).unwrap();
    let inked = looks(&r);
    assert_ne!(before, inked, "the hover follows its token");
    let (_, set) = r.set_token("page", "#000000", &[]).unwrap();
    assert_eq!(set, 1, "the base's row alone");
    assert_eq!(looks(&r), inked);
}

/// One spelling a node (LLP 1121 D8): `:active` and D7's pressed paint
/// saying the same paint on one node (or its class) is refused, naming both;
/// different paint, or another state, is not.
#[test]
fn an_active_look_and_the_pressed_paint_say_one_paint_once() {
    let refused = |rows: &str, part: &str| {
        let src = format!("style Card\n  {rows}\n\ncomponent App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column class=Card press=go\n      {part}\n      text \"c\"\n");
        contract::compile(&src)
            .err()
            .map(|e| (e.id.to_string(), e.message.clone()))
    };
    let (id, message) = refused(
        "-exact-press-background-color = \"#000000\"",
        ":active\n        background-color = \"#111111\"",
    )
    .expect("refused");
    assert_eq!(id, "lower-state-press-paint");
    assert!(
        message.contains("-exact-press-background-color") && message.contains("background-color"),
        "{message}"
    );
    let (id, _) = refused(
        "-exact-press-opacity = 0.5",
        ":active\n        opacity = 0.9",
    )
    .expect("refused");
    assert_eq!(id, "lower-state-press-paint");
    assert!(refused(
        "-exact-press-opacity = 0.5",
        ":active\n        background-color = \"#111111\""
    )
    .is_none());
    assert!(refused(
        "-exact-press-background-color = \"#000000\"",
        ":hover\n        background-color = \"#111111\""
    )
    .is_none());
}

/// A look's explicit clear (`unset`) crosses as a static row's does:
/// written, with no value.
#[test]
fn a_looks_unset_crosses_written_with_no_value() {
    let src = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column testId=\"card\" press=go opacity=0.5\n      :hover\n        opacity = \"unset\"\n      text \"c\"\n";
    let r = booted(contract::compile(src).unwrap_or_else(|e| panic!("{e:?}")));
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
    let (state, look) = &node.looks()[0];
    assert_eq!(*state, LOOK_HOVER);
    assert!(
        look.written.has(exact_kernel::StyleId::Opacity)
            && !look.mask.has(exact_kernel::StyleId::Opacity)
    );
}

/// An `:active` row lowered to the press paint is used where it is written
/// (theme2's 06:14 condition): `uses` names its `:active` line and state.
#[test]
fn uses_names_an_active_rows_own_line() {
    let path = app("uses", &[("app.tamber", DESIGN), ("app.contract", VIEW)]);
    let (plan, map) = contract::compile_path_mapped(&path).unwrap();
    let map: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let sites = contract::tamber_uses::sites(&plan, &map, "accent.soft", &Default::default(), &[]);
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert_eq!(sites[0]["row"], "press_background_color", "{sites:?}");
    assert_eq!(sites[0]["state"], "active");
    assert_eq!(sites[0]["line"], 8, "the `:active` row's line: {sites:?}");
}

/// A token a `:hover` row reads is used where that row is written, as that
/// state's: its `:hover` line, not its node's (LLP 1122's queue item (1);
/// theme2's 03:18, a correction of the line it named).
#[test]
fn uses_finds_a_hover_looks_token() {
    let path = app(
        "uses-hover",
        &[("app.tamber", DESIGN), ("app.contract", VIEW)],
    );
    let (plan, map) = contract::compile_path_mapped(&path).unwrap();
    let map: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let sites = contract::tamber_uses::sites(&plan, &map, "ink", &Default::default(), &[]);
    assert_eq!(sites.len(), 4, "a border colour's four sides: {sites:?}");
    assert!(sites.iter().all(|s| s["state"] == "hover"), "{sites:?}");
    assert!(
        sites.iter().all(|s| s["line"] == 11),
        "the `:hover` row's line: {sites:?}"
    );
}

/// LLP 1122 D8 (stage 9 E): `layout <node>` says a node's looks' rows as it
/// says its own: each row's value and the token it read, so inspect on a
/// held hover names the look's tokens; a literal row reads none.
#[test]
fn a_nodes_layout_says_its_looks_rows_and_their_tokens() {
    let view = VIEW.replace(
        "        border-color = \"ink\"\n",
        "        border-color = \"ink\"\n        opacity = 0.5\n",
    );
    let path = app(
        "looks-layout",
        &[("app.tamber", DESIGN), ("app.contract", &view)],
    );
    let r = booted(contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}")));
    let k = r.kernel();
    let id = k.node_by_key(k.find_by_test_id("card")[0]).unwrap().id;
    let node: serde_json::Value = serde_json::from_str(&exact_runner::agent::node(&r, id)).unwrap();
    let hover = &node["looks"]["hover"];
    assert_eq!(
        hover["border_color_top"]["token"]["names"],
        serde_json::json!(["ink"]),
        "{hover}"
    );
    assert_eq!(hover["border_color_top"]["value"], "#111111", "{hover}");
    assert_eq!(hover["opacity"]["value"], 0.5, "{hover}");
    assert!(
        hover["opacity"].get("token").is_none(),
        "a literal reads no token: {hover}"
    );
    assert!(
        node["looks"].get("focus_visible").is_none(),
        "only the states it has"
    );
    // A look's clear (`unset`) is said, not dropped.
    let cleared = VIEW.replace(
        "        border-color = \"ink\"\n",
        "        border-color = \"ink\"\n        opacity = \"unset\"\n",
    );
    let path = app(
        "looks-unset",
        &[("app.tamber", DESIGN), ("app.contract", &cleared)],
    );
    let r2 = booted(contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}")));
    let k2 = r2.kernel();
    let id2 = k2.node_by_key(k2.find_by_test_id("card")[0]).unwrap().id;
    let node2: serde_json::Value =
        serde_json::from_str(&exact_runner::agent::node(&r2, id2)).unwrap();
    assert_eq!(
        node2["looks"]["hover"]["opacity"],
        serde_json::json!({"value": "unset"}),
        "{node2}"
    );
    // The press's rows are the node's own, with their token as before.
    assert_eq!(
        node["style"]["press_background_color"]["token"]["names"],
        serde_json::json!(["accent.soft"])
    );
}

/// A conditional class's `:active` part says no place (either style's):
/// `uses` reads it at its node, still as `:active`'s (Astra, round 2 of
/// LLP 1122's queue item (1)).
#[test]
fn uses_keeps_a_conditional_active_parts_state_at_its_node() {
    let view = "style A\n  :active\n    background-color = \"accent.soft\"\n\nstyle B\n  :active\n    background-color = \"accent.soft\"\n\ncomponent App\n  state n = 0\n  state flag = true\n  action go\n    n = n + 1\n  view\n    column class=(flag ? A : B) press=go testId=\"card\"\n      text \"c\"\n";
    let path = app(
        "uses-either",
        &[("app.tamber", DESIGN), ("app.contract", view)],
    );
    let (plan, map) = contract::compile_path_mapped(&path).unwrap();
    let map: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let sites = contract::tamber_uses::sites(&plan, &map, "accent.soft", &Default::default(), &[]);
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert_eq!(sites[0]["state"], "active", "{sites:?}");
    assert_eq!(sites[0]["line"], 15, "the node's line: {sites:?}");
}

/// Stage 12 B3 (round 1, Grok 1): a look resolved again keeps what its new
/// text read beside what it read, as a row does, so a base a re-set shadow
/// newly reads sets the look again when it is edited.
#[test]
fn a_look_follows_a_base_its_shadows_new_value_reads() {
    let design =
        "tamber t\n\ntheme\n  space.md = 10px\n  card.shadow: box-shadow = 0 2px 4px #000\n";
    let view = "component App\n  view\n    column testId=\"card\" padding=\"space.md\"\n      :hover\n        box-shadow = \"card.shadow\"\n      text \"c\"\n";
    let path = app(
        "look-base",
        &[("app.tamber", design), ("app.contract", view)],
    );
    let mut r = booted(contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}")));
    let shadow = |r: &Runner<NoData>| {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id("card")[0]).unwrap();
        format!(
            "{:?}",
            node.looks()
                .iter()
                .map(|(s, p)| (*s, p.box_shadow.clone()))
                .collect::<Vec<_>>()
        )
    };
    assert!(shadow(&r).contains("y: 2.0"), "{}", shadow(&r));
    r.set_token("card.shadow", "0px space.md 4px #000000ff", &[])
        .unwrap();
    assert!(shadow(&r).contains("y: 10.0"), "{}", shadow(&r));
    r.set_token("space.md", "30px", &[]).unwrap();
    assert!(shadow(&r).contains("y: 30.0"), "{}", shadow(&r));
}
