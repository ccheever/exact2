//! LLP 1065: `path` — its tag, SVG's attributes, and a stroke drawn by
//! `transition` or `keyframes` — proven on the kernel after boot and on what
//! the compiler refuses.

use exact_kernel::{ColorValue, Kernel, NodeType, Paint, PropId, StrokeLinecap, StyleId};
use exact_motion::{Property, TransitionProperty, Value};
use exact_plan::{Plan, Value as PlanValue};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[PlanValue]) -> Result<PlanValue, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/path.contract");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_path_reaches_the_kernel_with_its_data_paint_and_motion() {
    let plan = contract::compile(&fixture()).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let mark = k.node_by_key(k.find_by_test_id("mark")[0]).unwrap();
    assert_eq!(mark.node_type, NodeType::Path);
    assert!(mark
        .props
        .str(PropId::PathData)
        .unwrap()
        .starts_with("M12 70c10-40"));
    assert_eq!(mark.props.str(PropId::ViewBox), Some("0 0 200 100"));
    // Painting through `class`, as SVG's properties.
    let s = mark.style;
    assert_eq!(s.fill, Paint::None, "fill=\"none\"");
    assert!(matches!(s.stroke, Paint::Color(ColorValue::LightDark(..))));
    assert_eq!(s.stroke_width, 9.0);
    assert_eq!(s.stroke_linecap, StrokeLinecap::Round);
    assert_eq!(s.stroke_end, 0.0);
    assert_eq!(
        s.transition.0[0].property,
        TransitionProperty::Property(Property::StrokeEnd)
    );
    // The keyframes resolve to the stroke row, as the four compositor rows do.
    let stamp = k.node_by_key(k.find_by_test_id("stamp")[0]).unwrap();
    let draw = &stamp.style.animation.0[0];
    assert_eq!(
        draw.keyframes.blocks[0].values,
        [(Property::StrokeEnd, Value::scalar(0.0))]
    );
    assert_eq!(stamp.props.str(PropId::AccessibilityLabel), Some("Signed"));
    let sign = k.node_by_key(k.find_by_test_id("sign")[0]).unwrap().id;
    r.dispatch(sign, Event::Press).unwrap();
    let k = r.kernel();
    let mark = k.node_by_key(k.find_by_test_id("mark")[0]).unwrap();
    assert_eq!(mark.style.stroke_end, 1.0);
    assert!(mark.style.mask.has(StyleId::StrokeEnd));
}

#[test]
fn what_is_not_a_drawable_path_is_refused_at_compile_time() {
    let app = |decls: &str, node: &str| {
        format!("{decls}component App\n  state on = true\n  view\n    column\n      {node}\n")
    };
    for (source, id, says) in [
        (
            app("", "text \"a\" d=\"M0 0\""),
            "lower-attr-tag",
            "`d` belongs to `path`, not `text`",
        ),
        (
            app("", "column stroke-end=0"),
            "lower-attr-tag",
            "`stroke-end` belongs to `path`",
        ),
        (
            app("", "path d=\"M0 0 L10\""),
            "lower-attr-value",
            "`d` is not SVG path data from byte 5",
        ),
        (
            app("", "path d=\"L0 0\""),
            "lower-attr-value",
            "from byte 0",
        ),
        (
            app("", "path d=\"M0 0\" viewBox=\"0 0 0 10\""),
            "lower-attr-value",
            "positive width and height",
        ),
        (
            app("", "path stroke-linecap=\"rounded\""),
            "lower-attr-value",
            "stroke-linecap",
        ),
        (
            app("", "path d=\"M0 0\"\n        text \"a\""),
            "lower-leaf-children",
            "`path` cannot hold children",
        ),
        (
            app("keyframes k\n  to\n    stroke-width=3\n", "path"),
            "lower-keyframes",
            "`stroke-width` cannot be in a keyframe",
        ),
        (
            app("keyframes k\n  to\n    fill=\"none\"\n", "path"),
            "lower-keyframes",
            "a keyframe's paint is a colour",
        ),
        (
            app("", "path d=\"M0 0\" preserveAspectRatio=\"xMidYMid fit\""),
            "lower-attr-value",
            "`preserveAspectRatio=\"xMidYMid fit\"`",
        ),
        (
            app("", "column preserveAspectRatio=\"none\""),
            "lower-attr-tag",
            "`preserveAspectRatio` belongs to `path`",
        ),
        (
            app("", "path stroke-miterlimit=0.5"),
            "lower-attr-value",
            "below 1",
        ),
        (
            app("", "path stroke-dasharray=\"4 -1\""),
            "lower-attr-value",
            "nonnegative numbers",
        ),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, id, "{source}\n{error}");
        assert!(error.message.contains(says), "{error}");
    }
    // Data from state is the host's to draw up to its first error, as SVG's.
    contract::compile(&app(
        "",
        "path d=(on ? \"M0 0 H10\" : \"\") stroke=\"#000\" stroke-start=(on ? 0.5 : 0)",
    ))
    .unwrap();
    // The painting properties inherit, so any box may set them.
    contract::compile(&app(
        "",
        "column stroke=\"#000\" stroke-width=2 fill=\"none\" fill-rule=\"evenodd\" stroke-dasharray=\"4, 2\" stroke-dashoffset=1 stroke-miterlimit=8",
    ))
    .unwrap();
}

/// SVG's remaining painting vocabulary reaches the kernel, and `fill` and
/// `stroke` keyframe as colours, `light-dark()` pairs included (LLP 1065).
#[test]
fn the_rest_of_svg_painting_and_paint_keyframes() {
    let source = "keyframes glow\n  from\n    fill=\"#ff0000\"\n    stroke=\"light-dark(#000000, #ffffff)\"\n  to\n    fill=\"#0000ff\"\ncomponent App\n  view\n    path testId=\"p\" d=\"M0 0 H10\" viewBox=\"0 0 10 10\" preserveAspectRatio=\"xMinYMax slice\" fill=\"currentColor\" fill-rule=\"evenodd\" stroke=\"#000\" stroke-miterlimit=10 stroke-dasharray=\"4 2\" stroke-dashoffset=1 vector-effect=\"non-scaling-stroke\" animation=\"glow 1s both\"\n";
    let plan = contract::compile(source).unwrap();
    let r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let p = k.node_by_key(k.find_by_test_id("p")[0]).unwrap();
    assert_eq!(
        p.props.str(PropId::PreserveAspectRatio),
        Some("xMinYMax slice")
    );
    let s = p.style;
    assert_eq!(s.fill, Paint::CurrentColor);
    assert_eq!(s.fill_rule, exact_kernel::FillRule::Evenodd);
    assert_eq!(s.stroke_miterlimit, 10.0);
    assert_eq!(s.stroke_dasharray.0, [4.0, 2.0]);
    assert_eq!(s.stroke_dashoffset, 1.0);
    assert_eq!(
        s.vector_effect,
        exact_kernel::VectorEffect::NonScalingStroke
    );
    let glow = &s.animation.0[0].keyframes.blocks;
    assert_eq!(
        glow[0].values,
        [
            (Property::Fill, Value::rgba(1.0, 0.0, 0.0, 1.0)),
            (Property::Stroke, Value::rgba(0.0, 0.0, 0.0, 1.0)),
        ]
    );
    assert_eq!(
        glow[0].dark,
        [(Property::Stroke, Value::rgba(1.0, 1.0, 1.0, 1.0))]
    );
    assert_eq!(
        glow[1].values,
        [(Property::Fill, Value::rgba(0.0, 0.0, 1.0, 1.0))]
    );
}
