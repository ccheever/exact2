//! LLP 1061: `press-scale` is a style row the hosts own; the user's motion
//! and transparency preferences are `exactViewport()` fields, re-answered in
//! one commit, so an app collapses its own motion.

use exact_kernel::{Kernel, StyleId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Preferences, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("host fact reached data: {source}")
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn style<'a>(r: &'a Runner<NoData>, test_id: &str) -> &'a exact_kernel::StyleProps {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().style
}

fn feel() -> Runner<NoData> {
    boot(include_str!("../../../corpus/motion-feel.contract"))
}

#[test]
fn press_scale_is_a_row_set_only_where_authored() {
    let r = feel();
    let record = style(&r, "record");
    assert!(record.mask.has(StyleId::PressScale));
    assert_eq!(record.press_scale, 0.97);
    assert_eq!(style(&r, "card").press_scale, 0.994);
    // A bare node has no feedback: the row is unset, and its default is none.
    let root = style(&r, "root");
    assert!(!root.mask.has(StyleId::PressScale));
    assert_eq!(root.press_scale, 1.0);
}

#[test]
fn a_press_scale_from_an_expression_follows_its_state() {
    let mut r = boot(concat!(
        "component App\n  state firm = false\n",
        "  action soften writes firm\n    firm = not firm\n",
        "  view\n    button press=soften press-scale=(firm ? 1 : 0.97) testId=\"b\"\n      text \"b\"\n",
    ));
    assert_eq!(style(&r, "b").press_scale, 0.97);
    let k = r.kernel();
    let id = k.node_by_key(k.find_by_test_id("b")[0]).unwrap().id;
    r.dispatch(id, exact_runner::Event::Press).unwrap();
    assert_eq!(style(&r, "b").press_scale, 1.0);
}

#[test]
fn reduced_motion_is_read_by_the_app_and_reanswered_in_one_commit() {
    let mut r = feel();
    let still = |r: &Runner<NoData>| style(r, "root").animation.0.is_empty();
    assert!(!still(&r), "no preference: the entry rises");
    assert!(style(&r, "record").mask.has(StyleId::Transition));
    let receipt = r
        .set_preferences(Preferences {
            reduced_motion: true,
            reduced_transparency: false,
        })
        .unwrap()
        .expect("a reader re-answers");
    assert_eq!(receipt.epoch, 2);
    assert!(still(&r), "reduced: the app chose `none`");
    assert!(style(&r, "record").transition.0.is_empty());
    // A size change keeps the preference; the same preference is no commit.
    r.set_viewport(1024.0, 768.0).unwrap();
    assert!(r.viewport().preferences.reduced_motion);
    assert!(r
        .set_preferences(Preferences {
            reduced_motion: true,
            reduced_transparency: false,
        })
        .unwrap()
        .is_none());
    r.set_preferences(Preferences {
        reduced_motion: true,
        reduced_transparency: true,
    })
    .unwrap()
    .unwrap();
    assert_eq!(style(&r, "card").opacity, 1.0);
}

#[test]
fn a_preference_is_a_bool_and_the_boot_fact_is_the_first_answer() {
    let src = "shape M\n  prefersReducedMotion: bool\ncomponent A\n  resource m = exactViewport() as shape M\n  view\n    text (m.prefersReducedMotion ? \"still\" : \"moving\") testId=\"t\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut viewport = exact_runner::Viewport::sized(390.0, 844.0);
    viewport.preferences = Preferences::from_bits(1);
    let r = Runner::boot(plan, NoData, Kernel::with_monospace(), viewport, "/").unwrap();
    let state: serde_json::Value = serde_json::from_str(&exact_runner::agent::state(&r)).unwrap();
    assert_eq!(state["resources"]["m"]["prefersReducedMotion"], true);
    let wrong = "shape M\n  prefersReducedMotion: number\ncomponent A\n  resource m = exactViewport() as shape M\n  view\n    text `${m.prefersReducedMotion}`\n";
    assert!(contract::bake(contract::compile(wrong).unwrap(), NoData).is_err());
}
