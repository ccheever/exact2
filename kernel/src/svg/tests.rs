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
    let p = geometry(crate::NodeType::SvgPolyline, &props, &style).unwrap();
    assert_eq!(p.0.len(), 3);
    props.set(PropId::PathLength, PropValue::Float(1.0));
    close(dash_scale(&p, &props) as f64, p.length(), 1e-4);
    let mut rect = PropList::default();
    rect.set(PropId::Rx, PropValue::Float(4.0));
    let mut rs = StyleProps::default();
    rs.width = crate::Dimension::Points(20.0);
    rs.height = crate::Dimension::Points(6.0);
    let r = geometry(crate::NodeType::SvgRect, &rect, &rs).unwrap();
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
    assert!(Paint::parse("url(#g)").is_none());
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
