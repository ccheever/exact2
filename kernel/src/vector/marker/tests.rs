use super::*;

fn arrow() -> Marker {
    Marker::parse("marker(arrow;0 0 10 10;xMidYMid meet;5 5;6 6;strokeWidth;auto-start-reverse;M0 0 L10 5 L0 10 Z|context-stroke|none|1|butt|miter|4|nonzero)").unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn the_row_text_round_trips() {
    let m = arrow();
    let def = m.def().unwrap();
    assert_eq!(def.name, "arrow");
    assert_eq!(def.orient, Orient::AutoStartReverse);
    assert_eq!(def.shapes[0].fill, ShapePaint::ContextStroke);
    assert_eq!(Marker::parse(&m.css()), Some(m.clone()));
    assert_eq!(Marker::parse("none"), Some(Marker(None)));
    assert_eq!(Marker::default().css(), "none");
    let dark = Marker::parse("marker(d;none;none;0 0;3 3;userSpaceOnUse;45;M0 0 H1|light-dark(#000, #fff)|#ff0000|2|round|round|4|evenodd)").unwrap();
    assert_eq!(Marker::parse(&dark.css()), Some(dark));
    for bad in [
        "marker()",
        "marker(a;0 0 0 10;xMidYMid meet;0 0;3 3;strokeWidth;auto)",
        "marker(a;none;xMidYMid meet;0 0;-3 3;strokeWidth;auto)",
        "marker(a;none;xMidYMid meet;0 0;3 3;points;auto)",
        "marker(a;none;xMidYMid meet;0 0;3 3;strokeWidth;auto;M0 0 L|none|none|1|butt|miter|4|nonzero)",
        "marker(a;none;xMidYMid meet;0 0;3 3;strokeWidth;auto;M0 0 H1|nothing|none|1|butt|miter|4|nonzero)",
    ] {
        assert_eq!(Marker::parse(bad), None, "{bad}");
    }
}

#[test]
fn vertices_follow_svg_start_mid_end_and_bisect() {
    let data = PathData::parse("M0 0 L10 0 L10 10");
    let v = data.vertices();
    let slots: Vec<Slot> = v.iter().map(|v| v.slot).collect();
    assert_eq!(slots, [Slot::Start, Slot::Mid, Slot::End]);
    // Start along the first segment, end along the last, the corner between.
    assert!(near(v[0].angle, 0.0) && near(v[1].angle, 45.0) && near(v[2].angle, 90.0));
    assert_eq!(
        v.iter().map(|v| v.at).collect::<Vec<_>>(),
        [0.0, 10.0, 20.0]
    );
    // A closed subpath: its closing point is a vertex, and its start takes
    // the closing segment as incoming (SVG 2's path directionality).
    let square = PathData::parse("M0 0 H10 V10 H0 Z").vertices();
    assert_eq!(square.len(), 5);
    assert!(near(square[0].angle, -45.0), "{}", square[0].angle);
    assert!(near(square[4].angle, -45.0), "{}", square[4].angle);
    assert_eq!(square[4].point, [0.0, 0.0]);
    // An arc is one segment, whatever pieces it is drawn in; a second
    // subpath's start is a mid vertex.
    let arc = PathData::parse("M0 0 A10 10 0 1 1 0 20 M50 0 L60 0").vertices();
    assert_eq!(arc.len(), 4);
    assert_eq!(arc[1].slot, Slot::Mid);
    assert!(
        near(arc[1].angle, 180.0) || near(arc[1].angle, -180.0),
        "{}",
        arc[1].angle
    );
    // A lone moveto has none; one vertex is both start and end.
    assert!(PathData::parse("M5 5").vertices().is_empty());
    let dot = PathData::parse("M5 5 Z").vertices();
    assert_eq!(dot.len(), 2);
    assert_eq!((dot[0].slot, dot[1].slot), (Slot::Start, Slot::End));
}

#[test]
fn a_marker_is_placed_scaled_turned_and_clipped() {
    let m = arrow();
    let data = PathData::parse("M0 0 L100 0");
    let placed = markers(&data, [&m, &Marker(None), &m], 2.0);
    assert_eq!(placed.len(), 2);
    let (_, end) = placed[1];
    // 6 × 6 at twice the stroke's width; the view box 10 → 6: 0.6 a unit.
    assert_eq!(end.fit, [0.6, 0.6, 0.0, 0.0]);
    assert_eq!(end.clip, [0.0, 0.0, 6.0, 6.0]);
    // refX/refY (5, 5) → (3, 3) in the viewport, on the vertex at (100, 0).
    let o = end.outer;
    let map = |x: f32, y: f32| [o[0] * x + o[2] * y + o[4], o[1] * x + o[3] * y + o[5]];
    assert_eq!(map(3.0, 3.0), [100.0, 0.0]);
    assert_eq!(map(4.0, 3.0), [102.0, 0.0], "along +x, twice as large");
    // The start marker is reversed: it points back along the path.
    let (_, start) = placed[0];
    let o = start.outer;
    assert!((o[0] + 2.0).abs() < 1e-5 && o[1].abs() < 1e-5, "{o:?}");
    // userSpaceOnUse ignores the stroke; a fixed orient ignores the path.
    let fixed = Marker::parse("marker(f;none;xMidYMid meet;0 0;4 4;userSpaceOnUse;90)").unwrap();
    let p = fixed
        .def()
        .unwrap()
        .place(&data.vertices()[1], 5.0)
        .unwrap();
    assert!(p.outer[0].abs() < 1e-6 && (p.outer[1] - 1.0).abs() < 1e-6);
    let empty = Marker::parse("marker(e;none;xMidYMid meet;0 0;0 4;strokeWidth;auto)").unwrap();
    assert!(empty
        .def()
        .unwrap()
        .place(&data.vertices()[0], 1.0)
        .is_none());
}
