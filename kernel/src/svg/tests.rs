use super::*;
use crate::generated::PropId;
use crate::props::{PropList, PropValue};
use crate::StyleProps;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b}");
}

#[test]
fn path_data_covers_every_command() {
    let p = parse_d("M10 20 L30 20 H40 V50 h-10 v-10 l5 5 Z");
    assert_eq!(
        p.0,
        vec![
            Seg::Move(10.0, 20.0),
            Seg::Line(30.0, 20.0),
            Seg::Line(40.0, 20.0),
            Seg::Line(40.0, 50.0),
            Seg::Line(30.0, 50.0),
            Seg::Line(30.0, 40.0),
            Seg::Line(35.0, 45.0),
            Seg::Close,
        ]
    );
    // Implicit repeats: pairs after M are lines; after m, relative lines.
    let q = parse_d("m1 1 2 2 3 3");
    assert_eq!(
        q.0,
        vec![
            Seg::Move(1.0, 1.0),
            Seg::Line(3.0, 3.0),
            Seg::Line(6.0, 6.0)
        ]
    );
    // Numbers without separators, as SVG allows: `0.5.5` is two numbers.
    let r = parse_d("M0.5.5L-1-1");
    assert_eq!(r.0, vec![Seg::Move(0.5, 0.5), Seg::Line(-1.0, -1.0)]);
    // S reflects the previous control point; T the previous quad's.
    let s = parse_d("M0 0 C0 10 10 10 10 0 S20 -10 20 0");
    assert_eq!(s.0[2], Seg::Cubic(10.0, -10.0, 20.0, -10.0, 20.0, 0.0));
    let t = parse_d("M0 0 Q5 10 10 0 T20 0");
    assert!(matches!(t.0[2], Seg::Cubic(..)));
    // A quadratic is an exact cubic: the curve's midpoint is (5, 5).
    let Seg::Cubic(x1, y1, x2, y2, ..) = t.0[1] else {
        panic!()
    };
    let mid_y = 0.125 * 0.0 + 0.375 * y1 + 0.375 * y2 + 0.125 * 0.0;
    close(mid_y as f64, 5.0, 1e-5);
    close((0.375 * x1 + 0.375 * x2 + 0.125 * 10.0) as f64, 5.0, 1e-5);
}

#[test]
fn errors_render_up_to_the_last_good_segment() {
    assert_eq!(parse_d("M0 0 L10 10 L20 x L30 30").0.len(), 2);
    assert!(
        parse_d("L10 10").0.is_empty(),
        "a path must start with a move"
    );
    assert_eq!(parse_d("M0 0 L10").0.len(), 1);
    // Drawing after a close starts again at the subpath's start.
    let p = parse_d("M5 5 L10 5 Z L5 10");
    assert_eq!(p.0[3], Seg::Move(5.0, 5.0));
}

#[test]
fn a_close_cannot_repeat_implicitly() {
    for d in ["M0 0 Z 1", "M0 0 z 1", "M0 0Z1 M20 20L30 30"] {
        let (path, ends) = super::path::parse_d_commands(d);
        assert_eq!(path.0, [Seg::Move(0.0, 0.0), Seg::Close], "{d}");
        assert_eq!(ends, [1, 2], "{d}");
        assert!(parse_d_whole(d).is_none(), "{d}");
    }
    assert_eq!(parse_d("M0 0ZZ").0, [Seg::Move(0.0, 0.0), Seg::Close]);
    assert!(parse_d_whole("M0 0Z M1 1L2 2").is_some());
}

#[test]
fn arcs_become_cubics_on_the_ellipse() {
    // A half circle of radius 10 from (0,0) to (20,0).
    let p = parse_d("M0 0 A10 10 0 0 1 20 0");
    assert_eq!(p.0.len(), 3, "180° is two quarter cubics");
    // Cubic quarters run 0.014% long; Skia's conics are exact. 0.004 units here.
    close(p.length(), std::f64::consts::PI * 10.0, 0.01);
    // Flags packed without separators; radii scaled up when too small.
    let q = parse_d("M0 0a1 1 0 0020 0");
    close(q.length(), std::f64::consts::PI * 10.0, 0.01);
    // A zero radius is a line.
    assert_eq!(parse_d("M0 0 A0 5 0 0 1 10 0").0[1], Seg::Line(10.0, 0.0));
}

#[test]
fn lengths_of_lines_polylines_and_circles() {
    let pts = parse_points("0,0 3,4 3 10, 99");
    assert_eq!(
        pts,
        vec![(0.0, 0.0), (3.0, 4.0), (3.0, 10.0)],
        "an odd trailing number is dropped"
    );
    close(Path::polyline(&pts, false).length(), 11.0, 1e-9);
    close(
        Path::polyline(&pts, true).length(),
        11.0 + 109f64.sqrt(),
        1e-5,
    );
    let c = shape::circle(0.0, 0.0, 10.0).unwrap();
    // Four cubic quarters: within 0.03% of 2πr, as every browser's is.
    close(c.length(), std::f64::consts::TAU * 10.0, 0.02);
    assert_eq!(
        c.0[0],
        Seg::Move(10.0, 0.0),
        "a circle starts at (cx + r, cy)"
    );
    assert!(shape::circle(0.0, 0.0, 0.0).is_none());
}

