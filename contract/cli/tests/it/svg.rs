//! LLP 1055: inline SVG and CSS animations, compiled and booted — the rows
//! and props on the kernel, the keyframes resolved onto the `animation` row,
//! the longhands composed, and every refusal by its id.

use exact_kernel::{Kernel, NodeType, PropId, StyleId};
use exact_motion::{Direction, Easing, FillMode, Property};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
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

fn refused(src: &str) -> String {
    match contract::compile(src) {
        Ok(_) => panic!("compiled: {src}"),
        Err(e) => {
            eprintln!("{e}");
            e.to_string()
        }
    }
}

const SPARK: &str = "\
keyframes draw
  from stroke-dashoffset=1
  to stroke-dashoffset=0
keyframes breathe
  from r=3 opacity=0.5
  to r=9 opacity=0
  50% animation-timing-function=\"linear\" r=5

component A
  view
    svg testId=\"chart\" width=96 height=32 viewBox=\"0 0 96 32\" overflow=\"visible\"
      polyline testId=\"line\" points=\"0,32 48,0 96,16\" fill=\"none\" stroke=\"#16a34a\" stroke-width=1.5 stroke-linejoin=\"round\" pathLength=1 stroke-dasharray=\"1\" animation=\"draw 600ms ease-out both\"
      g stroke=\"currentcolor\"
        circle testId=\"ring\" cx=96 cy=16 r=3 fill=\"none\" opacity=0 animation-name=\"breathe\" animation-duration=\"1200ms\" animation-timing-function=\"ease-out\" animation-delay=\"600ms\" animation-iteration-count=\"infinite\"
";

#[test]
fn the_sparkline_compiles_to_nodes_rows_and_resolved_animations() {
    let r = boot(SPARK);
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let chart = node("chart");
    assert_eq!(chart.node_type, NodeType::Svg);
    assert_eq!(chart.props.str(PropId::ViewBox), Some("0 0 96 32"));
    assert_eq!(
        chart.style.overflow_x,
        exact_kernel::Overflow::Visible,
        "authored over the tag's hidden"
    );
    let line = node("line");
    assert_eq!(line.node_type, NodeType::SvgPolyline);
    assert_eq!(
        line.props
            .get(PropId::PathLength)
            .and_then(|v| v.as_float()),
        Some(1.0)
    );
    assert_eq!(line.style.stroke_width, 1.5);
    assert_eq!(line.style.fill, exact_kernel::svg::Paint::None);
    let draw = &line.style.animation.0[0];
    assert_eq!(
        (draw.name.as_str(), draw.duration, draw.fill),
        ("draw", 0.6, FillMode::Both)
    );
    assert_eq!(draw.keyframes.0.len(), 2, "the rule rides with the row");
    let ring = node("ring");
    assert_eq!(ring.style.r, 3.0);
    let breathe = &ring.style.animation.0[0];
    assert!(breathe.iterations.is_infinite());
    assert_eq!(
        (breathe.delay, breathe.easing.clone(), breathe.direction),
        (0.6, Easing::EaseOut, Direction::Normal)
    );
    assert_eq!(breathe.keyframes.0[1].easing, Some(Easing::Linear));
    assert_eq!(
        breathe.keyframes.properties(),
        vec![Property::Opacity, Property::R]
    );
    // The ring's stroke is inherited from its `g`.
    let computed = ring.computed_style(exact_kernel::StyleMask::of(StyleId::Stroke));
    assert_eq!(computed.stroke, exact_kernel::svg::Paint::CurrentColor);
}

#[test]
fn a_computed_part_composes_one_animation() {
    let r = boot(
        "keyframes spin\n  to opacity=0\ncomponent A\n  state frozen = true\n  view\n    column testId=\"b\" animation=\"spin 1s linear infinite\" animation-play-state=(frozen ? \"paused\" : \"running\")\n",
    );
    let k = r.kernel();
    let s = &k.node_by_key(k.find_by_test_id("b")[0]).unwrap().style;
    assert!(s.animation.0[0].paused);
    assert!(s.animation.0[0].iterations.is_infinite());
}

#[test]
fn refusals_are_named() {
    let svg = |body: &str| {
        format!("component A\n  state n = 0\n  action go writes n\n    n = 1\n  view\n    svg width=10 height=10\n      {body}\n")
    };
    assert!(refused("component A\n  view\n    circle r=3\n").contains("lower-svg-content"));
    assert!(refused(&svg("text \"hi\"")).contains("lower-svg-content"));
    assert!(refused(&svg("ellipse")).contains("not in exact2's SVG subset"));
    assert!(refused(&svg("animate")).contains("SMIL is refused"));
    assert!(
        refused(&svg("circle r=3 press=go")).contains("lower-svg-attr"),
        "shapes handle no events"
    );
    assert!(refused(&svg("circle r=3 padding=4")).contains("lower-svg-attr"));
    assert!(refused(&svg("rect points=\"0,0\"")).contains("lower-attr-tag"));
    assert!(refused("component A\n  view\n    column points=\"0,0\"\n").contains("lower-attr-tag"));
    assert!(refused(&svg("circle r=3 fill=\"url(#g)\"")).contains("paint servers"));
    assert!(refused(&svg("circle r=3 animation=\"nope 1s\"")).contains("lower-animation-name"));
    assert!(
        refused("keyframes k\n  to color=\"#fff\"\ncomponent A\n  view\n    column\n")
            .contains("lower-keyframe-property")
    );
    assert!(refused("keyframes k\n  to scale=2\ncomponent A\n  view\n    svg\n      circle r=1 animation=\"k 1s\"\n").contains("lower-animation-target"));
    assert!(
        refused("keyframes k\n  120% opacity=1\ncomponent A\n  view\n    column\n")
            .contains("syntax-keyframe-selector")
    );
}
