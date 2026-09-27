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
    assert_eq!(ring.style.r, exact_kernel::Dimension::Points(3.0));
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
    assert!(refused(&svg("foreignObject")).contains("refused"));
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
        refused("keyframes k\n  to width=3\ncomponent A\n  view\n    column\n")
            .contains("lower-keyframe-property")
    );
    assert!(refused("keyframes k\n  to r=2\ncomponent A\n  view\n    svg\n      rect width=1 animation=\"k 1s\"\n").contains("lower-animation-target"));
    assert!(
        refused("keyframes k\n  120% opacity=1\ncomponent A\n  view\n    column\n")
            .contains("syntax-keyframe-selector")
    );
}

// LLP 1055.000 stage 1: ellipse, nested viewports, geometry rows and
// lengths, transforms in both grammars, `class=` on SVG elements, and SVG
// inside a component.
const STAGE1: &str = "\
style Accent
  fill=\"#ff0000\"
  stroke=\"#000000\"

component A
  state w = 40
  view
    svg testId=\"root\" viewBox=\"0 0 100 50\"
      ellipse testId=\"e\" cx=50 cy=\"50%\" rx=10 class=Accent transform=\"rotate(30 50 25)\" transform-box=\"fill-box\" transform-origin=\"center\"
      rect testId=\"r\" x=\"10%\" y=2 width=w height=\"1in\" rx=2 rotate=15
      g testId=\"g\" transform=\"translate(10px, 5px) scale(2)\" visibility=\"hidden\"
        Tick(at=w)
      svg testId=\"inner\" x=5 y=5 width=\"50%\" height=20 viewBox=\"0 0 10 10\" overflow=\"visible\"
        circle cx=5 cy=5 r=\"50%\"

component Tick
  props
    at: number
  view
    line testId=\"tick\" x1=at x2=at y1=\"0\" y2=\"100%\" vector-effect=\"non-scaling-stroke\"
";

#[test]
fn stage_one_geometry_transforms_and_classes() {
    use exact_kernel::Dimension::{Percent, Points};
    let r = boot(STAGE1);
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let e = node("e");
    assert_eq!(e.node_type, NodeType::SvgEllipse);
    assert_eq!(
        (e.style.cx, e.style.cy, e.style.rx),
        (Points(50.0), Percent(50.0), Points(10.0))
    );
    assert_eq!(e.style.ry, exact_kernel::Dimension::Auto);
    assert_eq!(e.style.fill.css(), "#ff0000ff", "the class's rows apply");
    assert_eq!(
        e.style.transform.css(),
        "translate(50px, 25px) rotate(30deg) translate(-50px, -25px)"
    );
    assert_eq!(e.style.transform_box, exact_kernel::TransformBox::FillBox);
    let rect = node("r");
    assert_eq!((rect.style.x, rect.style.y), (Percent(10.0), Points(2.0)));
    assert_eq!(rect.style.height, Points(96.0), "1in is 96 user units");
    assert_eq!((rect.style.width, rect.style.rotate), (Points(40.0), 15.0));
    // A component's root inside a `g`; a computed number is a length's text.
    let tick = node("tick");
    assert_eq!(tick.node_type, NodeType::SvgLine);
    assert_eq!(tick.props.str(PropId::X1), Some("40"));
    assert_eq!(tick.props.str(PropId::Y2), Some("100%"));
    assert_eq!(
        tick.style.vector_effect,
        exact_kernel::VectorEffect::NonScalingStroke
    );
    assert_eq!(node("g").style.visibility, exact_kernel::Visibility::Hidden);
    let inner = node("inner");
    assert_eq!(inner.node_type, NodeType::SvgViewport);
    assert_eq!(inner.style.width, Percent(50.0));
}

// LLP 1055.000 stage 2: colour keyframes and transitions, transforms on SVG
// elements animated.
#[test]
fn stage_two_colour_and_transform_motion() {
    let r = boot(
        "keyframes flash\n  from color=\"#16a34a\"\n  to color=\"#000000\"\nkeyframes spin\n  to rotate=90\ncomponent A\n  view\n    column\n      text \"$1\" testId=\"p\" animation=\"flash 400ms ease\" transition=\"background-color 200ms\"\n      svg width=10 height=10\n        rect testId=\"r\" width=4 height=4 animation=\"spin 1s linear infinite\"\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let p = node("p");
    let flash = &p.style.animation.0[0];
    assert_eq!(flash.keyframes.properties(), vec![Property::Color]);
    let from = flash.keyframes.0[0].values[0].1;
    assert_eq!(from.to_rgba8(), [0x16, 0xa3, 0x4a, 255]);
    assert!(p.style.transition.0[0]
        .property
        .covers(Property::BackgroundColor));
    let spin = &node("r").style.animation.0[0];
    assert_eq!(spin.keyframes.properties(), vec![Property::Rotate]);
}