#[test]
fn geometry_reads_the_node() {
    let mut props = PropList::default();
    props.set(PropId::Points, PropValue::Str("0,32 48,0 96,16".into()));
    let style = StyleProps::default();
    let vp = Viewport {
        width: 100.0,
        height: 50.0,
    };
    let p = geometry(crate::NodeType::SvgPolyline, &props, &style, vp).unwrap();
    assert_eq!(p.0.len(), 3);
    props.set(PropId::PathLength, PropValue::Float(1.0));
    close(dash_scale(&p, &props) as f64, p.length(), 1e-4);
    let rect = PropList::default();
    let mut rs = StyleProps::default();
    rs.rx = crate::Dimension::Points(4.0);
    rs.width = crate::Dimension::Points(20.0);
    rs.height = crate::Dimension::Points(6.0);
    let r = geometry(crate::NodeType::SvgRect, &rect, &rs, vp).unwrap();
    // ry takes rx (auto), and is clamped to half the height.
    assert_eq!(r.0[0], Seg::Move(4.0, 0.0));
    assert_eq!(
        r.0[2],
        Seg::Cubic(
            4.0 + 4.0 * 0.552_284_8 - 4.0 + 16.0,
            0.0,
            20.0,
            3.0 - 3.0 * 0.552_284_8,
            20.0,
            3.0
        )
    );
}

#[test]
fn the_view_box_equations() {
    let vb = Some(ViewBox {
        x: 0.0,
        y: 0.0,
        width: 96.0,
        height: 32.0,
    });
    assert_eq!(
        view_box_transform(vb, None, 96.0, 32.0),
        Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    );
    // meet: the smaller scale, centred.
    assert_eq!(
        view_box_transform(vb, None, 192.0, 32.0),
        Some([1.0, 0.0, 0.0, 1.0, 48.0, 0.0])
    );
    assert_eq!(
        view_box_transform(vb, Some("xMinYMin meet"), 192.0, 32.0),
        Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    );
    assert_eq!(
        view_box_transform(vb, Some("xMaxYMid slice"), 192.0, 32.0),
        Some([2.0, 0.0, 0.0, 2.0, 0.0, -16.0])
    );
    assert_eq!(
        view_box_transform(vb, Some("none"), 192.0, 16.0),
        Some([2.0, 0.0, 0.0, 0.5, 0.0, 0.0])
    );
    let offset = Some(ViewBox {
        x: 10.0,
        y: 10.0,
        width: 10.0,
        height: 10.0,
    });
    assert_eq!(
        view_box_transform(offset, None, 20.0, 20.0),
        Some([2.0, 0.0, 0.0, 2.0, -20.0, -20.0])
    );
    assert_eq!(
        view_box_transform(
            Some(ViewBox {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 5.0
            }),
            None,
            5.0,
            5.0
        ),
        None
    );
    let mut props = PropList::default();
    props.set(PropId::ViewBox, PropValue::Str("0, 0 96 32".into()));
    assert_eq!(view_box(&props), vb);
    props.set(PropId::ViewBox, PropValue::Str("0 0 -1 32".into()));
    assert_eq!(view_box(&props), None, "a negative size invalidates it");
}

#[test]
fn paint_and_dasharray_grammar() {
    assert_eq!(Paint::parse("none"), Some(Paint::None));
    assert_eq!(Paint::parse("currentColor"), Some(Paint::CurrentColor));
    assert_eq!(Paint::parse("#16a34a").unwrap().css(), "#16a34aff");
    assert_eq!(Paint::parse("url(#g)").unwrap().css(), "url(#g)");
    assert_eq!(
        Paint::parse("url('#g') none").unwrap().css(),
        "url(#g) none"
    );
    assert!(
        Paint::parse("url(other.svg#g)").is_none(),
        "external references are refused"
    );
    assert_eq!(Paint::parse(&Paint::BLACK.css()), Some(Paint::BLACK));
    assert_eq!(DashArray::parse("none"), Some(DashArray::default()));
    assert_eq!(DashArray::parse("1, 2px 3").unwrap().0, vec![1.0, 2.0, 3.0]);
    assert!(DashArray::parse("1 -2").is_none());
    assert_eq!(
        DashArray::parse("1 2 3").unwrap().pattern(2.0),
        vec![2.0, 4.0, 6.0, 2.0, 4.0, 6.0]
    );
    assert!(
        DashArray::parse("0 0").unwrap().pattern(1.0).is_empty(),
        "a zero sum is solid"
    );
    assert_eq!(DashArray::parse("1").unwrap().css(), "1");
}

