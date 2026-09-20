use super::*;
fn prepared(text: &str) -> u64 {
    let mut advances = vec![0.; text.encode_utf16().count()];
    let mut offset = 0;
    for ch in text.chars() {
        advances[offset] = 8.;
        offset += ch.len_utf16();
    }
    prepare(
        text.as_ptr(),
        text.len(),
        advances.as_ptr(),
        advances.len(),
        2,
        0,
        8.,
    )
}
fn layout(id: u64, shapes: &[Shape], cap: usize) -> (Result, Vec<CFragment>) {
    let mut output = vec![CFragment::default(); cap];
    let result = flow(
        id,
        shapes.as_ptr(),
        shapes.len(),
        240.,
        22.,
        5.,
        0,
        0,
        output.as_mut_ptr(),
        cap,
    );
    output.truncate(result.count.min(cap));
    (result, output)
}
#[test]
fn unicode_round_trip_and_source_coverage() {
    let text = "Latin fi שלום العربية 中文 e\u{301} 👨‍👩‍👧‍👦 end ".repeat(16);
    let id = prepared(&text);
    assert_ne!(id, 0);
    let shapes = Shapes::new(&[FlowShape::Circle {
        cx: 120.,
        cy: 90.,
        r: 40.,
    }]);
    let (result, fragments) = layout(id, &shapes.flat, text.len());
    assert!(result.height > 0. && fragments.len() > 2);
    let mut byte = 0;
    for f in &fragments {
        assert_eq!(f.start, byte);
        assert!(text.is_char_boundary(f.end));
        assert_eq!(f.utf16_start, text[..f.start].encode_utf16().count());
        assert_eq!(f.utf16_end, text[..f.end].encode_utf16().count());
        assert!(f.paint_start >= f.utf16_start && f.paint_end <= f.utf16_end);
        byte = f.end;
    }
    assert_eq!(byte, text.len());
    assert!(fragments
        .windows(2)
        .any(|p| p[0].line == p[1].line && p[1].x > p[0].x));
    free(id);
    free(id);
    assert_eq!(layout(id, &[], 4).0.count, 0);
}
#[test]
fn null_empty_invalid_and_small_output_are_defined() {
    use std::ptr::null;
    assert_eq!(prepare(null(), 1, null(), 0, 0, 0, 0.), 0);
    assert_eq!(
        prepare([255u8].as_ptr(), 1, [1f32].as_ptr(), 1, 0, 0, 0.),
        0
    );
    assert_eq!(prepare("😀".as_ptr(), 4, [8f32].as_ptr(), 1, 0, 0, 0.), 0);
    let empty = prepare(null(), 0, null(), 0, 0, 0, 0.);
    assert_ne!(empty, 0);
    assert_eq!(layout(empty, &[], 0).0.count, 0);
    free(empty);
    free(0);
    let id = prepared(&"some words ".repeat(100));
    let query = flow(id, null(), 0, 240., 22., 5., 0, 0, std::ptr::null_mut(), 0);
    let (small, fragments) = layout(id, &[], 1);
    assert_eq!(query.count, small.count);
    assert!(small.count > fragments.len());
    assert_eq!(
        flow(id, null(), 1, 240., 22., 5., 0, 0, std::ptr::null_mut(), 0).count,
        0
    );
    free(id);
}
#[test]
fn every_shape_round_trips_and_obstruction_is_bounded() {
    let shapes = [
        FlowShape::Circle {
            cx: 60.,
            cy: 50.,
            r: 15.,
        },
        FlowShape::Ellipse {
            cx: 180.,
            cy: 60.,
            rx: 20.,
            ry: 30.,
        },
        FlowShape::RoundRect {
            x: 40.,
            y: 100.,
            width: 90.,
            height: 30.,
            radius: 10.,
        },
        FlowShape::Polygon(vec![(10., 10.), (30., 20.), (40., 60.)].into()),
        FlowShape::EvenOddPolygon(vec![(10., 10.), (30., 20.), (40., 60.)].into()),
        FlowShape::Spans {
            x: 5.,
            y: 150.,
            row_height: 2.,
            rows: vec![(10., 30.); 64].into(),
        },
    ];
    let flat = Shapes::new(&shapes);
    for (a, b) in shapes.iter().zip(&flat.flat) {
        assert_eq!(Some(a), decode(b).as_ref());
    }
    let id = prepared(&"x".repeat(20_000));
    let (_, clear) = layout(id, &[], 20_000);
    assert_eq!(clear.last().unwrap().end, 20_000);
    let wall = Shapes::new(&[FlowShape::RoundRect {
        x: 0.,
        y: 0.,
        width: 240.,
        height: f32::MAX,
        radius: 0.,
    }]);
    let (result, fragments) = layout(id, &wall.flat, 20_000);
    assert!(fragments.is_empty());
    assert!(result.height.is_finite());
    assert_eq!(result.complete, 0);
    assert_eq!(result.clamped, 0);
    free(id);
}

#[test]
fn rtl_hebrew_and_arabic_abi_take_right_interval_first() {
    for text in [
        "שלום עולם מילים רבות בעברית ".repeat(8),
        "مرحبا بالعالم هذه كلمات عربية ".repeat(8),
    ] {
        let id = prepared(&text);
        let shapes = Shapes::new(&[FlowShape::RoundRect {
            x: 140.,
            y: 0.,
            width: 120.,
            height: 100.,
            radius: 0.,
        }]);
        let mut out = vec![CFragment::default(); text.len()];
        let result = flow(
            id,
            shapes.flat.as_ptr(),
            1,
            400.,
            22.,
            16.,
            0,
            1,
            out.as_mut_ptr(),
            out.len(),
        );
        out.truncate(result.count);
        assert_eq!(out[0].x, 260.);
        assert_eq!(out[0].start, 0);
        assert!(out
            .windows(2)
            .any(|p| p[0].line == p[1].line && p[0].x > p[1].x));
        for p in out.windows(2) {
            assert_eq!(p[0].end, p[1].start);
        }
        assert_eq!(out.last().unwrap().end, text.len());
        free(id);
    }
}

#[test]
fn tall_wall_jumps_and_completes_the_c_abi_paragraph() {
    let id = prepared("word");
    let shapes = Shapes::new(&[FlowShape::RoundRect {
        x: 0.,
        y: 0.,
        width: 240.,
        height: 30_000.,
        radius: 0.,
    }]);
    let (result, out) = layout(id, &shapes.flat, 20);
    assert_eq!(result.complete, 1);
    assert_eq!(out.len(), 1);
    assert!(out[0].y >= 30_000.);
    assert_eq!(out[0].end, 4);
    free(id);
    assert_eq!(layout(0, &shapes.flat, 20).0.complete, 0);
}

#[test]
fn soft_hyphen_flag_crosses_abi_beside_hole() {
    let text = "ab\u{ad}cdefghij";
    let id = prepared(text);
    let shapes = Shapes::new(&[FlowShape::RoundRect {
        x: 36.,
        y: 0.,
        width: 150.,
        height: 100.,
        radius: 0.,
    }]);
    let (_, fragments) = layout(id, &shapes.flat, 30);
    assert_eq!(fragments[0].end, 4);
    assert_eq!(fragments[0].hyphenated, 1);
    assert_eq!(fragments.last().unwrap().hyphenated, 0);
    let (_, clear) = layout(id, &[], 30);
    assert_eq!(clear[0].hyphenated, 0);
    free(id);
}
