//! The binding core (LLP 1121 D12, stage 5, commit 3): a token by its name,
//! resolved in one session for a place, and what tells its observers.

use exact_kernel::{ColorValue, Kernel};
use exact_plan::{Plan, Value};
use exact_runner::{design, DataError, DataSource, Place, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const DESIGN: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = light-dark(#111111, #eeeeee)\n    when dark-style=dim: light-dark(#111111, #cccccc)\n  CanvasText = #101010\n";
const VIEW: &str = "component App\n  view\n    column color=\"ink\"\n      text \"a\" testId=\"outer\"\n      column testId=\"card\" -exact-mode=\"dark-style=dim\" color=\"ink\"\n        text \"b\" testId=\"inner\"\n";

fn plan(name: &str, design: &str) -> Plan {
    let dir = std::env::temp_dir().join(format!("exact2-binding-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), VIEW).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    plan
}

fn booted(plan: Plan) -> Runner<NoData> {
    let mut r = Runner::boot(
        contract::bake(plan, NoData).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.admit_live(None);
    r
}

fn parsed(text: &str) -> ColorValue {
    ColorValue::parse_light_dark(text)
        .or_else(|| exact_kernel::Color::parse(text).map(ColorValue::Fixed))
        .unwrap()
}

fn node(r: &Runner<NoData>, id: &str) -> Place {
    let k = r.kernel();
    Place::Node(k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id)
}

/// Two places in one session under different `-exact-mode` pins answer
/// differently; the root is the window's tuple; a role override reads as
/// the session's.
#[test]
fn a_place_resolves_its_own_tuple() {
    let mut r = booted(plan("places", DESIGN));
    let (outer, inner) = (node(&r, "outer"), node(&r, "inner"));
    let mut t = r.tamber();
    let black = parsed("light-dark(#111111, #eeeeee)");
    assert_eq!(t.color("ink", Place::Root), Some(black));
    assert_eq!(t.color("ink", outer), Some(black));
    assert_eq!(
        t.color("ink", inner),
        Some(parsed("light-dark(#111111, #cccccc)"))
    );
    assert_ne!(t.tuple(outer), t.tuple(inner));
    assert_eq!(t.color("CanvasText", Place::Root), Some(parsed("#101010")));
}

/// A name the plan lacks (a token renamed away) keeps the caller's
/// fallback, said once in the journal.
#[test]
fn a_name_the_plan_lacks_is_said_once() {
    let mut r = booted(plan("missing", DESIGN));
    assert_eq!(r.tamber().color("text.primary", Place::Root), None);
    assert_eq!(r.tamber().color("text.primary", Place::Root), None);
    let said = r
        .journal()
        .filter(|l| l.contains("no token `text.primary`"))
        .count();
    assert_eq!(said, 1);
    // A live edit that is no colour: the same, said once.
    let _ = r.set_token("ink", "nope", &[]);
    assert_eq!(r.tamber().color("ink", Place::Root), None);
    assert_eq!(r.tamber().color("ink", Place::Root), None);
    let said = r
        .journal()
        .filter(|l| l.contains("`ink`'s value is no colour"))
        .count();
    assert_eq!(said, 1);
}

/// `inputs` moves on each input a binding reads, and holds when none does:
/// `setMode`, the scheme, the contrast, the root font size, a live batch,
/// a role override's live edit, and a token's edit.
#[test]
fn inputs_move_on_every_input_and_only_then() {
    let a = plan("inputs-a", DESIGN);
    let mut r = booted(a.clone());
    let place = node(&r, "outer");
    let mut last = r.tamber().inputs(place);
    let mut moved = |r: &mut Runner<NoData>, what: &str| {
        let now = r.tamber().inputs(place);
        assert_ne!(now, last, "{what} moves the inputs");
        last = now;
    };
    r.set_modes(&[("dark-style", "dim")]).unwrap();
    moved(&mut r, "setMode");
    let mut preferences = r.viewport().preferences;
    preferences.dark = true;
    r.set_preferences(preferences).unwrap();
    moved(&mut r, "the scheme");
    preferences.contrast = exact_runner::Contrast::More;
    r.set_preferences(preferences).unwrap();
    moved(&mut r, "the contrast");
    r.set_root_font_size(20.0).unwrap();
    moved(&mut r, "the root font size");
    let b = plan("inputs-b", &DESIGN.replace("#cccccc", "#bbbbbb"));
    let batch =
        design::batch(&design::shape(&a), &design::tokens(&a), &design::tokens(&b)).unwrap();
    r.apply_live(&batch).unwrap();
    moved(&mut r, "a live batch");
    let c = plan(
        "inputs-c",
        &DESIGN
            .replace("#cccccc", "#bbbbbb")
            .replace("#101010", "#202020"),
    );
    let roles =
        design::batch(&design::shape(&b), &design::tokens(&b), &design::tokens(&c)).unwrap();
    assert!(r.apply_live(&roles).unwrap().roles);
    moved(&mut r, "a role override's edit");
    r.set_token("ink", "#123456", &[]).unwrap();
    moved(&mut r, "a token's edit");
    let accent = exact_kernel::style::roles::role("AccentColor").unwrap();
    exact_kernel::style::roles::set_reported([(
        ColorValue::Role(accent),
        false,
        exact_kernel::Color::parse("#ff9500").unwrap(),
    )]);
    moved(&mut r, "a platform colour reported");
    r.set_tokens(&[]).unwrap();
    r.set_token("ink", "#123456", &[]).unwrap();
    assert_eq!(r.tamber().inputs(place), last, "nothing moved");
    // A batch that moves nothing a binding reads (a mode it has already).
    r.set_modes(&[("dark-style", "dim")]).unwrap();
    assert_eq!(
        r.tamber().inputs(place),
        last,
        "nothing a binding reads moved"
    );
    assert_eq!(
        r.tamber().color("CanvasText", place),
        Some(parsed("#202020"))
    );
}

/// A pin above a place that moves (bound to state) moves its tuple, its
/// inputs and what it resolves.
#[test]
fn a_pin_above_moves_the_places_inputs() {
    let dir = std::env::temp_dir().join(format!("exact2-binding-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state dim = false\n  action flip\n    dim = not dim\n  view\n    column\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column -exact-mode=(dim ? \"dark-style=dim\" : \"\") color=\"ink\"\n        text \"b\" testId=\"inner\"\n").unwrap();
    let mut r = booted(contract::compile_path(&dir.join("app.contract")).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    let inner = node(&r, "inner");
    let before = r.tamber().inputs(inner);
    let black = r.tamber().color("ink", inner);
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_ne!(r.tamber().inputs(inner), before);
    assert_ne!(r.tamber().color("ink", inner), black);
    assert_eq!(
        r.tamber().color("ink", Place::Root),
        black,
        "the window's stays"
    );
}

/// Two sessions with different designs answer differently, each its own.
#[test]
fn two_sessions_answer_from_their_own_designs() {
    let mut first = booted(plan("first", DESIGN));
    let mut second = booted(plan(
        "second",
        &DESIGN
            .replace("#101010", "#202020")
            .replace("#eeeeee", "#dddddd"),
    ));
    for _ in 0..2 {
        assert_eq!(
            first.tamber().color("CanvasText", Place::Root),
            Some(parsed("#101010"))
        );
        assert_eq!(
            second.tamber().color("CanvasText", Place::Root),
            Some(parsed("#202020"))
        );
        assert_eq!(
            second.tamber().color("ink", Place::Root),
            Some(parsed("light-dark(#111111, #dddddd)"))
        );
        assert_eq!(
            first.tamber().color("ink", Place::Root),
            Some(parsed("light-dark(#111111, #eeeeee)"))
        );
    }
}

/// A `color-scheme` above a place is its scheme: a boundary that turns dark
/// moves the place's inputs, though the window's scheme did not move.
#[test]
fn a_scheme_above_moves_the_places_inputs() {
    let dir = std::env::temp_dir().join(format!("exact2-binding-scheme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state dark = false\n  action flip\n    dark = not dark\n  view\n    column\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      column color-scheme=(dark ? \"dark\" : \"light\")\n        text \"b\" testId=\"inner\"\n").unwrap();
    let mut r = booted(contract::compile_path(&dir.join("app.contract")).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    let inner = node(&r, "inner");
    let before = r.tamber().inputs(inner);
    assert!(!r.tamber().dark(inner));
    let flip = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id
    };
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert!(r.tamber().dark(inner));
    assert!(!r.tamber().dark(Place::Root), "the window's stays light");
    assert_ne!(r.tamber().inputs(inner), before);
}

fn plan_of(name: &str, design: &str, view: &str) -> Plan {
    let dir = std::env::temp_dir().join(format!("exact2-binding-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A native module the views show is one the app declares.
    if view.contains("map-view") {
        std::fs::write(
            dir.join("app.json"),
            r#"{"name": "T", "app": {"id": "com.exact.t", "name": "T"}, "modules": ["map-view"]}"#,
        )
        .unwrap();
    }
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    plan
}

fn names(plan: &Plan) -> Vec<String> {
    plan.theme
        .iter()
        .map(|t| plan.str(t.name).to_string())
        .collect()
}

const KEPT: &str =
    "tamber t\n\ntheme\n  brand = #c8102e\n  badge.ink = #123456\n  CanvasText = #101010\n";

/// LLP 1121 D7's retention for an app with generated bindings: one that
/// reads a data module, or shows a native module, keeps every token (a
/// binding may name one no view does), and the binding core resolves it; a
/// plain app keeps what its views name.
#[test]
fn an_app_that_can_name_tokens_outside_its_views_keeps_them_all() {
    let view = "component App\n  view\n    column\n      text \"a\" color=\"brand\"\n";
    let plain = plan_of("plain", KEPT, view);
    assert_eq!(names(&plain), ["brand", "CanvasText"], "a plain app prunes");
    let data = plan_of(
        "data",
        KEPT,
        "component App\n  resource badge = badge() as shape string\n  view\n    column\n      text badge color=\"brand\"\n",
    );
    assert_eq!(names(&data), ["brand", "badge.ink", "CanvasText"]);
    let native = plan_of(
        "native",
        KEPT,
        "component App\n  view\n    column\n      map-view\n      text \"a\" color=\"brand\"\n",
    );
    assert_eq!(names(&native), ["brand", "badge.ink", "CanvasText"]);
    let mut r = Runner::boot(
        contract::bake(native, NoData).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(
        r.tamber().color("badge.ink", Place::Root),
        Some(parsed("#123456"))
    );
}

/// A design's `exports` bounds what such an app keeps for bindings: the
/// exported tokens, what the views name, and the role overrides.
#[test]
fn exports_bound_what_the_bindings_may_name() {
    let design = "tamber t\n\nexports\n  badge.ink\n\ntheme\n  brand = #c8102e\n  badge.ink = #123456\n  other = #654321\n  CanvasText = #101010\n";
    let native = plan_of(
        "exports",
        design,
        "component App\n  view\n    column\n      map-view\n      text \"a\" color=\"brand\"\n",
    );
    assert_eq!(names(&native), ["brand", "badge.ink", "CanvasText"]);
}

/// What counts as code that can name a token outside the views (D7's one
/// test): an `app.ts` or hatches beside the app do; a source the runner
/// answers (`exactViewport()`) does not.
#[test]
fn the_retention_test_reads_what_is_beside_the_app() {
    let view = "component App\n  view\n    column\n      text \"a\" color=\"brand\"\n";
    let with = |name: &str, file: &str, text: &str| {
        let dir =
            std::env::temp_dir().join(format!("exact2-binding-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.tamber"), KEPT).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        std::fs::write(dir.join(file), text).unwrap();
        let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        names(&plan)
    };
    let all = ["brand", "badge.ink", "CanvasText"];
    assert_eq!(
        with("ts", "app.ts", "export const answer = () => null;\n"),
        all
    );
    assert_eq!(
        with(
            "hatch",
            "app.json",
            r#"{"name": "T", "app": {"id": "com.exact.t", "name": "T"}, "hatches": {"badge": ["ios"]}}"#
        ),
        all
    );
    let runner = plan_of(
        "runner-source",
        KEPT,
        "shape Viewport\n  pointer: string\n\ncomponent App\n  resource size = exactViewport() as shape Viewport\n  view\n    column\n      text \"a\" color=\"brand\"\n",
    );
    assert_eq!(
        names(&runner),
        ["brand", "CanvasText"],
        "the runner's source is no data module"
    );
}

/// The `ColorToken` union is the bindable set: the design's `exports`, when
/// it lists them, though a view names more.
#[test]
fn the_token_union_follows_exports() {
    let design =
        "tamber t\n\nexports\n  badge.ink\n\ntheme\n  brand = #c8102e\n  badge.ink = #123456\n";
    let dir = std::env::temp_dir().join(format!("exact2-binding-union-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.ts"), "export const answer = () => null;\n").unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column\n      text \"a\" color=\"brand\"\n",
    )
    .unwrap();
    let contract = dir.join("app.contract");
    let plan = contract::compile_path(&contract).unwrap();
    let exports = contract::design_exports(&contract);
    let types = contract::typescript_exporting(&plan, exports.as_deref()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        types.contains(r#"export type ColorToken = "badge.ink";"#),
        "{types}"
    );
}
