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
