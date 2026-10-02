//! Drag timelines in Contract (LLP 1057.003 D1, D4): `drag-timeline`,
//! `animation-timeline`, `animation-range` and `timeline-scope` reach the
//! kernel's rows, the siblings' name resolves through the scope, and a
//! timeline drives paint rows only (Q1): a bound animation whose keyframes
//! animate layout or geometry is refused when the app compiles.

use exact_kernel::timeline::{
    AnimationRange, AnimationTimeline, Axis, DragTimeline, TimelineScope,
};
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use exact_runner::{agent, DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = "keyframes fade\n  from opacity=1\n  to opacity=0 background-color=\"#000\"\ncomponent App\n  state zoom = 1\n  action zoomIn\n    zoom = 2\n  view\n    column testId=\"clip\" timeline-scope=\"--dismiss\"\n      box testId=\"backdrop\" animation=(zoom == 1 ? \"fade 1s linear both\" : \"none\") animation-timeline=\"--dismiss\" animation-range=\"0 300px\"\n      box testId=\"photo\" drag-timeline=\"--dismiss\" translate=\"0px 0px\"\n      button testId=\"zoom\" press=zoomIn\n        text \"2×\"\n";

#[test]
fn the_rows_reach_the_kernel_and_a_computed_animation_keeps_its_binding() {
    let plan = contract::compile(APP).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let style = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .clone()
    };
    assert_eq!(
        style(&r, "photo").drag_timeline,
        DragTimeline {
            name: Some("--dismiss".into()),
            axis: Axis::Y
        }
    );
    let backdrop = style(&r, "backdrop");
    assert_eq!(
        backdrop.animation_timeline,
        AnimationTimeline(Some("--dismiss".into()))
    );
    assert_eq!(backdrop.animation_range, AnimationRange(Some([0.0, 300.0])));
    assert_eq!(backdrop.animation.0[0].name, "fade");
    assert_eq!(
        style(&r, "clip").timeline_scope,
        TimelineScope::Names("--dismiss".into())
    );
    // The backdrop and the photo are siblings: the clip's scope finds it.
    let k = r.kernel();
    assert_eq!(
        k.timeline_of(k.find_by_test_id("backdrop")[0]),
        Some(exact_motion::NamedTimeline::Source(
            exact_kernel::motion_node(k.find_by_test_id("photo")[0])
        ))
    );
    // The agent's node shows each row as its CSS text.
    let shown = |test_id: &str, row: &str| {
        let k = r.kernel();
        let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
        let node: serde_json::Value = serde_json::from_str(&agent::node(&r, id)).unwrap();
        node["style"][row]["value"].clone()
    };
    assert_eq!(shown("photo", "drag_timeline"), "--dismiss y");
    assert_eq!(shown("backdrop", "animation_timeline"), "--dismiss");
    assert_eq!(shown("backdrop", "animation_range"), "0px 300px");
    assert_eq!(shown("clip", "timeline_scope"), "--dismiss");
    let k = r.kernel();
    let zoom = k.node_by_key(k.find_by_test_id("zoom")[0]).unwrap().id;
    r.dispatch(zoom, Event::Press).unwrap();
    let backdrop = style(&r, "backdrop");
    assert!(backdrop.animation.0.is_empty(), "zoomed, nothing follows");
    assert_eq!(
        backdrop.animation_timeline,
        AnimationTimeline(Some("--dismiss".into()))
    );
}

/// Keyframes refuse `height` already (`lower-keyframe-property`), and an SVG
/// element takes no timeline in phase 1, so what reaches this rule is a box
/// binding geometry keyframes.
#[test]
fn a_timeline_drives_paint_rows_only() {
    let app = |keyframes: &str, attrs: &str| {
        format!("keyframes k\n  to {keyframes}\ncomponent App\n  state on = true\n  view\n    column\n      box {attrs}\n")
    };
    let bound = "animation-timeline=\"--pull\" animation-range=\"0px 100px\"";
    for keyframes in ["r=4", "cx=3", "stroke-dashoffset=2"] {
        for animation in [
            "animation=\"k 1s both\"",
            "animation=(on ? \"none\" : \"k 1s both\")",
        ] {
            let error =
                contract::compile(&app(keyframes, &format!("{animation} {bound}"))).unwrap_err();
            assert_eq!(error.id, "lower-timeline-row", "{keyframes}: {error}");
            assert!(error.message.contains("paint rows only"), "{error}");
            assert!(error.message.contains("LLP 1057.003"), "{error}");
        }
        // Unbound, or bound to the clock, the same keyframes play as ever.
        contract::compile(&app(keyframes, "animation=\"k 1s both\"")).unwrap();
        contract::compile(&app(
            keyframes,
            "animation=\"k 1s both\" animation-timeline=\"auto\"",
        ))
        .unwrap();
    }
    for keyframes in [
        "opacity=0.5",
        "translate=\"0px 4px\" scale=2 rotate=\"30deg\"",
        "color=\"#f00\" background-color=\"#00f\" box-shadow=\"0 2px 4px #0003\"",
    ] {
        contract::compile(&app(keyframes, &format!("animation=\"k 1s both\" {bound}"))).unwrap();
    }
    // A timeline spans the animation's whole length: an endless one has none.
    let error = contract::compile(&app(
        "opacity=0",
        &format!("animation=\"k 1s infinite both\" {bound}"),
    ))
    .unwrap_err();
    assert_eq!(error.id, "lower-timeline-endless", "{error}");
    // An SVG element takes no timeline yet.
    let error = contract::compile("keyframes k\n  to opacity=0\ncomponent App\n  view\n    svg viewBox=\"0 0 10 10\" width=10 height=10\n      circle r=2 animation=\"k 1s both\" animation-timeline=\"--pull\"\n").unwrap_err();
    assert_eq!(error.id, "lower-svg-attr", "{error}");
}

#[test]
fn a_row_outside_its_grammar_is_refused_by_name() {
    for (attr, says) in [
        ("drag-timeline=\"dismiss\"", "drag-timeline"),
        ("drag-timeline=\"--dismiss z\"", "drag-timeline"),
        ("animation-timeline=\"scroll()\"", "animation-timeline"),
        ("animation-range=\"0% 100%\"", "animation-range"),
        ("timeline-scope=\"--a --b\"", "timeline-scope"),
        ("timeline-scope=\"dismiss\"", "timeline-scope"),
        ("timelineScope=\"--a\"", "timeline-scope"),
    ] {
        let error = contract::compile(&format!(
            "component App\n  view\n    column\n      box {attr}\n"
        ))
        .unwrap_err();
        assert!(error.message.contains(says), "{attr}: {error}");
    }
}
