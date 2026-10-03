use super::*;

#[test]
fn the_shorthand_expands_as_border_radius_does_and_round_trips() {
    let s = CornerShape::check("squircle").unwrap();
    assert_eq!(s.0, [Corner::Superellipse(2.0); 4]);
    assert_eq!(s.css(), "squircle");
    let s = CornerShape::check("round -apple-continuous").unwrap();
    assert_eq!(s.0[1], Corner::AppleContinuous);
    assert_eq!(s.0[3], Corner::AppleContinuous);
    assert_eq!(s.css(), "round -apple-continuous round -apple-continuous");
    let s = CornerShape::check("superellipse(3)  bevel superellipse(-infinity)").unwrap();
    assert_eq!(s.0[2], Corner::Superellipse(f32::NEG_INFINITY));
    assert_eq!(s.css(), "superellipse(3) bevel notch bevel");
    assert_eq!(CornerShape::check(&s.css()).unwrap(), s);
    assert!(CornerShape::default().is_round());
    assert!(CornerShape::check("-Apple-Continuous")
        .unwrap()
        .is_apple_continuous());
}

#[test]
fn what_is_refused_is_named() {
    for (text, says) in [
        ("", "one to four"),
        ("round round round round round", "one to four"),
        ("circle", "squircle"),
        ("superellipse(nan)", "a number"),
        ("superellipse(2", "its )"),
    ] {
        let e = CornerShape::check(text).unwrap_err();
        assert!(e.contains(says), "{text:?}: {e}");
    }
}

fn points(path: &Path) -> Vec<(f32, f32)> {
    path.0
        .iter()
        .filter_map(|s| match *s {
            Seg::Move(x, y) | Seg::Line(x, y) => Some((x, y)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_shape_stays_in_its_box_and_meets_the_edges_at_the_radius() {
    for word in [
        "round",
        "squircle",
        "bevel",
        "scoop",
        "square",
        "notch",
        "superellipse(0.5)",
        "-apple-continuous",
    ] {
        let shape = CornerShape::check(word).unwrap();
        let path = outline((10.0, 20.0, 100.0, 60.0), [(12.0, 12.0); 4], &shape);
        let pts = points(&path);
        for &(x, y) in &pts {
            assert!(
                (10.0..=110.0).contains(&x) && (20.0..=80.0).contains(&y),
                "{word}: ({x}, {y})"
            );
        }
        let reach = if word == "-apple-continuous" {
            12.0 * APPLE_EXTENT
        } else {
            12.0
        };
        // The top-left corner starts on the left edge at the radius and ends
        // on the top edge at it.
        assert_eq!(pts[0], (10.0, 20.0 + reach), "{word}");
        assert!(pts.contains(&(10.0 + reach, 20.0)), "{word}");
        assert_eq!(path.0.last(), Some(&Seg::Close));
    }
}

#[test]
fn a_squircle_bulges_past_the_round_corner_and_a_scoop_falls_inside_it() {
    let diagonal = |word: &str| {
        let shape = CornerShape::check(word).unwrap();
        let pts = points(&outline(
            (0.0, 0.0, 100.0, 100.0),
            [(20.0, 20.0); 4],
            &shape,
        ));
        // The point of the top-left corner nearest the corner point.
        pts.iter()
            .filter(|(x, y)| *x < 20.0 && *y < 20.0)
            .map(|(x, y)| (x * x + y * y).sqrt())
            .fold(f32::INFINITY, f32::min)
    };
    let round = diagonal("round");
    // A circle of radius 20 about (20, 20) passes 20 (√2 − 1) from the corner.
    assert!((round - 20.0 * (2f32.sqrt() - 1.0)).abs() < 0.2, "{round}");
    assert!(diagonal("squircle") < round);
    assert!(diagonal("scoop") > round);
    assert!(diagonal("square") < 0.01);
}

#[test]
fn apple_continuous_takes_a_smaller_radius_in_a_small_box() {
    let shape = CornerShape::check("-apple-continuous").unwrap();
    let pts = points(&outline((0.0, 0.0, 30.0, 30.0), [(20.0, 20.0); 4], &shape));
    // 30 / 2 / 1.5287 ≈ 9.81: the corner reaches half the side, no further.
    assert!((pts[0].1 - 15.0).abs() < 0.01, "{:?}", pts[0]);
}

#[test]
fn the_inner_edge_takes_the_radii_less_the_border() {
    let shape = CornerShape::check("squircle").unwrap();
    let pts = points(&inner_outline(
        (0.0, 0.0, 100.0, 50.0),
        [(16.0, 16.0); 4],
        [2.0, 4.0, 2.0, 4.0],
        &shape,
    ));
    assert_eq!(pts[0], (4.0, 2.0 + 14.0));
}
