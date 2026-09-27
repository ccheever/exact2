use super::*;

fn css(d: &str) -> String {
    PathData::parse(d).css()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 3e-4 * b
}

#[test]
fn every_command_normalizes_to_absolute_move_line_cubic_close() {
    assert_eq!(
        css("m10 10 l10 0 v10 h-10 z"),
        "M 10 10 L 20 10 L 20 20 L 10 20 Z"
    );
    assert_eq!(css("M0 0 H5 V5 h-5 v-5"), "M 0 0 L 5 0 L 5 5 L 0 5 L 0 0");
    // A quadratic is its exact cubic.
    assert_eq!(css("M0 0 Q 3 3 6 0"), "M 0 0 C 2 2 4 2 6 0");
    assert_eq!(css("M1 1 q 3 3 6 0"), "M 1 1 C 3 3 5 3 7 1");
    assert_eq!(css("M0 0 c 1 2 3 4 5 6"), "M 0 0 C 1 2 3 4 5 6");
}

#[test]
fn implicit_repeats_and_the_number_grammar() {
    // Pairs after a moveto are linetos; relative after `m`.
    assert_eq!(css("M0 0 10 0 10 10"), "M 0 0 L 10 0 L 10 10");
    assert_eq!(css("m1 1 2 0 0 2"), "M 1 1 L 3 1 L 3 3");
    // `.5.5` is two numbers, and a sign starts a new one.
    assert_eq!(css("M.5.5-1-2"), "M 0.5 0.5 L -1 -2");
    assert_eq!(css("M1e1,2E0 L+3 , -4"), "M 10 2 L 3 -4");
    assert_eq!(
        css("M0 0C1 1 2 2 3 3 4 4 5 5 6 6"),
        "M 0 0 C 1 1 2 2 3 3 C 4 4 5 5 6 6"
    );
}

#[test]
fn smooth_curves_reflect_the_previous_control_point() {
    assert_eq!(
        css("M0 0 C0 10 10 10 10 0 S20 -10 20 0"),
        "M 0 0 C 0 10 10 10 10 0 C 10 -10 20 -10 20 0"
    );
    // After a non-cubic, `S`'s first control is the current point.
    assert_eq!(
        css("M0 0 L10 0 S20 10 20 0"),
        "M 0 0 L 10 0 C 10 0 20 10 20 0"
    );
    // `T` reflects the quadratic's control: (5,5) about (10,0) is (15,-5).
    let t = PathData::parse("M0 0 Q5 5 10 0 T20 0");
    let q = PathData::parse("M0 0 Q5 5 10 0 Q15 -5 20 0");
    assert_eq!(t.commands(), q.commands());
}

#[test]
fn arcs_are_cubics_on_the_ellipse() {
    let half = PathData::parse("M0 0 A10 10 0 0 1 20 0");
    // A 90° cubic sits within 0.03% of its circle, so its length does too.
    assert!(
        near(half.length(), std::f64::consts::PI * 10.0),
        "{}",
        half.length()
    );
    let Some(Command::Cubic(_, _, end)) = half.commands().last() else {
        panic!("{:?}", half.commands())
    };
    assert_eq!(*end, [20.0, 0.0]);
    // Flags need no separators; radii too small to reach scale up.
    assert_eq!(
        PathData::parse("M0 0 a10 10 0 0120 0").commands(),
        half.commands()
    );
    let scaled = PathData::parse("M0 0 A1 1 0 0 1 20 0");
    assert!(near(scaled.length(), std::f64::consts::PI * 10.0));
    // The large-arc flag takes the long way round a circle of radius 10
    // whose chord is 10: 5/6 of the circumference.
    let large = PathData::parse("M0 0 A10 10 0 1 0 10 0");
    assert!(
        near(large.length(), std::f64::consts::PI * 20.0 * 5.0 / 6.0),
        "{}",
        large.length()
    );
    // A zero radius is a line; identical endpoints are nothing.
    assert_eq!(css("M0 0 A0 5 0 0 1 10 0"), "M 0 0 L 10 0");
    assert_eq!(css("M0 0 A5 5 0 0 1 0 0"), "M 0 0");
}

#[test]
fn errors_render_up_to_the_command_that_holds_them() {
    let partial = PathData::parse("M0 0 L10 0 L20");
    assert_eq!(partial.css(), "M 0 0 L 10 0");
    assert_eq!(partial.error(), Some(PathError { at: 11 }));
    // Implicit repeats before the malformed set still draw.
    assert_eq!(css("M0 0 L10 0 20 0 30"), "M 0 0 L 10 0 L 20 0");
    for (bad, at) in [("L0 0", 0), ("M0 0 X1 1", 5), ("M,0 0", 0), ("M0 0 Z 3", 7)] {
        let data = PathData::parse(bad);
        assert_eq!(data.error(), Some(PathError { at }), "{bad}");
    }
    assert!(PathData::parse("L0 0").commands().is_empty());
    let empty = PathData::parse("  ");
    assert!(empty.commands().is_empty() && empty.error().is_none());
    assert!(PathData::parse("M0 0 L1e39 0").error().is_some());
}

