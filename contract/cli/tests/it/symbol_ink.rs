//! A symbol's authored `-exact-tint-color` is no paint of it (theme2 2026-10-23, after `lower-symbol-tint`): a symbol
//! is drawn in its `color` on every host (LLP 1081 stage 2), so lowering answers the tint with `currentcolor`, each
//! state's row too, which Apple already drew and the web now draws. A native button's face keeps its tint (LLP 1115
//! D4), and a raster's tint is still its silhouette's paint.

use exact_kernel::{Color, ColorValue, Kernel, StyleId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn a_symbols_tint_is_its_colour_and_a_native_face_or_a_raster_keeps_its_own() {
    let src = "component A\n  action go\n  view\n    column color=\"#0000ff\"\n      image \"symbol:heart\" testId=\"tinted\" -exact-tint-color=\"#ff0000\" alt=\"Heart\"\n      image \"symbol:heart\" testId=\"stated\" -exact-tint-color=\"#ff0000\" alt=\"Heart\"\n        :disabled\n          -exact-tint-color=\"#00ff00\"\n      image \"https://example.com/a.png\" testId=\"raster\" -exact-tint-color=\"#ff0000\" alt=\"A\"\n      button press=go testId=\"lock\"\n        image \"symbol:lock\" testId=\"face\" -exact-tint-color=\"#ff0000\"\n        text \"Lock\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let tint = |id: &str| {
        let n = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
        (n.style.mask.has(StyleId::TintColor), n.style.tint_color)
    };
    let red: ColorValue = Color::parse_hex("#ff0000").unwrap().into();
    // `currentcolor`: the keyword, which each host paints in `color` (styles_rows.rs).
    assert_eq!(tint("tinted"), (true, None));
    assert_eq!(tint("stated"), (true, None));
    assert_eq!(tint("raster"), (true, Some(red)));
    assert_eq!(tint("face"), (true, Some(red)));
}

#[test]
fn a_states_tint_is_answered_in_and_out_of_the_state_and_a_computed_source_by_what_it_is() {
    let src = "component A\n  state off = false\n  state pic = \"symbol:heart\"\n  action flip\n    off = ! off\n  action raster\n    pic = \"https://example.com/a.png\"\n  view\n    column color=\"#0000ff\"\n      button \"Flip\" press=flip testId=\"flip\"\n      button \"Raster\" press=raster testId=\"to-raster\"\n      image \"symbol:heart\" testId=\"state-only\" color=\"#ff0000\" disabled=off alt=\"Heart\"\n        :disabled\n          -exact-tint-color=\"#00ff00\"\n      image pic testId=\"computed\" -exact-tint-color=\"#ff0000\" alt=\"Pic\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let tint = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .tint_color
    };
    let press = |r: &mut Runner<NoData>, id: &str| {
        let k = r.kernel();
        let view = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
        r.dispatch(view, exact_runner::Event::Press).unwrap();
    };
    let red: ColorValue = Color::parse_hex("#ff0000").unwrap().into();
    // Out of the state and in it, the symbol is its `color` (`currentcolor`), never the state's green.
    assert_eq!(tint(&r, "state-only"), None);
    press(&mut r, "flip");
    assert_eq!(tint(&r, "state-only"), None);
    // A computed source: a symbol's tint answered, a raster's kept.
    assert_eq!(tint(&r, "computed"), None);
    press(&mut r, "to-raster");
    assert_eq!(tint(&r, "computed"), Some(red));
}

#[test]
fn a_grouped_rows_symbol_with_a_colour_and_only_a_states_tint_is_its_colour_not_the_sheets() {
    // (Astra's round 1: a state-only tint must not drop the base answer, or the sheet's accent tint shows.)
    let src = "component A\n  state off = false\n  action go\n  view\n    list appearance=\"auto\" listStyle=\"inset-grouped\"\n      section\n        button press=go testId=\"row\"\n          image \"symbol:star\" testId=\"lead\" color=\"#ff0000\" disabled=off\n            :disabled\n              -exact-tint-color=\"#00ff00\"\n          text \"Row\"\n        button press=go testId=\"plain-row\"\n          image \"symbol:star\" testId=\"plain-lead\"\n          text \"Plain\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let tint = |id: &str| {
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .tint_color
    };
    // The plain row's symbol takes the sheet's tint; the coloured one its `color`.
    assert!(tint("plain-lead").is_some());
    assert_eq!(tint("lead"), None);
}

#[test]
fn a_computed_sources_state_tint_a_partial_template_and_an_apps_own_starts_with() {
    let src = "component A\n  state off = false\n  state pic = \"symbol:heart\"\n  state suffix = \"bol:heart\"\n  action flip\n    off = ! off\n  action raster\n    pic = \"https://example.com/a.png\"\n  view\n    column color=\"#0000ff\"\n      button \"Flip\" press=flip testId=\"flip\"\n      button \"Raster\" press=raster testId=\"to-raster\"\n      image pic testId=\"computed\" color=\"#0000ff\" disabled=off alt=\"Pic\"\n        :disabled\n          -exact-tint-color=\"#00ff00\"\n      image `sym${suffix}` testId=\"partial\" -exact-tint-color=\"#ff0000\" alt=\"Part\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let tint = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .tint_color
    };
    let green: ColorValue = Color::parse_hex("#00ff00").unwrap().into();
    assert_ne!(tint(&r, "computed"), Some(green));
    assert_eq!(tint(&r, "partial"), None);
    let k = r.kernel();
    let flip = k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id;
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    // Disabled, the state's green is answered too: the symbol is its `color`.
    assert_ne!(tint(&r, "computed"), Some(green));
    // Still disabled, the source a raster now: its state's green is its paint again.
    let k = r.kernel();
    let raster = k.node_by_key(k.find_by_test_id("to-raster")[0]).unwrap().id;
    r.dispatch(raster, exact_runner::Event::Press).unwrap();
    assert_eq!(tint(&r, "computed"), Some(green));
    // An app's own `startsWith` is the app's: no answer is made through it, so nothing traps.
    let own = "fn startsWith(s: string, p: string): number = 1\ncomponent A\n  state pic = \"symbol:heart\"\n  view\n    column\n      image pic testId=\"computed\" -exact-tint-color=\"#ff0000\" alt=\"Pic\"\n";
    let plan = contract::bake(contract::compile(own).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        r.journal()
            .all(|l| !l.contains("poisoned") && !l.contains("TypeMismatch")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
}

#[test]
fn a_shape_named_starts_with_makes_no_answer_through_it() {
    // (Astra's re-check: a `shape startsWith` is resolved before the roster function, as an app's `fn` is.)
    let src = "shape startsWith\n  s: string\ncomponent A\n  state off = true\n  state pic = \"symbol:heart\"\n  view\n    column\n      image pic testId=\"computed\" disabled=off alt=\"Pic\"\n        :disabled\n          -exact-tint-color=\"#00ff00\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        r.journal()
            .all(|l| !l.contains("poisoned") && !l.contains("TypeMismatch")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
}
