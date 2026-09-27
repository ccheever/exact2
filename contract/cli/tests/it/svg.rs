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
    assert!(refused(&svg("column")).contains("lower-svg-content"));
    assert!(refused(&svg("text \"hi\" rotate=\"10 20\"")).contains("later stage"));
    assert!(refused(&svg("foreignObject")).contains("deferred"));
    assert!(refused(&svg("animate")).contains("SMIL is refused"));
    assert!(
        refused(&svg("defs press=go")).contains("lower-svg-attr"),
        "a definition handles no events"
    );
    assert!(refused(&svg("circle r=3 dblclick=go")).contains("later stage"));
    assert!(refused(&svg("circle r=3 padding=4")).contains("lower-svg-attr"));
    assert!(refused(&svg("rect points=\"0,0\"")).contains("lower-attr-tag"));
    assert!(refused("component A\n  view\n    column points=\"0,0\"\n").contains("lower-attr-tag"));
    assert!(refused(&svg("mask")).contains("later stage"));
    assert!(refused(&svg("hatch")).contains("Chrome does not implement"));
    assert!(refused(&svg("linearGradient\n        rect width=1")).contains("does not hold"));
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

// LLP 1055.000 stage 3: definitions, gradients, use and symbol, paint order.
#[test]
fn stage_three_references_and_servers() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      defs\n        radialGradient id=\"g\" fx=\"30%\" fr=0.1 spreadMethod=\"reflect\"\n          stop testId=\"s\" offset=\"50%\" stop-color=\"#ff0000\" stop-opacity=0.5\n        symbol id=\"i\" viewBox=\"0 0 24 24\"\n          path d=\"M0 0L24 24\"\n      circle testId=\"c\" r=4 fill=\"url(#g) #00ff00\" paint-order=\"stroke\"\n      use testId=\"u\" href=\"#i\" x=2 width=6 height=6\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let c = node("c");
    assert_eq!(c.style.fill.css(), "url(#g) #00ff00ff");
    assert_eq!(c.style.paint_order.css(), "stroke fill markers");
    let s = node("s");
    assert_eq!(s.node_type, NodeType::SvgStop);
    assert_eq!(s.props.str(PropId::Offset), Some("50%"));
    assert_eq!(s.style.stop_opacity, 0.5);
    let u = node("u");
    assert_eq!(u.node_type, NodeType::SvgUse);
    assert_eq!(u.props.str(PropId::Href), Some("#i"));
    assert_eq!(
        k.resolve_id(u.id, "i")
            .and_then(|t| k.node(t))
            .map(|n| n.node_type),
        Some(NodeType::SvgSymbol)
    );
}

// LLP 1055.000 stage 4: clipPath and clip-path on SVG elements.
#[test]
fn stage_four_clipping() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      clipPath id=\"c\" clipPathUnits=\"objectBoundingBox\"\n        circle cx=0.5 cy=0.5 r=0.5 clip-rule=\"evenodd\"\n      rect testId=\"r\" width=4 height=4 clip-path=\"url(#c)\"\n",
    );
    let k = r.kernel();
    let rect = k.node_by_key(k.find_by_test_id("r")[0]).unwrap();
    assert_eq!(rect.style.clip_path.url(), Some("c"));
    assert!(
        refused("component A\n  view\n    column clip-path=\"url(#c)\"\n")
            .contains("clips SVG elements")
    );
    assert!(
        refused("component A\n  view\n    svg\n      clipPath\n        g\n")
            .contains("does not hold")
    );
}

// LLP 1055.000 stage 5: SVG text and its runs.
#[test]
fn stage_five_text() {
    let r = boot(
        "component A\n  view\n    column\n      text \"a box's text\" testId=\"box\"\n      svg width=100 height=40\n        text \"Q1\" testId=\"t\" x=50 y=20 text-anchor=\"middle\" dominant-baseline=\"central\" font-size=12\n        text testId=\"u\" x=4 y=36\n          tspan \"12\" testId=\"s\" dx=2 font-weight=700\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    assert_eq!(
        node("box").node_type,
        NodeType::Text,
        "outside an svg, text is a box"
    );
    let t = node("t");
    assert_eq!(t.node_type, NodeType::SvgText);
    assert_eq!(t.props.str(PropId::TextX), Some("50"));
    assert_eq!(t.props.str(PropId::Text), Some("Q1"));
    assert_eq!(t.style.text_anchor, exact_kernel::TextAnchor::Middle);
    let s = node("s");
    assert_eq!(s.node_type, NodeType::SvgTSpan);
    assert_eq!(s.props.str(PropId::TextDx), Some("2"));
}

// LLP 1055.000 stage 6: an element that renders takes events, and
// `pointer-events` decides what of it is hit.
#[test]
fn stage_six_events() {
    let r = boot(
        "component A\n  state n = 0\n  action go writes n\n    n = 1\n  view\n    svg width=10 height=10\n      g press=go\n        circle r=3 testId=\"c\" press=go pointer-events=\"stroke\"\n",
    );
    let k = r.kernel();
    let c = k.node_by_key(k.find_by_test_id("c")[0]).unwrap();
    assert_eq!(c.style.pointer_events, exact_kernel::PointerEvents::Stroke);
}

// LLP 1055.000 stage 7: `marker`, its attributes, and the marker rows.
#[test]
fn stage_seven_markers() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      marker id=\"m\" testId=\"m\" markerWidth=4 refX=2 orient=\"auto\"\n        circle r=2\n      polyline testId=\"p\" points=\"0,0 5,5\" marker=\"url(#m)\"\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let m = node("m");
    assert_eq!(m.node_type, NodeType::SvgMarker);
    assert_eq!(m.props.str(PropId::MarkerWidth), Some("4"));
    assert_eq!(m.style.overflow_x, exact_kernel::Overflow::Hidden);
    let p = node("p");
    for row in [
        &p.style.marker_start,
        &p.style.marker_mid,
        &p.style.marker_end,
    ] {
        assert_eq!(row.url(), Some("m"), "`marker` sets all three");
    }
    let svg =
        |body: &str| format!("component A\n  view\n    svg width=10 height=10\n      {body}\n");
    assert!(refused(&svg("rect markerWidth=3")).contains("lower-attr-tag"));
}