#[test]
fn subpaths_are_measured_in_pen_order() {
    let data = PathData::parse("M0 0 H10 M0 5 H30");
    assert_eq!(data.lengths().collect::<Vec<_>>(), [10.0, 30.0]);
    assert_eq!(data.length(), 40.0);
    let subpaths: Vec<_> = data.subpaths().map(commands_css).collect();
    assert_eq!(subpaths, ["M 0 0 L 10 0", "M 0 5 L 30 5"]);
    // A closepath's line counts, and a drawing command after it starts a
    // subpath of its own at the same point (SVG 2 §9.3.3).
    let closed = PathData::parse("M0 0 L10 0 Z L0 10");
    assert_eq!(closed.css(), "M 0 0 L 10 0 Z M 0 0 L 0 10");
    assert_eq!(closed.lengths().collect::<Vec<_>>(), [20.0, 10.0]);
    // A straight cubic is as long as its chord.
    assert!(close(PathData::parse("M0 0 C1 0 2 0 3 0").length(), 3.0));
}

#[test]
fn canonical_text_reparses_to_the_same_commands() {
    let d = "M12 2a10 10 0 1 1-8 4q2-3 5-3t4 1s3 2 5 0c1-1 2-1 3 0zM3 3h1v1";
    let data = PathData::parse(d);
    assert!(data.error().is_none());
    assert_eq!(PathData::parse(&data.css()).commands(), data.commands());
}

#[test]
fn view_box_parses_and_fits_as_xmidymid_meet() {
    assert_eq!(
        parse_view_box("0 0 1307 840"),
        Some([0.0, 0.0, 1307.0, 840.0])
    );
    assert_eq!(
        parse_view_box(" -5,-5 , 10 10 "),
        Some([-5.0, -5.0, 10.0, 10.0])
    );
    for bad in [
        "0 0 0 10",
        "0 0 10 -1",
        "0 0 10",
        "0 0 10 10 5",
        "a b c d",
        "",
    ] {
        assert_eq!(parse_view_box(bad), None, "{bad}");
    }
    // Wider than tall into a square: scale by width, centred vertically.
    assert_eq!(
        fit(Some([0.0, 0.0, 100.0, 50.0]), 200.0, 200.0),
        (2.0, 0.0, 50.0)
    );
    // The view box's origin maps to the fitted rectangle's corner.
    assert_eq!(
        fit(Some([10.0, 10.0, 10.0, 10.0]), 20.0, 40.0),
        (2.0, -20.0, -10.0)
    );
    assert_eq!(fit(None, 20.0, 40.0), (1.0, 0.0, 0.0));
}

#[test]
fn trimming_measures_the_whole_path_through_its_subpaths_in_order() {
    let data = PathData::parse("M0 0 H10 M0 5 H30");
    let trim = |a, b| commands_css(&data.trimmed(a, b));
    // Half of 40 is all of the first stroke and 10 of the second.
    assert_eq!(trim(0.0, 0.5), "M 0 0 L 10 0 M 0 5 L 10 5");
    assert_eq!(trim(0.125, 0.5), "M 5 0 L 10 0 M 0 5 L 10 5");
    assert_eq!(trim(0.0, 1.0), data.css());
    assert_eq!(trim(0.5, 0.5), "");
    assert_eq!(trim(0.6, 0.4), "");
    // A subpath shown whole keeps its closepath; a cut one is open.
    let square = PathData::parse("M0 0 H10 V10 H0 Z M20 0 H30");
    assert_eq!(
        commands_css(&square.trimmed(0.0, 0.8)),
        "M 0 0 L 10 0 L 10 10 L 0 10 Z"
    );
    assert_eq!(
        commands_css(&square.trimmed(0.2, 0.8)),
        "M 10 0 L 10 10 L 0 10 L 0 0"
    );
    // A cubic is cut by length: the midpoint of a symmetric arch.
    let arch = PathData::parse("M0 0 C0 10 10 10 10 0");
    let half = arch.trimmed(0.0, 0.5);
    let Some(Command::Cubic(_, _, end)) = half.last() else {
        panic!("{half:?}")
    };
    assert!(
        (end[0] - 5.0).abs() < 1e-3 && (end[1] - 7.5).abs() < 1e-3,
        "{end:?}"
    );
    let piece = PathData::parse(&commands_css(&half));
    assert!(
        close(piece.length(), arch.length() / 2.0),
        "{}",
        piece.length()
    );
}