#[test]
fn lengths_take_units_and_percentages() {
    use super::length::Length;
    assert_eq!(Length::parse("12"), Some(Length::Units(12.0)));
    assert_eq!(Length::parse("12px"), Some(Length::Units(12.0)));
    assert_eq!(Length::parse("1in"), Some(Length::Units(96.0)));
    assert_eq!(Length::parse("72pt"), Some(Length::Units(96.0)));
    assert_eq!(Length::parse("1pc"), Some(Length::Units(16.0)));
    close(
        Length::parse("2.54cm").unwrap().resolve(0.0) as f64,
        96.0,
        1e-3,
    );
    assert_eq!(Length::parse("1e1"), Some(Length::Units(10.0)));
    assert_eq!(Length::parse("50%"), Some(Length::Percent(50.0)));
    assert_eq!(Length::Percent(50.0).resolve(30.0), 15.0);
    // Font-relative and viewport units wait for a consumer (LLP 1055.000 D4).
    for refused in ["2em", "1ex", "3vw", "px", ""] {
        assert_eq!(Length::parse(refused), None, "{refused}");
    }
}

#[test]
fn transform_lists_read_both_grammars() {
    use super::transform::{TransformFn, TransformList, TransformOrigin};
    let svg = TransformList::parse("rotate(30 10 10), translate(5,2)scale(2)").unwrap();
    assert_eq!(
        svg.0,
        vec![
            TransformFn::Rotate(30.0, 10.0, 10.0),
            TransformFn::Translate(5.0, 2.0),
            TransformFn::Scale(2.0, 2.0)
        ]
    );
    let css = TransformList::parse("rotate(0.5turn) translateX(4px) skew(10deg, 1grad)").unwrap();
    assert_eq!(css.0[0], TransformFn::Rotate(180.0, 0.0, 0.0));
    assert_eq!(css.0[1], TransformFn::Translate(4.0, 0.0));
    assert_eq!(css.0[2], TransformFn::Skew(10.0, 0.9));
    // The web host emits CSS: a centred rotate is its three functions.
    assert_eq!(
        TransformList::parse("rotate(30 10 10)").unwrap().css(),
        "translate(10px, 10px) rotate(30deg) translate(-10px, -10px)"
    );
    assert_eq!(TransformList::parse("none").unwrap().css(), "none");
    for bad in [
        "rotate(1 2)",
        "translate(10%)",
        "spin(3)",
        "matrix(1 2 3)",
        "rotate(4",
    ] {
        assert!(TransformList::parse(bad).is_none(), "{bad}");
    }
    let o = |t: &str| TransformOrigin::parse(t).map(|o| o.point((0.0, 0.0, 100.0, 50.0)));
    assert_eq!(o("center"), Some((50.0, 25.0)));
    assert_eq!(o("top left"), Some((0.0, 0.0)));
    assert_eq!(o("bottom"), Some((50.0, 50.0)));
    assert_eq!(o("10px 20%"), Some((10.0, 10.0)));
    assert_eq!(o("right 5 0"), Some((100.0, 5.0)));
    assert_eq!(o("left right"), None);
}

#[test]
fn a_filter_chain_reaches_as_far_as_its_primitives_read() {
    use crate::svg::filter::{Filter, Input, Op, Primitive};
    let p = |op| Primitive {
        op,
        inputs: [Input::SourceGraphic, Input::None],
        subregion: (0.0, 0.0, 10.0, 10.0),
        linear: false,
    };
    let chain = |ops: Vec<Op>| Filter {
        region: (0.0, 0.0, 10.0, 10.0),
        primitives: ops.into_iter().map(p).collect(),
    };
    assert_eq!(chain(vec![Op::Flood([0.0; 4])]).reach(), Some((0.0, 0.0)));
    assert_eq!(
        chain(vec![Op::DropShadow(2.0, 1.0, 0.0, -3.0, [0.0; 4])]).reach(),
        Some((9.0, 0.0))
    );
    assert_eq!(
        chain(vec![Op::Blur(1.0, 1.0), Op::Offset(4.0, 0.0)]).reach(),
        Some((7.0, 0.0))
    );
    assert_eq!(chain(vec![Op::Tile]).reach(), None);
}

#[test]
fn a_boxs_css_filter_is_the_functions_chain_over_a_box_of_no_size() {
    use crate::svg::filter::{FilterList, Op};
    let black = crate::style::Color::rgba(0, 0, 0, 255);
    let blur =
        crate::svg::scene::box_filter(&FilterList::parse("blur(2px)").unwrap(), black).unwrap();
    assert_eq!(
        blur.region,
        (-6.0, -6.0, 12.0, 12.0),
        "how far past the box it reaches"
    );
    assert!(matches!(blur.primitives[0].op, Op::Blur(s, t) if s == 2.0 && t == 2.0));
    let chain = crate::svg::scene::box_filter(
        &FilterList::parse("drop-shadow(0 10px 12px) saturate(1.8)").unwrap(),
        black,
    )
    .unwrap();
    assert_eq!(chain.primitives.len(), 2);
    assert!(crate::svg::scene::box_filter(&FilterList::parse("url(#f)").unwrap(), black).is_none());
    assert!(crate::svg::scene::box_filter(&FilterList::parse("none").unwrap(), black).is_none());
}
